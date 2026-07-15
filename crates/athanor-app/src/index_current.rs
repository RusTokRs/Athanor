use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::CanonicalLatestIdentity;
use athanor_domain::{GenerationId, SnapshotId};
use athanor_projector_support::replace_output_file;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const INDEX_CURRENT_SCHEMA: &str = "athanor.index_current.v2";
pub(crate) const INDEX_CURRENT_SCHEMA_V1: &str = "athanor.index_current.v1";
const LEGACY_READ_MODEL_PATH: &str = ".athanor/generated/current/jsonl";
const LEGACY_INDEX_STATE_PATH: &str = ".athanor/state/index-state.json";

/// The single application-level pointer selecting one complete transactional index generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexCurrent {
    schema: String,
    generation: GenerationId,
    snapshot: SnapshotId,
    read_model: String,
    index_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    read_model_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    index_state_sha256: Option<String>,
}

impl IndexCurrent {
    /// Builds a legacy migration pointer without checksum binding.
    pub(crate) fn for_snapshot(snapshot: SnapshotId) -> Self {
        Self::new(snapshot, INDEX_CURRENT_SCHEMA_V1, None, None)
    }

    pub(crate) fn for_snapshot_with_checksums(
        snapshot: SnapshotId,
        read_model_manifest_sha256: String,
        index_state_sha256: String,
    ) -> Self {
        Self::new(
            snapshot,
            INDEX_CURRENT_SCHEMA,
            Some(read_model_manifest_sha256),
            Some(index_state_sha256),
        )
    }

    fn new(
        snapshot: SnapshotId,
        schema: &str,
        read_model_manifest_sha256: Option<String>,
        index_state_sha256: Option<String>,
    ) -> Self {
        let generation = GenerationId::for_snapshot(&snapshot);
        Self {
            schema: schema.to_string(),
            read_model: read_model_relative(&generation),
            index_state: index_state_relative(&generation),
            generation,
            snapshot,
            read_model_manifest_sha256,
            index_state_sha256,
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
        current.validate_artifacts(root)?;
        Ok(Some(current))
    }

    pub(crate) fn write(&self, root: &Path) -> Result<()> {
        self.validate_artifacts(root)?;
        let path = Self::path(root);
        let content = serde_json::to_string_pretty(self)
            .context("failed to serialize index current pointer")?;
        replace_output_file(&path, &content, "index current pointer").map_err(anyhow::Error::new)
    }

    pub(crate) fn path(root: &Path) -> PathBuf {
        root.join(".athanor/state/index-current.json")
    }

    pub(crate) fn generation(&self) -> &GenerationId {
        &self.generation
    }

    pub(crate) fn canonical_identity(&self) -> CanonicalLatestIdentity {
        CanonicalLatestIdentity {
            snapshot: self.snapshot.clone(),
            generation: self.generation.clone(),
        }
    }

    pub(crate) fn is_checksum_bound(&self) -> bool {
        self.schema == INDEX_CURRENT_SCHEMA
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> &SnapshotId {
        &self.snapshot
    }

    #[cfg(test)]
    pub(crate) fn read_model_manifest_sha256(&self) -> Option<&str> {
        self.read_model_manifest_sha256.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn index_state_sha256(&self) -> Option<&str> {
        self.index_state_sha256.as_deref()
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
        validate_artifact_identity(
            &manifest,
            crate::read_model::JSONL_MANIFEST_SCHEMA,
            &self.snapshot,
            &self.generation,
            "read-model manifest",
        )?;

        let state = self.index_state_path(root);
        if !state.is_file() {
            bail!(
                "index current generation {} has no index state at {}",
                self.generation,
                state.display()
            );
        }
        validate_artifact_identity(
            &state,
            crate::index_state::INDEX_STATE_SCHEMA,
            &self.snapshot,
            &self.generation,
            "index state",
        )?;

        if self.is_checksum_bound() {
            crate::artifact_checksum::validate_read_model(
                &read_model,
                self.read_model_manifest_sha256
                    .as_deref()
                    .context("checksummed index current pointer has no manifest digest")?,
            )?;
            crate::artifact_checksum::validate_file_digest(
                &state,
                self.index_state_sha256
                    .as_deref()
                    .context("checksummed index current pointer has no state digest")?,
                "immutable index state",
            )?;
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        match self.schema.as_str() {
            INDEX_CURRENT_SCHEMA_V1 => {
                if self.read_model_manifest_sha256.is_some() || self.index_state_sha256.is_some() {
                    bail!("legacy index current pointer must not contain checksum fields");
                }
            }
            INDEX_CURRENT_SCHEMA => {
                if self.read_model_manifest_sha256.is_none() || self.index_state_sha256.is_none() {
                    bail!("checksummed index current pointer is missing required digests");
                }
            }
            _ => bail!("unsupported index current pointer schema `{}`", self.schema),
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

/// Resolves the validated immutable JSONL read-model directory, or the legacy path when no pointer
/// has been published yet. A present invalid pointer never falls back.
pub fn resolve_read_model_path(root: &Path) -> Result<PathBuf> {
    match IndexCurrent::load(root)
        .context("failed to resolve index current pointer for read model")?
    {
        Some(current) => Ok(current.read_model_path(root)),
        None => Ok(root.join(LEGACY_READ_MODEL_PATH)),
    }
}

/// Resolves the validated immutable index-state file, or the legacy path when no pointer
/// has been published yet. A present invalid pointer never falls back.
pub fn resolve_index_state_path(root: &Path) -> Result<PathBuf> {
    match IndexCurrent::load(root)
        .context("failed to resolve index current pointer for index state")?
    {
        Some(current) => Ok(current.index_state_path(root)),
        None => Ok(root.join(LEGACY_INDEX_STATE_PATH)),
    }
}

fn read_model_relative(generation: &GenerationId) -> String {
    format!(
        ".athanor/generated/index-generations/{}/jsonl",
        generation.as_str()
    )
}

fn index_state_relative(generation: &GenerationId) -> String {
    format!(".athanor/state/index-state-{}.json", generation.as_str())
}

fn validate_artifact_identity(
    path: &Path,
    expected_schema: &str,
    expected_snapshot: &SnapshotId,
    expected_generation: &GenerationId,
    label: &str,
) -> Result<()> {
    let value: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {label} {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {label} {}", path.display()))?;
    let schema = value
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("{label} {} has no schema", path.display()))?;
    if schema != expected_schema {
        bail!(
            "{label} {} has schema `{schema}`, expected `{expected_schema}`",
            path.display()
        );
    }
    let snapshot = value
        .get("snapshot")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("{label} {} has no snapshot identity", path.display()))?;
    if snapshot != expected_snapshot.0.as_str() {
        bail!(
            "{label} {} identifies snapshot `{snapshot}`, expected `{}`",
            path.display(),
            expected_snapshot.0
        );
    }
    let generation = value
        .get("generation")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("{label} {} has no generation identity", path.display()))?;
    if generation != expected_generation.as_str() {
        bail!(
            "{label} {} identifies generation `{generation}`, expected `{expected_generation}`",
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

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
            ".athanor/state/index-state-gen_snap_test.json"
        );
        assert_eq!(
            current.canonical_identity(),
            CanonicalLatestIdentity::for_snapshot(SnapshotId("snap_test".to_string()))
        );
        assert!(!current.is_checksum_bound());
        current.validate().unwrap();
    }

    #[test]
    fn pointer_rejects_foreign_generation_paths_and_checksum_contracts() {
        let mut current = IndexCurrent::for_snapshot(SnapshotId("snap_test".to_string()));
        current.generation = GenerationId("gen_foreign".to_string());
        assert!(
            current
                .validate()
                .unwrap_err()
                .to_string()
                .contains("does not match")
        );

        let mut current = IndexCurrent::for_snapshot(SnapshotId("snap_test".to_string()));
        current.read_model = "../../foreign".to_string();
        assert!(
            current
                .validate()
                .unwrap_err()
                .to_string()
                .contains("read-model path")
        );

        let mut current = IndexCurrent::for_snapshot(SnapshotId("snap_test".to_string()));
        current.schema = INDEX_CURRENT_SCHEMA.to_string();
        assert!(current.validate().is_err());
    }

    #[test]
    fn pointer_round_trip_is_fail_closed() {
        let current = IndexCurrent::for_snapshot(SnapshotId("snap_test".to_string()));
        let value = serde_json::to_value(&current).unwrap();
        let decoded: IndexCurrent = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, current);
    }

    #[test]
    fn pointer_write_and_load_require_matching_complete_artifacts() {
        let root = test_root("complete");
        let current = write_complete_current(&root, "snap_test");

        current.write(&root).unwrap();
        assert_eq!(IndexCurrent::load(&root).unwrap(), Some(current));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checksum_bound_pointer_detects_tampering() {
        let root = test_root("checksum");
        let legacy = write_complete_current(&root, "snap_checksum");
        write_read_model_files(&legacy.read_model_path(&root));
        let manifest_digest =
            crate::artifact_checksum::seal_read_model(&legacy.read_model_path(&root)).unwrap();
        let state_digest =
            crate::artifact_checksum::sha256_file(&legacy.index_state_path(&root)).unwrap();
        let current = IndexCurrent::for_snapshot_with_checksums(
            SnapshotId("snap_checksum".to_string()),
            manifest_digest,
            state_digest,
        );

        current.write(&root).unwrap();
        assert!(current.is_checksum_bound());
        assert!(current.read_model_manifest_sha256().is_some());
        assert!(current.index_state_sha256().is_some());
        fs::write(
            current.read_model_path(&root).join("entities.jsonl"),
            "tampered\n",
        )
        .unwrap();
        assert!(IndexCurrent::load(&root).is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolvers_use_pointer_and_fall_back_only_when_absent() {
        let root = test_root("resolver");
        assert_eq!(
            resolve_read_model_path(&root).unwrap(),
            root.join(LEGACY_READ_MODEL_PATH)
        );
        assert_eq!(
            resolve_index_state_path(&root).unwrap(),
            root.join(LEGACY_INDEX_STATE_PATH)
        );

        let current = write_complete_current(&root, "snap_current");
        current.write(&root).unwrap();
        assert_eq!(
            resolve_read_model_path(&root).unwrap(),
            current.read_model_path(&root)
        );
        assert_eq!(
            resolve_index_state_path(&root).unwrap(),
            current.index_state_path(&root)
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn write_complete_current(root: &Path, snapshot: &str) -> IndexCurrent {
        let current = IndexCurrent::for_snapshot(SnapshotId(snapshot.to_string()));
        let generation = format!("gen_{snapshot}");
        let read_model = current.read_model_path(root);
        fs::create_dir_all(&read_model).unwrap();
        fs::write(
            read_model.join("manifest.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": crate::read_model::JSONL_MANIFEST_SCHEMA,
                "snapshot": snapshot,
                "generation": generation
            }))
            .unwrap(),
        )
        .unwrap();
        let state = current.index_state_path(root);
        fs::create_dir_all(state.parent().unwrap()).unwrap();
        fs::write(
            &state,
            serde_json::to_vec_pretty(&json!({
                "schema": crate::index_state::INDEX_STATE_SCHEMA,
                "snapshot": snapshot,
                "generation": format!("gen_{snapshot}"),
                "files": {}
            }))
            .unwrap(),
        )
        .unwrap();
        current
    }

    fn write_read_model_files(read_model: &Path) {
        for name in [
            "diagnostics.jsonl",
            "entities.jsonl",
            "facts.jsonl",
            "relations.jsonl",
        ] {
            fs::write(read_model.join(name), "").unwrap();
        }
    }

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("athanor-index-current-{label}-{nonce}"))
    }
}
