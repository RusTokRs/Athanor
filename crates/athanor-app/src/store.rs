use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::OnceLock;

use crate::config::ProjectConfig;
use anyhow::{Result, bail};
use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, KnowledgeStore, RelationQuery, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};

pub trait AthanorStoreBackend:
    KnowledgeStore + CanonicalSnapshotStore + EntityResolver + Send + Sync
{
}

impl<T> AthanorStoreBackend for T where
    T: KnowledgeStore + CanonicalSnapshotStore + EntityResolver + Send + Sync
{
}

#[derive(Clone)]
pub struct AthanorStore {
    inner: Arc<dyn AthanorStoreBackend>,
}

impl AthanorStore {
    pub fn new(store: impl AthanorStoreBackend + 'static) -> Self {
        Self {
            inner: Arc::new(store),
        }
    }
}

pub type StoreFactory =
    for<'a> fn(
        &'a Path,
        &'a ProjectConfig,
    ) -> Pin<Box<dyn Future<Output = Result<AthanorStore>> + Send + 'a>>;

static STORE_FACTORY: OnceLock<StoreFactory> = OnceLock::new();

pub fn install_store_factory(factory: StoreFactory) {
    let _ = STORE_FACTORY.set(factory);
}

pub async fn init_store(root: &Path, config: &ProjectConfig) -> Result<AthanorStore> {
    #[cfg(test)]
    crate::ensure_test_runtime();

    let Some(factory) = STORE_FACTORY.get() else {
        bail!("no Athanor store factory is installed");
    };

    factory(root, config).await
}

#[async_trait]
impl KnowledgeStore for AthanorStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        self.inner.begin_snapshot(repo, base).await
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        self.inner.put_entities(snapshot, entities).await
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        self.inner.put_facts(snapshot, facts).await
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        self.inner.put_relations(snapshot, relations).await
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        self.inner.put_diagnostics(snapshot, diagnostics).await
    }

    async fn query_entities(
        &self,
        snapshot: SnapshotSelector,
        query: EntityQuery,
    ) -> CoreResult<Vec<Entity>> {
        self.inner.query_entities(snapshot, query).await
    }

    async fn query_relations(
        &self,
        snapshot: SnapshotSelector,
        query: RelationQuery,
    ) -> CoreResult<Vec<Relation>> {
        self.inner.query_relations(snapshot, query).await
    }

    async fn query_diagnostics(
        &self,
        snapshot: SnapshotSelector,
        query: DiagnosticQuery,
    ) -> CoreResult<Vec<Diagnostic>> {
        self.inner.query_diagnostics(snapshot, query).await
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.inner.commit_snapshot(snapshot).await
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.inner.abort_snapshot(snapshot).await
    }
}

#[async_trait]
impl CanonicalSnapshotStore for AthanorStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_snapshot(snapshot).await
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_latest_snapshot().await
    }
}

#[async_trait]
impl EntityResolver for AthanorStore {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        self.inner.resolve_stable_key(snapshot, stable_key).await
    }
}
