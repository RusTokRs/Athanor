use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CancellationHandle, CoreError, OperationContext, OperationContextCancellation,
};
use serde_json::{Map, Value, json};
use tokio::io::{
    self, AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio::sync::{Mutex, mpsc};
use tokio::task::{JoinError, JoinSet};

use crate::legacy;
use crate::transport_contract::{JSON_RPC_VERSION, MCP_PROTOCOL_VERSION};

pub const DEFAULT_MAX_IN_FLIGHT_REQUESTS: usize = 32;
pub const DEFAULT_RESPONSE_QUEUE_CAPACITY: usize = 32;

type ActiveReads = Arc<Mutex<HashMap<String, CancellationHandle>>>;
type SessionState = Arc<Mutex<McpSessionPhase>>;
type RequestTasks = JoinSet<Result<()>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct McpServerLimits {
    max_in_flight_requests: usize,
    response_queue_capacity: usize,
}

impl Default for McpServerLimits {
    fn default() -> Self {
        Self {
            max_in_flight_requests: DEFAULT_MAX_IN_FLIGHT_REQUESTS,
            response_queue_capacity: DEFAULT_RESPONSE_QUEUE_CAPACITY,
        }
    }
}

impl McpServerLimits {
    fn validate(self) -> Result<Self> {
        if self.max_in_flight_requests == 0 {
            bail!("MCP max in-flight request limit must be greater than zero");
        }
        if self.response_queue_capacity == 0 {
            bail!("MCP response queue capacity must be greater than zero");
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpSessionPhase {
    AwaitingInitialize,
    AwaitingInitialized,
    Ready,
}

#[derive(Debug)]
enum RequestId {
    Omitted,
    Present(Value),
}

impl RequestId {
    fn is_notification(&self) -> bool {
        matches!(self, Self::Omitted)
    }

    fn into_value(self) -> Option<Value> {
        match self {
            Self::Omitted => None,
            Self::Present(value) => Some(value),
        }
    }

    fn response_value(&self) -> Value {
        match self {
            Self::Omitted => Value::Null,
            Self::Present(value) => value.clone(),
        }
    }
}

#[derive(Debug)]
struct JsonRpcRequest {
    id: RequestId,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Clone)]
struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl RpcError {
    fn parse(error: &serde_json::Error) -> Self {
        Self {
            code: -32700,
            message: format!("Parse error: {error}"),
            data: None,
        }
    }

    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: message.into(),
            data: None,
        }
    }

    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }

    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    fn server_not_initialized() -> Self {
        Self {
            code: -32002,
            message: "Server not initialized".to_string(),
            data: None,
        }
    }

    fn value(&self) -> Value {
        let mut error = Map::new();
        error.insert("code".to_string(), json!(self.code));
        error.insert("message".to_string(), Value::String(self.message.clone()));
        if let Some(data) = &self.data {
            error.insert("data".to_string(), data.clone());
        }
        Value::Object(error)
    }
}

#[derive(Debug)]
struct ProtocolFailure {
    id: Value,
    error: RpcError,
}

impl ProtocolFailure {
    fn parse(error: &serde_json::Error) -> Self {
        Self {
            id: Value::Null,
            error: RpcError::parse(error),
        }
    }

    fn invalid_request(id: Value, message: impl Into<String>) -> Self {
        Self {
            id,
            error: RpcError::invalid_request(message),
        }
    }

    fn response(&self) -> String {
        error_response_json(self.id.clone(), self.error.value())
    }
}

#[derive(Debug)]
enum DispatchError {
    Protocol(RpcError),
    Application(anyhow::Error),
}

type DispatchResult = std::result::Result<Value, DispatchError>;

/// Runs the MCP stdio server with bounded concurrent request processing,
/// serialized response writing, and request-scoped cancellation.
pub async fn run_mcp_server(root: PathBuf) -> Result<()> {
    run_mcp_server_io(
        root,
        BufReader::new(io::stdin()),
        io::stdout(),
        McpServerLimits::default(),
    )
    .await
}

async fn run_mcp_server_io<R, W>(
    root: PathBuf,
    reader: R,
    writer: W,
    limits: McpServerLimits,
) -> Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let limits = limits.validate()?;
    let root = Arc::new(root);
    let active_reads = ActiveReads::default();
    let session = Arc::new(Mutex::new(McpSessionPhase::AwaitingInitialize));
    let (responses_tx, responses_rx) = mpsc::channel::<String>(limits.response_queue_capacity);
    let writer_task = tokio::spawn(write_responses(writer, responses_rx));
    let mut requests = RequestTasks::new();
    let mut lines = reader.lines();
    let mut stdin_open = true;

    eprintln!("Athanor MCP server starting in {}", root.display());

    while stdin_open || !requests.is_empty() {
        if stdin_open && requests.len() < limits.max_in_flight_requests {
            tokio::select! {
                biased;
                joined = requests.join_next(), if !requests.is_empty() => {
                    log_request_task(joined.expect("guarded non-empty request set"));
                }
                line = lines.next_line() => {
                    match line? {
                        Some(line) => {
                            process_line(
                                &root,
                                &active_reads,
                                &session,
                                &responses_tx,
                                &mut requests,
                                line,
                            )
                            .await?;
                        }
                        None => {
                            stdin_open = false;
                            cancel_all(&active_reads).await;
                        }
                    }
                }
            }
        } else if let Some(joined) = requests.join_next().await {
            log_request_task(joined);
        } else {
            break;
        }
    }

    cancel_all(&active_reads).await;
    drop(responses_tx);
    writer_task
        .await
        .context("MCP response writer task terminated unexpectedly")?
        .context("failed to write MCP response stream")?;
    Ok(())
}

async fn process_line(
    root: &Arc<PathBuf>,
    active_reads: &ActiveReads,
    session: &SessionState,
    responses_tx: &mpsc::Sender<String>,
    requests: &mut RequestTasks,
    line: String,
) -> Result<()> {
    if line.trim().is_empty() {
        return Ok(());
    }

    let request = match parse_request(&line) {
        Ok(request) => request,
        Err(failure) => {
            send_response(responses_tx, failure.response()).await?;
            return Ok(());
        }
    };

    if request.id.is_notification() {
        handle_notification(active_reads, session, request).await;
        return Ok(());
    }

    if request.method == "initialize" {
        let id = request.id.response_value();
        let result = handle_initialize(request.params, session).await;
        send_response(responses_tx, response_json(id, result)).await?;
        return Ok(());
    }

    let root = Arc::clone(root);
    let active_reads = Arc::clone(active_reads);
    let session = Arc::clone(session);
    let responses_tx = responses_tx.clone();
    requests.spawn(async move {
        let id = request
            .id
            .into_value()
            .expect("request tasks only contain explicit ids");
        let response = handle_request(
            &root,
            request.method,
            request.params,
            &id,
            &active_reads,
            &session,
        )
        .await;
        send_response(&responses_tx, response_json(id, response)).await
    });
    Ok(())
}

async fn write_responses<W>(
    mut writer: W,
    mut responses: mpsc::Receiver<String>,
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    while let Some(response) = responses.recv().await {
        writer.write_all(response.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }
    writer.flush().await
}

async fn send_response(responses: &mpsc::Sender<String>, response: String) -> Result<()> {
    responses
        .send(response)
        .await
        .context("MCP response writer is unavailable")
}

fn log_request_task(joined: std::result::Result<Result<()>, JoinError>) {
    match joined {
        Ok(Ok(())) => {}
        Ok(Err(error)) => eprintln!("MCP request response failed: {error:#}"),
        Err(error) => eprintln!("MCP request task terminated unexpectedly: {error}"),
    }
}

fn parse_request(line: &str) -> std::result::Result<JsonRpcRequest, ProtocolFailure> {
    let value: Value = serde_json::from_str(line).map_err(|error| ProtocolFailure::parse(&error))?;
    let object = value.as_object().ok_or_else(|| {
        ProtocolFailure::invalid_request(Value::Null, "JSON-RPC request must be an object")
    })?;

    let id = match object.get("id") {
        None => RequestId::Omitted,
        Some(value) if value.is_string() || value.is_number() || value.is_null() => {
            RequestId::Present(value.clone())
        }
        Some(_) => {
            return Err(ProtocolFailure::invalid_request(
                Value::Null,
                "JSON-RPC request id must be a string, number, or null",
            ));
        }
    };
    let response_id = id.response_value();

    if object.get("jsonrpc").and_then(Value::as_str) != Some(JSON_RPC_VERSION) {
        return Err(ProtocolFailure::invalid_request(
            response_id,
            format!("JSON-RPC request requires version {JSON_RPC_VERSION}"),
        ));
    }

    let method = object
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ProtocolFailure::invalid_request(
                response_id.clone(),
                "JSON-RPC request requires string method",
            )
        })?
        .to_string();

    let params = object.get("params").cloned();
    if params
        .as_ref()
        .is_some_and(|value| !value.is_object() && !value.is_array())
    {
        return Err(ProtocolFailure::invalid_request(
            response_id,
            "JSON-RPC params must be an object or array",
        ));
    }

    Ok(JsonRpcRequest { id, method, params })
}

async fn handle_notification(
    active_reads: &ActiveReads,
    session: &SessionState,
    request: JsonRpcRequest,
) {
    if is_cancel_notification(&request.method) {
        cancel_notification(active_reads, request.params.as_ref()).await;
    } else if is_initialized_notification(&request.method)
        && let Err(error) = mark_initialized(session).await
    {
        eprintln!(
            "ignored invalid MCP initialized notification: {}",
            error.message
        );
    }
}

async fn handle_initialize(params: Option<Value>, session: &SessionState) -> DispatchResult {
    let params = params_object(params, "initialize")?;
    let requested_version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            DispatchError::Protocol(RpcError::invalid_params(
                "initialize requires string protocolVersion",
            ))
        })?;
    if requested_version != MCP_PROTOCOL_VERSION {
        return Err(DispatchError::Protocol(RpcError::invalid_params(format!(
            "unsupported MCP protocol version {requested_version}; expected {MCP_PROTOCOL_VERSION}",
        ))));
    }

    let mut phase = session.lock().await;
    if *phase != McpSessionPhase::AwaitingInitialize {
        return Err(DispatchError::Protocol(RpcError::invalid_request(
            "MCP session is already initialized",
        )));
    }
    *phase = McpSessionPhase::AwaitingInitialized;

    Ok(json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "athanor",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

async fn handle_request(
    root: &Path,
    method: String,
    params: Option<Value>,
    request_id: &Value,
    active_reads: &ActiveReads,
    session: &SessionState,
) -> DispatchResult {
    match method.as_str() {
        "initialized" | "notifications/initialized" => {
            mark_initialized(session)
                .await
                .map_err(DispatchError::Protocol)?;
            Ok(json!({}))
        }
        "tools/list" => {
            require_ready(session).await?;
            Ok(legacy::tools_list_bridge())
        }
        "tools/call" => {
            require_ready(session).await?;
            handle_tool_call(root, request_id, params, active_reads).await
        }
        other => Err(DispatchError::Protocol(RpcError::method_not_found(other))),
    }
}

async fn require_ready(session: &SessionState) -> std::result::Result<(), DispatchError> {
    if *session.lock().await == McpSessionPhase::Ready {
        Ok(())
    } else {
        Err(DispatchError::Protocol(RpcError::server_not_initialized()))
    }
}

async fn mark_initialized(session: &SessionState) -> std::result::Result<(), RpcError> {
    let mut phase = session.lock().await;
    match *phase {
        McpSessionPhase::AwaitingInitialized => {
            *phase = McpSessionPhase::Ready;
            Ok(())
        }
        McpSessionPhase::Ready => Ok(()),
        McpSessionPhase::AwaitingInitialize => Err(RpcError::invalid_request(
            "initialized notification received before initialize",
        )),
    }
}

fn params_object(
    params: Option<Value>,
    method: &str,
) -> std::result::Result<Map<String, Value>, DispatchError> {
    match params {
        Some(Value::Object(params)) => Ok(params),
        Some(_) => Err(DispatchError::Protocol(RpcError::invalid_params(format!(
            "{method} params must be an object",
        )))),
        None => Err(DispatchError::Protocol(RpcError::invalid_params(format!(
            "{method} requires params",
        )))),
    }
}

async fn handle_tool_call(
    root: &Path,
    request_id: &Value,
    params: Option<Value>,
    active_reads: &ActiveReads,
) -> DispatchResult {
    let params = params_object(params, "tools/call")?;
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            DispatchError::Protocol(RpcError::invalid_params(
                "tools/call requires non-empty string name",
            ))
        })?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return Err(DispatchError::Protocol(RpcError::invalid_params(
            "tools/call arguments must be an object",
        )));
    }

    let result = if is_read_tool(tool_name) {
        call_read_tool(root, request_id, tool_name, arguments, active_reads).await
    } else {
        legacy::call_tool_bridge(root, tool_name, arguments).await
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

    if is_drained_operation_tool(tool_name) {
        let root = root.to_path_buf();
        let tool_name = tool_name.to_string();
        let execution_operation = operation.clone();
        run_registered_drained_read(
            active_reads,
            request_key,
            operation,
            async move {
                call_operation_read_tool(&root, &tool_name, arguments, &execution_operation).await
            },
        )
        .await
    } else {
        run_registered_read(
            active_reads,
            request_key,
            operation,
            legacy::call_tool_inner_bridge(root, tool_name, arguments),
        )
        .await
    }
}

async fn call_operation_read_tool(
    root: &Path,
    tool_name: &str,
    arguments: Value,
    operation: &OperationContext,
) -> Result<String> {
    match tool_name {
        "search" => {
            let query = arguments
                .get("query")
                .and_then(Value::as_str)
                .context("missing query")?
                .to_string();
            let limit = arguments
                .get("limit")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .unwrap_or(10);
            let report = athanor_app::search_project_with_operation_context(
                athanor_app::SearchOptions {
                    root: root.to_path_buf(),
                    query,
                    limit,
                },
                operation,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "context" => {
            let task = arguments
                .get("task")
                .and_then(Value::as_str)
                .context("missing task")?
                .to_string();
            let level = match arguments
                .get("level")
                .and_then(Value::as_str)
                .unwrap_or("normal")
            {
                "summary" => athanor_domain::ContextLevel::Summary,
                "deep" => athanor_domain::ContextLevel::Deep,
                "full" => athanor_domain::ContextLevel::Full,
                _ => athanor_domain::ContextLevel::Normal,
            };
            let optional_limit = |name: &str| {
                arguments
                    .get(name)
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
            };
            let report = athanor_app::context_project_with_operation_context(
                athanor_app::ContextOptions {
                    root: root.to_path_buf(),
                    task,
                    diff: false,
                    level,
                    limits: athanor_app::ContextLimitOverrides {
                        max_tokens: optional_limit("max_tokens"),
                        max_files: optional_limit("max_files"),
                        max_entities: optional_limit("max_entities"),
                        max_diagnostics: optional_limit("max_diagnostics"),
                        max_depth: optional_limit("max_depth"),
                    },
                },
                operation,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        other => bail!("operation-aware MCP read is not implemented for `{other}`"),
    }
}

async fn run_registered_read<T>(
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

async fn run_registered_drained_read<T>(
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

fn request_key(id: &Value) -> Result<String> {
    if !id.is_string() && !id.is_number() && !id.is_null() {
        bail!("MCP request id must be a string, number, or null");
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

fn is_drained_operation_tool(tool_name: &str) -> bool {
    matches!(tool_name, "search" | "context")
}

fn is_cancel_notification(method: &str) -> bool {
    matches!(method, "notifications/cancelled" | "$/cancelRequest")
}

fn is_initialized_notification(method: &str) -> bool {
    matches!(method, "notifications/initialized" | "initialized")
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
            matches!(
                error,
                CoreError::Cancelled(_) | CoreError::DeadlineExceeded(_)
            )
        })
    })
}

fn response_json(id: Value, result: DispatchResult) -> String {
    let response = match result {
        Ok(result) => json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": id,
            "result": result
        }),
        Err(DispatchError::Protocol(error)) => json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": id,
            "error": error.value()
        }),
        Err(DispatchError::Application(error)) => json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": id,
            "error": legacy::rpc_error_bridge(&error)
        }),
    };
    serialize_response(response)
}

fn error_response_json(id: Value, error: Value) -> String {
    serialize_response(json!({
        "jsonrpc": JSON_RPC_VERSION,
        "id": id,
        "error": error
    }))
}

fn serialize_response(response: Value) -> String {
    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"internal MCP tool error"}}"#.to_string()
    })
}

#[cfg(test)]
mod tests {
    use std::future::pending;
    use std::sync::atomic::{AtomicBool, Ordering};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    async fn protocol_session_negotiates_before_tools_are_available() {
        let responses = exchange(&[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        ])
        .await;
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["id"], 1);
        assert_eq!(
            responses[0]["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
        assert_eq!(responses[1]["id"], 2);
        assert!(responses[1]["result"]["tools"].is_array());
    }

    #[tokio::test]
    async fn protocol_errors_use_standard_json_rpc_codes() {
        let parse = exchange(&["not-json"]).await;
        assert_eq!(parse[0]["error"]["code"], -32700);

        let invalid = exchange(&[
            r#"{"jsonrpc":"1.0","id":7,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        ])
        .await;
        assert_eq!(invalid[0]["id"], 7);
        assert_eq!(invalid[0]["error"]["code"], -32600);

        let not_initialized =
            exchange(&[r#"{"jsonrpc":"2.0","id":8,"method":"tools/list"}"#]).await;
        assert_eq!(not_initialized[0]["error"]["code"], -32002);

        let unknown = exchange(&[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":9,"method":"missing/method"}"#,
        ])
        .await;
        assert_eq!(unknown[1]["error"]["code"], -32601);

        let invalid_params = exchange(&[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":10,"method":"tools/call"}"#,
        ])
        .await;
        assert_eq!(invalid_params[1]["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn omitted_id_is_notification_but_explicit_null_receives_response() {
        let notification = exchange(&[
            r#"{"jsonrpc":"2.0","method":"unknown/notification"}"#,
        ])
        .await;
        assert!(notification.is_empty());

        let explicit_null = exchange(&[
            r#"{"jsonrpc":"2.0","id":null,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        ])
        .await;
        assert_eq!(explicit_null.len(), 1);
        assert!(explicit_null[0]["id"].is_null());
        assert_eq!(
            explicit_null[0]["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
    }

    #[tokio::test]
    async fn bounded_response_queue_applies_backpressure() {
        let (sender, mut receiver) = mpsc::channel(1);
        sender.send("first".to_string()).await.unwrap();
        let second = sender.send("second".to_string());
        tokio::pin!(second);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut second)
                .await
                .is_err()
        );
        assert_eq!(receiver.recv().await.as_deref(), Some("first"));
        tokio::time::timeout(Duration::from_secs(1), &mut second)
            .await
            .expect("second response admitted after capacity released")
            .unwrap();
    }

    #[tokio::test]
    async fn completed_request_tasks_are_reaped() {
        let mut requests = RequestTasks::new();
        requests.spawn(async { Ok(()) });
        let joined = requests.join_next().await.expect("completed request task");
        log_request_task(joined);
        assert!(requests.is_empty());
    }

    #[test]
    fn default_server_limits_are_bounded() {
        let limits = McpServerLimits::default().validate().unwrap();
        assert_eq!(
            limits.max_in_flight_requests,
            DEFAULT_MAX_IN_FLIGHT_REQUESTS
        );
        assert_eq!(
            limits.response_queue_capacity,
            DEFAULT_RESPONSE_QUEUE_CAPACITY
        );
        assert!(
            McpServerLimits {
                max_in_flight_requests: 0,
                response_queue_capacity: 1,
            }
            .validate()
            .is_err()
        );
        assert!(
            McpServerLimits {
                max_in_flight_requests: 1,
                response_queue_capacity: 0,
            }
            .validate()
            .is_err()
        );
    }

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
    async fn drained_read_waits_for_operation_future_cleanup() {
        let active_reads = ActiveReads::default();
        let operation = OperationContext::new("mcp:\"drained\"");
        let execution_operation = operation.clone();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_in_future = Arc::clone(&completed);
        let task_active = Arc::clone(&active_reads);
        let task = tokio::spawn(async move {
            run_registered_drained_read(
                &task_active,
                "\"drained\"".to_string(),
                operation,
                async move {
                    while !execution_operation.is_cancelled() {
                        tokio::task::yield_now().await;
                    }
                    completed_in_future.store(true, Ordering::Release);
                    execution_operation
                        .check_active()
                        .map_err(anyhow::Error::new)?;
                    Ok::<_, anyhow::Error>(())
                },
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if active_reads.lock().await.contains_key("\"drained\"") {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("drained read registered");

        cancel_notification(&active_reads, Some(&json!({ "requestId": "drained" }))).await;
        let error = task
            .await
            .expect("drained read task joined")
            .expect_err("cancelled drained read must fail");

        assert!(completed.load(Ordering::Acquire));
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

    #[test]
    fn rebuild_capable_tools_use_drained_operation_dispatch() {
        assert!(is_drained_operation_tool("search"));
        assert!(is_drained_operation_tool("context"));
        assert!(!is_drained_operation_tool("impact"));
    }

    #[test]
    fn request_ids_distinguish_omitted_from_explicit_null() {
        assert_eq!(request_key(&json!(7)).unwrap(), "7");
        assert_eq!(request_key(&json!("seven")).unwrap(), "\"seven\"");
        assert_eq!(request_key(&Value::Null).unwrap(), "null");
        assert!(request_key(&json!(true)).is_err());

        let omitted = parse_request(r#"{"jsonrpc":"2.0","method":"tools/list"}"#).unwrap();
        assert!(omitted.id.is_notification());
        let explicit_null =
            parse_request(r#"{"jsonrpc":"2.0","id":null,"method":"tools/list"}"#).unwrap();
        assert!(!explicit_null.id.is_notification());
    }

    async fn exchange(lines: &[&str]) -> Vec<Value> {
        let (mut input_client, input_server) = tokio::io::duplex(16 * 1024);
        let (output_server, mut output_client) = tokio::io::duplex(64 * 1024);
        let server = tokio::spawn(run_mcp_server_io(
            PathBuf::from("."),
            BufReader::new(input_server),
            output_server,
            McpServerLimits {
                max_in_flight_requests: 4,
                response_queue_capacity: 4,
            },
        ));

        for line in lines {
            input_client.write_all(line.as_bytes()).await.unwrap();
            input_client.write_all(b"\n").await.unwrap();
        }
        input_client.shutdown().await.unwrap();

        let mut output = String::new();
        output_client.read_to_string(&mut output).await.unwrap();
        server.await.unwrap().unwrap();

        output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}
