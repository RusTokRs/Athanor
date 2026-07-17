use std::collections::BTreeSet;

use athanor_transport_mcp::{
    DEFAULT_MAX_IN_FLIGHT_REQUESTS, DEFAULT_RESPONSE_QUEUE_CAPACITY, JSON_RPC_VERSION,
    MCP_PROTOCOL_VERSION, MCP_TRANSPORT_CONTRACTS, validate_initialize_result,
    validate_json_rpc_request, validate_json_rpc_response, validate_tool_call_result,
};
use serde_json::Value;

const RUNTIME_SOURCE: &str = include_str!("../src/runtime.rs");
const SERVER_SOURCE: &str = include_str!("../src/server.rs");
const TOOLS_SOURCE: &str = include_str!("../src/tools.rs");
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
    assert!(MCP_TRANSPORT_CONTRACTS.iter().all(|contract| {
        contract.version == JSON_RPC_VERSION || contract.version == MCP_PROTOCOL_VERSION
    }));

    assert!(SERVER_SOURCE.contains("struct JsonRpcRequest"));
    assert!(SERVER_SOURCE.contains("struct RpcError"));
    assert!(SERVER_SOURCE.contains("MCP_PROTOCOL_VERSION"));
    assert!(SERVER_SOURCE.contains("\"isError\": true"));
    assert!(SERVER_SOURCE.contains("\"type\": \"text\""));
    assert!(
        !SERVER_SOURCE.contains("athanor.mcp_") && !TOOLS_SOURCE.contains("athanor.mcp_"),
        "standard MCP envelopes must not acquire an Athanor schema id"
    );
}

#[test]
fn active_mcp_server_is_bounded_and_reaps_request_tasks() {
    assert!(DEFAULT_MAX_IN_FLIGHT_REQUESTS > 0);
    assert!(DEFAULT_RESPONSE_QUEUE_CAPACITY > 0);
    assert!(SERVER_SOURCE.contains("mpsc::channel::<String>"));
    assert!(!SERVER_SOURCE.contains("unbounded_channel"));
    assert!(SERVER_SOURCE.contains("requests.len() < limits.max_in_flight_requests"));
    assert!(SERVER_SOURCE.contains("requests.join_next()"));
    assert!(SERVER_SOURCE.contains("bounded_response_queue_applies_backpressure"));
    assert!(SERVER_SOURCE.contains("completed_request_tasks_are_reaped"));
}

#[test]
fn active_mcp_server_enforces_protocol_and_session_semantics() {
    assert!(SERVER_SOURCE.contains("Some(JSON_RPC_VERSION)"));
    assert!(SERVER_SOURCE.contains("McpSessionPhase::AwaitingInitialize"));
    assert!(SERVER_SOURCE.contains("McpSessionPhase::AwaitingInitialized"));
    assert!(SERVER_SOURCE.contains("McpSessionPhase::Ready"));
    assert!(SERVER_SOURCE.contains("code: -32700"));
    assert!(SERVER_SOURCE.contains("code: -32600"));
    assert!(SERVER_SOURCE.contains("code: -32601"));
    assert!(SERVER_SOURCE.contains("code: -32602"));
    assert!(SERVER_SOURCE.contains("code: -32002"));
    assert!(SERVER_SOURCE.contains("explicit_null_receives_response"));
}

#[test]
fn active_mcp_dispatch_is_explicitly_composed() {
    assert!(RUNTIME_SOURCE.contains("run_mcp_server_with_composition"));
    assert!(!RUNTIME_SOURCE.contains("include!("));
    assert!(SERVER_SOURCE.contains("Arc<RuntimeComposition>"));
    assert!(SERVER_SOURCE.contains("Arc::clone(composition)"));
    assert!(TOOLS_SOURCE.contains("index_project_with_composition"));
    assert!(TOOLS_SOURCE.contains("search_project_with_composition_and_operation_context"));
    assert!(TOOLS_SOURCE.contains(
        "rustok_architecture_context_with_composition_and_operation_context"
    ));
    assert!(!TOOLS_SOURCE.contains("athanor_runtime_defaults::install()"));
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
