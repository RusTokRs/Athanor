use std::fs;
use std::path::Path;

use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CanonicalLatestIdentity, CanonicalLatestPointer, CanonicalSnapshot,
    CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery, EntityResolver,
    KnowledgeStore, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use serde::Deserialize;

mod current {
    include!("lib.rs");
}

pub use current::{PathIndex, PathIndexEntry, StableKeyIndex};

const LATEST_SCHEMA: &str = "athanor.canonical_latest.v1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictLatestDocument {
    schema: String,
    snapshot: SnapshotId,
    generation: athanor_domain::GenerationId,
}

#[derive(Debug, Clone)]
pub struct JsonlKnowledgeStore {
    inner: current::JsonlKnowledgeStore,
}

impl JsonlKnowledgeStore {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            inner: current::JsonlKnowledgeStore::new(root),
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
        self.inner.publish_snapshot_batch(snapshot, batch).await
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
        self.inner.load_snapshot(snapshot).await
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_latest_snapshot().await
    }
}

#[async_trait]
impl CanonicalLatestPointer for JsonlKnowledgeStore {
    async fn load_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        let path = self.root().join("latest.json");
        if !path.exists() {
            return Ok(None);
        }
        let document: StrictLatestDocument = serde_json::from_slice(
            &fs::read(&path).map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to read canonical latest pointer {}: {error}",
                    path.display()
                ))
            })?,
        )
        .map_err(|error| {
            CoreError::AdapterProtocol(format!(
                "canonical latest pointer {} requires normalization: {error}",
                path.display()
            ))
        })?;
        if document.schema != LATEST_SCHEMA {
            return Err(CoreError::AdapterProtocol(format!(
                "canonical latest pointer {} has unsupported schema {}",
                path.display(),
                document.schema
            )));
        }
        let identity = CanonicalLatestIdentity {
            snapshot: document.snapshot,
            generation: document.generation,
        };
        identity.validate()?;
        let compatible = self.inner.load_latest_identity().await?;
        if compatible.as_ref() != Some(&identity) {
            return Err(CoreError::AdapterProtocol(format!(
                "canonical latest pointer {} does not resolve its strict identity",
                path.display()
            )));
        }
        Ok(Some(identity))
    }

    async fn discover_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        self.inner.discover_latest_identity().await
    }

    async fn validate_latest_identity(&self, identity: &CanonicalLatestIdentity) -> CoreResult<()> {
        self.inner.validate_latest_identity(identity).await
    }

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()> {
        self.inner.repair_latest_identity(identity).await
    }
}

#[cfg(test)]
mod tests {
    use athanor_core::{
        AtomicSnapshotPublication, CanonicalLatestPointer, CanonicalSnapshotStore, KnowledgeStore,
        SnapshotBatch,
    };
    use athanor_domain::{RepoId, SnapshotBase};

    use super::*;

    #[tokio::test]
    async fn legacy_latest_remains_queryable_but_requires_repair_normalization() {
        let root = test_root();
        let store = JsonlKnowledgeStore::new(&root);
        let snapshot = store
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        store
            .publish_snapshot_batch(snapshot.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        fs::write(
            root.join("latest.json"),
            serde_json::to_vec_pretty(&serde_json::json!({ "snapshot": snapshot.0 })).unwrap(),
        )
        .unwrap();

        assert!(store.load_latest_snapshot().await.unwrap().is_some());
        assert!(store.load_latest_identity().await.is_err());
        let target = store.discover_latest_identity().await.unwrap().unwrap();
        store.repair_latest_identity(target.clone()).await.unwrap();
        assert_eq!(store.load_latest_identity().await.unwrap(), Some(target));
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

    fn test_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "athanor-jsonl-strict-latest-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
