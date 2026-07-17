use std::collections::BTreeSet;

use athanor_transport_mcp::{
    JSON_RPC_VERSION, MCP_PROTOCOL_VERSION, MCP_TRANSPORT_CONTRACTS, validate_initialize_result,
    validate_json_rpc_request, validate_json_rpc_response, validate_tool_call_result,
};
use serde_json::Value;

const LEGACY_RUNTIME_SOURCE: &str = include_str!("../src/lib.rs");
const FIXTURE: &str = include_str!("fixtures/mcp_transport_contracts.v1.json");

#[test]
fn standard_mcp_transport_fixture_is_valid() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("valid MCP transport fixture");

    validate_json_rpc_request(&fixture["request"]).expect("valid JSON-RPC request");
    validate_json_rpc_response(&fixture["success_response"])
        .expect("valid JSON-RPC success response");
    validate_json_rpc_response(&fixture["error_response"])
        .expect("valid JSON-RPC error response");
    validate_tool_call_result(&fixture["tool_success"]).expect("valid MCP tool success result");
    validate_tool_call_result(&fixture["tool_error"]).expect("valid MCP tool error result");
    validate_initialize_result(&fixture["initialize_result"])
        .expect("valid MCP initialize result");

    assert_eq!(fixture["request"]["jsonrpc"], JSON_RPC_VERSION);
    assert_eq!(
        fixture["initialize_result"]["protocolVersion"],
        MCP_PROTOCOL_VERSION
    );
}

#[test]
fn mcp_transport_registry_is_unique_and_separate_from_athanor_schema_registry() {
    let names = MCP_TRANSPORT_CONTRACTS
        .iter()
        .map(|contract| contract.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), MCP_TRANSPORT_CONTRACTS.len());
    assert_eq!(MCP_TRANSPORT_CONTRACTS.len(), 4);
    assert!(
        MCP_TRANSPORT_CONTRACTS
            .iter()
            .all(|contract| contract.version == JSON_RPC_VERSION || contract.version == MCP_PROTOCOL_VERSION)
    );

    assert!(LEGACY_RUNTIME_SOURCE.contains("struct JsonRpcRequest"));
    assert!(LEGACY_RUNTIME_SOURCE.contains("struct JsonRpcResponse"));
    assert!(LEGACY_RUNTIME_SOURCE.contains("struct JsonRpcError"));
    assert!(LEGACY_RUNTIME_SOURCE.contains("\"protocolVersion\": \"2024-11-05\""));
    assert!(LEGACY_RUNTIME_SOURCE.contains("\"isError\": true"));
    assert!(LEGACY_RUNTIME_SOURCE.contains("\"type\": \"text\""));
    assert!(
        !LEGACY_RUNTIME_SOURCE.contains("athanor.mcp_"),
        "standard MCP envelopes must not acquire an Athanor schema id"
    );
}

#[test]
fn malformed_envelopes_fail_closed() {
    let invalid_response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {},
        "error": { "code": -32603, "message": "invalid" }
    });
    assert!(validate_json_rpc_response(&invalid_response).is_err());

    let invalid_tool_result = serde_json::json!({
        "content": [{ "type": "image", "text": "not supported" }]
    });
    assert!(validate_tool_call_result(&invalid_tool_result).is_err());
}
