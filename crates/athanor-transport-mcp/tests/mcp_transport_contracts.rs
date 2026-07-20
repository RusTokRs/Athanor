use std::collections::BTreeSet;

use athanor_transport_mcp::{
    DEFAULT_MAX_IN_FLIGHT_REQUESTS, DEFAULT_RESPONSE_QUEUE_CAPACITY, JSON_RPC_VERSION,
    MCP_PROTOCOL_VERSION, MCP_TRANSPORT_CONTRACTS, validate_initialize_result,
    validate_json_rpc_request, validate_json_rpc_response, validate_tool_call_result,
};
use serde_json::Value;

const RUNTIME_SOURCE: &str = include_str!("../src/runtime.rs");
const SERVER_MOD_SOURCE: &str = include_str!("../src/server/mod.rs");
const SERVER_TYPES_SOURCE: &str = include_str!("../src/server/types.rs");
const SERVER_PROTOCOL_SOURCE: &str = include_str!("../src/server/protocol.rs");
const SERVER_LIFECYCLE_SOURCE: &str = include_str!("../src/server/lifecycle.rs");
const SERVER_OPERATION_SOURCE: &str = include_str!("../src/server/operation.rs");
const SERVER_TESTS_SOURCE: &str = include_str!("../src/server/tests.rs");
const TOOLS_ROOT_SOURCE: &str = include_str!("../src/tools.rs");
const TOOLS_DISPATCH_SOURCE: &str = include_str!("../src/tools/dispatch.rs");
const TOOLS_SCHEMA_SOURCE: &str = include_str!("../src/tools/schema.rs");
const FIXTURE: &str = include_str!("fixtures/mcp_transport_contracts.v1.json");

/// Returns the concatenated source of all production server submodules (excluding tests).
fn server_source() -> String {
    [
        SERVER_MOD_SOURCE,
        SERVER_TYPES_SOURCE,
        SERVER_PROTOCOL_SOURCE,
        SERVER_LIFECYCLE_SOURCE,
        SERVER_OPERATION_SOURCE,
        SERVER_TESTS_SOURCE,
    ]
    .join("\n")
}

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

    let server_source = server_source();
    assert!(server_source.contains("struct JsonRpcRequest"));
    assert!(server_source.contains("struct RpcError"));
    assert!(server_source.contains("MCP_PROTOCOL_VERSION"));
    assert!(server_source.contains("\"isError\": true"));
    assert!(server_source.contains("\"type\": \"text\""));
    assert!(
        !server_source.contains("athanor.mcp_")
            && !TOOLS_DISPATCH_SOURCE.contains("athanor.mcp_")
            && !TOOLS_SCHEMA_SOURCE.contains("athanor.mcp_"),
        "standard MCP envelopes must not acquire an Athanor schema id"
    );
}

#[test]
fn active_mcp_server_is_bounded_and_reaps_request_tasks() {
    assert!(DEFAULT_MAX_IN_FLIGHT_REQUESTS > 0);
    assert!(DEFAULT_RESPONSE_QUEUE_CAPACITY > 0);
    let server_source = server_source();
    assert!(server_source.contains("mpsc::channel::<String>"));
    assert!(!server_source.contains("unbounded_channel"));
    assert!(server_source.contains("requests.len() < limits.max_in_flight_requests"));
    assert!(server_source.contains("requests.join_next()"));
    assert!(server_source.contains("bounded_response_queue_applies_backpressure"));
    assert!(server_source.contains("completed_request_tasks_are_reaped"));
}

#[test]
fn active_mcp_server_enforces_protocol_and_session_semantics() {
    let server_source = server_source();
    assert!(server_source.contains("Some(JSON_RPC_VERSION)"));
    assert!(server_source.contains("McpSessionPhase::AwaitingInitialize"));
    assert!(server_source.contains("McpSessionPhase::AwaitingInitialized"));
    assert!(server_source.contains("McpSessionPhase::Ready"));
    assert!(server_source.contains("code: -32700"));
    assert!(server_source.contains("code: -32600"));
    assert!(server_source.contains("code: -32601"));
    assert!(server_source.contains("code: -32602"));
    assert!(server_source.contains("code: -32002"));
    assert!(server_source.contains("explicit_null_receives_response"));
}

#[test]
fn active_mcp_dispatch_is_explicitly_composed() {
    assert!(RUNTIME_SOURCE.contains("run_mcp_server_with_composition"));
    assert!(!RUNTIME_SOURCE.contains("include!("));
    let server_source = server_source();
    assert!(server_source.contains("Arc<RuntimeComposition>"));
    assert!(server_source.contains("Arc::clone(composition)"));
    assert!(TOOLS_ROOT_SOURCE.contains("mod dispatch;"));
    assert!(TOOLS_ROOT_SOURCE.contains("mod schema;"));
    assert!(TOOLS_DISPATCH_SOURCE.contains("index_project_with_composition"));
    assert!(TOOLS_DISPATCH_SOURCE.contains(
        "search_project_with_composition_and_operation_context"
    ));
    assert!(TOOLS_DISPATCH_SOURCE.contains(
        "rustok_architecture_context_with_composition_and_operation_context"
    ));
    assert!(!TOOLS_DISPATCH_SOURCE.contains("athanor_runtime_defaults::install()"));
}

#[test]
fn mcp_production_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("runtime", RUNTIME_SOURCE, 90),
        ("tools root", TOOLS_ROOT_SOURCE, 20),
        ("tools schema", TOOLS_SCHEMA_SOURCE, 220),
        ("tools dispatch", TOOLS_DISPATCH_SOURCE, 330),
        ("server mod", SERVER_MOD_SOURCE, 25),
        ("server types", SERVER_TYPES_SOURCE, 220),
        ("server protocol", SERVER_PROTOCOL_SOURCE, 260),
        ("server lifecycle", SERVER_LIFECYCLE_SOURCE, 280),
        ("server operation", SERVER_OPERATION_SOURCE, 340),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
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
