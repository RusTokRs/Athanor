use std::path::{Path, PathBuf};

use athanor_domain::SnapshotId;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::prepared_publication::PreparedSnapshot;

pub(crate) const INDEX_PUBLICATION_JOURNAL_SCHEMA_V1: &str = "athanor.index_publication.v1";
pub(crate) const INDEX_PUBLICATION_JOURNAL_SCHEMA_V2: &str = "athanor.index_publication.v2";

/// Typed recovery record for one prepared index publication.
///
/// Version 2 persists `PreparedSnapshot` directly. Version 1 records containing a raw `snapshot`
/// string remain readable and are normalized to the version-2 representation in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct IndexPublicationJournal {
    schema: String,
    prepared: PreparedSnapshot,
    id: String,
    read_model: PathBuf,
    index_state: PathBuf,
}

impl IndexPublicationJournal {
    pub(crate) fn new(root: &Path, prepared: PreparedSnapshot) -> Self {
        Self::with_id(root, prepared, publication_id())
    }

    fn with_id(root: &Path, prepared: PreparedSnapshot, id: String) -> Self {
        Self {
            schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V2.to_string(),
            prepared,
            id,
            read_model: root.join(".athanor/generated/current/jsonl"),
            index_state: root.join(".athanor/state/index-state.json"),
        }
    }

    pub(crate) fn prepared(&self) -> &PreparedSnapshot {
        &self.prepared
    }

    pub(crate) fn snapshot(&self) -> &SnapshotId {
        self.prepared.snapshot()
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
            prepared: PreparedSnapshot,
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
                    prepared: journal.prepared,
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
                    prepared: PreparedSnapshot::new(SnapshotId(journal.snapshot)),
                    id: journal.id,
                    read_model: journal.read_model,
                    index_state: journal.index_state,
                })
            }
        }
    }
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
    fn version_two_round_trip_preserves_typed_prepared_handle() {
        let root = PathBuf::from("project");
        let journal = IndexPublicationJournal::with_id(
            &root,
            PreparedSnapshot::new(SnapshotId("snap_test_0002".to_string())),
            "publication-2".to_string(),
        );

        let value = serde_json::to_value(&journal).expect("serialize version-two journal");
        assert_eq!(value["schema"], INDEX_PUBLICATION_JOURNAL_SCHEMA_V2);
        assert_eq!(value["prepared"], "snap_test_0002");
        assert!(value.get("snapshot").is_none());

        let decoded: IndexPublicationJournal =
            serde_json::from_value(value).expect("deserialize version-two journal");
        assert_eq!(decoded, journal);
        assert_eq!(decoded.snapshot().0, "snap_test_0002");
        assert_eq!(decoded.id(), "publication-2");
        assert_eq!(
            decoded.read_model(),
            Path::new("project/.athanor/generated/current/jsonl")
        );
        assert_eq!(
            decoded.index_state(),
            Path::new("project/.athanor/state/index-state.json")
        );
    }

    #[test]
    fn version_one_journal_is_normalized_to_typed_version_two() {
        let value = json!({
            "schema": INDEX_PUBLICATION_JOURNAL_SCHEMA_V1,
            "snapshot": "snap_test_0001",
            "id": "publication-1",
            "read_model": "project/.athanor/generated/current/jsonl",
            "index_state": "project/.athanor/state/index-state.json"
        });

        let decoded: IndexPublicationJournal =
            serde_json::from_value(value).expect("deserialize version-one journal");
        assert_eq!(decoded.snapshot().0, "snap_test_0001");
        assert_eq!(decoded.prepared().snapshot().0, "snap_test_0001");

        let normalized = serde_json::to_value(decoded).expect("serialize normalized journal");
        assert_eq!(normalized["schema"], INDEX_PUBLICATION_JOURNAL_SCHEMA_V2);
        assert_eq!(normalized["prepared"], "snap_test_0001");
        assert!(normalized.get("snapshot").is_none());
    }

    #[test]
    fn unknown_journal_schema_is_rejected() {
        let value = json!({
            "schema": "athanor.index_publication.v999",
            "prepared": "snap_test_unknown",
            "id": "publication-unknown",
            "read_model": "project/.athanor/generated/current/jsonl",
            "index_state": "project/.athanor/state/index-state.json"
        });

        let error = serde_json::from_value::<IndexPublicationJournal>(value)
            .expect_err("unknown publication journal schema must fail closed");
        assert!(error.to_string().contains("unsupported publication journal schema"));
    }
}
