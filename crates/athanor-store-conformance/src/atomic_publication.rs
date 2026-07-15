use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshotStore, CoreError, KnowledgeStore, SnapshotBatch,
};
use athanor_domain::{Entity, EntityId, EntityKind, RepoId, SnapshotBase, StableKey};
use serde_json::json;

/// Verifies the backend-neutral data-plus-marker publication boundary.
///
/// Before publication the exact snapshot must not be observable as committed. One atomic call must
/// replace any partial staged data with the complete batch, make exact/latest reads agree, and make
/// the generation non-republishable and non-abortable.
pub async fn verify_atomic_publication_contract<S>(store: &S)
where
    S: KnowledgeStore + CanonicalSnapshotStore + AtomicSnapshotPublication,
{
    let snapshot = store
        .begin_snapshot(
            RepoId("repo_atomic_conformance".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin atomic conformance snapshot");

    store
        .put_entities(
            snapshot.clone(),
            vec![entity("ent_atomic_partial", "partial.md")],
        )
        .await
        .expect("stage partial atomic conformance data");
    match store.load_snapshot(&snapshot).await {
        Ok(None) | Err(CoreError::SnapshotNotCommitted(_)) => {}
        Ok(Some(_)) => panic!("partial snapshot must not be visible as committed"),
        Err(error) => panic!("unexpected pre-publication read error: {error}"),
    }

    let committed = entity("ent_atomic_committed", "committed.md");
    store
        .publish_snapshot_batch(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![committed.clone()],
                ..SnapshotBatch::default()
            },
        )
        .await
        .expect("atomically publish complete conformance batch and marker");

    let exact = store
        .load_snapshot(&snapshot)
        .await
        .expect("load exact committed snapshot")
        .expect("exact committed snapshot exists");
    assert_eq!(exact.snapshot.as_ref(), Some(&snapshot));
    assert_eq!(exact.entities, vec![committed.clone()]);
    assert!(
        exact
            .entities
            .iter()
            .all(|entity| entity.id.0 != "ent_atomic_partial")
    );

    let latest = store
        .load_latest_snapshot()
        .await
        .expect("load latest committed snapshot")
        .expect("latest committed snapshot exists");
    assert_eq!(latest.snapshot.as_ref(), Some(&snapshot));
    assert_eq!(latest.entities, vec![committed]);

    let republish = store
        .publish_snapshot_batch(snapshot.clone(), SnapshotBatch::default())
        .await
        .expect_err("committed snapshot must not be republished");
    assert!(matches!(republish, CoreError::Conflict(_)));

    let abort = store
        .abort_snapshot(snapshot)
        .await
        .expect_err("committed snapshot must not be aborted");
    assert!(matches!(abort, CoreError::Conflict(_)));
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
