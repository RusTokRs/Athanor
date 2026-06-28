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
    KnowledgeStore, RelationQuery,
};
use athanor_domain::{Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId};

pub trait AthanorStoreBackend: KnowledgeStore + CanonicalSnapshotStore + Send + Sync {}

impl<T> AthanorStoreBackend for T where T: KnowledgeStore + CanonicalSnapshotStore + Send + Sync {}

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

type StoreFactory = for<'a> fn(
    &'a Path,
    &'a ProjectConfig,
) -> Pin<Box<dyn Future<Output = Result<AthanorStore>> + Send + 'a>>;

static STORE_FACTORY: OnceLock<StoreFactory> = OnceLock::new();

pub fn install_store_factory(factory: StoreFactory) {
    let _ = STORE_FACTORY.set(factory);
}

pub async fn init_store(root: &Path, config: &ProjectConfig) -> Result<AthanorStore> {
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

    async fn query_entities(&self, query: EntityQuery) -> CoreResult<Vec<Entity>> {
        self.inner.query_entities(query).await
    }

    async fn query_relations(&self, query: RelationQuery) -> CoreResult<Vec<Relation>> {
        self.inner.query_relations(query).await
    }

    async fn query_diagnostics(&self, query: DiagnosticQuery) -> CoreResult<Vec<Diagnostic>> {
        self.inner.query_diagnostics(query).await
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.inner.commit_snapshot(snapshot).await
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
