use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

use crate::config::ProjectConfig;
use anyhow::{Result, bail};
use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CanonicalLatestIdentity, CanonicalLatestPointer, CanonicalSnapshot,
    CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery, EntityResolver,
    KnowledgeStore, OperationContext, OperationContextCancellation, RelationQuery, SnapshotBatch,
    SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};

pub trait AthanorStoreBackend:
    KnowledgeStore + AtomicSnapshotPublication + CanonicalSnapshotStore + EntityResolver + Send + Sync
{
}

impl<T> AthanorStoreBackend for T where
    T: KnowledgeStore
        + AtomicSnapshotPublication
        + CanonicalSnapshotStore
        + EntityResolver
        + Send
        + Sync
{
}

#[derive(Debug)]
struct PendingSnapshotBatch {
    snapshot: SnapshotId,
    batch: SnapshotBatch,
}

#[derive(Clone)]
pub struct AthanorStore {
    inner: Arc<dyn AthanorStoreBackend>,
    latest_pointer: Arc<dyn CanonicalLatestPointer>,
    pending_batches: Arc<Mutex<Vec<PendingSnapshotBatch>>>,
}

impl AthanorStore {
    pub fn new(store: impl AthanorStoreBackend + 'static) -> Self {
        let inner: Arc<dyn AthanorStoreBackend> = Arc::new(store);
        Self {
            latest_pointer: Arc::new(DerivedLatestPointer {
                inner: inner.clone(),
            }),
            inner,
            pending_batches: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn new_with_latest_pointer<T>(store: T) -> Self
    where
        T: AthanorStoreBackend + CanonicalLatestPointer + 'static,
    {
        let store = Arc::new(store);
        let inner: Arc<dyn AthanorStoreBackend> = store.clone();
        let latest_pointer: Arc<dyn CanonicalLatestPointer> = store;
        Self {
            inner,
            latest_pointer,
            pending_batches: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn stage_pending_batch(&self, snapshot: SnapshotId, batch: SnapshotBatch) {
        let mut pending = self
            .pending_batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = pending
            .iter_mut()
            .find(|existing| existing.snapshot == snapshot)
        {
            existing.batch = batch;
        } else {
            pending.push(PendingSnapshotBatch { snapshot, batch });
        }
    }

    fn has_pending_batch(&self, snapshot: &SnapshotId) -> bool {
        self.pending_batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|pending| &pending.snapshot == snapshot)
    }

    fn take_pending_batch(&self, snapshot: &SnapshotId) -> Option<SnapshotBatch> {
        let mut pending = self
            .pending_batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = pending
            .iter()
            .position(|pending| &pending.snapshot == snapshot)?;
        Some(pending.swap_remove(index).batch)
    }

    fn clear_pending_batch(&self, snapshot: &SnapshotId) {
        let _ = self.take_pending_batch(snapshot);
    }
}

#[derive(Clone)]
struct DerivedLatestPointer {
    inner: Arc<dyn AthanorStoreBackend>,
}

#[async_trait]
impl CanonicalLatestPointer for DerivedLatestPointer {
    async fn load_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        let Some(latest) = self.inner.load_latest_snapshot().await? else {
            return Ok(None);
        };
        let snapshot = latest.snapshot.ok_or_else(|| {
            CoreError::AdapterProtocol("latest canonical snapshot has no identity".to_string())
        })?;
        Ok(Some(CanonicalLatestIdentity::for_snapshot(snapshot)))
    }

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()> {
        identity.validate()?;
        let exact = self
            .inner
            .load_snapshot(&identity.snapshot)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", identity.snapshot.0)))?;
        if exact.snapshot.as_ref() != Some(&identity.snapshot) {
            return Err(CoreError::AdapterProtocol(format!(
                "exact canonical snapshot returned identity {:?}, expected {}",
                exact.snapshot, identity.snapshot.0
            )));
        }
        let latest = self.load_latest_identity().await?.ok_or_else(|| {
            CoreError::NotFound("canonical store has no committed latest snapshot".to_string())
        })?;
        if latest != identity {
            return Err(CoreError::Conflict(format!(
                "derived latest is {} / {}, requested {} / {}",
                latest.snapshot.0,
                latest.generation,
                identity.snapshot.0,
                identity.generation
            )));
        }
        Ok(())
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

    async fn begin_snapshot_with_context(
        &self,
        repo: RepoId,
        base: SnapshotBase,
        context: &OperationContext,
    ) -> CoreResult<SnapshotId> {
        AtomicSnapshotPublication::begin_snapshot_allocation(self, repo, base, context).await
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        self.inner.put_entities(snapshot, entities).await
    }

    async fn put_entities_with_context(
        &self,
        snapshot: SnapshotId,
        entities: Vec<Entity>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.inner
            .put_entities_with_context(snapshot, entities, context)
            .await
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        self.inner.put_facts(snapshot, facts).await
    }

    async fn put_facts_with_context(
        &self,
        snapshot: SnapshotId,
        facts: Vec<Fact>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.inner
            .put_facts_with_context(snapshot, facts, context)
            .await
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        self.inner.put_relations(snapshot, relations).await
    }

    async fn put_relations_with_context(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.inner
            .put_relations_with_context(snapshot, relations, context)
            .await
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        self.inner.put_diagnostics(snapshot, diagnostics).await
    }

    async fn put_diagnostics_with_context(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.inner
            .put_diagnostics_with_context(snapshot, diagnostics, context)
            .await
    }

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        self.inner.put_snapshot(snapshot, batch).await
    }

    async fn put_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_active()?;
        self.stage_pending_batch(snapshot, batch);
        Ok(())
    }

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        if self.has_pending_batch(&snapshot) {
            return Ok(());
        }
        self.inner.prepare_snapshot(snapshot).await
    }

    async fn prepare_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        if self.has_pending_batch(&snapshot) {
            context.check_active()?;
            return Ok(());
        }
        self.inner
            .prepare_snapshot_with_context(snapshot, context)
            .await
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
        if let Some(batch) = self.take_pending_batch(&snapshot) {
            return self.inner.publish_snapshot_batch(snapshot, batch).await;
        }
        self.inner.commit_snapshot(snapshot).await
    }

    async fn commit_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        if let Some(batch) = self.take_pending_batch(&snapshot) {
            return self
                .inner
                .publish_snapshot_batch_with_context(snapshot, batch, context)
                .await;
        }
        self.inner
            .commit_snapshot_with_context(snapshot, context)
            .await
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.clear_pending_batch(&snapshot);
        self.inner.abort_snapshot(snapshot).await
    }

    async fn abort_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.clear_pending_batch(&snapshot);
        self.inner
            .abort_snapshot_with_context(snapshot, context)
            .await
    }
}

#[async_trait]
impl AtomicSnapshotPublication for AthanorStore {
    async fn begin_snapshot_allocation(
        &self,
        repo: RepoId,
        base: SnapshotBase,
        context: &OperationContext,
    ) -> CoreResult<SnapshotId> {
        self.inner
            .begin_snapshot_allocation(repo, base, context)
            .await
    }

    async fn recover_orphan_snapshot_allocations(
        &self,
        repo: &RepoId,
        stale_before_unix_ms: u64,
        limit: usize,
    ) -> CoreResult<Vec<SnapshotId>> {
        self.inner
            .recover_orphan_snapshot_allocations(repo, stale_before_unix_ms, limit)
            .await
    }

    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        self.clear_pending_batch(&snapshot);
        self.inner.publish_snapshot_batch(snapshot, batch).await
    }

    async fn publish_snapshot_batch_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.clear_pending_batch(&snapshot);
        self.inner
            .publish_snapshot_batch_with_context(snapshot, batch, context)
            .await
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
impl CanonicalLatestPointer for AthanorStore {
    async fn load_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        self.latest_pointer.load_latest_identity().await
    }

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()> {
        self.latest_pointer.repair_latest_identity(identity).await
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
