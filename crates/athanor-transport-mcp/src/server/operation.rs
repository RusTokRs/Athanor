use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use athanor_app::RuntimeComposition;
use athanor_core::{CoreError, OperationContext, OperationContextCancellation};
use serde_json::{Value, json};

use crate::tools;

use super::types::{ActiveReads, DispatchError, DispatchResult};

pub(super) async fn handle_tool_call(
    root: &PathBuf,
    composition: &RuntimeComposition,
    request_id: &Value,
    params: Option<Value>,
    active_reads: &ActiveReads,
) -> DispatchResult {
    let params = super::protocol::params_object(params, "tools/call")?;
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            DispatchError::Protocol(super::types::RpcError::invalid_params(
                "tools/call requires non-empty string name",
            ))
        })?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return Err(DispatchError::Protocol(
            super::types::RpcError::invalid_params("tools/call arguments must be an object"),
        ));
    }

    let operation =
        operation_context(request_id, &arguments).map_err(DispatchError::Application)?;
    let request_key = request_key(request_id).map_err(DispatchError::Application)?;
    let result = if is_durable_commit_tool(tool_name) {
        let future = tools::call(root, tool_name, arguments, composition, &operation);
        run_registered_durable_operation(active_reads, request_key, operation, future).await
    } else if is_read_tool(tool_name) {
        let future = tools::call(root, tool_name, arguments, composition, &operation);
        if is_drained_operation_tool(tool_name) {
            run_registered_drained_read(active_reads, request_key, operation, future).await
        } else {
            run_registered_read(active_reads, request_key, operation, future).await
        }
    } else {
        await_read_operation(
            &operation,
            tools::call(root, tool_name, arguments, composition, &operation),
        )
        .await
    };

    match result {
        Ok(content) => Ok(json!({
            "content": [{ "type": "text", "text": content }]
        })),
        Err(error) if is_operation_termination(&error) => Err(DispatchError::Application(error)),
        Err(error) => Ok(json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!("Error calling tool: {error:?}")
            }]
        })),
    }
}

fn operation_context(request_id: &Value, arguments: &Value) -> Result<OperationContext> {
    let mut operation = OperationContext::new(format!("mcp:{}", request_key(request_id)?));
    if let Some(deadline_unix_ms) = parse_deadline(arguments)? {
        operation = operation.with_deadline_unix_ms(deadline_unix_ms);
    }
    Ok(operation)
}

pub(super) async fn run_registered_read<T>(
    active_reads: &ActiveReads,
    request_key: String,
    operation: OperationContext,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    register_read(active_reads, &request_key, &operation).await?;
    let result = await_read_operation(&operation, future).await;
    active_reads.lock().await.remove(&request_key);
    result
}

pub(super) async fn run_registered_drained_read<T>(
    active_reads: &ActiveReads,
    request_key: String,
    operation: OperationContext,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    register_read(active_reads, &request_key, &operation).await?;
    let result = future.await;
    let terminal = operation.check_active().map_err(anyhow::Error::new);
    let result = match terminal {
        Err(error) => Err(error),
        Ok(()) => result,
    };
    active_reads.lock().await.remove(&request_key);
    result
}

/// Registers cancellation for an operation with a durable commit boundary.
///
/// The application future owns pre-commit cancellation and post-commit reconciliation. The MCP
/// transport deliberately does not poll or postflight-check the operation after the future returns,
/// because cancellation racing with a durable commit must not replace a successful report.
pub(super) async fn run_registered_durable_operation<T>(
    active_reads: &ActiveReads,
    request_key: String,
    operation: OperationContext,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    register_read(active_reads, &request_key, &operation).await?;
    let result = future.await;
    active_reads.lock().await.remove(&request_key);
    result
}

async fn register_read(
    active_reads: &ActiveReads,
    request_key: &str,
    operation: &OperationContext,
) -> Result<()> {
    operation.check_active().map_err(anyhow::Error::new)?;
    let cancellation = operation
        .cancellation_handle()
        .map_err(anyhow::Error::new)?;
    let mut active = active_reads.lock().await;
    if active.contains_key(request_key) {
        bail!("MCP request id `{request_key}` is already active");
    }
    active.insert(request_key.to_string(), cancellation);
    Ok(())
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

pub(super) fn request_key(id: &Value) -> Result<String> {
    if !id.is_string() && !id.is_number() && !id.is_null() {
        bail!("MCP request id must be a string, number, or null");
    }
    serde_json::to_string(id).context("failed to encode MCP request id")
}

fn is_durable_commit_tool(tool_name: &str) -> bool {
    matches!(tool_name, "index")
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

fn is_drained_operation_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "search" | "context" | "change_map" | "rustok_architecture_context"
    )
}

pub(super) async fn cancel_notification(active_reads: &ActiveReads, params: Option<&Value>) {
    let request_id = params.and_then(|params| params.get("requestId").or_else(|| params.get("id")));
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

pub(super) async fn cancel_all(active_reads: &ActiveReads) {
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
            matches!(
                error,
                CoreError::Cancelled(_) | CoreError::DeadlineExceeded(_)
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use tokio::sync::Mutex;

    use super::*;

    #[tokio::test]
    async fn durable_operation_preserves_success_after_registered_cancellation() {
        let active_reads: ActiveReads = Arc::new(Mutex::new(HashMap::new()));
        let request_key = format!("durable-{}", test_nonce());
        let operation = OperationContext::new(format!("mcp-durable-{}", test_nonce()));
        let active_for_future = Arc::clone(&active_reads);
        let request_for_future = request_key.clone();

        let (value, cancellation) = run_registered_durable_operation(
            &active_reads,
            request_key.clone(),
            operation.clone(),
            async move {
                let cancellation = active_for_future
                    .lock()
                    .await
                    .get(&request_for_future)
                    .cloned()
                    .expect("durable operation must be registered");
                cancellation.cancel();
                Ok((42_u8, cancellation))
            },
        )
        .await
        .expect("registered cancellation after durable success must not mask the result");

        assert_eq!(value, 42);
        assert!(cancellation.is_cancelled());
        assert!(operation.is_cancelled());
        assert!(!active_reads.lock().await.contains_key(&request_key));
    }

    #[test]
    fn index_is_the_durable_commit_tool() {
        assert!(is_durable_commit_tool("index"));
        assert!(!is_durable_commit_tool("search"));
    }

    fn test_nonce() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time")
            .as_nanos()
    }
}
