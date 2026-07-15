use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::SourceFile;
use athanor_domain::{GenerationId, SnapshotId};
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "js-ts-precision"))]
pub const INDEX_STATE_SCHEMA: &str = "athanor.index_state.v46";
#[cfg(feature = "js-ts-precision")]
pub const INDEX_STATE_SCHEMA: &str = "athanor.index_state.v46-js-ts-precision-v1";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexState {
    pub schema: String,
    pub snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    pub files: BTreeMap<String, FileState>,
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
            generation: None,
            files: BTreeMap::new(),
        }
    }

    pub fn from_sources(snapshot: impl Into<String>, sources: &[SourceFile]) -> Self {
        let snapshot = snapshot.into();
        let generation = GenerationId::for_snapshot(&SnapshotId(snapshot.clone()));
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
            snapshot: Some(snapshot),
            generation: Some(generation.0),
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
        let state = serde_json::from_str::<IndexState>(&content)
            .with_context(|| format!("failed to parse {}", self.path.display()))?;

        if state.schema != INDEX_STATE_SCHEMA {
            return Ok(IndexState::empty());
        }

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
    fn state_joins_snapshot_to_immutable_generation() {
        let state = IndexState::from_sources("snap_test", &[]);

        assert_eq!(state.snapshot.as_deref(), Some("snap_test"));
        assert_eq!(state.generation.as_deref(), Some("gen_snap_test"));
    }

    #[test]
    fn legacy_state_without_generation_remains_readable() {
        let state: IndexState = serde_json::from_str(&format!(
            r#"{{"schema":"{}","snapshot":"snap_old","files":{{}}}}"#,
            INDEX_STATE_SCHEMA
        ))
        .unwrap();

        assert_eq!(state.snapshot.as_deref(), Some("snap_old"));
        assert_eq!(state.generation, None);
    }

    #[test]
    fn computes_changed_unchanged_and_removed_files() {
        let previous = IndexState {
            schema: INDEX_STATE_SCHEMA.to_string(),
            snapshot: Some("snap_previous".to_string()),
            generation: Some("gen_snap_previous".to_string()),
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
}
