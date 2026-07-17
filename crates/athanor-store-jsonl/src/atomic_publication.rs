use std::fs;

use async_trait::async_trait;
use athanor_core::{AtomicSnapshotPublication, CoreError, CoreResult, SnapshotBatch};
use athanor_domain::SnapshotId;

use crate::commit_marker::{declare_commit_marker_requirement, write_commit_marker};
use crate::latest::write_latest_identity;
use crate::pointer_publication::unique_suffix;
use crate::snapshot_io::write_snapshot_contents;
use crate::store::{JsonlKnowledgeStore, SnapshotData};

#[async_trait]
impl AtomicSnapshotPublication for JsonlKnowledgeStore {
    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        let _writer_lock = self.acquire_writer_lock()?;
        let data = {
            let state = self.lock_state()?;
            let current = state.snapshot(&snapshot)?;
            if current.committed {
                return Err(CoreError::Conflict(format!(
                    "cannot republish committed snapshot {}",
                    snapshot.0
                )));
            }
            SnapshotData {
                committed: true,
                prepared: false,
                entities: batch.entities,
                facts: batch.facts,
                relations: batch.relations,
                diagnostics: batch.diagnostics,
            }
        };

        let prepared = self.prepared_snapshot_dir(&snapshot);
        if prepared.exists() {
            fs::remove_dir_all(&prepared).map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to remove superseded prepared snapshot {}: {error}",
                    prepared.display()
                ))
            })?;
        }

        publish_exact_generation(self, &snapshot, &data)?;
        *self.lock_state()?.snapshot_mut(&snapshot)? = data;
        write_latest_identity(&self.root, &snapshot).map_err(|error| {
            CoreError::Adapter(format!(
                "snapshot {} is committed but latest pointer update failed: {error}",
                snapshot.0
            ))
        })
    }
}

fn publish_exact_generation(
    store: &JsonlKnowledgeStore,
    snapshot: &SnapshotId,
    data: &SnapshotData,
) -> CoreResult<()> {
    let target = store.snapshot_dir(snapshot);
    let parent = target.parent().ok_or_else(|| {
        CoreError::Adapter(format!("snapshot {} has no parent directory", snapshot.0))
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| CoreError::Adapter(format!("failed to create store dir: {error}")))?;
    if target.exists() {
        return Err(CoreError::Conflict(format!(
            "snapshot directory {} already exists",
            target.display()
        )));
    }

    let staging = parent.join(format!(
        ".{}.staging-atomic-{}",
        snapshot.0,
        unique_suffix()
    ));
    fs::create_dir(&staging).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to create atomic snapshot staging directory: {error}"
        ))
    })?;

    let result = (|| {
        write_snapshot_contents(&staging, snapshot, data)?;
        declare_commit_marker_requirement(&staging, snapshot)?;
        write_commit_marker(&staging, snapshot)?;
        fs::rename(&staging, &target).map_err(|error| {
            CoreError::Adapter(format!(
                "failed to publish committed snapshot atomically: {error}"
            ))
        })
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}
