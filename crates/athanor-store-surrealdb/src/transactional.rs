//! Process-local serialization boundary for the SurrealDB knowledge store.
//!
//! SurrealDB 2.x executes each SDK request in its own transaction. The legacy adapter remains in
//! `lib.rs`; this crate root wraps it with one shared async write gate so cloned store handles cannot
//! race snapshot allocation, writes, prepare, commit, or abort operations inside one Athanor
//! process. Cross-process coordination and native multi-statement `put_snapshot` transactions remain
//! separate backend work.

use std::sync::Arc;

use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, KnowledgeStore, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use tokio::sync::Mutex;

#[path = "lib.rs"]
mod legacy;

/// SurrealDB adapter with a process-local write serialization boundary shared by all clones.
#[derive(Debug, Clone)]
pub struct SurrealKnowledgeStore {
    inner: legacy::SurrealKnowledgeStore,
    write_gate: Arc<Mutex<()>>,
}

impl SurrealKnowledgeStore {
    pub async fn connect(uri: &str) -> Result<Self, athanor_core::CoreError> {
        Ok(Self {
            inner: legacy::SurrealKnowledgeStore::connect(uri).await?,
            write_gate: Arc::new(Mutex::new(())),
        })
    }
}

#[async_trait]
impl KnowledgeStore for SurrealKnowledgeStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        let _guard = self.write_gate.lock().await;
        self.inner.begin_snapshot(repo, base).await
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.inner.put_entities(snapshot, entities).await
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.inner.put_facts(snapshot, facts).await
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.inner.put_relations(snapshot, relations).await
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.inner.put_diagnostics(snapshot, diagnostics).await
    }

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.inner.put_snapshot(snapshot, batch).await
    }

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
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
        let _guard = self.write_gate.lock().await;
        self.inner.commit_snapshot(snapshot).await
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.inner.abort_snapshot(snapshot).await
    }
}

#[async_trait]
impl EntityResolver for SurrealKnowledgeStore {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        self.inner.resolve_stable_key(snapshot, stable_key).await
    }
}

#[async_trait]
impl CanonicalSnapshotStore for SurrealKnowledgeStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_snapshot(snapshot).await
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_latest_snapshot().await
    }
}
