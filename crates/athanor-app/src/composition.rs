//! Explicit runtime dependencies for embedding Athanor without process-global factories.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use athanor_core::{OperationContext, OperationContextCancellation, SearchDocument, SearchIndex};

use crate::projection::ProjectionFactory;
use crate::{
    AdapterRegistry, AdapterRegistryFactory, AthanorStore, BuiltinAdapterResolver, ProjectConfig,
    SearchIndexFactory, SearchIndexOperationFactory, StoreFactory,
};

/// Dependencies required by an Athanor application instance.
///
/// A composition is immutable and can safely be shared by multiple independently configured
/// application instances. It deliberately does not install any process-global state.
#[derive(Clone)]
pub struct RuntimeComposition {
    adapter_registry_factory: AdapterRegistryFactory,
    builtin_adapter_resolver: BuiltinAdapterResolver,
    store_factory: StoreFactory,
    search_index_factory: SearchIndexFactory,
    search_index_operation_factory: Option<SearchIndexOperationFactory>,
    wiki_projector: ProjectionFactory,
    html_projector: ProjectionFactory,
}

impl RuntimeComposition {
    pub fn new(
        adapter_registry_factory: AdapterRegistryFactory,
        builtin_adapter_resolver: BuiltinAdapterResolver,
        store_factory: StoreFactory,
        search_index_factory: SearchIndexFactory,
        wiki_projector: ProjectionFactory,
        html_projector: ProjectionFactory,
    ) -> Self {
        Self {
            adapter_registry_factory,
            builtin_adapter_resolver,
            store_factory,
            search_index_factory,
            search_index_operation_factory: None,
            wiki_projector,
            html_projector,
        }
    }

    pub fn with_search_index_operation_factory(
        mut self,
        factory: SearchIndexOperationFactory,
    ) -> Self {
        self.search_index_operation_factory = Some(factory);
        self
    }

    pub fn adapter_registry(&self) -> AdapterRegistry {
        (self.adapter_registry_factory)()
    }

    pub fn builtin_adapter_resolver(&self) -> BuiltinAdapterResolver {
        self.builtin_adapter_resolver
    }

    pub async fn init_store(&self, root: &Path, config: &ProjectConfig) -> Result<AthanorStore> {
        (self.store_factory)(root, config).await
    }

    pub fn build_search_index(
        &self,
        index_dir: &Path,
        documents: Option<Vec<SearchDocument>>,
    ) -> Result<Arc<dyn SearchIndex>> {
        (self.search_index_factory)(index_dir, documents)
    }

    pub fn build_search_index_with_operation_context(
        &self,
        index_dir: &Path,
        documents: Option<Vec<SearchDocument>>,
        operation: &OperationContext,
    ) -> Result<Arc<dyn SearchIndex>> {
        operation.check_active().map_err(anyhow::Error::new)?;
        let index = match self.search_index_operation_factory {
            Some(factory) => factory(index_dir, documents, operation)?,
            None => (self.search_index_factory)(index_dir, documents)?,
        };
        operation.check_active().map_err(anyhow::Error::new)?;
        Ok(index)
    }

    pub fn project_wiki(
        &self,
        target: &Path,
        snapshot: &str,
        payload: serde_json::Value,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        (self.wiki_projector)(target, snapshot, payload, is_cancelled)
    }

    pub fn project_html(
        &self,
        target: &Path,
        snapshot: &str,
        payload: serde_json::Value,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        (self.html_projector)(target, snapshot, payload, is_cancelled)
    }
}
