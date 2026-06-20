use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use athanor_core::{
    CoreError, CoreResult, DiagnosticQuery, EntityQuery, KnowledgeStore, RelationQuery,
};
use athanor_domain::{
    Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
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
        let mut state = self.lock_state()?;
        state.snapshot_mut(&snapshot)?.committed = true;
        Ok(())
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
    use athanor_core::EntityQuery;
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
            .query_entities(EntityQuery {
                stable_key: Some(StableKey("file://README.md".to_string())),
                ..EntityQuery::default()
            })
            .await
            .unwrap();

        assert_eq!(found, vec![entity]);
    }
}
