use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::SourceFile;
use athanor_domain::{GenerationId, SnapshotId};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(not(feature = "js-ts-precision"))]
pub const INDEX_STATE_SCHEMA: &str = "athanor.index_state.v46";
#[cfg(feature = "js-ts-precision")]
pub const INDEX_STATE_SCHEMA: &str = "athanor.index_state.v46-js-ts-precision-v1";

/// Incremental index state.
///
/// The persisted wire document includes a deterministic `generation` field whenever `snapshot` is
/// present. The field is intentionally derived rather than stored in the public Rust structure so
/// existing callers cannot construct a mismatched snapshot/generation pair.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexState {
    pub schema: String,
    pub snapshot: Option<String>,
    pub files: BTreeMap<String, FileState>,
}

impl Serialize for IndexState {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct IndexStateWire<'a> {
            schema: &'a str,
            snapshot: &'a Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            generation: Option<GenerationId>,
            files: &'a BTreeMap<String, FileState>,
        }

        let generation = self
            .snapshot
            .as_ref()
            .map(|snapshot| GenerationId::for_snapshot(&SnapshotId(snapshot.clone())));
        IndexStateWire {
            schema: &self.schema,
            snapshot: &self.snapshot,
            generation,
            files: &self.files,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for IndexState {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct IndexStateWire {
            schema: String,
            snapshot: Option<String>,
            #[serde(default)]
            generation: Option<GenerationId>,
            files: BTreeMap<String, FileState>,
        }

        let wire = IndexStateWire::deserialize(deserializer)?;
        match (&wire.snapshot, &wire.generation) {
            (Some(snapshot), Some(generation)) => {
                let expected = GenerationId::for_snapshot(&SnapshotId(snapshot.clone()));
                if generation != &expected {
                    return Err(D::Error::custom(format!(
                        "index state generation `{generation}` does not match snapshot `{snapshot}`"
                    )));
                }
            }
            (None, Some(generation)) => {
                return Err(D::Error::custom(format!(
                    "index state generation `{generation}` has no snapshot identity"
                )));
            }
            _ => {}
        }

        Ok(Self {
            schema: wire.schema,
            snapshot: wire.snapshot,
            files: wire.files,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileState {
    pub content_hash: Option<String>,
    pub language_hint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AffectedFileSet {
    pub changed: BTreeSet<String>,
    pub unchanged: BTreeSet<String>,
    pub removed: BTreeSet<String>,
}

impl IndexState {
    pub fn empty() -> Self {
        Self {
            schema: INDEX_STATE_SCHEMA.to_string(),
            snapshot: None,
            files: BTreeMap::new(),
        }
    }

    pub fn from_sources(snapshot: impl Into<String>, sources: &[SourceFile]) -> Self {
        let files = sources
            .iter()
            .map(|source| {
                (
                    source.path.clone(),
                    FileState {
                        content_hash: source.content_hash.clone(),
                        language_hint: source.language_hint.clone(),
                    },
                )
            })
            .collect();

        Self {
            schema: INDEX_STATE_SCHEMA.to_string(),
            snapshot: Some(snapshot.into()),
            files,
        }
    }

    pub fn affected_files(&self, current: &[SourceFile]) -> AffectedFileSet {
        let mut affected = AffectedFileSet::default();
        let current_paths = current
            .iter()
            .map(|source| source.path.clone())
            .collect::<BTreeSet<_>>();

        for source in current {
            match self.files.get(&source.path) {
                Some(previous)
                    if previous.content_hash == source.content_hash
                        && previous.language_hint == source.language_hint =>
                {
                    affected.unchanged.insert(source.path.clone());
                }
                _ => {
                    affected.changed.insert(source.path.clone());
                }
            }
        }

        for previous_path in self.files.keys() {
            if !current_paths.contains(previous_path) {
                affected.removed.insert(previous_path.clone());
            }
        }

        affected
    }
}

#[derive(Debug, Clone)]
pub struct IndexStateStore {
    path: PathBuf,
}

/// A published index-state replacement retaining its prior file until finalization.
#[derive(Debug)]
pub struct PreparedIndexState {
    path: PathBuf,
    backup: Option<PathBuf>,
}

impl PreparedIndexState {
    pub fn finalize(mut self) -> Result<()> {
        if let Some(backup) = self.backup.take() {
            fs::remove_file(&backup).with_context(|| {
                format!("failed to remove index state backup {}", backup.display())
            })?;
        }
        Ok(())
    }

    pub fn rollback(mut self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path).with_context(|| {
                format!(
                    "failed to remove unpublished index state {}",
                    self.path.display()
                )
            })?;
        }
        if let Some(backup) = self.backup.take() {
            fs::rename(&backup, &self.path).with_context(|| {
                format!("failed to restore index state backup {}", backup.display())
            })?;
        }
        Ok(())
    }
}

impl IndexStateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load(&self) -> Result<IndexState> {
        if !self.path.exists() {
            return Ok(IndexState::empty());
        }

        let content = fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read {}", self.path.display()))?;
        let value: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", self.path.display()))?;
        let schema = value
            .get("schema")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("index state {} has no schema", self.path.display()))?;
        if schema != INDEX_STATE_SCHEMA {
            return Ok(IndexState::empty());
        }
        let state = serde_json::from_value::<IndexState>(value)
            .with_context(|| format!("failed to decode {}", self.path.display()))?;

        Ok(state)
    }

    pub fn save(&self, state: &IndexState) -> Result<()> {
        self.prepare(state)?.finalize()
    }

    pub fn prepare(&self, state: &IndexState) -> Result<PreparedIndexState> {
        self.prepare_with_publication_id(state, &publication_nonce())
    }

    pub fn prepare_with_publication_id(
        &self,
        state: &IndexState,
        publication_id: &str,
    ) -> Result<PreparedIndexState> {
        write_staged_json(&self.path, state, publication_id)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Publishes a complete state document only after its staged replacement is ready.
///
/// The backup keeps the prior valid state recoverable if the final rename fails on platforms
/// where replacing an existing path is not supported by a single rename.
fn write_staged_json(
    path: &Path,
    state: &IndexState,
    publication_id: &str,
) -> Result<PreparedIndexState> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("index state path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid index state path: {}", path.display()))?;
    let staging = parent.join(format!(".{name}.staging-{publication_id}"));
    let backup = parent.join(format!(".{name}.backup-{publication_id}"));
    let content = serde_json::to_string_pretty(state)?;
    fs::write(&staging, format!("{content}\n"))
        .with_context(|| format!("failed to write staged index state {}", staging.display()))?;

    if path.exists() {
        fs::rename(path, &backup)
            .with_context(|| format!("failed to stage previous index state {}", path.display()))?;
    }
    if let Err(error) = fs::rename(&staging, path) {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&staging);
        return Err(error)
            .with_context(|| format!("failed to publish index state {}", path.display()));
    }
    Ok(PreparedIndexState {
        path: path.to_path_buf(),
        backup: backup.exists().then_some(backup),
    })
}

fn publication_nonce() -> String {
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
    use super::*;

    #[test]
    fn state_wire_joins_snapshot_to_immutable_generation() {
        let value = serde_json::to_value(IndexState::from_sources("snap_test", &[])).unwrap();

        assert_eq!(value["snapshot"], "snap_test");
        assert_eq!(value["generation"], "gen_snap_test");
    }

    #[test]
    fn legacy_state_without_generation_remains_readable() {
        let state: IndexState = serde_json::from_str(&format!(
            r#"{{"schema":"{}","snapshot":"snap_old","files":{{}}}}"#,
            INDEX_STATE_SCHEMA
        ))
        .unwrap();

        assert_eq!(state.snapshot.as_deref(), Some("snap_old"));
    }

    #[test]
    fn mismatched_generation_is_rejected() {
        let error = serde_json::from_str::<IndexState>(&format!(
            r#"{{"schema":"{}","snapshot":"snap_one","generation":"gen_snap_two","files":{{}}}}"#,
            INDEX_STATE_SCHEMA
        ))
        .expect_err("mismatched generation must fail closed");

        assert!(error.to_string().contains("does not match snapshot"));
    }

    #[test]
    fn generation_without_snapshot_is_rejected() {
        let error = serde_json::from_str::<IndexState>(&format!(
            r#"{{"schema":"{}","snapshot":null,"generation":"gen_orphan","files":{{}}}}"#,
            INDEX_STATE_SCHEMA
        ))
        .expect_err("generation without snapshot must fail closed");

        assert!(error.to_string().contains("has no snapshot identity"));
    }

    #[test]
    fn computes_changed_unchanged_and_removed_files() {
        let previous = IndexState {
            schema: INDEX_STATE_SCHEMA.to_string(),
            snapshot: Some("snap_previous".to_string()),
            files: BTreeMap::from([
                (
                    "docs/auth.md".to_string(),
                    FileState {
                        content_hash: Some("hash-a".to_string()),
                        language_hint: Some("markdown".to_string()),
                    },
                ),
                (
                    "docs/removed.md".to_string(),
                    FileState {
                        content_hash: Some("hash-r".to_string()),
                        language_hint: Some("markdown".to_string()),
                    },
                ),
            ]),
        };
        let current = vec![
            SourceFile {
                path: "docs/auth.md".to_string(),
                language_hint: Some("markdown".to_string()),
                content_hash: Some("hash-a".to_string()),
                content: None,
            },
            SourceFile {
                path: "docs/new.md".to_string(),
                language_hint: Some("markdown".to_string()),
                content_hash: Some("hash-n".to_string()),
                content: None,
            },
        ];

        let affected = previous.affected_files(&current);

        assert!(affected.unchanged.contains("docs/auth.md"));
        assert!(affected.changed.contains("docs/new.md"));
        assert!(affected.removed.contains("docs/removed.md"));
    }

    #[test]
    fn incompatible_state_schema_forces_a_full_rebuild_before_current_wire_validation() {
        let root = std::env::temp_dir().join(format!(
            "athanor-index-state-schema-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("index-state.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            &path,
            r#"{"schema":"athanor.index_state.v999","snapshot":"snap_old","generation":"gen_wrong","future_field":true,"files":{}}"#,
        )
        .unwrap();

        let state = IndexStateStore::new(&path).load().unwrap();

        assert_eq!(state, IndexState::empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prepared_state_rolls_back_to_the_previous_generation() {
        let root = std::env::temp_dir().join(format!(
            "athanor-index-state-rollback-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("index-state.json");
        let store = IndexStateStore::new(&path);
        store
            .save(&IndexState {
                schema: INDEX_STATE_SCHEMA.to_string(),
                snapshot: Some("snap_old".to_string()),
                files: BTreeMap::new(),
            })
            .unwrap();
        store
            .prepare(&IndexState {
                schema: INDEX_STATE_SCHEMA.to_string(),
                snapshot: Some("snap_new".to_string()),
                files: BTreeMap::new(),
            })
            .unwrap()
            .rollback()
            .unwrap();

        assert_eq!(store.load().unwrap().snapshot.as_deref(), Some("snap_old"));
        let restored: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(restored["generation"], "gen_snap_old");
        fs::remove_dir_all(root).unwrap();
    }
}
