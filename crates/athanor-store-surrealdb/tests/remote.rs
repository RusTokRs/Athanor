#![cfg(feature = "remote")]

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use athanor_core::{
    CanonicalSnapshotStore, KnowledgeStore, OperationContext, SnapshotBatch,
};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, RepoId, SnapshotBase, StableKey,
};
use athanor_store_surrealdb::SurrealKnowledgeStore;
use serde_json::json;
use tokio::task::JoinSet;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_remote_connections_allocate_unique_snapshot_ids() {
    let uri = remote_uri();
    let writers = [
        Arc::new(
            SurrealKnowledgeStore::connect(&uri)
                .await
                .expect("connect first remote SurrealDB writer"),
        ),
        Arc::new(
            SurrealKnowledgeStore::connect(&uri)
                .await
                .expect("connect second remote SurrealDB writer"),
        ),
    ];
    let mut tasks = JoinSet::new();

    for index in 0..32 {
        let writer = Arc::clone(&writers[index % writers.len()]);
        tasks.spawn(async move {
            let context = deadline_after("remote-allocation", 10_000);
            writer
                .begin_snapshot_with_context(
                    RepoId(format!("repo_remote_{index}")),
                    snapshot_base(),
                    &context,
                )
                .await
                .expect("allocate remote snapshot")
        });
    }

    let mut ids = HashSet::new();
    while let Some(result) = tasks.join_next().await {
        let snapshot = result.expect("join remote allocation task");
        assert!(ids.insert(snapshot.0), "remote snapshot IDs must be unique");
    }
    assert_eq!(ids.len(), 32);
}

#[tokio::test]
async fn committed_batch_is_visible_from_an_independent_remote_connection() {
    let uri = remote_uri();
    let writer = SurrealKnowledgeStore::connect(&uri)
        .await
        .expect("connect remote writer");
    let reader = SurrealKnowledgeStore::connect(&uri)
        .await
        .expect("connect independent remote reader");
    let context = deadline_after("remote-publication", 10_000);

    let snapshot = writer
        .begin_snapshot_with_context(
            RepoId("repo_remote_publication".to_string()),
            snapshot_base(),
            &context,
        )
        .await
        .expect("begin remote snapshot");
    let entity = Entity {
        id: EntityId("ent_remote".to_string()),
        stable_key: StableKey("file://remote.md".to_string()),
        kind: EntityKind::File,
        name: "remote.md".to_string(),
        title: None,
        source: None,
        language: None,
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    };
    let fact = Fact {
        id: FactId("fact_remote".to_string()),
        kind: FactKind::Other("remote-conformance".to_string()),
        subject: entity.id.clone(),
        object: None,
        value: json!({"visible": true}),
        evidence: Vec::new(),
        ownership: Vec::new(),
        snapshot: snapshot.clone(),
        extractor: "remote-conformance".to_string(),
        confidence: 1.0,
    };

    writer
        .put_snapshot_with_context(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![entity.clone()],
                facts: vec![fact.clone()],
                ..SnapshotBatch::default()
            },
            &context,
        )
        .await
        .expect("write remote snapshot batch");
    writer
        .prepare_snapshot_with_context(snapshot.clone(), &context)
        .await
        .expect("prepare remote snapshot");
    writer
        .commit_snapshot_with_context(snapshot.clone(), &context)
        .await
        .expect("commit remote snapshot");

    let loaded = reader
        .load_snapshot(&snapshot)
        .await
        .expect("load committed snapshot through independent connection")
        .expect("committed remote snapshot exists");
    assert_eq!(loaded.entities, vec![entity]);
    assert_eq!(loaded.facts, vec![fact]);
}

fn remote_uri() -> String {
    std::env::var("ATHANOR_SURREAL_REMOTE_URI")
        .expect("ATHANOR_SURREAL_REMOTE_URI must point to the test server")
}

fn snapshot_base() -> SnapshotBase {
    SnapshotBase {
        branch: None,
        commit: None,
        parent_snapshot: None,
        working_tree: true,
    }
}

fn deadline_after(operation: &str, milliseconds: u64) -> OperationContext {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time after Unix epoch")
        .as_millis() as u64;
    OperationContext::new(operation).with_deadline_unix_ms(now + milliseconds)
}
