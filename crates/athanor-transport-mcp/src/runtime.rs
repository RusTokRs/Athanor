//! MCP transport entry points with request-scoped operation lifecycle.

mod server;
mod tools;
pub mod transport_contract;

use std::path::PathBuf;

use anyhow::Result;
use athanor_app::RuntimeComposition;

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

/// Type-level assertion that the explicit server entry point accepts the
/// application runtime composition rather than relying on process globals.
const _: fn(PathBuf, RuntimeComposition) -> _ = run_mcp_server_with_composition;
