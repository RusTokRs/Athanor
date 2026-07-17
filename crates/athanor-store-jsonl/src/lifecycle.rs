use std::fs;

use async_trait::async_trait;
use athanor_core::{
    CoreError, CoreResult, DiagnosticQuery, EntityQuery, EntityResolver, KnowledgeStore,
    RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};

use crate::latest::write_latest_identity;
use crate::snapshot_io::{publish_prepared_snapshot, write_prepared_snapshot, write_snapshot};
use crate::store::JsonlKnowledgeStore;

#[async_trait]
impl KnowledgeStore for JsonlKnowledgeStore {
    async fn begin_snapshot(&self, _repo: RepoId, _base: SnapshotBase) -> CoreResult<SnapshotId> {
        let _writer_lock = self.acquire_writer_lock()?;
        self.recover_known_staging()?;
        let next_snapshot = self.allocate_snapshot_number()?;
        Ok(self.lock_state()?.reserve_snapshot(next_snapshot))
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        self.lock_state()?
            .snapshot_mut(&snapshot)?
            .entities
            .extend(entities);
        Ok(())
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        self.lock_state()?
            .snapshot_mut(&snapshot)?
            .facts
            .extend(facts);
        Ok(())
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        self.lock_state()?
            .snapshot_mut(&snapshot)?
            .relations
            .extend(relations);
        Ok(())
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        self.lock_state()?
            .snapshot_mut(&snapshot)?
            .diagnostics
            .extend(diagnostics);
        Ok(())
    }

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        let mut state = self.lock_state()?;
        let data = state.snapshot_mut(&snapshot)?;
        data.entities.extend(batch.entities);
        data.facts.extend(batch.facts);
        data.relations.extend(batch.relations);
        data.diagnostics.extend(batch.diagnostics);
        Ok(())
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
        let data = self.lock_state()?.snapshot(&snapshot)?.clone();
        if data.prepared {
            publish_prepared_snapshot(&self.root, &snapshot)?;
        } else {
            write_snapshot(&self.root, &snapshot, &data)?;
        }
        self.lock_state()?.snapshot_mut(&snapshot)?.committed = true;
        write_latest_identity(&self.root, &snapshot).map_err(|error| {
            CoreError::Adapter(format!(
                "snapshot {} is committed but latest pointer update failed: {error}",
                snapshot.0
            ))
        })
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        let _writer_lock = self.acquire_writer_lock()?;
        self.lock_state()?.remove_snapshot(&snapshot)?;
        for path in [
            self.snapshot_dir(&snapshot),
            self.prepared_snapshot_dir(&snapshot),
        ] {
            if path.exists() {
                fs::remove_dir_all(&path).map_err(|error| {
                    CoreError::Adapter(format!(
                        "failed to remove aborted snapshot {}: {error}",
                        path.display()
                    ))
                })?;
            }
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
            .map_or(&[][..], |data| data.entities.as_slice())
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
            .map_or(&[][..], |data| data.relations.as_slice())
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
            .map_or(&[][..], |data| data.diagnostics.as_slice())
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
