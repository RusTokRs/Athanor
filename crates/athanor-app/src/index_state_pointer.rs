use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

mod legacy {
    include!("index_state.rs");
}

pub use legacy::{AffectedFileSet, FileState, INDEX_STATE_SCHEMA, IndexState, PreparedIndexState};

/// Index-state store with pointer-first reads and compatibility writes to the configured path.
#[derive(Debug, Clone)]
pub struct IndexStateStore {
    inner: legacy::IndexStateStore,
}

impl IndexStateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            inner: legacy::IndexStateStore::new(path),
        }
    }

    pub fn load(&self) -> Result<IndexState> {
        let read_path = resolve_read_path(self.inner.path())?;
        legacy::IndexStateStore::new(read_path)
            .load()
            .context("failed to load pointer-selected index state")
    }

    pub fn save(&self, state: &IndexState) -> Result<()> {
        self.inner.save(state)
    }

    pub fn prepare(&self, state: &IndexState) -> Result<PreparedIndexState> {
        self.inner.prepare(state)
    }

    pub fn prepare_with_publication_id(
        &self,
        state: &IndexState,
        publication_id: &str,
    ) -> Result<PreparedIndexState> {
        self.inner
            .prepare_with_publication_id(state, publication_id)
    }

    pub fn path(&self) -> &Path {
        self.inner.path()
    }
}

fn resolve_read_path(configured: &Path) -> Result<PathBuf> {
    let Some(root) = legacy_project_root(configured) else {
        return Ok(configured.to_path_buf());
    };
    crate::index_current::resolve_index_state_path(root)
        .context("failed to resolve index current pointer for index-state read")
}

fn legacy_project_root(path: &Path) -> Option<&Path> {
    if path.file_name()?.to_str()? != "index-state.json" {
        return None;
    }
    let state_dir = path.parent()?;
    if state_dir.file_name()?.to_str()? != "state" {
        return None;
    }
    let athanor_dir = state_dir.parent()?;
    if athanor_dir.file_name()?.to_str()? != ".athanor" {
        return None;
    }
    athanor_dir.parent()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use athanor_domain::SnapshotId;
    use serde_json::json;

    use super::*;
    use crate::index_current::IndexCurrent;

    #[test]
    fn load_prefers_valid_pointer_selected_state() {
        let root = test_root("selected");
        let legacy_path = root.join(".athanor/state/index-state.json");
        legacy::IndexStateStore::new(&legacy_path)
            .save(&IndexState::from_sources("snap_legacy", &[]))
            .unwrap();
        let current = IndexCurrent::for_snapshot(SnapshotId("snap_current".to_string()));
        let read_model = current.read_model_path(&root);
        fs::create_dir_all(&read_model).unwrap();
        fs::write(
            read_model.join("manifest.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": crate::read_model::JSONL_MANIFEST_SCHEMA,
                "snapshot": "snap_current",
                "generation": "gen_snap_current"
            }))
            .unwrap(),
        )
        .unwrap();
        legacy::IndexStateStore::new(current.index_state_path(&root))
            .save(&IndexState::from_sources("snap_current", &[]))
            .unwrap();
        current.write(&root).unwrap();

        let loaded = IndexStateStore::new(&legacy_path).load().unwrap();

        assert_eq!(loaded.snapshot.as_deref(), Some("snap_current"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_falls_back_to_legacy_state_without_pointer() {
        let root = test_root("fallback");
        let legacy_path = root.join(".athanor/state/index-state.json");
        legacy::IndexStateStore::new(&legacy_path)
            .save(&IndexState::from_sources("snap_legacy", &[]))
            .unwrap();

        let loaded = IndexStateStore::new(&legacy_path).load().unwrap();

        assert_eq!(loaded.snapshot.as_deref(), Some("snap_legacy"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_rejects_present_pointer_with_missing_artifacts() {
        let root = test_root("corrupt");
        let legacy_path = root.join(".athanor/state/index-state.json");
        legacy::IndexStateStore::new(&legacy_path)
            .save(&IndexState::from_sources("snap_legacy", &[]))
            .unwrap();
        fs::write(
            root.join(".athanor/state/index-current.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": "athanor.index_current.v1",
                "generation": "gen_snap_current",
                "snapshot": "snap_current",
                "read_model": ".athanor/generated/index-generations/gen_snap_current/jsonl",
                "index_state": ".athanor/state/index-state-gen_snap_current.json"
            }))
            .unwrap(),
        )
        .unwrap();

        let error = IndexStateStore::new(&legacy_path)
            .load()
            .expect_err("present corrupt pointer must fail closed");

        assert!(
            error
                .to_string()
                .contains("failed to resolve index current pointer")
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("athanor-index-state-pointer-{label}-{nonce}"))
    }
}
