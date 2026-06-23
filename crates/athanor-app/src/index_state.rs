use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::SourceFile;
use serde::{Deserialize, Serialize};

pub const INDEX_STATE_SCHEMA: &str = "athanor.index_state.v24";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexState {
    pub schema: String,
    pub snapshot: Option<String>,
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
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        fs::write(&self.path, serde_json::to_string_pretty(state)?)
            .with_context(|| format!("failed to write {}", self.path.display()))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn incompatible_state_schema_forces_a_full_rebuild() {
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
            r#"{"schema":"athanor.index_state.v9","snapshot":"snap_old","files":{}}"#,
        )
        .unwrap();

        let state = IndexStateStore::new(&path).load().unwrap();

        assert_eq!(state, IndexState::empty());
        fs::remove_dir_all(root).unwrap();
    }
}
