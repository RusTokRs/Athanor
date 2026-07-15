use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_domain::{GenerationId, SnapshotId};
use athanor_projector_support::replace_output_file;
use serde::{Deserialize, Serialize};

pub(crate) const INDEX_CURRENT_SCHEMA: &str = "athanor.index_current.v1";

/// The single application-level pointer selecting one complete transactional index generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexCurrent {
    schema: String,
    generation: GenerationId,
    snapshot: SnapshotId,
    read_model: String,
    index_state: String,
}

impl IndexCurrent {
    pub(crate) fn for_snapshot(snapshot: SnapshotId) -> Self {
        let generation = GenerationId::for_snapshot(&snapshot);
        Self {
            schema: INDEX_CURRENT_SCHEMA.to_string(),
            read_model: read_model_relative(&generation),
            index_state: index_state_relative(&generation),
            generation,
            snapshot,
        }
    }

    pub(crate) fn load(root: &Path) -> Result<Option<Self>> {
        let path = Self::path(root);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read index current pointer {}", path.display()))?;
        let current: Self = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse index current pointer {}", path.display()))?;
        current.validate()?;
        Ok(Some(current))
    }

    pub(crate) fn write(&self, root: &Path) -> Result<()> {
        self.validate()?;
        let path = Self::path(root);
        let content = serde_json::to_string_pretty(self)
            .context("failed to serialize index current pointer")?;
        replace_output_file(&path, &content, "index current pointer")
            .map_err(anyhow::Error::new)
    }

    pub(crate) fn path(root: &Path) -> PathBuf {
        root.join(".athanor/state/index-current.json")
    }

    pub(crate) fn generation(&self) -> &GenerationId {
        &self.generation
    }

    pub(crate) fn snapshot(&self) -> &SnapshotId {
        &self.snapshot
    }

    pub(crate) fn read_model_path(&self, root: &Path) -> PathBuf {
        root.join(&self.read_model)
    }

    pub(crate) fn index_state_path(&self, root: &Path) -> PathBuf {
        root.join(&self.index_state)
    }

    pub(crate) fn validate_artifacts(&self, root: &Path) -> Result<()> {
        self.validate()?;
        let read_model = self.read_model_path(root);
        let manifest = read_model.join("manifest.json");
        if !read_model.is_dir() || !manifest.is_file() {
            bail!(
                "index current generation {} has no complete read model at {}",
                self.generation,
                read_model.display()
            );
        }
        let state = self.index_state_path(root);
        if !state.is_file() {
            bail!(
                "index current generation {} has no index state at {}",
                self.generation,
                state.display()
            );
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.schema != INDEX_CURRENT_SCHEMA {
            bail!(
                "unsupported index current pointer schema `{}`",
                self.schema
            );
        }
        if self.snapshot.0.trim().is_empty() {
            bail!("index current pointer has an empty snapshot identity");
        }
        let expected_generation = GenerationId::for_snapshot(&self.snapshot);
        if self.generation != expected_generation {
            bail!(
                "index current generation `{}` does not match snapshot `{}`",
                self.generation,
                self.snapshot.0
            );
        }
        let expected_read_model = read_model_relative(&self.generation);
        if self.read_model != expected_read_model {
            bail!(
                "index current read-model path `{}` does not match generation `{}`",
                self.read_model,
                self.generation
            );
        }
        let expected_index_state = index_state_relative(&self.generation);
        if self.index_state != expected_index_state {
            bail!(
                "index current state path `{}` does not match generation `{}`",
                self.index_state,
                self.generation
            );
        }
        Ok(())
    }
}

pub(crate) fn read_model_path(root: &Path, generation: &GenerationId) -> PathBuf {
    root.join(read_model_relative(generation))
}

pub(crate) fn index_state_path(root: &Path, generation: &GenerationId) -> PathBuf {
    root.join(index_state_relative(generation))
}

fn read_model_relative(generation: &GenerationId) -> String {
    format!(
        ".athanor/generated/index-generations/{}/jsonl",
        generation.as_str()
    )
}

fn index_state_relative(generation: &GenerationId) -> String {
    format!(
        ".athanor/state/index-generations/{}/index-state.json",
        generation.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_paths_are_deterministic_and_generation_scoped() {
        let current = IndexCurrent::for_snapshot(SnapshotId("snap_test".to_string()));

        assert_eq!(current.generation().as_str(), "gen_snap_test");
        assert_eq!(
            current.read_model,
            ".athanor/generated/index-generations/gen_snap_test/jsonl"
        );
        assert_eq!(
            current.index_state,
            ".athanor/state/index-generations/gen_snap_test/index-state.json"
        );
        current.validate().unwrap();
    }

    #[test]
    fn pointer_rejects_foreign_generation_and_paths() {
        let mut current = IndexCurrent::for_snapshot(SnapshotId("snap_test".to_string()));
        current.generation = GenerationId("gen_foreign".to_string());
        assert!(current.validate().unwrap_err().to_string().contains("does not match"));

        let mut current = IndexCurrent::for_snapshot(SnapshotId("snap_test".to_string()));
        current.read_model = "../../foreign".to_string();
        assert!(current.validate().unwrap_err().to_string().contains("read-model path"));
    }

    #[test]
    fn pointer_round_trip_is_fail_closed() {
        let current = IndexCurrent::for_snapshot(SnapshotId("snap_test".to_string()));
        let value = serde_json::to_value(&current).unwrap();
        let decoded: IndexCurrent = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, current);
    }
}
