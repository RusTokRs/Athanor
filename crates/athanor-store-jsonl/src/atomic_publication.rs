use std::fs;

use async_trait::async_trait;
use athanor_core::{AtomicSnapshotPublication, CoreError, CoreResult, SnapshotBatch};
use athanor_domain::{GenerationId, SnapshotId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    JsonlKnowledgeStore, SnapshotData, unique_suffix, write_latest, write_snapshot_contents,
};

pub(crate) const SNAPSHOT_COMMIT_SCHEMA_V1: &str = "athanor.canonical_commit.v1";
pub(crate) const SNAPSHOT_COMMIT_SCHEMA: &str = "athanor.canonical_commit.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotCommitV2 {
    schema: String,
    snapshot: String,
    generation: GenerationId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotCommitV1 {
    schema: String,
    snapshot: String,
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

fn publish_exact_generation(
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
        declare_commit_marker_requirement(&staging_dir, snapshot)?;
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

fn declare_commit_marker_requirement(
    snapshot_dir: &std::path::Path,
    snapshot: &SnapshotId,
) -> CoreResult<()> {
    let path = snapshot_dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to read atomic snapshot manifest {}: {error}",
            path.display()
        ))
    })?)
    .map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "failed to parse atomic snapshot manifest {}: {error}",
            path.display()
        ))
    })?;
    let object = manifest.as_object_mut().ok_or_else(|| {
        CoreError::AdapterProtocol(format!(
            "atomic snapshot manifest {} must be a JSON object",
            path.display()
        ))
    })?;
    if object.get("snapshot").and_then(Value::as_str) != Some(snapshot.0.as_str()) {
        return Err(CoreError::AdapterProtocol(format!(
            "atomic snapshot manifest {} does not identify `{}`",
            path.display(),
            snapshot.0
        )));
    }
    object.insert(
        "commit_marker_schema".to_string(),
        Value::String(SNAPSHOT_COMMIT_SCHEMA.to_string()),
    );
    fs::write(
        &path,
        serde_json::to_vec_pretty(&manifest).map_err(|error| {
            CoreError::Adapter(format!("failed to serialize atomic snapshot manifest: {error}"))
        })?,
    )
    .map_err(|error| {
        CoreError::Adapter(format!(
            "failed to write atomic snapshot manifest {}: {error}",
            path.display()
        ))
    })
}

pub(crate) fn write_commit_marker(
    snapshot_dir: &std::path::Path,
    snapshot: &SnapshotId,
) -> CoreResult<()> {
    let marker = SnapshotCommitV2 {
        schema: SNAPSHOT_COMMIT_SCHEMA.to_string(),
        snapshot: snapshot.0.clone(),
        generation: GenerationId::for_snapshot(snapshot),
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
    validate_commit_marker_inner(snapshot_dir, snapshot, None)
}

pub(crate) fn validate_commit_marker_schema(
    snapshot_dir: &std::path::Path,
    snapshot: &SnapshotId,
    expected_schema: &str,
) -> CoreResult<()> {
    validate_commit_marker_inner(snapshot_dir, snapshot, Some(expected_schema))
}

fn validate_commit_marker_inner(
    snapshot_dir: &std::path::Path,
    snapshot: &SnapshotId,
    expected_schema: Option<&str>,
) -> CoreResult<()> {
    let path = snapshot_dir.join("commit.json");
    let value: Value = serde_json::from_slice(
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
    let schema = value
        .get("schema")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            CoreError::AdapterProtocol(format!(
                "snapshot commit marker {} has no schema",
                path.display()
            ))
        })?;
    if let Some(expected_schema) = expected_schema
        && schema != expected_schema
    {
        return Err(CoreError::AdapterProtocol(format!(
            "snapshot commit marker {} has schema `{schema}`, manifest requires `{expected_schema}`",
            path.display()
        )));
    }

    match schema.as_str() {
        SNAPSHOT_COMMIT_SCHEMA => {
            let marker: SnapshotCommitV2 = serde_json::from_value(value).map_err(|error| {
                CoreError::AdapterProtocol(format!(
                    "failed to decode snapshot commit marker {}: {error}",
                    path.display()
                ))
            })?;
            validate_snapshot_identity(&path, snapshot, &marker.snapshot)?;
            let expected = GenerationId::for_snapshot(snapshot);
            if marker.generation != expected {
                return Err(CoreError::AdapterProtocol(format!(
                    "snapshot commit marker {} identifies generation `{}`, expected `{}`",
                    path.display(),
                    marker.generation,
                    expected
                )));
            }
            Ok(())
        }
        SNAPSHOT_COMMIT_SCHEMA_V1 => {
            let marker: SnapshotCommitV1 = serde_json::from_value(value).map_err(|error| {
                CoreError::AdapterProtocol(format!(
                    "failed to decode legacy snapshot commit marker {}: {error}",
                    path.display()
                ))
            })?;
            validate_snapshot_identity(&path, snapshot, &marker.snapshot)
        }
        other => Err(CoreError::AdapterProtocol(format!(
            "snapshot commit marker {} has unsupported schema `{other}`",
            path.display()
        ))),
    }
}

fn validate_snapshot_identity(
    path: &std::path::Path,
    snapshot: &SnapshotId,
    actual: &str,
) -> CoreResult<()> {
    if actual != snapshot.0.as_str() {
        return Err(CoreError::AdapterProtocol(format!(
            "snapshot commit marker {} identifies `{actual}`, expected `{}`",
            path.display(),
            snapshot.0
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_contains_immutable_generation_identity() {
        let root = std::env::temp_dir().join(format!(
            "athanor-canonical-generation-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let snapshot = SnapshotId("snap_test".to_string());

        write_commit_marker(&root, &snapshot).unwrap();
        let marker: SnapshotCommitV2 =
            serde_json::from_slice(&fs::read(root.join("commit.json")).unwrap()).unwrap();

        assert_eq!(marker.snapshot, "snap_test");
        assert_eq!(marker.generation.0, "gen_snap_test");
        validate_commit_marker_schema(&root, &snapshot, SNAPSHOT_COMMIT_SCHEMA).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn declared_v2_schema_rejects_legacy_v1_marker() {
        let root = std::env::temp_dir().join(format!(
            "athanor-canonical-marker-schema-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let snapshot = SnapshotId("snap_test".to_string());
        let marker = SnapshotCommitV1 {
            schema: SNAPSHOT_COMMIT_SCHEMA_V1.to_string(),
            snapshot: snapshot.0.clone(),
        };
        fs::write(
            root.join("commit.json"),
            serde_json::to_vec_pretty(&marker).unwrap(),
        )
        .unwrap();

        let error = validate_commit_marker_schema(&root, &snapshot, SNAPSHOT_COMMIT_SCHEMA)
            .expect_err("manifest-declared v2 must reject a v1 marker");
        assert!(matches!(error, CoreError::AdapterProtocol(_)));
        assert!(error.to_string().contains("manifest requires"));
        fs::remove_dir_all(root).unwrap();
    }
}
