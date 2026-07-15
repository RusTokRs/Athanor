use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_domain::SnapshotId;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

pub(crate) const INDEX_PUBLICATION_JOURNAL_SCHEMA_V1: &str = "athanor.index_publication.v1";
pub(crate) const INDEX_PUBLICATION_JOURNAL_SCHEMA_V2: &str = "athanor.index_publication.v2";

/// Production publication journal keyed only by canonical snapshot identity.
///
/// The version-two wire field remains named `prepared` for compatibility with already persisted
/// journals, but the active coordinator no longer stores a `PreparedSnapshot` in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct IndexPublicationJournal {
    schema: String,
    #[serde(rename = "prepared")]
    snapshot: SnapshotId,
    id: String,
    read_model: PathBuf,
    index_state: PathBuf,
}

impl IndexPublicationJournal {
    pub(crate) fn new(root: &Path, snapshot: SnapshotId) -> Self {
        Self::with_id(root, snapshot, publication_id())
    }

    fn with_id(root: &Path, snapshot: SnapshotId, id: String) -> Self {
        Self {
            schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V2.to_string(),
            snapshot,
            id,
            read_model: expected_read_model(root),
            index_state: expected_index_state(root),
        }
    }

    pub(crate) fn load(root: &Path) -> Result<Option<Self>> {
        let path = Self::path(root);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read publication journal {}", path.display()))?;
        let journal: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse publication journal {}", path.display()))?;
        journal.validate_for_root(root)?;
        Ok(Some(journal))
    }

    pub(crate) fn write(&self) -> Result<()> {
        validate_publication_id(&self.id)?;
        let path = Self::path_from_artifact(&self.index_state);
        let parent = path.parent().ok_or_else(|| {
            anyhow::anyhow!("publication journal has no parent: {}", path.display())
        })?;
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create publication journal directory {}",
                parent.display()
            )
        })?;

        let staging = parent.join(format!(".index-publication.staging-{}", self.id));
        let backup = parent.join(format!(".index-publication.backup-{}", self.id));
        fs::write(&staging, serde_json::to_vec_pretty(self)?).with_context(|| {
            format!("failed to write publication journal {}", staging.display())
        })?;
        if path.exists() {
            fs::rename(&path, &backup).with_context(|| {
                format!(
                    "failed to stage previous publication journal {}",
                    path.display()
                )
            })?;
        }
        if let Err(error) = fs::rename(&staging, &path) {
            if backup.exists() {
                let _ = fs::rename(&backup, &path);
            }
            let _ = fs::remove_file(&staging);
            return Err(error).with_context(|| {
                format!("failed to publish publication journal {}", path.display())
            });
        }
        if backup.exists() {
            fs::remove_file(&backup).with_context(|| {
                format!(
                    "failed to remove publication journal backup {}",
                    backup.display()
                )
            })?;
        }
        Ok(())
    }

    pub(crate) fn clear(&self) -> Result<()> {
        let path = Self::path_from_artifact(&self.index_state);
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("failed to clear publication journal {}", path.display())
            })?;
        }
        Ok(())
    }

    pub(crate) fn path(root: &Path) -> PathBuf {
        root.join(".athanor/state/index-publication.json")
    }

    pub(crate) fn snapshot(&self) -> &SnapshotId {
        &self.snapshot
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn read_model(&self) -> &Path {
        &self.read_model
    }

    pub(crate) fn index_state(&self) -> &Path {
        &self.index_state
    }

    fn path_from_artifact(index_state: &Path) -> PathBuf {
        index_state.with_file_name("index-publication.json")
    }

    fn validate_for_root(&self, root: &Path) -> Result<()> {
        validate_publication_id(&self.id)?;
        if self.snapshot.0.is_empty() {
            bail!("publication journal has an empty snapshot identity");
        }
        let expected_read_model = expected_read_model(root);
        if self.read_model != expected_read_model {
            bail!(
                "publication journal read-model path {} does not match expected {}",
                self.read_model.display(),
                expected_read_model.display()
            );
        }
        let expected_index_state = expected_index_state(root);
        if self.index_state != expected_index_state {
            bail!(
                "publication journal index-state path {} does not match expected {}",
                self.index_state.display(),
                expected_index_state.display()
            );
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for IndexPublicationJournal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct JournalV2 {
            schema: String,
            prepared: SnapshotId,
            id: String,
            read_model: PathBuf,
            index_state: PathBuf,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct JournalV1 {
            schema: String,
            snapshot: String,
            id: String,
            read_model: PathBuf,
            index_state: PathBuf,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum JournalWire {
            V2(JournalV2),
            V1(JournalV1),
        }

        match JournalWire::deserialize(deserializer)? {
            JournalWire::V2(journal) => {
                if journal.schema != INDEX_PUBLICATION_JOURNAL_SCHEMA_V2 {
                    return Err(D::Error::custom(format!(
                        "unsupported publication journal schema {}",
                        journal.schema
                    )));
                }
                Ok(Self {
                    schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V2.to_string(),
                    snapshot: journal.prepared,
                    id: journal.id,
                    read_model: journal.read_model,
                    index_state: journal.index_state,
                })
            }
            JournalWire::V1(journal) => {
                if journal.schema != INDEX_PUBLICATION_JOURNAL_SCHEMA_V1 {
                    return Err(D::Error::custom(format!(
                        "unsupported publication journal schema {}",
                        journal.schema
                    )));
                }
                Ok(Self {
                    schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V2.to_string(),
                    snapshot: SnapshotId(journal.snapshot),
                    id: journal.id,
                    read_model: journal.read_model,
                    index_state: journal.index_state,
                })
            }
        }
    }
}

fn expected_read_model(root: &Path) -> PathBuf {
    root.join(".athanor/generated/current/jsonl")
}

fn expected_index_state(root: &Path) -> PathBuf {
    root.join(".athanor/state/index-state.json")
}

fn validate_publication_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("invalid publication id `{id}`");
    }
    Ok(())
}

fn publication_id() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn v2_round_trip_preserves_wire_and_snapshot_identity() {
        let root = PathBuf::from("project");
        let journal = IndexPublicationJournal::with_id(
            &root,
            SnapshotId("snap_test_0002".to_string()),
            "publication-2".to_string(),
        );

        let value = serde_json::to_value(&journal).expect("serialize v2 journal");
        assert_eq!(value["schema"], INDEX_PUBLICATION_JOURNAL_SCHEMA_V2);
        assert_eq!(value["prepared"], "snap_test_0002");
        assert!(value.get("snapshot").is_none());

        let decoded: IndexPublicationJournal =
            serde_json::from_value(value).expect("deserialize v2 journal");
        assert_eq!(decoded, journal);
        assert_eq!(decoded.snapshot().0, "snap_test_0002");
    }

    #[test]
    fn v1_is_normalized_to_snapshot_native_v2() {
        let decoded: IndexPublicationJournal = serde_json::from_value(json!({
            "schema": INDEX_PUBLICATION_JOURNAL_SCHEMA_V1,
            "snapshot": "snap_test_0001",
            "id": "publication-1",
            "read_model": "project/.athanor/generated/current/jsonl",
            "index_state": "project/.athanor/state/index-state.json"
        }))
        .expect("deserialize v1 journal");

        assert_eq!(decoded.snapshot().0, "snap_test_0001");
        let normalized = serde_json::to_value(decoded).expect("serialize normalized journal");
        assert_eq!(normalized["schema"], INDEX_PUBLICATION_JOURNAL_SCHEMA_V2);
        assert_eq!(normalized["prepared"], "snap_test_0001");
        assert!(normalized.get("snapshot").is_none());
    }
}
