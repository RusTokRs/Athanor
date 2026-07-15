use std::fs;
use std::path::{Path, PathBuf};

use athanor_core::{
    CanonicalSnapshotStore, CoreError, KnowledgeStore, OperationContext,
    PreparedSnapshotPublication, SnapshotBatch,
};
use athanor_domain::{Entity, EntityId, EntityKind, RepoId, SnapshotBase, StableKey};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde_json::{Value, json};

use crate::index_publication::{publish_index_snapshot, recover_interrupted_publication};
use crate::{
    AffectedFileSet, AthanorStore, IndexPipelineMetrics, IndexPipelineOutput, IndexStateStore,
};

#[tokio::test]
async fn exact_commit_survives_latest_pointer_failure_and_recovery() {
    let root = test_root("exact-commit-latest-failure");
    let canonical_root = root.join(".athanor/store/canonical/jsonl");
    let store = AthanorStore::new(JsonlKnowledgeStore::new(&canonical_root));
    let snapshot = store
        .begin_snapshot(
            RepoId("repo_atomic_coordinator".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin coordinator snapshot");
    let committed = entity("ent_atomic_coordinator", "coordinator.md");
    store
        .put_snapshot(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![entity("ent_partial", "partial.md")],
                ..SnapshotBatch::default()
            },
        )
        .await
        .expect("stage compatibility snapshot data");
    let operation = OperationContext::new("test.atomic-coordinator.latest-failure");
    let prepared = store
        .prepare_publication(snapshot.clone(), &operation)
        .await
        .expect("prepare compatibility cleanup handle");
    let output = IndexPipelineOutput {
        snapshot: snapshot.clone(),
        files: Vec::new(),
        entities: vec![committed.clone()],
        facts: Vec::new(),
        relations: Vec::new(),
        diagnostics: Vec::new(),
        affected_files: AffectedFileSet::default(),
        metrics: IndexPipelineMetrics::default(),
    };
    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    let output_dir = root.join(".athanor/generated/current/jsonl");

    fs::create_dir_all(canonical_root.join("latest.json"))
        .expect("block latest pointer finalization");
    let error = publish_index_snapshot(
        &root,
        &store,
        &state_store,
        &output_dir,
        &output,
        snapshot.clone(),
        &operation,
    )
    .await
    .expect_err("latest pointer failure must be reported after exact commit");
    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("committed"))
    );
    let journal_path = publication_journal(&root);
    assert!(journal_path.exists());
    let journal: Value = serde_json::from_slice(&fs::read(&journal_path).expect("read journal"))
        .expect("parse journal");
    assert_eq!(journal["schema"], "athanor.index_publication.v2");
    assert_eq!(journal["prepared"].as_str(), Some(snapshot.0.as_str()));
    assert!(journal.get("snapshot").is_none());

    let exact = store
        .load_snapshot(&snapshot)
        .await
        .expect("load exact committed snapshot")
        .expect("exact committed snapshot exists");
    assert_eq!(exact.entities, vec![committed]);
    assert!(matches!(
        store
            .abort_prepared(&prepared)
            .await
            .expect_err("exact committed snapshot must not be aborted"),
        CoreError::Conflict(_)
    ));
    assert_eq!(
        artifact_snapshot(&output_dir.join("manifest.json")),
        snapshot.0
    );
    assert_eq!(
        artifact_snapshot(&root.join(".athanor/state/index-state.json")),
        snapshot.0
    );

    fs::remove_dir_all(canonical_root.join("latest.json")).expect("remove pointer fault");
    recover_interrupted_publication(&root, &store)
        .await
        .expect("recover committed exact generation");
    assert!(!publication_journal(&root).exists());
    assert_eq!(
        artifact_snapshot(&output_dir.join("manifest.json")),
        snapshot.0
    );
    assert_eq!(
        artifact_snapshot(&root.join(".athanor/state/index-state.json")),
        snapshot.0
    );
    assert!(
        store
            .load_snapshot(&snapshot)
            .await
            .expect("reload exact")
            .is_some()
    );

    recover_interrupted_publication(&root, &store)
        .await
        .expect("repeated recovery is a no-op");
    fs::remove_dir_all(root).expect("remove atomic coordinator fixture");
}

fn entity(id: &str, path: &str) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(format!("file://{path}")),
        kind: EntityKind::File,
        name: path.to_string(),
        title: None,
        source: None,
        language: None,
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    }
}

fn artifact_snapshot(path: &Path) -> String {
    let value: Value = serde_json::from_slice(&fs::read(path).expect("read artifact"))
        .expect("parse artifact JSON");
    value["snapshot"]
        .as_str()
        .expect("artifact snapshot identity")
        .to_string()
}

fn publication_journal(root: &Path) -> PathBuf {
    root.join(".athanor/state/index-publication.json")
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-atomic-coordinator-{label}-{nonce}"))
}
