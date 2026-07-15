use std::fs;
use std::path::Path;

use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult,
    DiagnosticQuery, EntityQuery, EntityResolver, KnowledgeStore, RelationQuery, SnapshotBatch,
    SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use serde_json::Value;

mod legacy {
    include!("store.rs");

    pub(crate) mod atomic_publication {
        include!("atomic_publication.rs");
    }
}

pub use legacy::{PathIndex, PathIndexEntry, StableKeyIndex};

const CANONICAL_MANIFEST_SCHEMA: &str = "athanor.canonical_snapshot.v1";

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
        self.inner.commit_snapshot(snapshot).await
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
        AtomicSnapshotPublication::publish_snapshot_batch(&self.inner, snapshot, batch).await
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
        let Some(latest) = self.inner.load_latest_snapshot().await? else {
            return Ok(None);
        };
        let snapshot = latest.snapshot.ok_or_else(|| {
            CoreError::AdapterProtocol("latest JSONL snapshot has no snapshot identity".to_string())
        })?;
        self.load_snapshot(&snapshot).await
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
            legacy::atomic_publication::validate_commit_marker(snapshot_dir, snapshot)
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
