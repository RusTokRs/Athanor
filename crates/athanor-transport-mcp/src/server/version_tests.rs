use std::sync::Arc;

use serde_json::json;
use tokio::sync::Mutex;

use crate::transport_contract::{MCP_PROTOCOL_VERSION, MCP_SUPPORTED_PROTOCOL_VERSIONS};

use super::protocol::handle_initialize;
use super::types::{McpSessionPhase, SessionState};

#[tokio::test]
async fn initialize_echoes_each_supported_protocol_version() {
    for requested in MCP_SUPPORTED_PROTOCOL_VERSIONS {
        let session = awaiting_initialize();
        let result = handle_initialize(Some(json!({ "protocolVersion": requested })), &session)
            .await
            .expect("supported MCP protocol version must initialize");

        assert_eq!(result["protocolVersion"], *requested);
        assert_eq!(*session.lock().await, McpSessionPhase::AwaitingInitialized);
    }
}

#[tokio::test]
async fn initialize_proposes_latest_version_when_requested_version_is_unknown() {
    let session = awaiting_initialize();
    let result = handle_initialize(Some(json!({ "protocolVersion": "2099-01-01" })), &session)
        .await
        .expect("unknown client version must receive the latest supported server version");

    assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
    assert_eq!(*session.lock().await, McpSessionPhase::AwaitingInitialized);
}

fn awaiting_initialize() -> SessionState {
    Arc::new(Mutex::new(McpSessionPhase::AwaitingInitialize))
}
