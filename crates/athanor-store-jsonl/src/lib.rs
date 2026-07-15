use std::fs::{self, File};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CanonicalLatestIdentity, CanonicalLatestPointer, CanonicalSnapshot,
    CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery, EntityResolver,
    KnowledgeStore, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, GenerationId, Relation, RepoId, SnapshotBase, SnapshotId,
    StableKey,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod legacy {
    include!("store.rs");

    pub(crate) mod atomic_publication {
        include!("atomic_publication.rs");
    }
}

pub use legacy::{PathIndex, PathIndexEntry, StableKeyIndex};

const CANONICAL_MANIFEST_SCHEMA: &str = "athanor.canonical_snapshot.v1";
const LATEST_SCHEMA: &str = "athanor.canonical_latest.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LatestIdentityDocument {
    schema: String,
    snapshot: String,
    generation: GenerationId,
}

#[derive(Debug, Clone)]
pub struct JsonlKnowledgeStore {
    inner: legacy::JsonlKnowledgeStore,
}

impl JsonlKnowledgeStore {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            inner: legacy::JsonlKnowledgeStore::new(root),
        }
    }

    pub fn root(&self) -> &Path {
        self.inner.root()
    }

    fn write_latest_identity(&self, snapshot: &SnapshotId) -> CoreResult<()> {
        let _lock = acquire_pointer_lock(self.root())?;
        write_latest_identity(self.root(), snapshot)
    }
}

#[async_trait]
impl KnowledgeStore for JsonlKnowledgeStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        self.inner.begin_snapshot(repo, base).await
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        self.inner.put_entities(snapshot, entities).await
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        self.inner.put_facts(snapshot, facts).await
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        self.inner.put_relations(snapshot, relations).await
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        self.inner.put_diagnostics(snapshot, diagnostics).await
    }

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        self.inner.put_snapshot(snapshot, batch).await
    }

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.inner.prepare_snapshot(snapshot).await
    }

    async fn query_entities(
        &self,
        snapshot: SnapshotSelector,
        query: EntityQuery,
    ) -> CoreResult<Vec<Entity>> {
        self.inner.query_entities(snapshot, query).await
    }

    async fn query_relations(
        &self,
        snapshot: SnapshotSelector,
        query: RelationQuery,
    ) -> CoreResult<Vec<Relation>> {
        self.inner.query_relations(snapshot, query).await
    }

    async fn query_diagnostics(
        &self,
        snapshot: SnapshotSelector,
        query: DiagnosticQuery,
    ) -> CoreResult<Vec<Diagnostic>> {
        self.inner.query_diagnostics(snapshot, query).await
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.inner.commit_snapshot(snapshot.clone()).await?;
        self.write_latest_identity(&snapshot).map_err(|error| {
            CoreError::Adapter(format!(
                "snapshot {} is committed but generation-bearing latest update failed: {error}",
                snapshot.0
            ))
        })
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.inner.abort_snapshot(snapshot).await
    }
}

#[async_trait]
impl AtomicSnapshotPublication for JsonlKnowledgeStore {
    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        AtomicSnapshotPublication::publish_snapshot_batch(&self.inner, snapshot.clone(), batch)
            .await?;
        self.write_latest_identity(&snapshot).map_err(|error| {
            CoreError::Adapter(format!(
                "snapshot {} is committed but generation-bearing latest update failed: {error}",
                snapshot.0
            ))
        })
    }
}

#[async_trait]
impl EntityResolver for JsonlKnowledgeStore {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        self.inner.resolve_stable_key(snapshot, stable_key).await
    }
}

#[async_trait]
impl CanonicalSnapshotStore for JsonlKnowledgeStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        let snapshot_dir = self.root().join("snapshots").join(&snapshot.0);
        if !snapshot_dir.exists() {
            return Ok(None);
        }
        validate_exact_generation(&snapshot_dir, snapshot)?;
        self.inner.load_snapshot(snapshot).await
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        let Some(identity) = self.load_latest_identity().await? else {
            return Ok(None);
        };
        self.load_snapshot(&identity.snapshot).await
    }
}

#[async_trait]
impl CanonicalLatestPointer for JsonlKnowledgeStore {
    async fn load_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        let path = self.root().join("latest.json");
        if !path.exists() {
            return Ok(None);
        }
        let value: Value = serde_json::from_slice(&fs::read(&path).map_err(|error| {
            CoreError::Adapter(format!("failed to read latest pointer {}: {error}", path.display()))
        })?)
        .map_err(|error| {
            CoreError::AdapterProtocol(format!(
                "failed to parse latest pointer {}: {error}",
                path.display()
            ))
        })?;
        let snapshot = value
            .get("snapshot")
            .and_then(Value::as_str)
            .map(|snapshot| SnapshotId(snapshot.to_string()))
            .ok_or_else(|| {
                CoreError::AdapterProtocol(format!(
                    "latest pointer {} has no snapshot identity",
                    path.display()
                ))
            })?;
        let identity = CanonicalLatestIdentity::for_snapshot(snapshot.clone());
        if let Some(schema) = value.get("schema") {
            if schema.as_str() != Some(LATEST_SCHEMA) {
                return Err(CoreError::AdapterProtocol(format!(
                    "latest pointer {} has unsupported schema",
                    path.display()
                )));
            }
        }
        if let Some(generation) = value.get("generation") {
            if generation.as_str() != Some(identity.generation.as_str()) {
                return Err(CoreError::AdapterProtocol(format!(
                    "latest pointer {} generation does not match snapshot {}",
                    path.display(),
                    snapshot.0
                )));
            }
        }
        let exact = self.load_snapshot(&snapshot).await?.ok_or_else(|| {
            CoreError::NotFound(format!("latest snapshot {} has no committed generation", snapshot.0))
        })?;
        if exact.snapshot.as_ref() != Some(&snapshot) {
            return Err(CoreError::AdapterProtocol(format!(
                "latest pointer {} resolved identity {:?}, expected {}",
                path.display(),
                exact.snapshot,
                snapshot.0
            )));
        }
        Ok(Some(identity))
    }

    async fn discover_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        discover_latest_identity(self.root())
    }

    async fn validate_latest_identity(&self, identity: &CanonicalLatestIdentity) -> CoreResult<()> {
        identity.validate()?;
        let snapshot_dir = self.root().join("snapshots").join(&identity.snapshot.0);
        validate_repair_target(&snapshot_dir, &identity.snapshot)?;
        let exact = self.load_snapshot(&identity.snapshot).await?.ok_or_else(|| {
            CoreError::NotFound(format!("snapshot {}", identity.snapshot.0))
        })?;
        if exact.snapshot.as_ref() != Some(&identity.snapshot) {
            return Err(CoreError::AdapterProtocol(format!(
                "exact JSONL snapshot returned identity {:?}, expected {}",
                exact.snapshot, identity.snapshot.0
            )));
        }
        let authoritative = self.discover_latest_identity().await?.ok_or_else(|| {
            CoreError::NotFound("JSONL store has no repairable committed generation".to_string())
        })?;
        if &authoritative != identity {
            return Err(CoreError::Conflict(format!(
                "authoritative JSONL latest is {} / {}, requested {} / {}",
                authoritative.snapshot.0,
                authoritative.generation,
                identity.snapshot.0,
                identity.generation
            )));
        }
        Ok(())
    }

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()> {
        self.validate_latest_identity(&identity).await?;
        self.write_latest_identity(&identity.snapshot)
    }
}

fn validate_exact_generation(snapshot_dir: &Path, snapshot: &SnapshotId) -> CoreResult<()> {
    let manifest_path = snapshot_dir.join("manifest.json");
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to read canonical manifest {}: {error}",
            manifest_path.display()
        ))
    })?)
    .map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "failed to parse canonical manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    if manifest.get("schema").and_then(Value::as_str) != Some(CANONICAL_MANIFEST_SCHEMA) {
        return Err(CoreError::AdapterProtocol(format!(
            "canonical manifest {} has an unsupported schema",
            manifest_path.display()
        )));
    }
    if manifest.get("snapshot").and_then(Value::as_str) != Some(snapshot.0.as_str()) {
        return Err(CoreError::AdapterProtocol(format!(
            "canonical manifest {} does not identify `{}`",
            manifest_path.display(),
            snapshot.0
        )));
    }

    let marker_path = snapshot_dir.join("commit.json");
    match manifest.get("commit_marker_schema") {
        Some(Value::String(schema))
            if schema == legacy::atomic_publication::SNAPSHOT_COMMIT_SCHEMA
                || schema == legacy::atomic_publication::SNAPSHOT_COMMIT_SCHEMA_V1 =>
        {
            legacy::atomic_publication::validate_commit_marker_schema(
                snapshot_dir,
                snapshot,
                schema,
            )
        }
        Some(Value::String(schema)) => Err(CoreError::AdapterProtocol(format!(
            "canonical manifest {} requires unsupported commit marker schema `{schema}`",
            manifest_path.display()
        ))),
        Some(_) => Err(CoreError::AdapterProtocol(format!(
            "canonical manifest {} has a non-string commit_marker_schema",
            manifest_path.display()
        ))),
        None if marker_path.exists() => {
            legacy::atomic_publication::validate_commit_marker(snapshot_dir, snapshot)
        }
        None => Ok(()),
    }
}

fn validate_repair_target(snapshot_dir: &Path, snapshot: &SnapshotId) -> CoreResult<()> {
    let manifest_path = snapshot_dir.join("manifest.json");
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to read canonical manifest {}: {error}",
            manifest_path.display()
        ))
    })?)
    .map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "failed to parse canonical manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    if manifest.get("commit_marker_schema").and_then(Value::as_str)
        != Some(legacy::atomic_publication::SNAPSHOT_COMMIT_SCHEMA)
    {
        return Err(CoreError::AdapterProtocol(format!(
            "latest repair target {} must declare commit marker schema {}",
            snapshot.0,
            legacy::atomic_publication::SNAPSHOT_COMMIT_SCHEMA
        )));
    }
    legacy::atomic_publication::validate_commit_marker_schema(
        snapshot_dir,
        snapshot,
        legacy::atomic_publication::SNAPSHOT_COMMIT_SCHEMA,
    )
}

fn discover_latest_identity(root: &Path) -> CoreResult<Option<CanonicalLatestIdentity>> {
    let snapshots = root.join("snapshots");
    let entries = match fs::read_dir(&snapshots) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CoreError::Adapter(format!(
                "failed to inspect canonical snapshots {}: {error}",
                snapshots.display()
            )));
        }
    };
    let mut latest = None;
    for entry in entries {
        let entry = entry.map_err(|error| {
            CoreError::Adapter(format!("failed to inspect canonical snapshot entry: {error}"))
        })?;
        if !entry
            .file_type()
            .map_err(|error| CoreError::Adapter(format!("failed to inspect snapshot type: {error}")))?
            .is_dir()
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let snapshot = SnapshotId(name);
        if latest
            .as_ref()
            .is_none_or(|current: &SnapshotId| snapshot.0 > current.0)
        {
            latest = Some(snapshot);
        }
    }
    let Some(latest) = latest else {
        return Ok(None);
    };
    let snapshot_dir = snapshots.join(&latest.0);
    validate_exact_generation(&snapshot_dir, &latest)?;
    validate_repair_target(&snapshot_dir, &latest)?;
    Ok(Some(CanonicalLatestIdentity::for_snapshot(latest)))
}

fn acquire_pointer_lock(root: &Path) -> CoreResult<File> {
    fs::create_dir_all(root)
        .map_err(|error| CoreError::Adapter(format!("failed to create store dir: {error}")))?;
    let path = root.join(".writer.lock");
    let file = File::create(&path)
        .map_err(|error| CoreError::Adapter(format!("failed to open writer lock: {error}")))?;
    file.lock_exclusive()
        .map_err(|error| CoreError::Adapter(format!("failed to acquire writer lock: {error}")))?;
    Ok(file)
}

fn write_latest_identity(root: &Path, snapshot: &SnapshotId) -> CoreResult<()> {
    fs::create_dir_all(root)
        .map_err(|error| CoreError::Adapter(format!("failed to create store dir: {error}")))?;
    let target = root.join("latest.json");
    let staging = root.join(format!(".latest.json.identity-staging-{}", unique_suffix()));
    let document = LatestIdentityDocument {
        schema: LATEST_SCHEMA.to_string(),
        snapshot: snapshot.0.clone(),
        generation: GenerationId::for_snapshot(snapshot),
    };
    fs::write(
        &staging,
        serde_json::to_vec_pretty(&document).map_err(|error| {
            CoreError::Adapter(format!("failed to serialize latest identity: {error}"))
        })?,
    )
    .map_err(|error| CoreError::Adapter(format!("failed to stage latest identity: {error}")))?;
    replace_pointer(&staging, &target)
}

fn replace_pointer(staging: &Path, target: &Path) -> CoreResult<()> {
    if !target.exists() {
        return fs::rename(staging, target)
            .map_err(|error| CoreError::Adapter(format!("failed to publish latest identity: {error}")));
    }
    let backup = target.with_extension(format!("json.identity-backup-{}", unique_suffix()));
    fs::rename(target, &backup)
        .map_err(|error| CoreError::Adapter(format!("failed to stage old latest identity: {error}")))?;
    if let Err(error) = fs::rename(staging, target) {
        let _ = fs::rename(&backup, target);
        return Err(CoreError::Adapter(format!(
            "failed to publish latest identity: {error}"
        )));
    }
    fs::remove_file(&backup).map_err(|error| {
        CoreError::Adapter(format!("failed to remove previous latest identity: {error}"))
    })
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod latest_pointer_tests {
    use athanor_core::{
        AtomicSnapshotPublication, CanonicalLatestIdentity, CanonicalLatestPointer, KnowledgeStore,
        SnapshotBatch,
    };
    use athanor_domain::{GenerationId, RepoId, SnapshotBase};

    use super::*;

    #[tokio::test]
    async fn repairs_missing_or_stale_latest_from_valid_v2_generation() {
        let root = test_root("repair");
        let store = JsonlKnowledgeStore::new(&root);
        let first = store
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        store
            .publish_snapshot_batch(first.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        let second = store
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        store
            .publish_snapshot_batch(second.clone(), SnapshotBatch::default())
            .await
            .unwrap();

        fs::write(
            root.join("latest.json"),
            serde_json::to_vec_pretty(&serde_json::json!({ "snapshot": first.0 })).unwrap(),
        )
        .unwrap();
        assert_eq!(
            store.discover_latest_identity().await.unwrap().unwrap(),
            CanonicalLatestIdentity::for_snapshot(second.clone())
        );
        store
            .validate_latest_identity(&CanonicalLatestIdentity::for_snapshot(second.clone()))
            .await
            .unwrap();
        store
            .repair_latest_identity(CanonicalLatestIdentity::for_snapshot(second.clone()))
            .await
            .unwrap();
        assert_eq!(
            store.load_latest_identity().await.unwrap().unwrap(),
            CanonicalLatestIdentity::for_snapshot(second.clone())
        );
        let error = store
            .validate_latest_identity(&CanonicalLatestIdentity::for_snapshot(first))
            .await
            .expect_err("repair must not rewind canonical latest");
        assert!(matches!(error, CoreError::Conflict(_)));

        store
            .repair_latest_identity(CanonicalLatestIdentity::for_snapshot(second))
            .await
            .unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn repair_rejects_generation_mismatch() {
        let root = test_root("mismatch");
        let store = JsonlKnowledgeStore::new(&root);
        let snapshot = store
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        store
            .publish_snapshot_batch(snapshot.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        let error = store
            .validate_latest_identity(&CanonicalLatestIdentity {
                snapshot,
                generation: GenerationId("gen_wrong".to_string()),
            })
            .await
            .expect_err("mismatched generation must fail");
        assert!(matches!(error, CoreError::InvalidInput(_)));
        fs::remove_dir_all(root).unwrap();
    }

    fn snapshot_base() -> SnapshotBase {
        SnapshotBase {
            branch: None,
            commit: None,
            parent_snapshot: None,
            working_tree: true,
        }
    }

    fn test_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "athanor-jsonl-latest-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
