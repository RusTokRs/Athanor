use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, KnowledgeStore, OperationContext, PreparedSnapshot, PreparedSnapshotPublication,
    RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use athanor_store_jsonl::JsonlKnowledgeStore;

use crate::index_publication::{publish_prepared_index, recover_interrupted_publication};
use crate::{
    AffectedFileSet, AthanorStore, IndexPipelineMetrics, IndexPipelineOutput, IndexState,
    IndexStateStore, JsonlReadModelWriter,
};

#[tokio::test]
async fn read_model_finalize_failure_recovers_committed_generation() {
    assert_finalize_failure_recovers(
        SabotageMode::ReadModelFinalize,
        "failed to remove read model backup",
    )
    .await;
}

#[tokio::test]
async fn index_state_finalize_failure_recovers_committed_generation() {
    assert_finalize_failure_recovers(
        SabotageMode::IndexStateFinalize,
        "failed to remove index state backup",
    )
    .await;
}

#[tokio::test]
async fn journal_clear_failure_recovers_committed_generation() {
    assert_finalize_failure_recovers(
        SabotageMode::JournalClear,
        "failed to clear publication journal",
    )
    .await;
}

async fn assert_finalize_failure_recovers(mode: SabotageMode, expected_error: &str) {
    let root = test_root(mode.label());
    let PublicationFixture {
        backend,
        store,
        snapshot,
        prepared,
        output,
        operation,
        state_store,
        output_dir,
    } = prepared_fixture(&root, mode).await;

    let error = publish_prepared_index(
        &root,
        &store,
        &state_store,
        &output_dir,
        &output,
        prepared,
        &operation,
    )
    .await
    .expect_err("injected finalization fault must fail publication");

    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains(expected_error)),
        "expected `{expected_error}` in error chain: {error:#}"
    );
    assert_latest_snapshot(&store, &snapshot).await;
    assert_current_generation(&root, &snapshot);
    assert!(publication_journal(&root).exists());

    backend.repair();
    recover_interrupted_publication(&root, &store)
        .await
        .expect("recover committed generation after transient finalization fault");

    assert_latest_snapshot(&store, &snapshot).await;
    assert_current_generation(&root, &snapshot);
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove finalization fault fixture");
}

struct PublicationFixture {
    backend: SabotagingStore,
    store: AthanorStore,
    snapshot: SnapshotId,
    prepared: PreparedSnapshot,
    output: IndexPipelineOutput,
    operation: OperationContext,
    state_store: IndexStateStore,
    output_dir: PathBuf,
}

async fn prepared_fixture(root: &Path, mode: SabotageMode) -> PublicationFixture {
    let backend = SabotagingStore::new(root, mode);
    let output_dir = root.join(".athanor/generated/current/jsonl");
    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    seed_previous_generation(&backend.inner, &output_dir, &state_store).await;

    let store = AthanorStore::new(backend.clone());
    let snapshot = store
        .begin_snapshot(
            RepoId(format!("repo_finalize_{}", mode.label())),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin snapshot");
    store
        .put_snapshot(snapshot.clone(), SnapshotBatch::default())
        .await
        .expect("write snapshot batch");
    let operation = OperationContext::new(format!("test.publication.{}", mode.label()));
    let prepared = store
        .prepare_publication(snapshot.clone(), &operation)
        .await
        .expect("prepare snapshot");
    let output = IndexPipelineOutput {
        snapshot: snapshot.clone(),
        files: Vec::new(),
        entities: Vec::new(),
        facts: Vec::new(),
        relations: Vec::new(),
        diagnostics: Vec::new(),
        affected_files: AffectedFileSet::default(),
        metrics: IndexPipelineMetrics::default(),
    };

    PublicationFixture {
        backend,
        store,
        snapshot,
        prepared,
        output,
        operation,
        state_store,
        output_dir,
    }
}

async fn seed_previous_generation(
    store: &JsonlKnowledgeStore,
    output_dir: &Path,
    state_store: &IndexStateStore,
) {
    let snapshot = store
        .begin_snapshot(
            RepoId("repo_finalize_previous".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin previous snapshot");
    store
        .put_snapshot(snapshot.clone(), SnapshotBatch::default())
        .await
        .expect("write previous snapshot");
    store
        .prepare_snapshot(snapshot.clone())
        .await
        .expect("prepare previous snapshot");
    store
        .commit_snapshot(snapshot.clone())
        .await
        .expect("commit previous snapshot");
    let canonical = store
        .load_snapshot(&snapshot)
        .await
        .expect("load previous snapshot")
        .expect("previous snapshot exists");

    JsonlReadModelWriter::new(output_dir)
        .write_canonical_snapshot(&canonical)
        .expect("write previous read model");
    state_store
        .save(&IndexState::from_sources(snapshot.0, &[]))
        .expect("write previous index state");
}

async fn assert_latest_snapshot(store: &AthanorStore, expected: &SnapshotId) {
    let latest = store
        .load_latest_snapshot()
        .await
        .expect("load latest snapshot")
        .expect("latest snapshot exists");
    assert_eq!(latest.snapshot.as_ref(), Some(expected));
}

fn assert_current_generation(root: &Path, expected: &SnapshotId) {
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(".athanor/generated/current/jsonl/manifest.json"))
            .expect("read current manifest"),
    )
    .expect("parse current manifest");
    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(".athanor/state/index-state.json"))
            .expect("read current index state"),
    )
    .expect("parse current index state");

    assert_eq!(manifest["snapshot"].as_str(), Some(expected.0.as_str()));
    assert_eq!(state["snapshot"].as_str(), Some(expected.0.as_str()));
}

#[derive(Debug, Clone, Copy)]
enum SabotageMode {
    ReadModelFinalize,
    IndexStateFinalize,
    JournalClear,
}

impl SabotageMode {
    fn label(self) -> &'static str {
        match self {
            Self::ReadModelFinalize => "read-model-finalize",
            Self::IndexStateFinalize => "index-state-finalize",
            Self::JournalClear => "journal-clear",
        }
    }
}

#[derive(Debug)]
enum RepairAction {
    RemoveFile(PathBuf),
    RemoveDirectory(PathBuf),
    RestoreFile { path: PathBuf, content: Vec<u8> },
}

#[derive(Clone)]
struct SabotagingStore {
    inner: JsonlKnowledgeStore,
    root: PathBuf,
    mode: SabotageMode,
    repair: Arc<Mutex<Option<RepairAction>>>,
}

impl SabotagingStore {
    fn new(root: &Path, mode: SabotageMode) -> Self {
        Self {
            inner: JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl")),
            root: root.to_path_buf(),
            mode,
            repair: Arc::new(Mutex::new(None)),
        }
    }

    fn sabotage(&self) -> CoreResult<()> {
        let repair = match self.mode {
            SabotageMode::ReadModelFinalize => {
                let backup = find_prefixed_entry(
                    &self.root.join(".athanor/generated/current"),
                    ".jsonl.backup-",
                )?;
                fs::remove_dir_all(&backup).map_err(adapter_error)?;
                fs::write(&backup, "blocked").map_err(adapter_error)?;
                RepairAction::RemoveFile(backup)
            }
            SabotageMode::IndexStateFinalize => {
                let backup = find_prefixed_entry(
                    &self.root.join(".athanor/state"),
                    ".index-state.json.backup-",
                )?;
                fs::remove_file(&backup).map_err(adapter_error)?;
                fs::create_dir(&backup).map_err(adapter_error)?;
                RepairAction::RemoveDirectory(backup)
            }
            SabotageMode::JournalClear => {
                let path = publication_journal(&self.root);
                let content = fs::read(&path).map_err(adapter_error)?;
                fs::remove_file(&path).map_err(adapter_error)?;
                fs::create_dir(&path).map_err(adapter_error)?;
                RepairAction::RestoreFile { path, content }
            }
        };

        *self
            .repair
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(repair);
        Ok(())
    }

    fn repair(&self) {
        let repair = self
            .repair
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .expect("sabotage must record a repair action");
        match repair {
            RepairAction::RemoveFile(path) => {
                fs::remove_file(path).expect("remove injected backup file");
            }
            RepairAction::RemoveDirectory(path) => {
                fs::remove_dir(path).expect("remove injected backup directory");
            }
            RepairAction::RestoreFile { path, content } => {
                fs::remove_dir(&path).expect("remove injected journal directory");
                fs::write(path, content).expect("restore publication journal");
            }
        }
    }
}

#[async_trait]
impl KnowledgeStore for SabotagingStore {
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

    async fn commit_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.inner
            .commit_snapshot_with_context(snapshot, context)
            .await?;
        self.sabotage()
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.inner.abort_snapshot(snapshot).await
    }
}

#[async_trait]
impl CanonicalSnapshotStore for SabotagingStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_snapshot(snapshot).await
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_latest_snapshot().await
    }
}

#[async_trait]
impl EntityResolver for SabotagingStore {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        self.inner.resolve_stable_key(snapshot, stable_key).await
    }
}

fn find_prefixed_entry(parent: &Path, prefix: &str) -> CoreResult<PathBuf> {
    fs::read_dir(parent)
        .map_err(adapter_error)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
        })
        .ok_or_else(|| {
            CoreError::Adapter(format!(
                "missing injected publication backup `{prefix}` under {}",
                parent.display()
            ))
        })
}

fn adapter_error(error: std::io::Error) -> CoreError {
    CoreError::Adapter(format!("publication fault injection failed: {error}"))
}

fn publication_journal(root: &Path) -> PathBuf {
    root.join(".athanor/state/index-publication.json")
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-publication-{label}-{nonce}"))
}
