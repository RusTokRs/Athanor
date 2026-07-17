use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_domain::{GenerationId, SnapshotId};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use tracing::warn;

pub(crate) const INDEX_PUBLICATION_JOURNAL_SCHEMA_V1: &str = "athanor.index_publication.v1";
pub(crate) const INDEX_PUBLICATION_JOURNAL_SCHEMA_V2: &str = "athanor.index_publication.v2";
pub(crate) const INDEX_PUBLICATION_JOURNAL_SCHEMA_V3: &str = "athanor.index_publication.v3";

/// Durable publication journal joining canonical and application artefacts into one immutable
/// generation.
///
/// The wire field remains named `prepared` for compatibility with already persisted v2 journals.
/// Version three adds the backend-neutral generation identity. Legacy v1/v2 journals are
/// normalized in memory while retaining whether pre-generation artifacts are allowed during
/// recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct IndexPublicationJournal {
    schema: String,
    #[serde(rename = "prepared")]
    snapshot: SnapshotId,
    generation: GenerationId,
    id: String,
    read_model: PathBuf,
    index_state: PathBuf,
    #[serde(skip)]
    legacy_generation: bool,
}

impl IndexPublicationJournal {
    pub(crate) fn new(root: &Path, snapshot: SnapshotId) -> Self {
        Self::with_id(root, snapshot, publication_id())
    }

    fn with_id(root: &Path, snapshot: SnapshotId, id: String) -> Self {
        let generation = GenerationId::for_snapshot(&snapshot);
        Self {
            schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V3.to_string(),
            snapshot,
            generation,
            id,
            read_model: expected_read_model(root),
            index_state: expected_index_state(root),
            legacy_generation: false,
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
        if backup.exists()
            && let Err(error) = fs::remove_file(&backup)
        {
            warn!(
                backup = %backup.display(),
                error = %error,
                "publication journal was published but backup cleanup failed"
            );
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

    pub(crate) fn generation(&self) -> &GenerationId {
        &self.generation
    }

    pub(crate) fn requires_generation_identity(&self) -> bool {
        !self.legacy_generation
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
        let expected_generation = GenerationId::for_snapshot(&self.snapshot);
        if self.generation != expected_generation {
            bail!(
                "publication journal generation `{}` does not match snapshot `{}`",
                self.generation,
                self.snapshot.0
            );
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
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct JournalV3 {
            schema: String,
            prepared: SnapshotId,
            generation: GenerationId,
            id: String,
            read_model: PathBuf,
            index_state: PathBuf,
        }

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
            V3(JournalV3),
            V2(JournalV2),
            V1(JournalV1),
        }

        match JournalWire::deserialize(deserializer)? {
            JournalWire::V3(journal) => {
                if journal.schema != INDEX_PUBLICATION_JOURNAL_SCHEMA_V3 {
                    return Err(D::Error::custom(format!(
                        "unsupported publication journal schema {}",
                        journal.schema
                    )));
                }
                Ok(Self {
                    schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V3.to_string(),
                    snapshot: journal.prepared,
                    generation: journal.generation,
                    id: journal.id,
                    read_model: journal.read_model,
                    index_state: journal.index_state,
                    legacy_generation: false,
                })
            }
            JournalWire::V2(journal) => {
                if journal.schema != INDEX_PUBLICATION_JOURNAL_SCHEMA_V2 {
                    return Err(D::Error::custom(format!(
                        "unsupported publication journal schema {}",
                        journal.schema
                    )));
                }
                let generation = GenerationId::for_snapshot(&journal.prepared);
                Ok(Self {
                    schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V3.to_string(),
                    snapshot: journal.prepared,
                    generation,
                    id: journal.id,
                    read_model: journal.read_model,
                    index_state: journal.index_state,
                    legacy_generation: true,
                })
            }
            JournalWire::V1(journal) => {
                if journal.schema != INDEX_PUBLICATION_JOURNAL_SCHEMA_V1 {
                    return Err(D::Error::custom(format!(
                        "unsupported publication journal schema {}",
                        journal.schema
                    )));
                }
                let snapshot = SnapshotId(journal.snapshot);
                let generation = GenerationId::for_snapshot(&snapshot);
                Ok(Self {
                    schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V3.to_string(),
                    snapshot,
                    generation,
                    id: journal.id,
                    read_model: journal.read_model,
                    index_state: journal.index_state,
                    legacy_generation: true,
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
    fn v3_round_trip_preserves_generation_and_snapshot_identity() {
        let root = PathBuf::from("project");
        let journal = IndexPublicationJournal::with_id(
            &root,
            SnapshotId("snap_test_0003".to_string()),
            "publication-3".to_string(),
        );

        let value = serde_json::to_value(&journal).expect("serialize v3 journal");
        assert_eq!(value["schema"], INDEX_PUBLICATION_JOURNAL_SCHEMA_V3);
        assert_eq!(value["prepared"], "snap_test_0003");
        assert_eq!(value["generation"], "gen_snap_test_0003");
        assert!(value.get("snapshot").is_none());

        let decoded: IndexPublicationJournal =
            serde_json::from_value(value).expect("deserialize v3 journal");
        assert_eq!(decoded, journal);
        assert_eq!(decoded.snapshot().0, "snap_test_0003");
        assert_eq!(decoded.generation().0, "gen_snap_test_0003");
        assert!(decoded.requires_generation_identity());
    }

    #[test]
    fn v2_is_normalized_to_generation_aware_v3() {
        let decoded: IndexPublicationJournal = serde_json::from_value(json!({
            "schema": INDEX_PUBLICATION_JOURNAL_SCHEMA_V2,
            "prepared": "snap_test_0002",
            "id": "publication-2",
            "read_model": "project/.athanor/generated/current/jsonl",
            "index_state": "project/.athanor/state/index-state.json"
        }))
        .expect("deserialize v2 journal");

        assert_eq!(decoded.snapshot().0, "snap_test_0002");
        assert_eq!(decoded.generation().0, "gen_snap_test_0002");
        assert!(!decoded.requires_generation_identity());
        let normalized = serde_json::to_value(decoded).expect("serialize normalized journal");
        assert_eq!(normalized["schema"], INDEX_PUBLICATION_JOURNAL_SCHEMA_V3);
        assert_eq!(normalized["prepared"], "snap_test_0002");
        assert_eq!(normalized["generation"], "gen_snap_test_0002");
    }

    #[test]
    fn v1_is_normalized_to_generation_aware_v3() {
        let decoded: IndexPublicationJournal = serde_json::from_value(json!({
            "schema": INDEX_PUBLICATION_JOURNAL_SCHEMA_V1,
            "snapshot": "snap_test_0001",
            "id": "publication-1",
            "read_model": "project/.athanor/generated/current/jsonl",
            "index_state": "project/.athanor/state/index-state.json"
        }))
        .expect("deserialize v1 journal");

        assert_eq!(decoded.snapshot().0, "snap_test_0001");
        assert_eq!(decoded.generation().0, "gen_snap_test_0001");
        assert!(!decoded.requires_generation_identity());
        let normalized = serde_json::to_value(decoded).expect("serialize normalized journal");
        assert_eq!(normalized["schema"], INDEX_PUBLICATION_JOURNAL_SCHEMA_V3);
        assert_eq!(normalized["prepared"], "snap_test_0001");
        assert_eq!(normalized["generation"], "gen_snap_test_0001");
        assert!(normalized.get("snapshot").is_none());
    }
}
