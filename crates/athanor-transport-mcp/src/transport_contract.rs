use std::error::Error;
use std::fmt;

use serde_json::Value;

pub const JSON_RPC_VERSION: &str = "2.0";
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTransportBoundary {
    JsonRpcRequest,
    JsonRpcResponse,
    JsonRpcError,
    ToolCallResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpTransportContractDescriptor {
    pub name: &'static str,
    pub version: &'static str,
    pub boundary: McpTransportBoundary,
}

pub const MCP_TRANSPORT_CONTRACTS: &[McpTransportContractDescriptor] = &[
    McpTransportContractDescriptor {
        name: "JsonRpcRequest",
        version: JSON_RPC_VERSION,
        boundary: McpTransportBoundary::JsonRpcRequest,
    },
    McpTransportContractDescriptor {
        name: "JsonRpcResponse",
        version: JSON_RPC_VERSION,
        boundary: McpTransportBoundary::JsonRpcResponse,
    },
    McpTransportContractDescriptor {
        name: "JsonRpcError",
        version: JSON_RPC_VERSION,
        boundary: McpTransportBoundary::JsonRpcError,
    },
    McpTransportContractDescriptor {
        name: "McpToolCallResult",
        version: MCP_PROTOCOL_VERSION,
        boundary: McpTransportBoundary::ToolCallResult,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpTransportContractError(pub String);

impl fmt::Display for McpTransportContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for McpTransportContractError {}

pub fn validate_json_rpc_request(value: &Value) -> Result<(), McpTransportContractError> {
    let request = object(value, "JSON-RPC request")?;
    version(request.get("jsonrpc"), JSON_RPC_VERSION, "JSON-RPC request")?;
    if request.get("method").and_then(Value::as_str).is_none() {
        return Err(McpTransportContractError(
            "JSON-RPC request requires string method".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_json_rpc_response(value: &Value) -> Result<(), McpTransportContractError> {
    let response = object(value, "JSON-RPC response")?;
    version(
        response.get("jsonrpc"),
        JSON_RPC_VERSION,
        "JSON-RPC response",
    )?;
    let has_result = response.contains_key("result");
    let has_error = response.contains_key("error");
    if has_result == has_error {
        return Err(McpTransportContractError(
            "JSON-RPC response requires exactly one of result or error".to_string(),
        ));
    }
    if let Some(error) = response.get("error") {
        validate_json_rpc_error(error)?;
    }
    Ok(())
}

pub fn validate_json_rpc_error(value: &Value) -> Result<(), McpTransportContractError> {
    let error = object(value, "JSON-RPC error")?;
    if error.get("code").and_then(Value::as_i64).is_none() {
        return Err(McpTransportContractError(
            "JSON-RPC error requires integer code".to_string(),
        ));
    }
    if error.get("message").and_then(Value::as_str).is_none() {
        return Err(McpTransportContractError(
            "JSON-RPC error requires string message".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_tool_call_result(value: &Value) -> Result<(), McpTransportContractError> {
    let result = object(value, "MCP tool-call result")?;
    if result
        .get("isError")
        .is_some_and(|value| !value.is_boolean())
    {
        return Err(McpTransportContractError(
            "MCP tool-call isError must be boolean".to_string(),
        ));
    }
    let content = result
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            McpTransportContractError("MCP tool-call result requires content array".to_string())
        })?;
    if content.is_empty() {
        return Err(McpTransportContractError(
            "MCP tool-call content must not be empty".to_string(),
        ));
    }
    for item in content {
        let item = object(item, "MCP content item")?;
        if item.get("type").and_then(Value::as_str) != Some("text") {
            return Err(McpTransportContractError(
                "MCP content item must use text type".to_string(),
            ));
        }
        if item.get("text").and_then(Value::as_str).is_none() {
            return Err(McpTransportContractError(
                "MCP text content requires string text".to_string(),
            ));
        }
    }
    Ok(())
}

pub fn validate_initialize_result(value: &Value) -> Result<(), McpTransportContractError> {
    let result = object(value, "MCP initialize result")?;
    version(
        result.get("protocolVersion"),
        MCP_PROTOCOL_VERSION,
        "MCP initialize result",
    )
}

fn object<'a>(
    value: &'a Value,
    label: &str,
) -> Result<&'a serde_json::Map<String, Value>, McpTransportContractError> {
    value
        .as_object()
        .ok_or_else(|| McpTransportContractError(format!("{label} must be an object")))
}

fn version(
    value: Option<&Value>,
    expected: &str,
    label: &str,
) -> Result<(), McpTransportContractError> {
    let actual = value.and_then(Value::as_str).unwrap_or("<missing>");
    if actual != expected {
        return Err(McpTransportContractError(format!(
            "{label} version {actual} does not match {expected}"
        )));
    }
    Ok(())
}
