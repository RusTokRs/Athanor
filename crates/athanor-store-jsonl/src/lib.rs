use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery,
    KnowledgeStore, RelationQuery,
};
use athanor_domain::{
    Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct JsonlKnowledgeStore {
    root: PathBuf,
    state: Arc<Mutex<State>>,
}

#[derive(Debug, Default)]
struct State {
    next_snapshot: u64,
    snapshots: HashMap<SnapshotId, SnapshotData>,
}

#[derive(Debug, Default, Clone)]
struct SnapshotData {
    committed: bool,
    entities: Vec<Entity>,
    facts: Vec<Fact>,
    relations: Vec<Relation>,
    diagnostics: Vec<Diagnostic>,
}

impl JsonlKnowledgeStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let next_snapshot = discover_next_snapshot(&root);

        Self {
            root,
            state: Arc::new(Mutex::new(State {
                next_snapshot,
                snapshots: HashMap::new(),
            })),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[async_trait]
impl KnowledgeStore for JsonlKnowledgeStore {
    async fn begin_snapshot(&self, _repo: RepoId, _base: SnapshotBase) -> CoreResult<SnapshotId> {
        let mut state = self.lock_state()?;
        state.next_snapshot += 1;

        let snapshot = SnapshotId(format!("snap_jsonl_{:08}", state.next_snapshot));
        state
            .snapshots
            .insert(snapshot.clone(), SnapshotData::default());

        Ok(snapshot)
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        let mut state = self.lock_state()?;
        state.snapshot_mut(&snapshot)?.entities.extend(entities);
        Ok(())
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        let mut state = self.lock_state()?;
        state.snapshot_mut(&snapshot)?.facts.extend(facts);
        Ok(())
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        let mut state = self.lock_state()?;
        state.snapshot_mut(&snapshot)?.relations.extend(relations);
        Ok(())
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        let mut state = self.lock_state()?;
        state
            .snapshot_mut(&snapshot)?
            .diagnostics
            .extend(diagnostics);
        Ok(())
    }

    async fn query_entities(&self, query: EntityQuery) -> CoreResult<Vec<Entity>> {
        let state = self.lock_state()?;
        let mut results = state
            .snapshots
            .values()
            .flat_map(|snapshot| snapshot.entities.iter())
            .filter(|entity| matches_stable_key(&query.stable_key, &entity.stable_key))
            .filter(|entity| {
                query
                    .kind
                    .as_ref()
                    .is_none_or(|kind| entity_kind_name(entity) == *kind)
            })
            .filter(|entity| {
                query.text.as_ref().is_none_or(|text| {
                    entity.name.contains(text)
                        || entity
                            .title
                            .as_ref()
                            .is_some_and(|title| title.contains(text))
                        || entity.aliases.iter().any(|alias| alias.contains(text))
                })
            })
            .cloned()
            .collect::<Vec<_>>();

        apply_limit(&mut results, query.limit);
        Ok(results)
    }

    async fn query_relations(&self, query: RelationQuery) -> CoreResult<Vec<Relation>> {
        let state = self.lock_state()?;
        let mut results = state
            .snapshots
            .values()
            .flat_map(|snapshot| snapshot.relations.iter())
            .filter(|relation| {
                query
                    .kind
                    .as_ref()
                    .is_none_or(|kind| relation_kind_name(relation) == *kind)
            })
            .cloned()
            .collect::<Vec<_>>();

        apply_limit(&mut results, query.limit);
        Ok(results)
    }

    async fn query_diagnostics(&self, query: DiagnosticQuery) -> CoreResult<Vec<Diagnostic>> {
        let state = self.lock_state()?;
        let mut results = state
            .snapshots
            .values()
            .flat_map(|snapshot| snapshot.diagnostics.iter())
            .filter(|diagnostic| {
                query.severity.as_ref().is_none_or(|severity| {
                    format!("{:?}", diagnostic.severity).eq_ignore_ascii_case(severity)
                })
            })
            .filter(|diagnostic| {
                query.status.as_ref().is_none_or(|status| {
                    format!("{:?}", diagnostic.status).eq_ignore_ascii_case(status)
                })
            })
            .cloned()
            .collect::<Vec<_>>();

        apply_limit(&mut results, query.limit);
        Ok(results)
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let snapshot_data = {
            let mut state = self.lock_state()?;
            let snapshot_data = state.snapshot_mut(&snapshot)?;
            snapshot_data.committed = true;
            snapshot_data.clone()
        };

        write_snapshot(&self.root, &snapshot, &snapshot_data)?;
        write_latest(&self.root, &snapshot)?;

        Ok(())
    }
}

#[async_trait]
impl CanonicalSnapshotStore for JsonlKnowledgeStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        let snapshot_dir = self.snapshot_dir(snapshot);

        if !snapshot_dir.exists() {
            return Ok(None);
        }

        Ok(Some(CanonicalSnapshot {
            snapshot: Some(snapshot.clone()),
            entities: read_jsonl(&snapshot_dir.join("entities.jsonl"))?,
            facts: read_jsonl(&snapshot_dir.join("facts.jsonl"))?,
            relations: read_jsonl(&snapshot_dir.join("relations.jsonl"))?,
            diagnostics: read_jsonl(&snapshot_dir.join("diagnostics.jsonl"))?,
        }))
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        let latest_path = self.root.join("latest.json");

        if !latest_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&latest_path)
            .map_err(|err| CoreError::Adapter(format!("failed to read latest snapshot: {err}")))?;
        let latest: LatestSnapshot = serde_json::from_str(&content)
            .map_err(|err| CoreError::Adapter(format!("failed to parse latest snapshot: {err}")))?;

        self.load_snapshot(&SnapshotId(latest.snapshot)).await
    }
}

impl JsonlKnowledgeStore {
    fn lock_state(&self) -> CoreResult<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| CoreError::Adapter("jsonl store lock poisoned".to_string()))
    }

    fn snapshot_dir(&self, snapshot: &SnapshotId) -> PathBuf {
        self.root.join("snapshots").join(&snapshot.0)
    }
}

impl State {
    fn snapshot_mut(&mut self, snapshot: &SnapshotId) -> CoreResult<&mut SnapshotData> {
        self.snapshots
            .get_mut(snapshot)
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))
    }
}

#[derive(serde::Deserialize, Serialize)]
struct LatestSnapshot {
    snapshot: String,
}

fn write_snapshot(root: &Path, snapshot: &SnapshotId, data: &SnapshotData) -> CoreResult<()> {
    let snapshot_dir = root.join("snapshots").join(&snapshot.0);

    write_jsonl(&snapshot_dir.join("entities.jsonl"), &data.entities)?;
    write_jsonl(&snapshot_dir.join("facts.jsonl"), &data.facts)?;
    write_jsonl(&snapshot_dir.join("relations.jsonl"), &data.relations)?;
    write_jsonl(&snapshot_dir.join("diagnostics.jsonl"), &data.diagnostics)?;

    let manifest = json!({
        "schema": "athanor.canonical_snapshot.v1",
        "snapshot": snapshot.0,
        "entities": data.entities.len(),
        "facts": data.facts.len(),
        "relations": data.relations.len(),
        "diagnostics": data.diagnostics.len(),
    });

    if let Some(parent) = snapshot_dir.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Adapter(format!("failed to create store dir: {err}")))?;
    }

    fs::create_dir_all(&snapshot_dir)
        .map_err(|err| CoreError::Adapter(format!("failed to create snapshot dir: {err}")))?;
    fs::write(
        snapshot_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)
            .map_err(|err| CoreError::Adapter(format!("failed to serialize manifest: {err}")))?,
    )
    .map_err(|err| CoreError::Adapter(format!("failed to write manifest: {err}")))?;

    Ok(())
}

fn write_latest(root: &Path, snapshot: &SnapshotId) -> CoreResult<()> {
    fs::create_dir_all(root)
        .map_err(|err| CoreError::Adapter(format!("failed to create store dir: {err}")))?;
    fs::write(
        root.join("latest.json"),
        serde_json::to_string_pretty(&LatestSnapshot {
            snapshot: snapshot.0.clone(),
        })
        .map_err(|err| CoreError::Adapter(format!("failed to serialize latest: {err}")))?,
    )
    .map_err(|err| CoreError::Adapter(format!("failed to write latest snapshot: {err}")))
}

fn write_jsonl<T: Serialize>(path: &Path, items: &[T]) -> CoreResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Adapter(format!("failed to create JSONL dir: {err}")))?;
    }

    let mut file = File::create(path)
        .map_err(|err| CoreError::Adapter(format!("failed to create JSONL file: {err}")))?;

    for item in items {
        serde_json::to_writer(&mut file, item)
            .map_err(|err| CoreError::Adapter(format!("failed to write JSONL item: {err}")))?;
        file.write_all(b"\n")
            .map_err(|err| CoreError::Adapter(format!("failed to write JSONL newline: {err}")))?;
    }

    Ok(())
}

fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> CoreResult<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(path)
        .map_err(|err| CoreError::Adapter(format!("failed to open JSONL file: {err}")))?;
    let reader = BufReader::new(file);
    let mut items = Vec::new();

    for line in reader.lines() {
        let line =
            line.map_err(|err| CoreError::Adapter(format!("failed to read JSONL line: {err}")))?;

        if line.trim().is_empty() {
            continue;
        }

        items.push(
            serde_json::from_str(&line)
                .map_err(|err| CoreError::Adapter(format!("failed to parse JSONL item: {err}")))?,
        );
    }

    Ok(items)
}

fn discover_next_snapshot(root: &Path) -> u64 {
    let snapshots_dir = root.join("snapshots");
    let Ok(entries) = fs::read_dir(snapshots_dir) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter_map(|name| name.strip_prefix("snap_jsonl_")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
}

fn matches_stable_key(expected: &Option<StableKey>, actual: &StableKey) -> bool {
    expected.as_ref().is_none_or(|expected| expected == actual)
}

fn apply_limit<T>(items: &mut Vec<T>, limit: Option<usize>) {
    if let Some(limit) = limit {
        items.truncate(limit);
    }
}

fn entity_kind_name(entity: &Entity) -> String {
    format!("{:?}", entity.kind).to_ascii_lowercase()
}

fn relation_kind_name(relation: &Relation) -> String {
    format!("{:?}", relation.kind).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use athanor_core::KnowledgeStore;
    use athanor_domain::{EntityId, EntityKind, SourceLocation};
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn persists_and_loads_latest_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "athanor-jsonl-store-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = JsonlKnowledgeStore::new(&root);
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

        let reloaded = JsonlKnowledgeStore::new(&root)
            .load_latest_snapshot()
            .await
            .unwrap()
            .unwrap();

        assert_eq!(reloaded.snapshot, Some(snapshot));
        assert_eq!(reloaded.entities, vec![entity]);

        fs::remove_dir_all(root).unwrap();
    }
}
