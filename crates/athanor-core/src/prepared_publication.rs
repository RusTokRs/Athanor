use async_trait::async_trait;
use athanor_domain::SnapshotId;
use serde::{Deserialize, Serialize};

use crate::cancellation::OperationContextCancellation;
use crate::ports::{CoreResult, KnowledgeStore, OperationContext};

/// Backend-neutral publication and cleanup authority for one uncommitted snapshot identity.
///
/// A store-specific prepare boundary may durably stage backend data or retain a complete batch in a
/// process-local composition facade until a durable application journal exists. The handle is opaque
/// to coordinators: publication and rollback must go back through the `KnowledgeStore` that produced
/// it. Its serialized representation remains the snapshot id so recovery journals do not depend on a
/// backend-specific token format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PreparedSnapshot {
    snapshot: SnapshotId,
}

impl PreparedSnapshot {
    pub fn new(snapshot: SnapshotId) -> Self {
        Self { snapshot }
    }

    pub fn snapshot(&self) -> &SnapshotId {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> SnapshotId {
        self.snapshot
    }
}

/// Explicit prepare/publish/abort protocol layered over the compatible `KnowledgeStore` lifecycle.
///
/// Existing stores remain source-compatible because the extension delegates to their context-aware
/// methods. Publication coordinators can migrate incrementally from an implicit `SnapshotId` state
/// machine to a typed authority without duplicating backend or composition-facade logic.
#[async_trait]
pub trait PreparedSnapshotPublication: KnowledgeStore {
    async fn prepare_publication(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<PreparedSnapshot> {
        context.check_active()?;
        self.prepare_snapshot_with_context(snapshot.clone(), context)
            .await?;

        // A successful prepare has established the store-specific publication/cleanup authority.
        // Always return it: a durable backend stage or a process-local complete batch must remain
        // publishable or cancellation-independently abortable by the coordinator.
        Ok(PreparedSnapshot::new(snapshot))
    }

    async fn publish_prepared(
        &self,
        prepared: &PreparedSnapshot,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_active()?;
        self.commit_snapshot_with_context(prepared.snapshot().clone(), context)
            .await
    }

    /// Rolls back prepared canonical data outside the caller cancellation/deadline budget.
    ///
    /// Cleanup must remain possible after user cancellation or deadline expiry, so this method uses
    /// the plain abort contract rather than `abort_snapshot_with_context`.
    async fn abort_prepared(&self, prepared: &PreparedSnapshot) -> CoreResult<()> {
        self.abort_snapshot(prepared.snapshot().clone()).await
    }
}

#[async_trait]
impl<T> PreparedSnapshotPublication for T where T: KnowledgeStore + ?Sized {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_handle_round_trips_as_snapshot_identity() {
        let prepared = PreparedSnapshot::new(SnapshotId("snap_test_0001".to_string()));
        let encoded = serde_json::to_string(&prepared).unwrap();
        assert_eq!(encoded, "\"snap_test_0001\"");

        let decoded: PreparedSnapshot = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, prepared);
        assert_eq!(decoded.snapshot().0, "snap_test_0001");
    }
}
