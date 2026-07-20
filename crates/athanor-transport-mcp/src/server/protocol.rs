use std::path::Path;

use athanor_app::RuntimeComposition;
use serde_json::{Map, Value, json};

use crate::tools;
use crate::transport_contract::{JSON_RPC_VERSION, MCP_PROTOCOL_VERSION};

use super::operation::{cancel_notification, handle_tool_call};
use super::types::{
    ActiveReads, DispatchError, DispatchResult, JsonRpcRequest, McpSessionPhase, ProtocolFailure,
    RequestId, RpcError, SessionState,
};

pub(super) fn parse_request(line: &str) -> std::result::Result<JsonRpcRequest, ProtocolFailure> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| ProtocolFailure::parse(&error))?;
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

pub(super) async fn handle_notification(
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

pub(super) async fn handle_initialize(
    params: Option<Value>,
    session: &SessionState,
) -> DispatchResult {
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

pub(super) async fn handle_request(
    root: &Path,
    composition: &RuntimeComposition,
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
            Ok(tools::list())
        }
        "tools/call" => {
            require_ready(session).await?;
            handle_tool_call(root, composition, request_id, params, active_reads).await
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

pub(super) fn params_object(
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

pub(super) fn is_cancel_notification(method: &str) -> bool {
    matches!(method, "notifications/cancelled" | "$/cancelRequest")
}

fn is_initialized_notification(method: &str) -> bool {
    matches!(method, "notifications/initialized" | "initialized")
}

pub(super) fn response_json(id: Value, result: DispatchResult) -> String {
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
            "error": tools::rpc_error(&error)
        }),
    };
    serialize_response(response)
}

fn serialize_response(response: Value) -> String {
    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"internal MCP tool error"}}"#.to_string()
    })
}
