use athanor_core::{
    EntityQuery, KnowledgeStore, OperationContext, PreparedSnapshotPublication, SnapshotBatch,
    SnapshotSelector,
};
use athanor_domain::{Entity, EntityId, EntityKind, RepoId, SnapshotBase, StableKey};
use athanor_store_memory::MemoryKnowledgeStore;
use serde_json::json;

#[tokio::test]
async fn typed_prepare_publish_and_abort_preserve_latest_committed() {
    let store = MemoryKnowledgeStore::new();
    let repo = RepoId("repo_memory_prepared_publication".to_string());

    let published = store
        .begin_snapshot(repo.clone(), working_tree_base())
        .await
        .expect("begin published snapshot");
    let published_entity = entity("ent_memory_published", "file://published.rs");
    let publish_context = OperationContext::new("test.memory.publish");
    store
        .put_snapshot_with_context(
            published.clone(),
            SnapshotBatch {
                entities: vec![published_entity.clone()],
                ..SnapshotBatch::default()
            },
            &publish_context,
        )
        .await
        .expect("write published snapshot");

    let prepared = store
        .prepare_publication(published.clone(), &publish_context)
        .await
        .expect("prepare published snapshot");
    assert_eq!(prepared.snapshot(), &published);
    assert!(
        store
            .query_entities(
                SnapshotSelector::LatestCommitted,
                EntityQuery {
                    stable_key: Some(published_entity.stable_key.clone()),
                    ..EntityQuery::default()
                },
            )
            .await
            .expect("query before publish")
            .is_empty(),
        "prepared snapshot must remain invisible"
    );

    store
        .publish_prepared(&prepared, &publish_context)
        .await
        .expect("publish prepared snapshot");
    assert_eq!(
        store
            .query_entities(
                SnapshotSelector::LatestCommitted,
                EntityQuery {
                    stable_key: Some(published_entity.stable_key.clone()),
                    ..EntityQuery::default()
                },
            )
            .await
            .expect("query published snapshot"),
        vec![published_entity.clone()]
    );

    let aborted = store
        .begin_snapshot(repo, working_tree_base())
        .await
        .expect("begin aborted snapshot");
    let aborted_entity = entity("ent_memory_aborted", "file://aborted.rs");
    let abort_context = OperationContext::new("test.memory.abort");
    store
        .put_snapshot_with_context(
            aborted.clone(),
            SnapshotBatch {
                entities: vec![aborted_entity.clone()],
                ..SnapshotBatch::default()
            },
            &abort_context,
        )
        .await
        .expect("write aborted snapshot");
    let prepared = store
        .prepare_publication(aborted, &abort_context)
        .await
        .expect("prepare aborted snapshot");
    store
        .abort_prepared(&prepared)
        .await
        .expect("abort prepared snapshot");

    assert_eq!(
        store
            .query_entities(
                SnapshotSelector::LatestCommitted,
                EntityQuery {
                    stable_key: Some(published_entity.stable_key.clone()),
                    ..EntityQuery::default()
                },
            )
            .await
            .expect("query latest after abort"),
        vec![published_entity]
    );
    assert!(
        store
            .query_entities(
                SnapshotSelector::LatestCommitted,
                EntityQuery {
                    stable_key: Some(aborted_entity.stable_key),
                    ..EntityQuery::default()
                },
            )
            .await
            .expect("query aborted entity")
            .is_empty(),
        "aborted prepared snapshot must not replace latest committed"
    );
}

fn entity(id: &str, stable_key: &str) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind: EntityKind::File,
        name: stable_key.to_string(),
        title: None,
        source: None,
        language: Some("rust".to_string()),
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    }
}

fn working_tree_base() -> SnapshotBase {
    SnapshotBase {
        branch: None,
        commit: None,
        parent_snapshot: None,
        working_tree: true,
    }
}
