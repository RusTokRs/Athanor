use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CanonicalLatestIdentity, CanonicalLatestPointer,
    CanonicalSnapshotStore, CoreError, CoreResult, EntityResolver, KnowledgeStore, SnapshotBatch,
};
use athanor_domain::SnapshotId;

use crate::config::ProjectConfig;

#[path = "store/canonical.rs"]
mod canonical;
#[path = "store/knowledge.rs"]
mod knowledge;
#[path = "store/publication.rs"]
mod publication;

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

    async fn discover_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        self.load_latest_identity().await
    }

    async fn validate_latest_identity(&self, identity: &CanonicalLatestIdentity) -> CoreResult<()> {
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
        let latest = self.discover_latest_identity().await?.ok_or_else(|| {
            CoreError::NotFound("canonical store has no committed latest snapshot".to_string())
        })?;
        if &latest != identity {
            return Err(CoreError::Conflict(format!(
                "derived latest is {} / {}, requested {} / {}",
                latest.snapshot.0, latest.generation, identity.snapshot.0, identity.generation
            )));
        }
        Ok(())
    }

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()> {
        self.validate_latest_identity(&identity).await
    }
}

pub type StoreFactory =
    for<'a> fn(
        &'a Path,
        &'a ProjectConfig,
    ) -> Pin<Box<dyn Future<Output = Result<AthanorStore>> + Send + 'a>>;
