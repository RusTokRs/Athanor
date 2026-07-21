use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, KnowledgeStore, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, GenerationId, Relation, RepoId, SnapshotBase, SnapshotId,
    StableKey,
};
use serde::de;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::sql::Thing;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

/// SurrealDB-backed canonical store.
///
/// A process-local gate keeps cloned handles from interleaving legacy single-record writes. Native
/// SurrealQL transactions protect counter allocation and complete `SnapshotBatch` writes at the
/// backend boundary. Independent processes still require a persistent-backend conflict/lease test
/// before Athanor can claim full multi-process writer safety.
#[derive(Debug, Clone)]
pub struct SurrealKnowledgeStore {
    db: Surreal<Any>,
    write_gate: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotRecord {
    #[serde(deserialize_with = "deserialize_id")]
    id: String,
    repo: String,
    base_branch: Option<String>,
    base_commit: Option<String>,
    base_parent_snapshot: Option<String>,
    base_working_tree: bool,
    #[serde(default)]
    sequence: u64,
    #[serde(default)]
    prepared: bool,
    committed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    generation: Option<GenerationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    allocation_operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    allocation_created_at_unix_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CounterRecord {
    count: u64,
}

#[derive(Debug, Deserialize)]
struct DummyRecord {}

fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: de::Deserializer<'de>,
{
    let thing = Thing::deserialize(deserializer)?;
    match thing.id {
        surrealdb::sql::Id::String(value) => Ok(value),
        other => Ok(other.to_string()),
    }
}

fn snapshot_allocation_nonce() -> String {
    static NEXT_NONCE: AtomicU64 = AtomicU64::new(0);
    let counter = NEXT_NONCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{timestamp:032x}{:08x}{counter:016x}", std::process::id())
}

impl SurrealKnowledgeStore {
    pub async fn connect(uri: &str) -> Result<Self, CoreError> {
        let db = surrealdb::engine::any::connect(uri)
            .await
            .map_err(|error| {
                CoreError::Adapter(format!("failed to connect to surrealdb: {error}"))
            })?;

        db.use_ns("athanor")
            .use_db("knowledge")
            .await
            .map_err(|error| CoreError::Adapter(format!("failed to use NS/DB: {error}")))?;

        initialize_snapshot_counter(&db).await?;

        Ok(Self {
            db,
            write_gate: Arc::new(Mutex::new(())),
        })
    }

    async fn allocate_snapshot_sequence(&self) -> CoreResult<u64> {
        const MAX_ATTEMPTS: usize = 8;
        for attempt in 0..MAX_ATTEMPTS {
            let response = self
                .db
                .query("UPDATE ONLY meta:snapshot_counter SET count += 1 RETURN AFTER;")
                .await
                .map_err(|error| {
                    CoreError::Adapter(format!("failed to increment snapshot counter: {error}"))
                })?;
            let mut response = match response.check() {
                Ok(response) => response,
                Err(error)
                    if attempt + 1 < MAX_ATTEMPTS
                        && is_retryable_counter_conflict(&error.to_string()) =>
                {
                    sleep(Duration::from_millis(2_u64.saturating_pow(attempt as u32))).await;
                    continue;
                }
                Err(error) => {
                    return Err(CoreError::Adapter(format!(
                        "snapshot counter transaction failed: {error}"
                    )));
                }
            };
            let counter: Option<CounterRecord> = response.take(0).map_err(|error| {
                CoreError::Adapter(format!("failed to parse snapshot counter: {error}"))
            })?;
            return counter.map(|counter| counter.count).ok_or_else(|| {
                CoreError::Adapter("snapshot counter returned no record".to_string())
            });
        }
        unreachable!("snapshot counter retry loop always returns");
    }

    async fn load_snapshot_record(&self, snapshot: &SnapshotId) -> CoreResult<SnapshotRecord> {
        self.db
            .select(("snapshot", &snapshot.0))
            .await
            .map_err(|error| CoreError::Adapter(format!("load snapshot failed: {error}")))?
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))
    }

    async fn ensure_writable_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<()> {
        let record = self.load_snapshot_record(snapshot).await?;
        if record.committed {
            return Err(CoreError::Conflict(format!(
                "cannot write committed snapshot {}",
                snapshot.0
            )));
        }
        if record.prepared {
            return Err(CoreError::Conflict(format!(
                "cannot write prepared snapshot {}",
                snapshot.0
            )));
        }
        Ok(())
    }

    async fn upsert_records<T>(
        &self,
        table: &'static str,
        snapshot: &SnapshotId,
        records: Vec<(String, T)>,
    ) -> CoreResult<()>
    where
        T: Serialize,
    {
        for (original_id, record) in records {
            let record_id = format!("{}_{}", snapshot.0, original_id);
            let content = storage_content(&record, snapshot, &original_id)?;
            let _: Option<DummyRecord> = self
                .db
                .upsert((table, record_id))
                .content(content)
                .await
                .map_err(|error| {
                    CoreError::Adapter(format!("failed to upsert {table} record: {error}"))
                })?;
        }
        Ok(())
    }

    async fn resolve_committed_snapshot(
        &self,
        selector: SnapshotSelector,
    ) -> CoreResult<SnapshotId> {
        let snapshot = match selector {
            SnapshotSelector::Exact(snapshot) => snapshot,
            SnapshotSelector::LatestCommitted => {
                let mut response = self
                    .db
                    .query(
                        "SELECT * FROM snapshot WHERE committed = true \
                         ORDER BY sequence DESC, id DESC LIMIT 1",
                    )
                    .await
                    .map_err(|error| {
                        CoreError::Adapter(format!("query latest snapshot failed: {error}"))
                    })?;
                let records: Vec<SnapshotRecord> = response.take(0).map_err(|error| {
                    CoreError::Adapter(format!("parse latest snapshot failed: {error}"))
                })?;
                let record = records
                    .into_iter()
                    .next()
                    .ok_or_else(|| CoreError::NotFound("no committed snapshot".to_string()))?;
                let snapshot = SnapshotId(record.id.clone());
                validate_committed_generation(&record, &snapshot)?;
                return Ok(snapshot);
            }
        };

        let record = self.load_snapshot_record(&snapshot).await?;
        if !record.committed {
            return Err(CoreError::SnapshotNotCommitted(snapshot.0));
        }
        validate_committed_generation(&record, &snapshot)?;
        Ok(snapshot)
    }

    #[cfg(test)]
    fn with_independent_write_gate(&self) -> Self {
        Self {
            db: self.db.clone(),
            write_gate: Arc::new(Mutex::new(())),
        }
    }
}

fn is_retryable_counter_conflict(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("read or write conflict") || message.contains("can be retried")
}

fn is_snapshot_record_collision(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("database record") && message.contains("already exists")
}

fn validate_committed_generation(
    record: &SnapshotRecord,
    snapshot: &SnapshotId,
) -> CoreResult<()> {
    let Some(generation) = &record.generation else {
        // Migration window for committed records written before generation identity existed.
        return Ok(());
    };
    let expected = GenerationId::for_snapshot(snapshot);
    if generation != &expected {
        return Err(CoreError::AdapterProtocol(format!(
            "committed snapshot {} identifies generation `{generation}`, expected `{expected}`",
            snapshot.0
        )));
    }
    Ok(())
}

async fn initialize_snapshot_counter(db: &Surreal<Any>) -> CoreResult<()> {
    db.query(
        "INSERT IGNORE INTO meta [{ id: 'snapshot_counter', count: 0 }] RETURN NONE;",
    )
    .await
    .map_err(|error| CoreError::Adapter(format!("failed to initialize snapshot counter: {error}")))?
    .check()
    .map_err(|error| {
        CoreError::Adapter(format!("snapshot counter initialization failed: {error}"))
    })?;
    Ok(())
}

fn storage_content<T>(
    record: &T,
    snapshot: &SnapshotId,
    original_id: &str,
) -> CoreResult<Value>
where
    T: Serialize,
{
    let mut value = serde_json::to_value(record)
        .map_err(|error| CoreError::Adapter(format!("failed to serialize record: {error}")))?;
    let Value::Object(map) = &mut value else {
        return Err(CoreError::Adapter(
            "serialized store record must be an object".to_string(),
        ));
    };
    map.insert("original_id".to_string(), json!(original_id));
    map.remove("id");
    map.insert("snapshot".to_string(), json!(snapshot.0));
    Ok(value)
}

fn insert_record<T>(
    snapshot: &SnapshotId,
    original_id: &str,
    record: &T,
) -> CoreResult<Value>
where
    T: Serialize,
{
    let mut value = storage_content(record, snapshot, original_id)?;
    let Value::Object(map) = &mut value else {
        unreachable!("storage_content always returns an object");
    };
    map.insert(
        "id".to_string(),
        json!(format!("{}_{}", snapshot.0, original_id)),
    );
    Ok(value)
}

#[async_trait]
impl KnowledgeStore for SurrealKnowledgeStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        let _guard = self.write_gate.lock().await;
        let repo = repo.0;
        let base_branch = base.branch;
        let base_commit = base.commit;
        let base_parent_snapshot = base.parent_snapshot.map(|snapshot| snapshot.0);
        let base_working_tree = base.working_tree;

        const MAX_ATTEMPTS: usize = 32;
        for attempt in 0..MAX_ATTEMPTS {
            let sequence = self.allocate_snapshot_sequence().await?;
            let snapshot_id = format!(
    "snap_surreal_{sequence:08}_{}",
    snapshot_allocation_nonce()
);
            let record = SnapshotRecord {
                id: snapshot_id.clone(),
                repo: repo.clone(),
                base_branch: base_branch.clone(),
                base_commit: base_commit.clone(),
                base_parent_snapshot: base_parent_snapshot.clone(),
                base_working_tree,
                sequence,
                prepared: false,
                committed: false,
                generation: None,
                allocation_operation_id: None,
                allocation_created_at_unix_ms: None,
            };

            let response = self
                .db
                .query(
                    "CREATE ONLY type::thing('snapshot', $snapshot_id) "
                        .to_owned()
                        + "CONTENT $record RETURN AFTER;",
                )
                .bind(("snapshot_id", snapshot_id.clone()))
                .bind(("record", record))
                .await
                .map_err(|error| {
                    CoreError::Adapter(format!(
                        "failed to execute snapshot record allocation: {error}"
                    ))
                })?;
            let mut response = match response.check() {
                Ok(response) => response,
                Err(error)
                    if attempt + 1 < MAX_ATTEMPTS
                        && (is_retryable_counter_conflict(&error.to_string())
                            || is_snapshot_record_collision(&error.to_string())) =>
                {
                    sleep(Duration::from_millis(1_u64 << attempt.min(6))).await;
                    continue;
                }
                Err(error) => {
                    return Err(CoreError::Adapter(format!(
                        "failed to create snapshot record: {error}"
                    )));
                }
            };
            let created: Option<SnapshotRecord> = response.take(0).map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to parse created snapshot record: {error}"
                ))
            })?;
            if created.is_some() {
                return Ok(SnapshotId(snapshot_id));
            }
            if attempt + 1 < MAX_ATTEMPTS {
                sleep(Duration::from_millis(1_u64 << attempt.min(6))).await;
                continue;
            }
            return Err(CoreError::Conflict(format!(
                "snapshot record {snapshot_id} was not created after {MAX_ATTEMPTS} attempts"
            )));
        }
        unreachable!("snapshot allocation retry loop always returns");
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.ensure_writable_snapshot(&snapshot).await?;
        let records = entities
            .into_iter()
            .map(|entity| (entity.id.0.clone(), entity))
            .collect();
        self.upsert_records("entity", &snapshot, records).await
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.ensure_writable_snapshot(&snapshot).await?;
        let records = facts
            .into_iter()
            .map(|fact| (fact.id.0.clone(), fact))
            .collect();
        self.upsert_records("fact", &snapshot, records).await
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.ensure_writable_snapshot(&snapshot).await?;
        let records = relations
            .into_iter()
            .map(|relation| (relation.id.0.clone(), relation))
            .collect();
        self.upsert_records("relation", &snapshot, records).await
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.ensure_writable_snapshot(&snapshot).await?;
        let records = diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.id.0.clone(), diagnostic))
            .collect();
        self.upsert_records("diagnostic", &snapshot, records)
            .await
    }

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        self.ensure_writable_snapshot(&snapshot).await?;

        let entities = batch
            .entities
            .iter()
            .map(|entity| insert_record(&snapshot, &entity.id.0, entity))
            .collect::<CoreResult<Vec<_>>>()?;
        let facts = batch
            .facts
            .iter()
            .map(|fact| insert_record(&snapshot, &fact.id.0, fact))
            .collect::<CoreResult<Vec<_>>>()?;
        let relations = batch
            .relations
            .iter()
            .map(|relation| insert_record(&snapshot, &relation.id.0, relation))
            .collect::<CoreResult<Vec<_>>>()?;
        let diagnostics = batch
            .diagnostics
            .iter()
            .map(|diagnostic| insert_record(&snapshot, &diagnostic.id.0, diagnostic))
            .collect::<CoreResult<Vec<_>>>()?;

        let mut sql = String::from("BEGIN;\n");
        if !entities.is_empty() {
            sql.push_str("INSERT INTO entity $entities RETURN NONE;\n");
        }
        if !facts.is_empty() {
            sql.push_str("INSERT INTO fact $facts RETURN NONE;\n");
        }
        if !relations.is_empty() {
            sql.push_str("INSERT INTO relation $relations RETURN NONE;\n");
        }
        if !diagnostics.is_empty() {
            sql.push_str("INSERT INTO diagnostic $diagnostics RETURN NONE;\n");
        }
        sql.push_str("COMMIT;");

        self.db
            .query(sql)
            .bind(("entities", entities))
            .bind(("facts", facts))
            .bind(("relations", relations))
            .bind(("diagnostics", diagnostics))
            .await
            .map_err(|error| {
                CoreError::Adapter(format!("failed to execute snapshot batch transaction: {error}"))
            })?
            .check()
            .map_err(|error| {
                CoreError::Adapter(format!("snapshot batch transaction rolled back: {error}"))
            })?;

        Ok(())
    }

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        let mut record = self.load_snapshot_record(&snapshot).await?;
        if record.committed {
            return Err(CoreError::Conflict(format!(
                "cannot prepare committed snapshot {}",
                snapshot.0
            )));
        }
        if record.prepared {
            return Ok(());
        }
        record.prepared = true;
        let _: Option<SnapshotRecord> = self
            .db
            .update(("snapshot", &snapshot.0))
            .content(record)
            .await
            .map_err(|error| {
                CoreError::Adapter(format!("failed to prepare snapshot {}: {error}", snapshot.0))
            })?;
        Ok(())
    }

    async fn query_entities(
        &self,
        snapshot: SnapshotSelector,
        query: EntityQuery,
    ) -> CoreResult<Vec<Entity>> {
        let snapshot = self.resolve_committed_snapshot(snapshot).await?;
        let mut sql = "SELECT * FROM entity WHERE snapshot = $snapshot".to_string();
        let mut params = serde_json::Map::new();
        params.insert("snapshot".to_string(), json!(snapshot.0));

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
            .map_err(|error| CoreError::Adapter(format!("query entities failed: {error}")))?;
        let value: surrealdb::Value = response.take(0).map_err(|error| {
            CoreError::Adapter(format!("failed to parse query entities: {error}"))
        })?;
        deserialize_list(value)
    }

    async fn query_relations(
        &self,
        snapshot: SnapshotSelector,
        query: RelationQuery,
    ) -> CoreResult<Vec<Relation>> {
        let snapshot = self.resolve_committed_snapshot(snapshot).await?;
        let mut sql = "SELECT * FROM relation WHERE snapshot = $snapshot".to_string();
        let mut params = serde_json::Map::new();
        params.insert("snapshot".to_string(), json!(snapshot.0));

        if let Some(kind) = &query.kind {
            sql.push_str(" AND kind = $kind");
            params.insert("kind".to_string(), json!(kind));
        }
        if let Some(from) = &query.from_entity {
            sql.push_str(" AND from = $from");
            params.insert("from".to_string(), json!(from.0));
        }
        if let Some(to) = &query.to_entity {
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
            .map_err(|error| CoreError::Adapter(format!("query relations failed: {error}")))?;
        let value: surrealdb::Value = response.take(0).map_err(|error| {
            CoreError::Adapter(format!("failed to parse query relations: {error}"))
        })?;
        deserialize_list(value)
    }

    async fn query_diagnostics(
        &self,
        snapshot: SnapshotSelector,
        query: DiagnosticQuery,
    ) -> CoreResult<Vec<Diagnostic>> {
        let snapshot = self.resolve_committed_snapshot(snapshot).await?;
        let mut sql = "SELECT * FROM diagnostic WHERE snapshot = $snapshot".to_string();
        let mut params = serde_json::Map::new();
        params.insert("snapshot".to_string(), json!(snapshot.0));

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
            .map_err(|error| CoreError::Adapter(format!("query diagnostics failed: {error}")))?;
        let value: surrealdb::Value = response.take(0).map_err(|error| {
            CoreError::Adapter(format!("failed to parse query diagnostics: {error}"))
        })?;
        deserialize_list(value)
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        let mut record = self.load_snapshot_record(&snapshot).await?;
        if record.committed {
            return validate_committed_generation(&record, &snapshot);
        }
        record.prepared = true;
        record.committed = true;
        record.generation = Some(GenerationId::for_snapshot(&snapshot));
        let _: Option<SnapshotRecord> = self
            .db
            .update(("snapshot", &snapshot.0))
            .content(record)
            .await
            .map_err(|error| {
                CoreError::Adapter(format!("failed to commit snapshot {}: {error}", snapshot.0))
            })?;
        Ok(())
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        let record = self.load_snapshot_record(&snapshot).await?;
        if record.committed {
            return Err(CoreError::Conflict(format!(
                "cannot abort committed snapshot {}",
                snapshot.0
            )));
        }

        self.db
            .query(
                "BEGIN; \
                 DELETE entity WHERE snapshot = $snapshot; \
                 DELETE fact WHERE snapshot = $snapshot; \
                 DELETE relation WHERE snapshot = $snapshot; \
                 DELETE diagnostic WHERE snapshot = $snapshot; \
                 COMMIT;",
            )
            .bind(("snapshot", snapshot.0.clone()))
            .await
            .map_err(|error| {
                CoreError::Adapter(format!("failed to delete aborted snapshot data: {error}"))
            })?
            .check()
            .map_err(|error| {
                CoreError::Adapter(format!("aborted snapshot cleanup rolled back: {error}"))
            })?;

        let _: Option<DummyRecord> = self
            .db
            .delete(("snapshot", &snapshot.0))
            .await
            .map_err(|error| {
                CoreError::Adapter(format!("failed to delete aborted snapshot: {error}"))
            })?;
        Ok(())
    }
}

#[async_trait]
impl EntityResolver for SurrealKnowledgeStore {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        let entities = self
            .query_entities(
                snapshot,
                EntityQuery {
                    stable_key: Some(stable_key.clone()),
                    limit: Some(2),
                    ..EntityQuery::default()
                },
            )
            .await?;
        if entities.len() > 1 {
            return Err(CoreError::Conflict(format!(
                "stable key {} resolves to multiple entities",
                stable_key.0
            )));
        }
        Ok(entities.into_iter().next().map(|entity| entity.id))
    }
}

#[async_trait]
impl CanonicalSnapshotStore for SurrealKnowledgeStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        let snapshot_record: Option<SnapshotRecord> = self
            .db
            .select(("snapshot", &snapshot.0))
            .await
            .map_err(|error| {
                CoreError::Adapter(format!("failed to load snapshot record: {error}"))
            })?;
        let Some(snapshot_record) = snapshot_record else {
            return Ok(None);
        };
        if !snapshot_record.committed {
            return Err(CoreError::SnapshotNotCommitted(snapshot.0.clone()));
        }
        validate_committed_generation(&snapshot_record, snapshot)?;

        let entities = load_records(&self.db, "entity", snapshot).await?;
        let facts = load_records(&self.db, "fact", snapshot).await?;
        let relations = load_records(&self.db, "relation", snapshot).await?;
        let diagnostics = load_records(&self.db, "diagnostic", snapshot).await?;

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
            .query(
                "SELECT * FROM snapshot WHERE committed = true \
                 ORDER BY sequence DESC, id DESC LIMIT 1",
            )
            .await
            .map_err(|error| {
                CoreError::Adapter(format!("failed to query latest committed snapshot: {error}"))
            })?;
        let snapshots: Vec<SnapshotRecord> = response.take(0).map_err(|error| {
            CoreError::Adapter(format!("failed to parse latest snapshot query: {error}"))
        })?;

        if let Some(latest) = snapshots.first() {
            self.load_snapshot(&SnapshotId(latest.id.clone())).await
        } else {
            Ok(None)
        }
    }
}

async fn load_records<T>(
    db: &Surreal<Any>,
    table: &str,
    snapshot: &SnapshotId,
) -> CoreResult<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let sql = format!("SELECT * FROM {table} WHERE snapshot = $snapshot");
    let mut response = db
        .query(sql)
        .bind(("snapshot", snapshot.0.clone()))
        .await
        .map_err(|error| {
            CoreError::Adapter(format!("failed to load snapshot {table} records: {error}"))
        })?;
    let value: surrealdb::Value = response.take(0).map_err(|error| {
        CoreError::Adapter(format!("failed to parse snapshot {table} records: {error}"))
    })?;
    deserialize_list(value)
}

#[allow(clippy::collapsible_if)]
fn restore_original_id(value: &mut Value) {
    if let Value::Object(map) = value {
        if let Some(original_id) = map.remove("original_id") {
            map.insert("id".to_string(), original_id);
        }
    }
}

fn ast_to_flat_json(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            if map.len() == 1 {
                let key = map.keys().next().expect("single map key").clone();
                match key.as_str() {
                    "None" | "Null" => Value::Null,
                    "Bool" | "Strand" | "Uuid" | "Datetime" | "Duration" | "Number" | "Int"
                    | "Float" | "Decimal" | "String" => {
                        let inner = map.remove(&key).expect("single map value");
                        ast_to_flat_json(inner)
                    }
                    "Array" => match map.remove("Array").expect("array value") {
                        Value::Array(values) => {
                            Value::Array(values.into_iter().map(ast_to_flat_json).collect())
                        }
                        other => ast_to_flat_json(other),
                    },
                    "Object" => match map.remove("Object").expect("object value") {
                        Value::Object(inner) => Value::Object(
                            inner
                                .into_iter()
                                .map(|(key, value)| (key, ast_to_flat_json(value)))
                                .collect(),
                        ),
                        other => ast_to_flat_json(other),
                    },
                    "Thing" => {
                        if let Some(Value::Object(mut thing)) = map.remove("Thing") {
                            if let Some(id) = thing.remove("id") {
                                ast_to_flat_json(id)
                            } else {
                                Value::Object(
                                    thing
                                        .into_iter()
                                        .map(|(key, value)| (key, ast_to_flat_json(value)))
                                        .collect(),
                                )
                            }
                        } else {
                            Value::Object(map)
                        }
                    }
                    _ => Value::Object(
                        map.into_iter()
                            .map(|(key, value)| (key, ast_to_flat_json(value)))
                            .collect(),
                    ),
                }
            } else {
                Value::Object(
                    map.into_iter()
                        .map(|(key, value)| (key, ast_to_flat_json(value)))
                        .collect(),
                )
            }
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(ast_to_flat_json).collect())
        }
        other => other,
    }
}

fn deserialize_list<T>(value: surrealdb::Value) -> CoreResult<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let json_value = serde_json::to_value(&value).map_err(|error| {
        CoreError::Adapter(format!("failed to convert surrealdb value to json: {error}"))
    })?;
    let flat_value = ast_to_flat_json(json_value);

    let Value::Array(values) = flat_value else {
        return Err(CoreError::Adapter(format!(
            "expected array result, got JSON: {value:?}"
        )));
    };

    values
        .into_iter()
        .map(|mut value| {
            restore_original_id(&mut value);
            serde_json::from_value(value).map_err(|error| {
                CoreError::Adapter(format!("failed to deserialize object: {error}"))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use athanor_core::{CanonicalSnapshotStore, EntityQuery};
    use athanor_domain::{EntityId, EntityKind, SourceLocation};
    use tokio::task::JoinSet;

    use super::*;

    fn base() -> SnapshotBase {
        SnapshotBase {
            branch: None,
            commit: None,
            parent_snapshot: None,
            working_tree: true,
        }
    }

    fn entity(id: &str, stable_key: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind: EntityKind::File,
            name: id.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: id.to_string(),
                line_start: None,
                line_end: None,
            }),
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }

    #[tokio::test]
    async fn persists_and_loads_latest_snapshot() {
        let store = SurrealKnowledgeStore::connect("mem://").await.unwrap();
        let snapshot = store
            .begin_snapshot(RepoId("repo_test".to_string()), base())
            .await
            .unwrap();
        let entity = entity("ent_file_readme", "file://README.md");

        store
            .put_entities(snapshot.clone(), vec![entity.clone()])
            .await
            .unwrap();
        store.commit_snapshot(snapshot.clone()).await.unwrap();

        let record = store.load_snapshot_record(&snapshot).await.unwrap();
        assert_eq!(
            record.generation,
            Some(GenerationId::for_snapshot(&snapshot))
        );
        let reloaded = store.load_latest_snapshot().await.unwrap().unwrap();
        assert_eq!(reloaded.snapshot, Some(snapshot));
        assert_eq!(reloaded.entities, vec![entity]);
    }

    #[tokio::test]
    async fn committed_generation_mismatch_fails_exact_and_latest_reads() {
        let store = SurrealKnowledgeStore::connect("mem://").await.unwrap();
        let snapshot = store
            .begin_snapshot(RepoId("repo_generation_mismatch".to_string()), base())
            .await
            .unwrap();
        store.commit_snapshot(snapshot.clone()).await.unwrap();

        let mut record = store.load_snapshot_record(&snapshot).await.unwrap();
        record.generation = Some(GenerationId("gen_foreign".to_string()));
        let _: Option<SnapshotRecord> = store
            .db
            .update(("snapshot", &snapshot.0))
            .content(record)
            .await
            .unwrap();

        let exact = store
            .load_snapshot(&snapshot)
            .await
            .expect_err("mismatched exact generation must fail closed");
        assert!(matches!(exact, CoreError::AdapterProtocol(_)));
        let latest = store
            .query_entities(SnapshotSelector::LatestCommitted, EntityQuery::default())
            .await
            .expect_err("mismatched latest generation must fail closed");
        assert!(matches!(latest, CoreError::AdapterProtocol(_)));
    }

    #[tokio::test]
    async fn legacy_committed_record_without_generation_remains_readable() {
        let store = SurrealKnowledgeStore::connect("mem://").await.unwrap();
        let snapshot = store
            .begin_snapshot(RepoId("repo_legacy_generation".to_string()), base())
            .await
            .unwrap();
        let mut record = store.load_snapshot_record(&snapshot).await.unwrap();
        record.prepared = true;
        record.committed = true;
        record.generation = None;
        let _: Option<SnapshotRecord> = store
            .db
            .update(("snapshot", &snapshot.0))
            .content(record)
            .await
            .unwrap();

        assert!(store.load_snapshot(&snapshot).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn native_batch_transaction_rolls_back_on_duplicate_record() {
        let store = SurrealKnowledgeStore::connect("mem://").await.unwrap();
        let snapshot = store
            .begin_snapshot(RepoId("repo_rollback".to_string()), base())
            .await
            .unwrap();
        let first = entity("ent_duplicate", "file://first.md");
        let second = entity("ent_duplicate", "file://second.md");

        store
            .put_snapshot(
                snapshot.clone(),
                SnapshotBatch {
                    entities: vec![first.clone(), second],
                    ..SnapshotBatch::default()
                },
            )
            .await
            .expect_err("duplicate record must roll back the transaction");

        store
            .put_snapshot(
                snapshot.clone(),
                SnapshotBatch {
                    entities: vec![first.clone()],
                    ..SnapshotBatch::default()
                },
            )
            .await
            .expect("a clean retry must succeed after rollback");
        store.commit_snapshot(snapshot.clone()).await.unwrap();

        let entities = store
            .query_entities(
                SnapshotSelector::Exact(snapshot),
                EntityQuery {
                    stable_key: Some(first.stable_key.clone()),
                    ..EntityQuery::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(entities, vec![first]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn atomic_counter_survives_independent_writer_gates() {
        let store = SurrealKnowledgeStore::connect("mem://").await.unwrap();
        let writers = [
            store.with_independent_write_gate(),
            store.with_independent_write_gate(),
        ];
        let mut tasks = JoinSet::new();

        for index in 0..32 {
            let writer = writers[index % writers.len()].clone();
            tasks.spawn(async move {
                writer
                    .begin_snapshot(RepoId(format!("repo_{index}")), base())
                    .await
                    .unwrap()
            });
        }

        let mut ids = HashSet::new();
        while let Some(result) = tasks.join_next().await {
            let snapshot = result.unwrap();
            assert!(ids.insert(snapshot.0), "snapshot ids must be unique");
        }
        assert_eq!(ids.len(), 32);
    }

    #[tokio::test]
    async fn prepared_snapshot_rejects_late_writes() {
        let store = SurrealKnowledgeStore::connect("mem://").await.unwrap();
        let snapshot = store
            .begin_snapshot(RepoId("repo_prepared".to_string()), base())
            .await
            .unwrap();
        store.prepare_snapshot(snapshot.clone()).await.unwrap();

        let error = store
            .put_entities(snapshot, vec![entity("ent_late", "file://late.md")])
            .await
            .expect_err("prepared snapshots must be immutable");
        assert!(matches!(error, CoreError::Conflict(_)));
    }
}
