//! MCP transport entry point with explicit composition and request-scoped lifecycle.

mod server;
mod tools;
pub mod transport_contract;

pub use server::{
    DEFAULT_MAX_IN_FLIGHT_REQUESTS, DEFAULT_RESPONSE_QUEUE_CAPACITY,
    run_mcp_server_with_composition,
};
pub use transport_contract::*;

#[cfg(test)]
extern crate self as athanor_runtime_defaults;

#[cfg(test)]
pub(crate) fn production() -> athanor_app::RuntimeComposition {
    use std::future::Future;
    use std::path::Path;
    use std::pin::Pin;
    use std::sync::Arc;

    use anyhow::{Result, bail};
    use athanor_app::{
        AdapterPluginKind, AdapterRegistry, AthanorStore, ProjectConfig, RuntimeComposition,
    };
    use athanor_core::{SearchDocument, SearchIndex};

    fn resolve_builtin(
        registry: AdapterRegistry,
        _kind: AdapterPluginKind,
        _id: &str,
    ) -> Option<AdapterRegistry> {
        Some(registry)
    }

    fn unavailable_store<'a>(
        _root: &'a Path,
        _config: &'a ProjectConfig,
    ) -> Pin<Box<dyn Future<Output = Result<AthanorStore>> + Send + 'a>> {
        Box::pin(async { bail!("test MCP composition has no store") })
    }

    fn unavailable_search(
        _index_dir: &Path,
        _documents: Option<Vec<SearchDocument>>,
    ) -> Result<Arc<dyn SearchIndex>> {
        bail!("test MCP composition has no search index")
    }

    fn unavailable_projector(
        _target: &Path,
        _snapshot: &str,
        _payload: serde_json::Value,
        _is_cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        bail!("test MCP composition has no projectors")
    }

    RuntimeComposition::new(
        AdapterRegistry::empty,
        resolve_builtin,
        unavailable_store,
        unavailable_search,
        unavailable_projector,
        unavailable_projector,
    )
}
