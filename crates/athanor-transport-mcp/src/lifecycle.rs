use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CancellationHandle, CoreError, OperationContext, OperationContextCancellation,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinSet;

use crate::legacy;

type ActiveReads = Arc<Mutex<HashMap<String, CancellationHandle>>>;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

/// Runs the MCP stdio server with concurrent request processing and request-scoped cancellation.
pub async fn run_mcp_server(root: PathBuf) -> Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let root = Arc::new(root);
    let active_reads = ActiveReads::default();
    let (responses_tx, mut responses_rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(response) = responses_rx.recv().await {
            println!("{response}");
        }
    });
    let mut requests = JoinSet::new();

    eprintln!("Athanor MCP server starting in {}", root.display());

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => request,
            Err(error) => {
                let _ = responses_tx.send(parse_error_response(&error));
                continue;
            }
        };
        if is_cancel_notification(&request.method) {
            cancel_notification(&active_reads, request.params.as_ref()).await;
            continue;
        }
        if request.id.is_none() {
            continue;
        }

        let root = Arc::clone(&root);
        let active_reads = Arc::clone(&active_reads);
        let responses_tx = responses_tx.clone();
        requests.spawn(async move {
            let response = handle_request(&root, request, &active_reads).await;
            let _ = responses_tx.send(response);
        });
    }

    cancel_all(&active_reads).await;
    while let Some(joined) = requests.join_next().await {
        if let Err(error) = joined {
            tracing::warn!(error = %error, "MCP request task terminated unexpectedly");
        }
    }
    drop(responses_tx);
    writer
        .await
        .context("MCP response writer task terminated unexpectedly")?;
    Ok(())
}

async fn handle_request(
    root: &Path,
    request: JsonRpcRequest,
    active_reads: &ActiveReads,
) -> String {
    let id = request.id.unwrap_or(Value::Null);
    let result = match request.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "athanor",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "initialized" => Ok(json!({})),
        "tools/list" => Ok(legacy::tools_list_bridge()),
        "tools/call" => handle_tool_call(root, &id, request.params, active_reads).await,
        other => Err(anyhow::anyhow!("Method not found: {other}")),
    };
    response_json(id, result)
}

async fn handle_tool_call(
    root: &Path,
    request_id: &Value,
    params: Option<Value>,
    active_reads: &ActiveReads,
) -> Result<Value> {
    let params = params.context("missing params for tools/call")?;
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .context("missing tool name")?;
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    let result = if is_read_tool(tool_name) {
        call_read_tool(root, request_id, tool_name, arguments, active_reads).await
    } else {
        legacy::call_tool_bridge(root, tool_name, arguments).await
    };

    match result {
        Ok(content) => Ok(json!({
            "content": [{ "type": "text", "text": content }]
        })),
        Err(error) if is_operation_termination(&error) => Err(error),
        Err(error) => Ok(json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!("Error calling tool: {error:?}")
            }]
        })),
    }
}

async fn call_read_tool(
    root: &Path,
    request_id: &Value,
    tool_name: &str,
    arguments: Value,
    active_reads: &ActiveReads,
) -> Result<String> {
    let request_key = request_key(request_id)?;
    let deadline_unix_ms = parse_deadline(&arguments)?;
    let mut operation = OperationContext::new(format!("mcp:{request_key}"));
    if let Some(deadline_unix_ms) = deadline_unix_ms {
        operation = operation.with_deadline_unix_ms(deadline_unix_ms);
    }
    run_registered_read(
        active_reads,
        request_key,
        operation,
        legacy::call_tool_inner_bridge(root, tool_name, arguments),
    )
    .await
}

async fn run_registered_read<T>(
    active_reads: &ActiveReads,
    request_key: String,
    operation: OperationContext,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    operation.check_active().map_err(anyhow::Error::new)?;
    let cancellation = operation
        .cancellation_handle()
        .map_err(anyhow::Error::new)?;
    {
        let mut active = active_reads.lock().await;
        if active.insert(request_key.clone(), cancellation).is_some() {
            bail!("MCP request id `{request_key}` is already active");
        }
    }

    let result = await_read_operation(&operation, future).await;
    active_reads.lock().await.remove(&request_key);
    result
}

async fn await_read_operation<T>(
    operation: &OperationContext,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    tokio::pin!(future);
    let poll_interval = Duration::from_millis(25);
    loop {
        operation.check_active().map_err(anyhow::Error::new)?;
        let wait = operation
            .remaining()
            .map(|remaining| remaining.min(poll_interval))
            .unwrap_or(poll_interval);
        tokio::select! {
            result = &mut future => {
                let result = result?;
                operation.check_active().map_err(anyhow::Error::new)?;
                return Ok(result);
            }
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

fn parse_deadline(arguments: &Value) -> Result<Option<u64>> {
    arguments
        .get("deadline_unix_ms")
        .map(|value| {
            value
                .as_u64()
                .context("MCP tool deadline_unix_ms must be an unsigned integer")
        })
        .transpose()
}

fn request_key(id: &Value) -> Result<String> {
    if id.is_null() || id.is_array() || id.is_object() {
        bail!("MCP request id must be a string or number");
    }
    serde_json::to_string(id).context("failed to encode MCP request id")
}

fn is_read_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "explain"
            | "search"
            | "context"
            | "impact"
            | "change_map"
            | "rustok_architecture_context"
            | "check"
    )
}

fn is_cancel_notification(method: &str) -> bool {
    matches!(method, "notifications/cancelled" | "$/cancelRequest")
}

async fn cancel_notification(active_reads: &ActiveReads, params: Option<&Value>) {
    let request_id = params.and_then(|params| {
        params
            .get("requestId")
            .or_else(|| params.get("id"))
    });
    let Some(request_id) = request_id else {
        return;
    };
    let Ok(request_key) = request_key(request_id) else {
        return;
    };
    if let Some(cancellation) = active_reads.lock().await.get(&request_key).cloned() {
        cancellation.cancel();
    }
}

async fn cancel_all(active_reads: &ActiveReads) {
    let cancellations = active_reads
        .lock()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for cancellation in cancellations {
        cancellation.cancel();
    }
}

fn is_operation_termination(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<CoreError>().is_some_and(|error| {
            matches!(error, CoreError::Cancelled(_) | CoreError::DeadlineExceeded(_))
        })
    })
}

fn response_json(id: Value, result: Result<Value>) -> String {
    let response = match result {
        Ok(result) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        }),
        Err(error) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": legacy::rpc_error_bridge(&error)
        }),
    };
    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"internal MCP tool error"}}"#.to_string()
    })
}

fn parse_error_response(error: &serde_json::Error) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": {
            "code": -32700,
            "message": format!("Parse error: {error}")
        }
    }))
    .expect("parse error response is serializable")
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use super::*;

    #[tokio::test]
    async fn cancellation_notification_terminates_registered_read() {
        let active_reads = ActiveReads::default();
        let operation = OperationContext::new("mcp:\"request-1\"");
        let task_active = Arc::clone(&active_reads);
        let task = tokio::spawn(async move {
            run_registered_read(
                &task_active,
                "\"request-1\"".to_string(),
                operation,
                pending::<Result<()>>(),
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if active_reads.lock().await.contains_key("\"request-1\"") {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("read registered");

        cancel_notification(
            &active_reads,
            Some(&json!({ "requestId": "request-1" })),
        )
        .await;
        let error = task
            .await
            .expect("read task joined")
            .expect_err("cancelled read must fail");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
        assert!(active_reads.lock().await.is_empty());
    }

    #[tokio::test]
    async fn deadline_terminates_registered_read_and_releases_lease() {
        let active_reads = ActiveReads::default();
        let operation = OperationContext::new("mcp:7").with_deadline_unix_ms(0);

        let error = run_registered_read(
            &active_reads,
            "7".to_string(),
            operation,
            pending::<Result<()>>(),
        )
        .await
        .expect_err("expired read deadline must fail");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::DeadlineExceeded(_))
        )));
        assert!(active_reads.lock().await.is_empty());
    }

    #[test]
    fn supports_standard_and_legacy_cancellation_notifications() {
        assert!(is_cancel_notification("notifications/cancelled"));
        assert!(is_cancel_notification("$/cancelRequest"));
        assert!(!is_cancel_notification("tools/call"));
    }

    #[test]
    fn read_tool_scope_excludes_transactional_index() {
        assert!(is_read_tool("search"));
        assert!(is_read_tool("change_map"));
        assert!(!is_read_tool("index"));
    }
}
