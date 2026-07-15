use async_trait::async_trait;
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};

use crate::cancellation::OperationContextCancellation;
use crate::{CoreResult, KnowledgeStore, OperationContext, SnapshotBatch};

/// Publishes canonical data and its committed marker through one backend-specific atomic boundary.
///
/// This capability is intentionally separate from `KnowledgeStore` compatibility methods. A backend
/// may stage data before publication, but successful return means readers can never observe a
/// committed marker without the complete batch, or the complete batch as committed without its
/// marker. Selecting the generation as `LatestCommitted` or switching an application-level current
/// pointer remains a separate layer.
#[async_trait]
pub trait AtomicSnapshotPublication: KnowledgeStore {
    /// Allocates one snapshot with transport-neutral operation metadata when the backend needs a
    /// durable allocation record. The default preserves the existing context-aware store contract.
    async fn begin_snapshot_allocation(
        &self,
        repo: RepoId,
        base: SnapshotBase,
        context: &OperationContext,
    ) -> CoreResult<SnapshotId> {
        context.check_active()?;
        self.begin_snapshot_with_context(repo, base, context).await
    }

    /// Removes a bounded set of stale, uncommitted and unprepared allocation records for one repo.
    ///
    /// Backends without durable allocation records return an empty set. Implementations must repeat
    /// the committed/prepared/age checks at the destructive boundary and must never remove a record
    /// newer than `stale_before_unix_ms`.
    async fn recover_orphan_snapshot_allocations(
        &self,
        _repo: &RepoId,
        _stale_before_unix_ms: u64,
        _limit: usize,
    ) -> CoreResult<Vec<SnapshotId>> {
        Ok(Vec::new())
    }

    /// Atomically writes the complete canonical batch and marks the snapshot committed.
    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()>;

    /// Context-aware atomic publication.
    ///
    /// Cancellation and deadline are checked before entering the backend boundary. They are not
    /// checked again after a successful publish because the durable commit has already happened and
    /// callers must retain the success result instead of attempting rollback.
    async fn publish_snapshot_batch_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_active()?;
        self.publish_snapshot_batch(snapshot, batch).await
    }
}
