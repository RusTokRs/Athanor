use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery,
    KnowledgeStore, RelationQuery,
};
use athanor_domain::{
    Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[derive(Debug, Clone)]
pub struct SurrealKnowledgeStore {
    db: Surreal<Any>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SnapshotRecord {
    id: String,
    repo: String,
    base_branch: Option<String>,
    base_commit: Option<String>,
    base_parent_snapshot: Option<String>,
    base_working_tree: bool,
    committed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CounterRecord {
    count: u64,
}

impl SurrealKnowledgeStore {
    pub async fn connect(uri: &str) -> Result<Self, CoreError> {
        let db = surrealdb::engine::any::connect(uri)
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to connect to surrealdb: {err}")))?;

        db.use_ns("athanor")
            .use_db("knowledge")
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to use NS/DB: {err}")))?;

        Ok(Self { db })
    }
}

#[async_trait]
impl KnowledgeStore for SurrealKnowledgeStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        let counter_id = ("meta", "snapshot_counter");
        let counter: Option<CounterRecord> = self
            .db
            .select(counter_id)
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to fetch snapshot counter: {err}")))?;

        let count = match counter {
            Some(mut c) => {
                c.count += 1;
                let _: Option<CounterRecord> = self
                    .db
                    .update(counter_id)
                    .content(c)
                    .await
                    .map_err(|err| {
                        CoreError::Adapter(format!("failed to update snapshot counter: {err}"))
                    })?;
                c.count
            }
            None => {
                let c = CounterRecord { count: 1 };
                let _: Option<CounterRecord> = self
                    .db
                    .create(counter_id)
                    .content(c)
                    .await
                    .map_err(|err| {
                        CoreError::Adapter(format!("failed to create snapshot counter: {err}"))
                    })?;
                1
            }
        };

        let snapshot_id = format!("snap_surreal_{:08}", count);
        let record = SnapshotRecord {
            id: snapshot_id.clone(),
            repo: repo.0,
            base_branch: base.branch,
            base_commit: base.commit,
            base_parent_snapshot: base.parent_snapshot.map(|s| s.0),
            base_working_tree: base.working_tree,
            committed: false,
        };

        let _: Option<SnapshotRecord> = self
            .db
            .create(("snapshot", &snapshot_id))
            .content(record)
            .await
            .map_err(|err| {
                CoreError::Adapter(format!("failed to create snapshot record: {err}"))
            })?;

        Ok(SnapshotId(snapshot_id))
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        for entity in entities {
            let record_id = format!("{}_{}", snapshot.0, entity.id.0);
            let _: Option<serde_json::Value> = self
                .db
                .create(("entity", &record_id))
                .content(entity)
                .await
                .map_err(|err| CoreError::Adapter(format!("failed to insert entity: {err}")))?;
        }
        Ok(())
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        for fact in facts {
            let record_id = format!("{}_{}", snapshot.0, fact.id.0);
            let _: Option<serde_json::Value> = self
                .db
                .create(("fact", &record_id))
                .content(fact)
                .await
                .map_err(|err| CoreError::Adapter(format!("failed to insert fact: {err}")))?;
        }
        Ok(())
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        for relation in relations {
            let record_id = format!("{}_{}", snapshot.0, relation.id.0);
            let _: Option<serde_json::Value> = self
                .db
                .create(("relation", &record_id))
                .content(relation)
                .await
                .map_err(|err| CoreError::Adapter(format!("failed to insert relation: {err}")))?;
        }
        Ok(())
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        for diagnostic in diagnostics {
            let record_id = format!("{}_{}", snapshot.0, diagnostic.id.0);
            let _: Option<serde_json::Value> = self
                .db
                .create(("diagnostic", &record_id))
                .content(diagnostic)
                .await
                .map_err(|err| {
                    CoreError::Adapter(format!("failed to insert diagnostic: {err}"))
                })?;
        }
        Ok(())
    }

    async fn query_entities(&self, query: EntityQuery) -> CoreResult<Vec<Entity>> {
        let mut sql = "SELECT * FROM entity WHERE 1=1".to_string();
        let mut params = serde_json::Map::new();

        if let Some(stable_key) = &query.stable_key {
            sql.push_str(" AND stable_key.0 = $stable_key");
            params.insert("stable_key".to_string(), json!(stable_key.0));
        }

        if let Some(kind) = &query.kind {
            sql.push_str(" AND kind = $kind");
            params.insert("kind".to_string(), json!(kind));
        }

        if let Some(text) = &query.text {
            sql.push_str(" AND (name CONTAINS $text OR title CONTAINS $text OR aliases CONTAINS $text)");
            params.insert("text".to_string(), json!(text));
        }

        if let Some(limit) = query.limit {
            sql.push_str(" LIMIT $limit");
            params.insert("limit".to_string(), json!(limit));
        }

        let mut response = self
            .db
            .query(sql)
            .bind(params)
            .await
            .map_err(|err| CoreError::Adapter(format!("query entities failed: {err}")))?;

        let entities: Vec<Entity> = response
            .take(0)
            .map_err(|err| CoreError::Adapter(format!("failed to parse query entities: {err}")))?;

        Ok(entities)
    }

    async fn query_relations(&self, query: RelationQuery) -> CoreResult<Vec<Relation>> {
        let mut sql = "SELECT * FROM relation WHERE 1=1".to_string();
        let mut params = serde_json::Map::new();

        if let Some(kind) = &query.kind {
            sql.push_str(" AND kind = $kind");
            params.insert("kind".to_string(), json!(kind));
        }

        if let Some(from) = &query.from {
            sql.push_str(" AND from.0 = $from");
            params.insert("from".to_string(), json!(from.0));
        }

        if let Some(to) = &query.to {
            sql.push_str(" AND to.0 = $to");
            params.insert("to".to_string(), json!(to.0));
        }

        if let Some(limit) = query.limit {
            sql.push_str(" LIMIT $limit");
            params.insert("limit".to_string(), json!(limit));
        }

        let mut response = self
            .db
            .query(sql)
            .bind(params)
            .await
            .map_err(|err| CoreError::Adapter(format!("query relations failed: {err}")))?;

        let relations: Vec<Relation> = response
            .take(0)
            .map_err(|err| CoreError::Adapter(format!("failed to parse query relations: {err}")))?;

        Ok(relations)
    }

    async fn query_diagnostics(&self, query: DiagnosticQuery) -> CoreResult<Vec<Diagnostic>> {
        let mut sql = "SELECT * FROM diagnostic WHERE 1=1".to_string();
        let mut params = serde_json::Map::new();

        if let Some(severity) = &query.severity {
            sql.push_str(" AND severity = $severity");
            params.insert("severity".to_string(), json!(severity));
        }

        if let Some(status) = &query.status {
            sql.push_str(" AND status = $status");
            params.insert("status".to_string(), json!(status));
        }

        if let Some(entity) = &query.entity {
            sql.push_str(" AND $entity IN entities");
            params.insert("entity".to_string(), json!(entity.0));
        }

        if let Some(limit) = query.limit {
            sql.push_str(" LIMIT $limit");
            params.insert("limit".to_string(), json!(limit));
        }

        let mut response = self
            .db
            .query(sql)
            .bind(params)
            .await
            .map_err(|err| CoreError::Adapter(format!("query diagnostics failed: {err}")))?;

        let diagnostics: Vec<Diagnostic> = response
            .take(0)
            .map_err(|err| {
                CoreError::Adapter(format!("failed to parse query diagnostics: {err}"))
            })?;

        Ok(diagnostics)
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let sql = "UPDATE ONLY snapshot SET committed = true WHERE id = $snapshot";
        let _: Option<serde_json::Value> = self
            .db
            .query(sql)
            .bind(("snapshot", snapshot.0))
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to commit snapshot: {err}")))?
            .take(0)
            .map_err(|err| CoreError::Adapter(format!("failed to parse committed snapshot: {err}")))?;

        Ok(())
    }
}

#[async_trait]
impl CanonicalSnapshotStore for SurrealKnowledgeStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        let snapshot_rec: Option<SnapshotRecord> = self
            .db
            .select(("snapshot", &snapshot.0))
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to load snapshot record: {err}")))?;

        if snapshot_rec.is_none() {
            return Ok(None);
        }

        let mut entities_res = self
            .db
            .query("SELECT * FROM entity WHERE snapshot.0 = $snapshot")
            .bind(("snapshot", snapshot.0.clone()))
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to load snapshot entities: {err}")))?;
        let entities: Vec<Entity> = entities_res
            .take(0)
            .map_err(|err| CoreError::Adapter(format!("failed to parse snapshot entities: {err}")))?;

        let mut facts_res = self
            .db
            .query("SELECT * FROM fact WHERE snapshot.0 = $snapshot")
            .bind(("snapshot", snapshot.0.clone()))
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to load snapshot facts: {err}")))?;
        let facts: Vec<Fact> = facts_res
            .take(0)
            .map_err(|err| CoreError::Adapter(format!("failed to parse snapshot facts: {err}")))?;

        let mut relations_res = self
            .db
            .query("SELECT * FROM relation WHERE snapshot.0 = $snapshot")
            .bind(("snapshot", snapshot.0.clone()))
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to load snapshot relations: {err}")))?;
        let relations: Vec<Relation> = relations_res
            .take(0)
            .map_err(|err| CoreError::Adapter(format!("failed to parse snapshot relations: {err}")))?;

        let mut diagnostics_res = self
            .db
            .query("SELECT * FROM diagnostic WHERE snapshot.0 = $snapshot")
            .bind(("snapshot", snapshot.0.clone()))
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to load snapshot diagnostics: {err}")))?;
        let diagnostics: Vec<Diagnostic> = diagnostics_res
            .take(0)
            .map_err(|err| {
                CoreError::Adapter(format!("failed to parse snapshot diagnostics: {err}"))
            })?;

        Ok(Some(CanonicalSnapshot {
            snapshot: Some(snapshot.clone()),
            entities,
            facts,
            relations,
            diagnostics,
        }))
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        let mut response = self
            .db
            .query("SELECT * FROM snapshot WHERE committed = true ORDER BY id DESC LIMIT 1")
            .await
            .map_err(|err| {
                CoreError::Adapter(format!("failed to query latest committed snapshot: {err}"))
            })?;

        let snapshots: Vec<SnapshotRecord> = response
            .take(0)
            .map_err(|err| {
                CoreError::Adapter(format!("failed to parse latest snapshot query: {err}"))
            })?;

        if let Some(latest) = snapshots.first() {
            self.load_snapshot(&SnapshotId(latest.id.clone())).await
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use athanor_core::KnowledgeStore;
    use athanor_domain::{EntityId, EntityKind, SourceLocation};

    async fn temp_store() -> SurrealKnowledgeStore {
        SurrealKnowledgeStore::connect("mem://").await.unwrap()
    }

    #[tokio::test]
    async fn persists_and_loads_latest_snapshot() {
        let store = temp_store().await;
        let snapshot = store
            .begin_snapshot(
                RepoId("repo_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();

        let entity = Entity {
            id: EntityId("ent_file_readme".to_string()),
            stable_key: StableKey("file://README.md".to_string()),
            kind: EntityKind::File,
            name: "README.md".to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "README.md".to_string(),
                line_start: None,
                line_end: None,
            }),
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        };

        store
            .put_entities(snapshot.clone(), vec![entity.clone()])
            .await
            .unwrap();
        store.commit_snapshot(snapshot.clone()).await.unwrap();

        let reloaded = store.load_latest_snapshot().await.unwrap().unwrap();

        assert_eq!(reloaded.snapshot, Some(snapshot));
        assert_eq!(reloaded.entities, vec![entity]);
    }
}
