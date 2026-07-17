//! MCP transport entry points with request-scoped operation lifecycle.

mod server;
mod tools;
pub mod transport_contract;

use std::path::PathBuf;

use anyhow::Result;

pub use server::{
    DEFAULT_MAX_IN_FLIGHT_REQUESTS, DEFAULT_RESPONSE_QUEUE_CAPACITY,
    run_mcp_server_with_composition,
};
pub use transport_contract::*;

/// Runs the MCP server with Athanor's production runtime composition.
///
/// Callers that already own a composition should prefer
/// [`run_mcp_server_with_composition`].
pub async fn run_mcp_server(root: PathBuf) -> Result<()> {
    run_mcp_server_with_composition(root, athanor_runtime_defaults::production()).await
}
