use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use athanor_core::{
    CoreError, CoreResult, DiagnosticQuery, EntityQuery, KnowledgeStore, RelationQuery,
    SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct TransientKnowledgeStore {
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

impl TransientKnowledgeStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KnowledgeStore for TransientKnowledgeStore {
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
                    .is_none_or(|kind| format!("{:?}", entity.kind).eq_ignore_ascii_case(kind))
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
                    .is_none_or(|kind| format!("{:?}", relation.kind).eq_ignore_ascii_case(kind))
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
}

impl TransientKnowledgeStore {
    fn lock_state(&self) -> CoreResult<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| CoreError::Adapter("transient store lock poisoned".to_string()))
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

    fn snapshot_data(&self, snapshot: Option<&SnapshotId>) -> Option<&SnapshotData> {
        snapshot.and_then(|snapshot| self.snapshots.get(snapshot))
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
