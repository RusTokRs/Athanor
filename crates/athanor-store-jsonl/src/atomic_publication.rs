use std::fs;

use async_trait::async_trait;
use athanor_core::{AtomicSnapshotPublication, CoreError, CoreResult, SnapshotBatch};
use athanor_domain::SnapshotId;
use serde::{Deserialize, Serialize};

use super::{
    JsonlKnowledgeStore, SnapshotData, unique_suffix, write_latest, write_snapshot_contents,
};

pub(crate) const SNAPSHOT_COMMIT_SCHEMA: &str = "athanor.canonical_commit.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotCommit {
    pub(crate) schema: String,
    pub(crate) snapshot: String,
}

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

        let prepared_dir = self.prepared_snapshot_dir(&snapshot);
        if prepared_dir.exists() {
            fs::remove_dir_all(&prepared_dir).map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to remove superseded prepared snapshot {}: {error}",
                    prepared_dir.display()
                ))
            })?;
        }

        publish_exact_generation(self, &snapshot, &data)?;
        *self.lock_state()?.snapshot_mut(&snapshot)? = data;

        write_latest(&self.root, &snapshot).map_err(|error| {
            CoreError::Adapter(format!(
                "snapshot {} is committed but latest pointer update failed: {error}",
                snapshot.0
            ))
        })
    }
}

pub(crate) fn publish_exact_generation(
    store: &JsonlKnowledgeStore,
    snapshot: &SnapshotId,
    data: &SnapshotData,
) -> CoreResult<()> {
    let snapshot_dir = store.snapshot_dir(snapshot);
    let parent = snapshot_dir.parent().ok_or_else(|| {
        CoreError::Adapter(format!("snapshot {} has no parent directory", snapshot.0))
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| CoreError::Adapter(format!("failed to create store dir: {error}")))?;
    if snapshot_dir.exists() {
        return Err(CoreError::Conflict(format!(
            "snapshot directory {} already exists",
            snapshot_dir.display()
        )));
    }

    let staging_dir = parent.join(format!(
        ".{}.staging-atomic-{}",
        snapshot.0,
        unique_suffix()
    ));
    fs::create_dir(&staging_dir).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to create atomic snapshot staging directory: {error}"
        ))
    })?;

    let publish_result = (|| {
        write_snapshot_contents(&staging_dir, snapshot, data)?;
        write_commit_marker(&staging_dir, snapshot)?;
        fs::rename(&staging_dir, &snapshot_dir).map_err(|error| {
            CoreError::Adapter(format!(
                "failed to publish committed snapshot atomically: {error}"
            ))
        })
    })();
    if publish_result.is_err() && staging_dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    publish_result
}

pub(crate) fn write_commit_marker(
    snapshot_dir: &std::path::Path,
    snapshot: &SnapshotId,
) -> CoreResult<()> {
    let marker = SnapshotCommit {
        schema: SNAPSHOT_COMMIT_SCHEMA.to_string(),
        snapshot: snapshot.0.clone(),
    };
    fs::write(
        snapshot_dir.join("commit.json"),
        serde_json::to_vec_pretty(&marker).map_err(|error| {
            CoreError::Adapter(format!("failed to serialize snapshot commit marker: {error}"))
        })?,
    )
    .map_err(|error| {
        CoreError::Adapter(format!("failed to write snapshot commit marker: {error}"))
    })
}

pub(crate) fn validate_commit_marker(
    snapshot_dir: &std::path::Path,
    snapshot: &SnapshotId,
) -> CoreResult<()> {
    let path = snapshot_dir.join("commit.json");
    let marker: SnapshotCommit = serde_json::from_slice(
        &fs::read(&path).map_err(|error| {
            CoreError::Adapter(format!(
                "failed to read snapshot commit marker {}: {error}",
                path.display()
            ))
        })?,
    )
    .map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "failed to parse snapshot commit marker {}: {error}",
            path.display()
        ))
    })?;
    if marker.schema != SNAPSHOT_COMMIT_SCHEMA {
        return Err(CoreError::AdapterProtocol(format!(
            "snapshot commit marker {} has schema `{}`, expected `{}`",
            path.display(),
            marker.schema,
            SNAPSHOT_COMMIT_SCHEMA
        )));
    }
    if marker.snapshot != snapshot.0 {
        return Err(CoreError::AdapterProtocol(format!(
            "snapshot commit marker {} identifies `{}`, expected `{}`",
            path.display(),
            marker.snapshot,
            snapshot.0
        )));
    }
    Ok(())
}
