use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery,
    KnowledgeStore, RelationQuery,
};
use athanor_domain::{Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;

#[derive(Debug, Clone)]
pub struct SurrealKnowledgeStore {
    db: Surreal<Any>,
}

use serde::de;
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize)]
struct SnapshotRecord {
    #[serde(deserialize_with = "deserialize_id")]
    id: String,
    repo: String,
    base_branch: Option<String>,
    base_commit: Option<String>,
    base_parent_snapshot: Option<String>,
    base_working_tree: bool,
    committed: bool,
}

fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: de::Deserializer<'de>,
{
    let thing = Thing::deserialize(deserializer)?;
    match thing.id {
        surrealdb::sql::Id::String(s) => Ok(s),
        other => Ok(other.to_string()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CounterRecord {
    count: u64,
}

#[derive(Debug, Deserialize)]
struct DummyRecord {}

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
        let counter: Option<CounterRecord> = self.db.select(counter_id).await.map_err(|err| {
            CoreError::Adapter(format!("failed to fetch snapshot counter: {err}"))
        })?;

        let count = match counter {
            Some(mut c) => {
                c.count += 1;
                let next_count = c.count;
                let _: Option<CounterRecord> =
                    self.db.update(counter_id).content(c).await.map_err(|err| {
                        CoreError::Adapter(format!("failed to update snapshot counter: {err}"))
                    })?;
                next_count
            }
            None => {
                let c = CounterRecord { count: 1 };
                let _: Option<CounterRecord> =
                    self.db.create(counter_id).content(c).await.map_err(|err| {
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
            let mut val = serde_json::to_value(&entity)
                .map_err(|err| CoreError::Adapter(format!("failed to serialize entity: {err}")))?;
            if let serde_json::Value::Object(map) = &mut val {
                map.insert("original_id".to_string(), json!(entity.id.0));
                map.remove("id");
                map.insert("snapshot".to_string(), json!(snapshot.0));
            }

            let _: Option<DummyRecord> = self
                .db
                .upsert(("entity", &record_id))
                .content(val)
                .await
                .map_err(|err| CoreError::Adapter(format!("failed to insert entity: {err}")))?;
        }
        Ok(())
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        for fact in facts {
            let record_id = format!("{}_{}", snapshot.0, fact.id.0);
            let mut val = serde_json::to_value(&fact)
                .map_err(|err| CoreError::Adapter(format!("failed to serialize fact: {err}")))?;
            if let serde_json::Value::Object(map) = &mut val {
                map.insert("original_id".to_string(), json!(fact.id.0));
                map.remove("id");
            }

            let _: Option<DummyRecord> = self
                .db
                .upsert(("fact", &record_id))
                .content(val)
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
            let mut val = serde_json::to_value(&relation).map_err(|err| {
                CoreError::Adapter(format!("failed to serialize relation: {err}"))
            })?;
            if let serde_json::Value::Object(map) = &mut val {
                map.insert("original_id".to_string(), json!(relation.id.0));
                map.remove("id");
            }

            let _: Option<DummyRecord> = self
                .db
                .upsert(("relation", &record_id))
                .content(val)
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
            let mut val = serde_json::to_value(&diagnostic).map_err(|err| {
                CoreError::Adapter(format!("failed to serialize diagnostic: {err}"))
            })?;
            if let serde_json::Value::Object(map) = &mut val {
                map.insert("original_id".to_string(), json!(diagnostic.id.0));
                map.remove("id");
            }

            let _: Option<DummyRecord> = self
                .db
                .upsert(("diagnostic", &record_id))
                .content(val)
                .await
                .map_err(|err| CoreError::Adapter(format!("failed to insert diagnostic: {err}")))?;
        }
        Ok(())
    }

    async fn query_entities(&self, query: EntityQuery) -> CoreResult<Vec<Entity>> {
        let mut sql = "SELECT * FROM entity WHERE 1=1".to_string();
        let mut params = serde_json::Map::new();

        if let Some(stable_key) = &query.stable_key {
            sql.push_str(" AND stable_key = $stable_key");
            params.insert("stable_key".to_string(), json!(stable_key.0));
        }

        if let Some(kind) = &query.kind {
            sql.push_str(" AND kind = $kind");
            params.insert("kind".to_string(), json!(kind));
        }

        if let Some(text) = &query.text {
            sql.push_str(
                " AND (name CONTAINS $text OR title CONTAINS $text OR aliases CONTAINS $text)",
            );
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

        let db_val: surrealdb::Value = response
            .take(0)
            .map_err(|err| CoreError::Adapter(format!("failed to parse query entities: {err}")))?;

        deserialize_list(db_val)
    }

    async fn query_relations(&self, query: RelationQuery) -> CoreResult<Vec<Relation>> {
        let mut sql = "SELECT * FROM relation WHERE 1=1".to_string();
        let mut params = serde_json::Map::new();

        if let Some(kind) = &query.kind {
            sql.push_str(" AND kind = $kind");
            params.insert("kind".to_string(), json!(kind));
        }

        if let Some(from) = &query.from {
            sql.push_str(" AND from = $from");
            params.insert("from".to_string(), json!(from.0));
        }

        if let Some(to) = &query.to {
            sql.push_str(" AND to = $to");
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

        let db_val: surrealdb::Value = response
            .take(0)
            .map_err(|err| CoreError::Adapter(format!("failed to parse query relations: {err}")))?;

        deserialize_list(db_val)
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

        let db_val: surrealdb::Value = response.take(0).map_err(|err| {
            CoreError::Adapter(format!("failed to parse query diagnostics: {err}"))
        })?;

        deserialize_list(db_val)
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let sql = format!("UPDATE ONLY snapshot:{} SET committed = true", snapshot.0);
        let _: Option<DummyRecord> = self
            .db
            .query(sql)
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to commit snapshot: {err}")))?
            .take(0)
            .map_err(|err| {
                CoreError::Adapter(format!("failed to parse committed snapshot: {err}"))
            })?;

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
            .query("SELECT * FROM entity WHERE snapshot = $snapshot")
            .bind(("snapshot", snapshot.0.clone()))
            .await
            .map_err(|err| {
                CoreError::Adapter(format!("failed to load snapshot entities: {err}"))
            })?;
        let entities_db_val: surrealdb::Value = entities_res.take(0).map_err(|err| {
            CoreError::Adapter(format!("failed to parse snapshot entities: {err}"))
        })?;
        let entities = deserialize_list(entities_db_val)?;

        let mut facts_res = self
            .db
            .query("SELECT * FROM fact WHERE snapshot = $snapshot")
            .bind(("snapshot", snapshot.0.clone()))
            .await
            .map_err(|err| CoreError::Adapter(format!("failed to load snapshot facts: {err}")))?;
        let facts_db_val: surrealdb::Value = facts_res
            .take(0)
            .map_err(|err| CoreError::Adapter(format!("failed to parse snapshot facts: {err}")))?;
        let facts = deserialize_list(facts_db_val)?;

        let mut relations_res = self
            .db
            .query("SELECT * FROM relation WHERE snapshot = $snapshot")
            .bind(("snapshot", snapshot.0.clone()))
            .await
            .map_err(|err| {
                CoreError::Adapter(format!("failed to load snapshot relations: {err}"))
            })?;
        let relations_db_val: surrealdb::Value = relations_res.take(0).map_err(|err| {
            CoreError::Adapter(format!("failed to parse snapshot relations: {err}"))
        })?;
        let relations = deserialize_list(relations_db_val)?;

        let mut diagnostics_res = self
            .db
            .query("SELECT * FROM diagnostic WHERE snapshot = $snapshot")
            .bind(("snapshot", snapshot.0.clone()))
            .await
            .map_err(|err| {
                CoreError::Adapter(format!("failed to load snapshot diagnostics: {err}"))
            })?;
        let diagnostics_db_val: surrealdb::Value = diagnostics_res.take(0).map_err(|err| {
            CoreError::Adapter(format!("failed to parse snapshot diagnostics: {err}"))
        })?;
        let diagnostics = deserialize_list(diagnostics_db_val)?;

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

        let snapshots: Vec<SnapshotRecord> = response.take(0).map_err(|err| {
            CoreError::Adapter(format!("failed to parse latest snapshot query: {err}"))
        })?;

        if let Some(latest) = snapshots.first() {
            self.load_snapshot(&SnapshotId(latest.id.clone())).await
        } else {
            Ok(None)
        }
    }
}

#[allow(clippy::collapsible_if)]
fn restore_original_id(val: &mut serde_json::Value) {
    if let serde_json::Value::Object(map) = val {
        if let Some(orig_id) = map.remove("original_id") {
            map.insert("id".to_string(), orig_id);
        }
    }
}

fn ast_to_flat_json(val: serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::Object(mut map) => {
            if map.len() == 1 {
                let key = map.keys().next().unwrap().clone();
                match key.as_str() {
                    "None" | "Null" => serde_json::Value::Null,
                    "Bool" | "Strand" | "Uuid" | "Datetime" | "Duration" | "Number" | "Int"
                    | "Float" | "Decimal" | "String" => {
                        let inner = map.remove(&key).unwrap();
                        ast_to_flat_json(inner)
                    }
                    "Array" => {
                        let inner = map.remove("Array").unwrap();
                        match inner {
                            serde_json::Value::Array(arr) => serde_json::Value::Array(
                                arr.into_iter().map(ast_to_flat_json).collect(),
                            ),
                            other => ast_to_flat_json(other),
                        }
                    }
                    "Object" => {
                        let inner = map.remove("Object").unwrap();
                        match inner {
                            serde_json::Value::Object(inner_map) => serde_json::Value::Object(
                                inner_map
                                    .into_iter()
                                    .map(|(k, v)| (k, ast_to_flat_json(v)))
                                    .collect(),
                            ),
                            other => ast_to_flat_json(other),
                        }
                    }
                    "Thing" => {
                        if let Some(serde_json::Value::Object(mut thing_map)) = map.remove("Thing")
                        {
                            if let Some(id_val) = thing_map.remove("id") {
                                ast_to_flat_json(id_val)
                            } else {
                                serde_json::Value::Object(
                                    thing_map
                                        .into_iter()
                                        .map(|(k, v)| (k, ast_to_flat_json(v)))
                                        .collect(),
                                )
                            }
                        } else {
                            serde_json::Value::Object(map)
                        }
                    }
                    _ => serde_json::Value::Object(
                        map.into_iter()
                            .map(|(k, v)| (k, ast_to_flat_json(v)))
                            .collect(),
                    ),
                }
            } else {
                serde_json::Value::Object(
                    map.into_iter()
                        .map(|(k, v)| (k, ast_to_flat_json(v)))
                        .collect(),
                )
            }
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(ast_to_flat_json).collect())
        }
        other => other,
    }
}

fn deserialize_list<T>(val: surrealdb::Value) -> Result<Vec<T>, CoreError>
where
    T: for<'de> Deserialize<'de>,
{
    let json_val = serde_json::to_value(&val).map_err(|err| {
        CoreError::Adapter(format!("failed to convert surrealdb value to json: {err}"))
    })?;

    let flat_val = ast_to_flat_json(json_val);

    if let serde_json::Value::Array(arr) = flat_val {
        let mut results = Vec::with_capacity(arr.len());
        for mut item in arr {
            restore_original_id(&mut item);
            let object: T = serde_json::from_value(item).map_err(|err| {
                CoreError::Adapter(format!("failed to deserialize object: {err}"))
            })?;
            results.push(object);
        }
        Ok(results)
    } else {
        Err(CoreError::Adapter(format!(
            "expected array result, got JSON: {:?}",
            val
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use athanor_core::KnowledgeStore;
    use athanor_domain::{EntityId, EntityKind, SourceLocation, StableKey};

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
