//! MCP transport entry point with explicit composition and request-scoped lifecycle.

mod server;
mod tools;
pub mod transport_contract;

pub use server::{
    DEFAULT_MAX_IN_FLIGHT_REQUESTS, DEFAULT_RESPONSE_QUEUE_CAPACITY,
    run_mcp_server_with_composition,
};
pub use transport_contract::*;
