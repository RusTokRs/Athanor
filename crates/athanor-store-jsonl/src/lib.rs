use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, KnowledgeStore, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use fs2::FileExt;
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
    prepared: bool,
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
        let _writer_lock = self.acquire_writer_lock()?;
        self.recover_known_staging()?;
        let next_snapshot = self.allocate_snapshot_number()?;
        let mut state = self.lock_state()?;
        state.next_snapshot = state.next_snapshot.max(next_snapshot);

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

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        let mut state = self.lock_state()?;
        let snapshot = state.snapshot_mut(&snapshot)?;
        snapshot.entities.extend(batch.entities);
        snapshot.facts.extend(batch.facts);
        snapshot.relations.extend(batch.relations);
        snapshot.diagnostics.extend(batch.diagnostics);
        Ok(())
    }

    async fn query_entities(
        &self,
        snapshot: SnapshotSelector,
        query: EntityQuery,
    ) -> CoreResult<Vec<Entity>> {
        let state = self.lock_state()?;
        let snapshot = state.committed_snapshot(&snapshot)?;
        let mut results = state
            .snapshot_data(snapshot.as_ref())
            .map_or(&[][..], |snapshot| snapshot.entities.as_slice())
            .iter()
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

    async fn query_relations(
        &self,
        snapshot: SnapshotSelector,
        query: RelationQuery,
    ) -> CoreResult<Vec<Relation>> {
        let state = self.lock_state()?;
        let snapshot = state.committed_snapshot(&snapshot)?;
        let mut results = state
            .snapshot_data(snapshot.as_ref())
            .map_or(&[][..], |snapshot| snapshot.relations.as_slice())
            .iter()
            .filter(|relation| {
                query
                    .from_entity
                    .as_ref()
                    .is_none_or(|from| &relation.from == from)
            })
            .filter(|relation| query.to_entity.as_ref().is_none_or(|to| &relation.to == to))
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

    async fn query_diagnostics(
        &self,
        snapshot: SnapshotSelector,
        query: DiagnosticQuery,
    ) -> CoreResult<Vec<Diagnostic>> {
        let state = self.lock_state()?;
        let snapshot = state.committed_snapshot(&snapshot)?;
        let mut results = state
            .snapshot_data(snapshot.as_ref())
            .map_or(&[][..], |snapshot| snapshot.diagnostics.as_slice())
            .iter()
            .filter(|diagnostic| {
                query
                    .entity
                    .as_ref()
                    .is_none_or(|entity| diagnostic.entities.contains(entity))
            })
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

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let _writer_lock = self.acquire_writer_lock()?;
        let data = {
            let state = self.lock_state()?;
            let data = state.snapshot(&snapshot)?;
            if data.committed {
                return Err(CoreError::Conflict(format!(
                    "cannot prepare committed snapshot {}",
                    snapshot.0
                )));
            }
            if data.prepared {
                return Ok(());
            }
            data.clone()
        };
        write_prepared_snapshot(&self.root, &snapshot, &data)?;
        self.lock_state()?.snapshot_mut(&snapshot)?.prepared = true;
        Ok(())
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let _writer_lock = self.acquire_writer_lock()?;
        let snapshot_data = {
            let state = self.lock_state()?;
            state.snapshot(&snapshot)?.clone()
        };

        if snapshot_data.prepared {
            publish_prepared_snapshot(&self.root, &snapshot)?;
        } else {
            write_snapshot(&self.root, &snapshot, &snapshot_data)?;
        }
        write_latest(&self.root, &snapshot)?;
        self.lock_state()?.snapshot_mut(&snapshot)?.committed = true;

        Ok(())
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let _writer_lock = self.acquire_writer_lock()?;
        {
            let mut state = self.lock_state()?;
            let data = state
                .snapshots
                .get(&snapshot)
                .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))?;
            if data.committed {
                return Err(CoreError::Conflict(format!(
                    "cannot abort committed snapshot {}",
                    snapshot.0
                )));
            }
            state.snapshots.remove(&snapshot);
        }
        let snapshot_dir = self.snapshot_dir(&snapshot);
        if snapshot_dir.exists() {
            fs::remove_dir_all(&snapshot_dir).map_err(|err| {
                CoreError::Adapter(format!(
                    "failed to remove aborted snapshot {}: {err}",
                    snapshot_dir.display()
                ))
            })?;
        }
        let prepared_dir = self.prepared_snapshot_dir(&snapshot);
        if prepared_dir.exists() {
            fs::remove_dir_all(&prepared_dir).map_err(|err| {
                CoreError::Adapter(format!(
                    "failed to remove aborted prepared snapshot {}: {err}",
                    prepared_dir.display()
                ))
            })?;
        }
        Ok(())
    }
}

#[async_trait]
impl EntityResolver for JsonlKnowledgeStore {
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

    fn prepared_snapshot_dir(&self, snapshot: &SnapshotId) -> PathBuf {
        self.root
            .join("snapshots")
            .join(format!(".{}.prepared", snapshot.0))
    }

    fn acquire_writer_lock(&self) -> CoreResult<File> {
        fs::create_dir_all(&self.root)
            .map_err(|err| CoreError::Adapter(format!("failed to create store dir: {err}")))?;
        let lock_path = self.root.join(".writer.lock");
        let lock = File::create(&lock_path)
            .map_err(|err| CoreError::Adapter(format!("failed to open writer lock: {err}")))?;
        lock.lock_exclusive()
            .map_err(|err| CoreError::Adapter(format!("failed to acquire writer lock: {err}")))?;
        Ok(lock)
    }

    fn allocate_snapshot_number(&self) -> CoreResult<u64> {
        let sequence_path = self.root.join("snapshot-sequence.json");
        let persisted = read_snapshot_sequence(&sequence_path)?;
        let next = discover_next_snapshot(&self.root)
            .max(persisted)
            .saturating_add(1);
        write_snapshot_sequence(&sequence_path, next)?;
        Ok(next)
    }

    fn recover_known_staging(&self) -> CoreResult<()> {
        let snapshots = self.root.join("snapshots");
        if snapshots.exists() {
            let active_prepared = self
                .lock_state()?
                .snapshots
                .iter()
                .filter(|(_, data)| data.prepared && !data.committed)
                .map(|(snapshot, _)| format!(".{}.prepared", snapshot.0))
                .collect::<HashSet<_>>();
            for entry in fs::read_dir(&snapshots).map_err(|err| {
                CoreError::Adapter(format!("failed to inspect snapshot staging: {err}"))
            })? {
                let entry = entry.map_err(|err| {
                    CoreError::Adapter(format!("failed to inspect snapshot staging entry: {err}"))
                })?;
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                if name.starts_with(".snap_jsonl_")
                    && (name.contains(".staging-")
                        || name.contains(".prepare-staging-")
                        || (name.ends_with(".prepared") && !active_prepared.contains(name)))
                {
                    fs::remove_dir_all(entry.path()).map_err(|err| {
                        CoreError::Adapter(format!(
                            "failed to remove stale snapshot staging {}: {err}",
                            entry.path().display()
                        ))
                    })?;
                }
            }
        }

        for prefix in [".latest.json.staging-", ".snapshot-sequence.staging-"] {
            for entry in fs::read_dir(&self.root).map_err(|err| {
                CoreError::Adapter(format!("failed to inspect store staging: {err}"))
            })? {
                let entry = entry.map_err(|err| {
                    CoreError::Adapter(format!("failed to inspect store staging entry: {err}"))
                })?;
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(prefix))
                {
                    fs::remove_file(entry.path()).map_err(|err| {
                        CoreError::Adapter(format!(
                            "failed to remove stale store staging {}: {err}",
                            entry.path().display()
                        ))
                    })?;
                }
            }
        }

        Ok(())
    }
}

impl State {
    fn snapshot(&self, snapshot: &SnapshotId) -> CoreResult<&SnapshotData> {
        self.snapshots
            .get(snapshot)
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))
    }

    fn snapshot_mut(&mut self, snapshot: &SnapshotId) -> CoreResult<&mut SnapshotData> {
        self.snapshots
            .get_mut(snapshot)
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))
    }

    fn committed_snapshot(&self, selector: &SnapshotSelector) -> CoreResult<Option<SnapshotId>> {
        match selector {
            SnapshotSelector::Exact(snapshot) => {
                let data = self
                    .snapshots
                    .get(snapshot)
                    .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))?;
                if !data.committed {
                    return Err(CoreError::SnapshotNotCommitted(snapshot.0.clone()));
                }
                Ok(Some(snapshot.clone()))
            }
            SnapshotSelector::LatestCommitted => Ok(self
                .snapshots
                .iter()
                .filter(|(_, data)| data.committed)
                .map(|(snapshot, _)| snapshot.clone())
                .max_by(|left, right| left.0.cmp(&right.0))),
        }
    }

    fn snapshot_data(&self, snapshot: Option<&SnapshotId>) -> Option<&SnapshotData> {
        snapshot.and_then(|snapshot| self.snapshots.get(snapshot))
    }
}

#[derive(serde::Deserialize, Serialize)]
struct LatestSnapshot {
    snapshot: String,
}

#[derive(serde::Deserialize, Serialize)]
struct SnapshotSequence {
    next_snapshot: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PathIndexEntry {
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PathIndex {
    pub schema: String,
    pub snapshot: String,
    pub entries: HashMap<String, PathIndexEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StableKeyIndex {
    pub schema: String,
    pub snapshot: String,
    pub entries: HashMap<String, String>,
}

fn collect_entity_paths(entity: &Entity) -> Vec<String> {
    let mut paths = HashSet::new();
    for o in &entity.ownership {
        paths.insert(o.source_file.clone());
    }
    if let Some(source) = &entity.source {
        paths.insert(source.path.clone());
    }
    paths.into_iter().collect()
}

fn collect_fact_paths(fact: &Fact) -> Vec<String> {
    let mut paths = HashSet::new();
    for o in &fact.ownership {
        paths.insert(o.source_file.clone());
    }
    for ev in &fact.evidence {
        if let Some(source_file) = &ev.source_file {
            paths.insert(source_file.clone());
        }
    }
    paths.into_iter().collect()
}

fn collect_relation_paths(relation: &Relation) -> Vec<String> {
    let mut paths = HashSet::new();
    for o in &relation.ownership {
        paths.insert(o.source_file.clone());
    }
    for ev in &relation.evidence {
        if let Some(source_file) = &ev.source_file {
            paths.insert(source_file.clone());
        }
    }
    paths.into_iter().collect()
}

fn collect_diagnostic_paths(diagnostic: &Diagnostic) -> Vec<String> {
    let mut paths = HashSet::new();
    for o in &diagnostic.ownership {
        paths.insert(o.source_file.clone());
    }
    for ev in &diagnostic.evidence {
        if let Some(source_file) = &ev.source_file {
            paths.insert(source_file.clone());
        }
    }
    paths.into_iter().collect()
}

fn write_snapshot(root: &Path, snapshot: &SnapshotId, data: &SnapshotData) -> CoreResult<()> {
    let snapshot_dir = root.join("snapshots").join(&snapshot.0);
    let parent = snapshot_dir.parent().ok_or_else(|| {
        CoreError::Adapter(format!("snapshot {} has no parent directory", snapshot.0))
    })?;
    fs::create_dir_all(parent)
        .map_err(|err| CoreError::Adapter(format!("failed to create store dir: {err}")))?;

    if snapshot_dir.exists() {
        return Err(CoreError::Conflict(format!(
            "snapshot directory {} already exists",
            snapshot_dir.display()
        )));
    }

    let staging_dir = parent.join(format!(".{}.staging-{}", snapshot.0, unique_suffix()));
    fs::create_dir(&staging_dir).map_err(|err| {
        CoreError::Adapter(format!(
            "failed to create snapshot staging directory: {err}"
        ))
    })?;

    if let Err(error) = write_snapshot_contents(&staging_dir, snapshot, data) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(error);
    }

    fs::rename(&staging_dir, &snapshot_dir).map_err(|err| {
        let _ = fs::remove_dir_all(&staging_dir);
        CoreError::Adapter(format!("failed to publish snapshot atomically: {err}"))
    })
}

fn write_prepared_snapshot(
    root: &Path,
    snapshot: &SnapshotId,
    data: &SnapshotData,
) -> CoreResult<()> {
    let prepared_dir = root
        .join("snapshots")
        .join(format!(".{}.prepared", snapshot.0));
    let parent = prepared_dir.parent().ok_or_else(|| {
        CoreError::Adapter(format!(
            "prepared snapshot {} has no parent directory",
            snapshot.0
        ))
    })?;
    fs::create_dir_all(parent)
        .map_err(|err| CoreError::Adapter(format!("failed to create store dir: {err}")))?;
    if prepared_dir.exists() {
        return Err(CoreError::Conflict(format!(
            "prepared snapshot directory {} already exists",
            prepared_dir.display()
        )));
    }
    let staging_dir = parent.join(format!(
        ".{}.prepare-staging-{}",
        snapshot.0,
        unique_suffix()
    ));
    fs::create_dir(&staging_dir).map_err(|err| {
        CoreError::Adapter(format!(
            "failed to create prepared snapshot staging directory: {err}"
        ))
    })?;
    if let Err(error) = write_snapshot_contents(&staging_dir, snapshot, data) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(error);
    }
    fs::rename(&staging_dir, &prepared_dir).map_err(|err| {
        let _ = fs::remove_dir_all(&staging_dir);
        CoreError::Adapter(format!("failed to finalize prepared snapshot: {err}"))
    })
}

fn publish_prepared_snapshot(root: &Path, snapshot: &SnapshotId) -> CoreResult<()> {
    let snapshots = root.join("snapshots");
    let prepared_dir = snapshots.join(format!(".{}.prepared", snapshot.0));
    let snapshot_dir = snapshots.join(&snapshot.0);
    if !prepared_dir.exists() {
        return Err(CoreError::SnapshotNotCommitted(format!(
            "prepared snapshot {} is missing",
            snapshot.0
        )));
    }
    if snapshot_dir.exists() {
        return Err(CoreError::Conflict(format!(
            "snapshot directory {} already exists",
            snapshot_dir.display()
        )));
    }
    fs::rename(&prepared_dir, &snapshot_dir).map_err(|err| {
        CoreError::Adapter(format!(
            "failed to publish prepared snapshot atomically: {err}"
        ))
    })
}

fn write_snapshot_contents(
    snapshot_dir: &Path,
    snapshot: &SnapshotId,
    data: &SnapshotData,
) -> CoreResult<()> {
    write_jsonl(&snapshot_dir.join("entities.jsonl"), &data.entities)?;
    write_jsonl(&snapshot_dir.join("facts.jsonl"), &data.facts)?;
    write_jsonl(&snapshot_dir.join("relations.jsonl"), &data.relations)?;
    write_jsonl(&snapshot_dir.join("diagnostics.jsonl"), &data.diagnostics)?;

    // Build StableKeyIndex
    let mut stable_key_entries = HashMap::new();
    for entity in &data.entities {
        stable_key_entries.insert(entity.stable_key.0.clone(), entity.id.0.clone());
    }
    let stable_key_index = StableKeyIndex {
        schema: "athanor.stable_key_index.v1".to_string(),
        snapshot: snapshot.0.clone(),
        entries: stable_key_entries,
    };

    // Build PathIndex
    let mut path_entries = HashMap::<String, PathIndexEntry>::new();
    for entity in &data.entities {
        for path in collect_entity_paths(entity) {
            path_entries
                .entry(path)
                .or_default()
                .entities
                .push(entity.id.0.clone());
        }
    }
    for fact in &data.facts {
        for path in collect_fact_paths(fact) {
            path_entries
                .entry(path)
                .or_default()
                .facts
                .push(fact.id.0.clone());
        }
    }
    for relation in &data.relations {
        for path in collect_relation_paths(relation) {
            path_entries
                .entry(path)
                .or_default()
                .relations
                .push(relation.id.0.clone());
        }
    }
    for diagnostic in &data.diagnostics {
        for path in collect_diagnostic_paths(diagnostic) {
            path_entries
                .entry(path)
                .or_default()
                .diagnostics
                .push(diagnostic.id.0.clone());
        }
    }
    let path_index = PathIndex {
        schema: "athanor.path_index.v1".to_string(),
        snapshot: snapshot.0.clone(),
        entries: path_entries,
    };

    // Write Indexes
    fs::write(
        snapshot_dir.join("stable_key_index.json"),
        serde_json::to_string_pretty(&stable_key_index).map_err(|err| {
            CoreError::Adapter(format!("failed to serialize stable key index: {err}"))
        })?,
    )
    .map_err(|err| CoreError::Adapter(format!("failed to write stable key index: {err}")))?;

    fs::write(
        snapshot_dir.join("path_index.json"),
        serde_json::to_string_pretty(&path_index)
            .map_err(|err| CoreError::Adapter(format!("failed to serialize path index: {err}")))?,
    )
    .map_err(|err| CoreError::Adapter(format!("failed to write path index: {err}")))?;

    let manifest = json!({
        "schema": "athanor.canonical_snapshot.v1",
        "snapshot": snapshot.0,
        "entities": data.entities.len(),
        "facts": data.facts.len(),
        "relations": data.relations.len(),
        "diagnostics": data.diagnostics.len(),
    });

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
    let latest = root.join("latest.json");
    let staging = root.join(format!(".latest.json.staging-{}", unique_suffix()));
    fs::write(
        &staging,
        serde_json::to_string_pretty(&LatestSnapshot {
            snapshot: snapshot.0.clone(),
        })
        .map_err(|err| CoreError::Adapter(format!("failed to serialize latest: {err}")))?,
    )
    .map_err(|err| CoreError::Adapter(format!("failed to write latest snapshot: {err}")))?;

    replace_file(&staging, &latest)
}

fn replace_file(staging: &Path, target: &Path) -> CoreResult<()> {
    if !target.exists() {
        return fs::rename(staging, target)
            .map_err(|err| CoreError::Adapter(format!("failed to publish latest pointer: {err}")));
    }

    let backup = target.with_extension(format!("json.backup-{}", unique_suffix()));
    fs::rename(target, &backup)
        .map_err(|err| CoreError::Adapter(format!("failed to stage latest pointer: {err}")))?;
    if let Err(error) = fs::rename(staging, target) {
        let _ = fs::rename(&backup, target);
        return Err(CoreError::Adapter(format!(
            "failed to publish latest pointer: {error}"
        )));
    }
    fs::remove_file(backup).map_err(|err| {
        CoreError::Adapter(format!("failed to remove previous latest pointer: {err}"))
    })
}

fn read_snapshot_sequence(path: &Path) -> CoreResult<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let content = fs::read_to_string(path)
        .map_err(|err| CoreError::Adapter(format!("failed to read snapshot sequence: {err}")))?;
    serde_json::from_str::<SnapshotSequence>(&content)
        .map(|sequence| sequence.next_snapshot)
        .map_err(|err| CoreError::Adapter(format!("failed to parse snapshot sequence: {err}")))
}

fn write_snapshot_sequence(path: &Path, next_snapshot: u64) -> CoreResult<()> {
    let parent = path.parent().ok_or_else(|| {
        CoreError::Adapter(format!("sequence path {} has no parent", path.display()))
    })?;
    let staging = parent.join(format!(".snapshot-sequence.staging-{}", unique_suffix()));
    let serialized =
        serde_json::to_string_pretty(&SnapshotSequence { next_snapshot }).map_err(|err| {
            CoreError::Adapter(format!("failed to serialize snapshot sequence: {err}"))
        })?;
    fs::write(&staging, serialized)
        .map_err(|err| CoreError::Adapter(format!("failed to write snapshot sequence: {err}")))?;
    replace_file(&staging, path)
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn write_jsonl<T: Serialize>(path: &Path, items: &[T]) -> CoreResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Adapter(format!("failed to create JSONL dir: {err}")))?;
    }

    let file = File::create(path)
        .map_err(|err| CoreError::Adapter(format!("failed to create JSONL file: {err}")))?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, file);

    for item in items {
        serde_json::to_writer(&mut writer, item)
            .map_err(|err| CoreError::Adapter(format!("failed to write JSONL item: {err}")))?;
        writer
            .write_all(b"\n")
            .map_err(|err| CoreError::Adapter(format!("failed to write JSONL newline: {err}")))?;
    }
    writer
        .flush()
        .map_err(|err| CoreError::Adapter(format!("failed to flush JSONL file: {err}")))?;

    Ok(())
}

fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> CoreResult<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(path)
        .map_err(|err| CoreError::Adapter(format!("failed to open JSONL file: {err}")))?;
    let mut reader = BufReader::new(file);
    let mut items = Vec::new();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|err| CoreError::Adapter(format!("failed to read JSONL line: {err}")))?;

        if bytes_read == 0 {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        items.push(
            serde_json::from_str(trimmed)
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

    #[tokio::test]
    async fn prepares_snapshot_without_exposing_it_until_commit() {
        let root = std::env::temp_dir().join(format!(
            "athanor-jsonl-store-prepare-test-{}",
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

        store.prepare_snapshot(snapshot.clone()).await.unwrap();
        assert!(store.prepared_snapshot_dir(&snapshot).exists());
        assert!(!store.snapshot_dir(&snapshot).exists());
        assert!(store.load_latest_snapshot().await.unwrap().is_none());

        let later_snapshot = store
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
        assert!(
            store.prepared_snapshot_dir(&snapshot).exists(),
            "starting a later snapshot must preserve an active prepared snapshot"
        );
        store.abort_snapshot(later_snapshot).await.unwrap();

        store.commit_snapshot(snapshot.clone()).await.unwrap();
        assert!(!store.prepared_snapshot_dir(&snapshot).exists());
        assert!(store.snapshot_dir(&snapshot).exists());
        assert_eq!(
            store
                .load_latest_snapshot()
                .await
                .unwrap()
                .unwrap()
                .snapshot,
            Some(snapshot)
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn abort_removes_a_prepared_snapshot_directory() {
        let root = std::env::temp_dir().join(format!(
            "athanor-jsonl-store-prepared-abort-test-{}",
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

        store.prepare_snapshot(snapshot.clone()).await.unwrap();
        assert!(store.prepared_snapshot_dir(&snapshot).exists());
        store.abort_snapshot(snapshot.clone()).await.unwrap();

        assert!(!store.prepared_snapshot_dir(&snapshot).exists());
        assert!(!store.snapshot_dir(&snapshot).exists());
        assert!(store.load_latest_snapshot().await.unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn new_store_recovers_orphaned_prepared_snapshot_directory() {
        let root = std::env::temp_dir().join(format!(
            "athanor-jsonl-store-prepared-recovery-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let snapshot = {
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
            store.prepare_snapshot(snapshot.clone()).await.unwrap();
            assert!(store.prepared_snapshot_dir(&snapshot).exists());
            snapshot
        };

        let recovered = JsonlKnowledgeStore::new(&root);
        recovered
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
        assert!(!recovered.prepared_snapshot_dir(&snapshot).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn writes_stable_key_and_path_indexes_on_commit() {
        let root = std::env::temp_dir().join(format!(
            "athanor-jsonl-store-index-test-{}",
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

        let snapshot_dir = root.join("snapshots").join(&snapshot.0);

        let stable_key_path = snapshot_dir.join("stable_key_index.json");
        assert!(stable_key_path.exists());
        let stable_key_content = fs::read_to_string(&stable_key_path).unwrap();
        let stable_key_index: StableKeyIndex = serde_json::from_str(&stable_key_content).unwrap();
        assert_eq!(
            stable_key_index.entries.get("file://README.md").unwrap(),
            "ent_file_readme"
        );

        let path_index_path = snapshot_dir.join("path_index.json");
        assert!(path_index_path.exists());
        let path_index_content = fs::read_to_string(&path_index_path).unwrap();
        let path_index: PathIndex = serde_json::from_str(&path_index_content).unwrap();
        let entry = path_index.entries.get("README.md").unwrap();
        assert_eq!(entry.entities, vec!["ent_file_readme".to_string()]);

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn allocates_distinct_snapshot_ids_across_store_instances() {
        let root = std::env::temp_dir().join(format!(
            "athanor-jsonl-store-sequence-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = JsonlKnowledgeStore::new(&root);
        let second = JsonlKnowledgeStore::new(&root);
        let base = || SnapshotBase {
            branch: None,
            commit: None,
            parent_snapshot: None,
            working_tree: true,
        };

        let first_snapshot = first
            .begin_snapshot(RepoId("repo_test".to_string()), base())
            .await
            .unwrap();
        let second_snapshot = second
            .begin_snapshot(RepoId("repo_test".to_string()), base())
            .await
            .unwrap();

        assert_ne!(first_snapshot, second_snapshot);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn removes_only_known_staging_artifacts_before_allocating_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "athanor-jsonl-store-recovery-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let snapshots = root.join("snapshots");
        fs::create_dir_all(snapshots.join(".snap_jsonl_00000099.staging-crash")).unwrap();
        fs::write(root.join(".latest.json.staging-crash"), "stale").unwrap();
        fs::write(root.join(".snapshot-sequence.staging-crash"), "stale").unwrap();
        fs::write(root.join(".unrelated.staging-crash"), "keep").unwrap();

        JsonlKnowledgeStore::new(&root)
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

        assert!(
            !snapshots
                .join(".snap_jsonl_00000099.staging-crash")
                .exists()
        );
        assert!(!root.join(".latest.json.staging-crash").exists());
        assert!(!root.join(".snapshot-sequence.staging-crash").exists());
        assert!(root.join(".unrelated.staging-crash").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
