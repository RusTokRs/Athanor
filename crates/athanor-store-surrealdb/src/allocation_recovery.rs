use std::time::{SystemTime, UNIX_EPOCH};

use athanor_core::{CoreError, CoreResult, OperationContext};
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};
use super::{SnapshotRecord, SurrealKnowledgeStore};

const MAX_ORPHAN_CLEANUP_BATCH: usize = 128;
const DEFAULT_ORPHAN_MIN_AGE_MS: u64 = 24 * 60 * 60 * 1_000;

impl SurrealKnowledgeStore {
    pub(crate) async fn begin_snapshot_allocation_atomic(
        &self,
        repo: RepoId,
        base: SnapshotBase,
        context: &OperationContext,
    ) -> CoreResult<SnapshotId> {
        let stale_before_unix_ms = current_unix_ms().saturating_sub(DEFAULT_ORPHAN_MIN_AGE_MS);
        self.recover_orphan_snapshot_allocations_bounded(
            &repo,
            stale_before_unix_ms,
            MAX_ORPHAN_CLEANUP_BATCH,
        )
        .await?;

        let _guard = self.write_gate.lock().await;
        let sequence = self.allocate_snapshot_sequence().await?;
        let snapshot_id = format!("snap_surreal_{sequence:08}");
        let record = SnapshotRecord {
            id: snapshot_id.clone(),
            repo: repo.0,
            base_branch: base.branch,
            base_commit: base.commit,
            base_parent_snapshot: base.parent_snapshot.map(|snapshot| snapshot.0),
            base_working_tree: base.working_tree,
            sequence,
            prepared: false,
            committed: false,
            generation: None,
            allocation_operation_id: context.operation_id.clone(),
            allocation_created_at_unix_ms: Some(current_unix_ms()),
        };

        let _: Option<SnapshotRecord> = self
            .db
            .create(("snapshot", &snapshot_id))
            .content(record)
            .await
            .map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to create context-owned snapshot allocation: {error}"
                ))
            })?;

        Ok(SnapshotId(snapshot_id))
    }

    pub(crate) async fn recover_orphan_snapshot_allocations_bounded(
        &self,
        repo: &RepoId,
        stale_before_unix_ms: u64,
        limit: usize,
    ) -> CoreResult<Vec<SnapshotId>> {
        let limit = limit.min(MAX_ORPHAN_CLEANUP_BATCH);
        if limit == 0 {
            return Ok(Vec::new());
        }

        let _guard = self.write_gate.lock().await;
        let mut response = self
            .db
            .query(
                "SELECT * FROM snapshot \
                 WHERE repo = $repo \
                   AND committed = false \
                   AND prepared = false \
                 ORDER BY sequence ASC \
                 LIMIT $limit;",
            )
            .bind(("repo", repo.0.clone()))
            .bind(("stale_before", stale_before_unix_ms))
            .bind(("limit", limit as u64))
            .await
            .map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to select stale snapshot allocations: {error}"
                ))
            })?
            .check()
            .map_err(|error| {
                CoreError::Adapter(format!(
                    "stale snapshot allocation selection failed: {error}"
                ))
            })?;
        let candidates: Vec<SnapshotRecord> = response.take(0).map_err(|error| {
            CoreError::Adapter(format!(
                "failed to parse stale snapshot allocations: {error}"
            ))
        })?;

        let mut removed = Vec::with_capacity(candidates.len());
        for candidate in candidates.into_iter().filter(|candidate| {
            candidate
                .allocation_created_at_unix_ms
                .is_some_and(|created| created <= stale_before_unix_ms)
        }) {
            let snapshot = SnapshotId(candidate.id);
            let mut delete_response = self
                .db
                .query(
                    "DELETE type::thing('snapshot', $snapshot) \
                     WHERE repo = $repo \
                       AND committed = false \
                       AND prepared = false \
                     RETURN BEFORE;",
                )
                .bind(("snapshot", snapshot.0.clone()))
                .bind(("repo", repo.0.clone()))
                .await
                .map_err(|error| {
                    CoreError::Adapter(format!(
                        "failed to delete stale snapshot allocation {}: {error}",
                        snapshot.0
                    ))
                })?
                .check()
                .map_err(|error| {
                    CoreError::Adapter(format!(
                        "stale snapshot allocation delete failed for {}: {error}",
                        snapshot.0
                    ))
                })?;
            let deleted: Option<SnapshotRecord> = delete_response.take(0).map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to parse deleted snapshot allocation {}: {error}",
                    snapshot.0
                ))
            })?;
            if deleted.is_none() {
                continue;
            }

            self.db
                .query(
                    "BEGIN; \
                     DELETE entity WHERE snapshot = $snapshot; \
                     DELETE fact WHERE snapshot = $snapshot; \
                     DELETE relation WHERE snapshot = $snapshot; \
                     DELETE diagnostic WHERE snapshot = $snapshot; \
                     COMMIT;",
                )
                .bind(("snapshot", snapshot.0.clone()))
                .await
                .map_err(|error| {
                    CoreError::Adapter(format!(
                        "failed to clean rows for orphan snapshot {}: {error}",
                        snapshot.0
                    ))
                })?
                .check()
                .map_err(|error| {
                    CoreError::Adapter(format!(
                        "orphan snapshot row cleanup failed for {}: {error}",
                        snapshot.0
                    ))
                })?;
            removed.push(snapshot);
        }

        Ok(removed)
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
