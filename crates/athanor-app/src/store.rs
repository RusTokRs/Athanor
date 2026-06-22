use crate::config::{ProjectConfig, StorageMode};
use anyhow::Result;
use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreResult, DiagnosticQuery, EntityQuery,
    KnowledgeStore, RelationQuery,
};
use athanor_domain::{Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId};
use std::path::Path;

#[derive(Clone)]
pub enum AthanorStore {
    Jsonl(athanor_store_jsonl::JsonlKnowledgeStore),
    #[cfg(feature = "store-surreal")]
    Surreal(athanor_store_surrealdb::SurrealKnowledgeStore),
}

#[async_trait]
impl KnowledgeStore for AthanorStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        match self {
            Self::Jsonl(store) => store.begin_snapshot(repo, base).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.begin_snapshot(repo, base).await,
        }
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        match self {
            Self::Jsonl(store) => store.put_entities(snapshot, entities).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.put_entities(snapshot, entities).await,
        }
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        match self {
            Self::Jsonl(store) => store.put_facts(snapshot, facts).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.put_facts(snapshot, facts).await,
        }
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        match self {
            Self::Jsonl(store) => store.put_relations(snapshot, relations).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.put_relations(snapshot, relations).await,
        }
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        match self {
            Self::Jsonl(store) => store.put_diagnostics(snapshot, diagnostics).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.put_diagnostics(snapshot, diagnostics).await,
        }
    }

    async fn query_entities(&self, query: EntityQuery) -> CoreResult<Vec<Entity>> {
        match self {
            Self::Jsonl(store) => store.query_entities(query).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.query_entities(query).await,
        }
    }

    async fn query_relations(&self, query: RelationQuery) -> CoreResult<Vec<Relation>> {
        match self {
            Self::Jsonl(store) => store.query_relations(query).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.query_relations(query).await,
        }
    }

    async fn query_diagnostics(&self, query: DiagnosticQuery) -> CoreResult<Vec<Diagnostic>> {
        match self {
            Self::Jsonl(store) => store.query_diagnostics(query).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.query_diagnostics(query).await,
        }
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        match self {
            Self::Jsonl(store) => store.commit_snapshot(snapshot).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.commit_snapshot(snapshot).await,
        }
    }
}

#[async_trait]
impl CanonicalSnapshotStore for AthanorStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        match self {
            Self::Jsonl(store) => store.load_snapshot(snapshot).await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.load_snapshot(snapshot).await,
        }
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        match self {
            Self::Jsonl(store) => store.load_latest_snapshot().await,
            #[cfg(feature = "store-surreal")]
            Self::Surreal(store) => store.load_latest_snapshot().await,
        }
    }
}

pub async fn init_store(root: &Path, config: &ProjectConfig) -> Result<AthanorStore> {
    match config.storage.mode {
        StorageMode::Jsonl => {
            let path = root.join(&config.storage.path);
            Ok(AthanorStore::Jsonl(
                athanor_store_jsonl::JsonlKnowledgeStore::new(path),
            ))
        }
        StorageMode::SurrealEmbedded => {
            #[cfg(feature = "store-surreal")]
            {
                let path = root.join(&config.storage.path);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let uri = format!("surrealkv://{}", path.to_string_lossy());
                let store = athanor_store_surrealdb::SurrealKnowledgeStore::connect(&uri)
                    .await
                    .map_err(|e| anyhow::anyhow!("failed to connect to SurrealDB: {}", e))?;
                Ok(AthanorStore::Surreal(store))
            }
            #[cfg(not(feature = "store-surreal"))]
            {
                anyhow::bail!("SurrealDB support is not compiled in this build of Athanor")
            }
        }
        StorageMode::SurrealMemory => {
            #[cfg(feature = "store-surreal")]
            {
                let store = athanor_store_surrealdb::SurrealKnowledgeStore::connect("mem://")
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!("failed to connect to SurrealDB in-memory: {}", e)
                    })?;
                Ok(AthanorStore::Surreal(store))
            }
            #[cfg(not(feature = "store-surreal"))]
            {
                anyhow::bail!("SurrealDB support is not compiled in this build of Athanor")
            }
        }
    }
}
