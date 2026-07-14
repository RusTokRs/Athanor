use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, KnowledgeStore, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};

#[derive(Debug, Clone, Default)]
pub struct MemoryKnowledgeStore {
    state: Arc<Mutex<State>>,
}

#[derive(Debug, Default)]
struct State {
    next_snapshot: u64,
    snapshots: HashMap<SnapshotId, SnapshotData>,
}

#[derive(Debug, Default)]
struct SnapshotData {
    committed: bool,
    entities: Vec<Entity>,
    facts: Vec<Fact>,
    relations: Vec<Relation>,
    diagnostics: Vec<Diagnostic>,
}

impl MemoryKnowledgeStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KnowledgeStore for MemoryKnowledgeStore {
    async fn begin_snapshot(&self, _repo: RepoId, _base: SnapshotBase) -> CoreResult<SnapshotId> {
        let mut state = self.lock_state()?;
        state.next_snapshot += 1;

        let snapshot = SnapshotId(format!("snap_memory_{:08}", state.next_snapshot));
        state.snapshots.insert(
            snapshot.clone(),
            SnapshotData {
                ..SnapshotData::default()
            },
        );

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

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let state = self.lock_state()?;
        let snapshot = state
            .snapshot_data(&snapshot)
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))?;
        if snapshot.committed {
            return Err(CoreError::Conflict(
                "cannot prepare a committed snapshot".to_string(),
            ));
        }
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

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let mut state = self.lock_state()?;
        state.snapshot_mut(&snapshot)?.committed = true;
        Ok(())
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
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
        Ok(())
    }
}

#[async_trait]
impl CanonicalSnapshotStore for MemoryKnowledgeStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        let state = self.lock_state()?;
        let Some(data) = state.snapshot_data(snapshot) else {
            return Ok(None);
        };
        if !data.committed {
            return Err(CoreError::SnapshotNotCommitted(snapshot.0.clone()));
        }
        Ok(Some(canonical_snapshot(snapshot, data)))
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        let state = self.lock_state()?;
        let Some(snapshot) = state.committed_snapshot(&SnapshotSelector::LatestCommitted)? else {
            return Ok(None);
        };
        let data = state
            .snapshot_data(&snapshot)
            .expect("latest committed snapshot must exist");
        Ok(Some(canonical_snapshot(&snapshot, data)))
    }
}

#[async_trait]
impl EntityResolver for MemoryKnowledgeStore {
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

impl MemoryKnowledgeStore {
    fn lock_state(&self) -> CoreResult<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| CoreError::Adapter("memory store lock poisoned".to_string()))
    }
}

impl State {
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

    fn snapshot_data(&self, snapshot: &SnapshotId) -> Option<&SnapshotData> {
        self.snapshots.get(snapshot)
    }
}

fn canonical_snapshot(snapshot: &SnapshotId, data: &SnapshotData) -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot: Some(snapshot.clone()),
        entities: data.entities.clone(),
        facts: data.facts.clone(),
        relations: data.relations.clone(),
        diagnostics: data.diagnostics.clone(),
    }
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
    use athanor_core::{EntityQuery, SnapshotBatch, SnapshotSelector};
    use athanor_domain::{EntityId, EntityKind};
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn stores_and_queries_entities() {
        let store = MemoryKnowledgeStore::new();
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
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        };

        store
            .put_entities(snapshot.clone(), vec![entity.clone()])
            .await
            .unwrap();
        store.commit_snapshot(snapshot).await.unwrap();

        let found = store
            .query_entities(
                SnapshotSelector::LatestCommitted,
                EntityQuery {
                    stable_key: Some(StableKey("file://README.md".to_string())),
                    ..EntityQuery::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(found, vec![entity]);
    }

    #[tokio::test]
    async fn stores_a_snapshot_batch_through_the_compatibility_path() {
        let store = MemoryKnowledgeStore::new();
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
            id: EntityId("ent_batch_readme".to_string()),
            stable_key: StableKey("file://BATCH.md".to_string()),
            kind: EntityKind::File,
            name: "BATCH.md".to_string(),
            title: None,
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        };

        store
            .put_snapshot(
                snapshot.clone(),
                SnapshotBatch {
                    entities: vec![entity.clone()],
                    ..SnapshotBatch::default()
                },
            )
            .await
            .unwrap();
        store.commit_snapshot(snapshot).await.unwrap();

        let found = store
            .query_entities(
                SnapshotSelector::LatestCommitted,
                EntityQuery {
                    stable_key: Some(entity.stable_key.clone()),
                    ..EntityQuery::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(found, vec![entity]);
    }

    #[tokio::test]
    async fn abort_discards_uncommitted_snapshot() {
        let store = MemoryKnowledgeStore::new();
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
        store.abort_snapshot(snapshot.clone()).await.unwrap();

        let error = store
            .query_entities(SnapshotSelector::Exact(snapshot), EntityQuery::default())
            .await
            .expect_err("aborted snapshot must not remain queryable");
        assert!(matches!(error, CoreError::NotFound(_)));
    }
}
