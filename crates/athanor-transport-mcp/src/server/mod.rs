mod lifecycle;
mod operation;
mod protocol;
mod types;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod version_tests;

pub use lifecycle::run_mcp_server_with_composition;
pub use types::{DEFAULT_MAX_IN_FLIGHT_REQUESTS, DEFAULT_RESPONSE_QUEUE_CAPACITY};
