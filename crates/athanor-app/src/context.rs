use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use crate::config::load_config;
use crate::index_state::IndexStateStore;
use crate::store::init_store;
use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore, SearchIndex, SourceProvider};
use athanor_domain::{ContextLevel, ContextPack, ContextPackId, Entity};
use athanor_extractor_basic::stable_hash;
use athanor_source_fs::LocalFileSystemSource;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::project_path::normalize_canonical_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextLimits {
    pub max_tokens: usize,
    pub max_files: usize,
    pub max_entities: usize,
    pub max_diagnostics: usize,
    pub max_depth: usize,
}

impl ContextLimits {
    pub fn for_level(level: ContextLevel) -> Self {
        match level {
            ContextLevel::Summary => Self {
                max_tokens: 2_000,
                max_files: 5,
                max_entities: 8,
                max_diagnostics: 5,
                max_depth: 0,
            },
            ContextLevel::Normal => Self {
                max_tokens: 8_000,
                max_files: 10,
                max_entities: 20,
                max_diagnostics: 10,
                max_depth: 1,
            },
            ContextLevel::Deep => Self {
                max_tokens: 16_000,
                max_files: 20,
                max_entities: 50,
                max_diagnostics: 20,
                max_depth: 2,
            },
            ContextLevel::Full => Self {
                max_tokens: 32_000,
                max_files: 100,
                max_entities: 200,
                max_diagnostics: 100,
                max_depth: 4,
            },
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextLimitOverrides {
    pub max_tokens: Option<usize>,
    pub max_files: Option<usize>,
    pub max_entities: Option<usize>,
    pub max_diagnostics: Option<usize>,
    pub max_depth: Option<usize>,
}

impl ContextLimitOverrides {
    fn apply(&self, limits: &mut ContextLimits) {
        limits.max_tokens = self.max_tokens.unwrap_or(limits.max_tokens);
        limits.max_files = self.max_files.unwrap_or(limits.max_files);
        limits.max_entities = self.max_entities.unwrap_or(limits.max_entities);
        limits.max_diagnostics = self.max_diagnostics.unwrap_or(limits.max_diagnostics);
        limits.max_depth = self.max_depth.unwrap_or(limits.max_depth);
    }
}

#[derive(Debug, Clone)]
pub struct ContextOptions {
    pub root: PathBuf,
    pub task: String,
    pub diff: bool,
    pub level: ContextLevel,
    pub limits: ContextLimitOverrides,
}

pub async fn context_project(options: ContextOptions) -> Result<ContextPack> {
    if options.task.trim().is_empty() && !options.diff {
        bail!("context task must not be empty");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;

    let mut limits = ContextLimits::for_level(options.level);
    options.limits.apply(&mut limits);
    if limits.max_tokens == 0 || limits.max_files == 0 || limits.max_entities == 0 {
        bail!("context token, file, and entity limits must be greater than zero");
    }

    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|s| s.0.clone())
        .unwrap_or_else(|| "unknown".to_string());

    if options.diff {
        let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
        let previous_state = state_store.load().context("failed to load index state")?;
        let source = LocalFileSystemSource::new(&root);
        let current_files = source
            .discover()
            .await
            .context("failed to discover source files for diff context")?;
        let affected_files = previous_state.affected_files(&current_files);
        let affected_paths = affected_files
            .changed
            .iter()
            .chain(affected_files.removed.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let direct_matches = snapshot
            .entities
            .iter()
            .filter(|entity| {
                entity
                    .ownership
                    .iter()
                    .any(|ownership| affected_paths.contains(&ownership.source_file))
                    || entity
                        .source
                        .as_ref()
                        .is_some_and(|source| affected_paths.contains(&source.path))
            })
            .map(|entity| entity.id.clone())
            .collect::<Vec<_>>();
        let task = if options.task.trim().is_empty() {
            "changed files".to_string()
        } else {
            options.task.clone()
        };
        let mut pack = generate_context_pack_internal(
            &snapshot,
            &task,
            options.level,
            limits,
            Some(direct_matches),
            false,
        );
        if let Some(payload) = pack.payload.as_object_mut() {
            payload.insert(
                "diff".to_string(),
                json!({
                    "changed_files": affected_files.changed.len(),
                    "unchanged_files": affected_files.unchanged.len(),
                    "removed_files": affected_files.removed.len(),
                }),
            );
        }
        return Ok(pack);
    }

    let index_dir = root.join(".athanor/generated/current/search");

    let direct_matches = if let Ok(index) =
        crate::search::get_or_build_search_index(&snapshot, &snapshot_id, &index_dir).await
    {
        if let Ok(search_results) = index
            .search(athanor_core::SearchQuery {
                query: options.task.clone(),
                limit: limits.max_entities,
            })
            .await
        {
            let ids = search_results
                .into_iter()
                .map(|res| athanor_domain::EntityId(res.id))
                .collect::<Vec<_>>();
            Some(ids)
        } else {
            None
        }
    } else {
        None
    };

    Ok(generate_context_pack(
        &snapshot,
        &options.task,
        options.level,
        limits,
        direct_matches,
    ))
}

pub fn generate_context_pack(
    snapshot: &CanonicalSnapshot,
    task: &str,
    level: ContextLevel,
    limits: ContextLimits,
    direct_matches: Option<Vec<athanor_domain::EntityId>>,
) -> ContextPack {
    generate_context_pack_internal(snapshot, task, level, limits, direct_matches, true)
}

fn generate_context_pack_internal(
    snapshot: &CanonicalSnapshot,
    task: &str,
    level: ContextLevel,
    limits: ContextLimits,
    direct_matches: Option<Vec<athanor_domain::EntityId>>,
    fallback_to_ranked: bool,
) -> ContextPack {
    let terms = tokenize(task);
    let mut ranked = snapshot
        .entities
        .iter()
        .filter_map(|entity| {
            let score = entity_score(entity, &terms);
            (score > 0).then_some((score, entity))
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(score, entity)| (Reverse(*score), entity.stable_key.0.clone()));

    let mut selected_ids = Vec::new();
    let mut selected_files = BTreeSet::new();

    let direct_ids = match &direct_matches {
        Some(matches) if !matches.is_empty() => {
            let mut ids = Vec::new();
            for id in matches {
                if try_select_entity(
                    snapshot,
                    id.clone(),
                    &mut selected_ids,
                    &mut selected_files,
                    limits,
                ) {
                    ids.push(id.clone());
                }
                if selected_ids.len() == limits.max_entities {
                    break;
                }
            }
            ids
        }
        Some(_) if !fallback_to_ranked => Vec::new(),
        _ => {
            let mut ids = Vec::new();
            for id in ranked.iter().map(|(_, entity)| entity.id.clone()) {
                if try_select_entity(
                    snapshot,
                    id.clone(),
                    &mut selected_ids,
                    &mut selected_files,
                    limits,
                ) {
                    ids.push(id.clone());
                }
                if selected_ids.len() == limits.max_entities {
                    break;
                }
            }
            ids
        }
    };

    let mut frontier = selected_ids.clone();
    for _ in 0..limits.max_depth {
        let frontier_set = frontier.iter().cloned().collect::<HashSet<_>>();
        let mut next_frontier = Vec::new();
        for relation in &snapshot.relations {
            let neighbor = if frontier_set.contains(&relation.from) {
                Some(&relation.to)
            } else if frontier_set.contains(&relation.to) {
                Some(&relation.from)
            } else {
                None
            };
            if let Some(neighbor) = neighbor
                && try_select_entity(
                    snapshot,
                    neighbor.clone(),
                    &mut selected_ids,
                    &mut selected_files,
                    limits,
                )
            {
                next_frontier.push(neighbor.clone());
            }
        }
        frontier = next_frontier;
        if frontier.is_empty() || selected_ids.len() == limits.max_entities {
            break;
        }
    }

    let entities_by_id = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.clone(), entity))
        .collect::<HashMap<_, _>>();
    let selected_entities = selected_ids
        .iter()
        .filter_map(|id| entities_by_id.get(id).copied())
        .collect::<Vec<_>>();
    let selected_id_set = selected_ids.iter().cloned().collect::<HashSet<_>>();
    let selected_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .entities
                .iter()
                .any(|entity| selected_id_set.contains(entity))
        })
        .take(limits.max_diagnostics)
        .collect::<Vec<_>>();
    let diagnostics = selected_diagnostics
        .iter()
        .map(|diagnostic| diagnostic.id.clone())
        .collect::<Vec<_>>();
    let selected_relations = snapshot
        .relations
        .iter()
        .filter(|relation| {
            selected_id_set.contains(&relation.from) && selected_id_set.contains(&relation.to)
        })
        .collect::<Vec<_>>();
    let files = selected_entities
        .iter()
        .flat_map(|entity| {
            entity
                .source
                .iter()
                .map(|source| source.path.clone())
                .chain(
                    entity
                        .ownership
                        .iter()
                        .map(|ownership| ownership.source_file.clone()),
                )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let scope = selected_entities
        .iter()
        .map(|entity| entity.stable_key.0.clone())
        .collect::<Vec<_>>();
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.as_str())
        .unwrap_or("unknown");
    let id_material = format!("{snapshot_id}\0{task}\0{level:?}\0{limits:?}");
    let matched_terms = terms
        .iter()
        .filter(|term| {
            selected_entities
                .iter()
                .any(|entity| entity_text(entity).contains(term.as_str()))
        })
        .count();
    let confidence = if terms.is_empty() {
        0.0
    } else {
        matched_terms as f32 / terms.len() as f32
    };
    let summary = if direct_ids.is_empty() {
        format!("No canonical entities matched task: {task}")
    } else {
        format!(
            "Selected {} canonical entities across {} files, including {} direct matches and {} related diagnostics.",
            selected_ids.len(),
            files.len(),
            direct_ids.len(),
            diagnostics.len()
        )
    };
    let omitted_files = snapshot
        .entities
        .iter()
        .flat_map(entity_files)
        .collect::<BTreeSet<_>>()
        .len()
        .saturating_sub(files.len());

    ContextPack {
        id: ContextPackId(format!("ctx_{:016x}", stable_hash(id_material.as_bytes()))),
        task: task.to_string(),
        scope,
        level,
        language: None,
        summary,
        entities: selected_ids,
        files,
        diagnostics,
        suggested_checks: Vec::new(),
        confidence,
        payload: json!({
            "schema": "athanor.context_pack.v1",
            "snapshot": snapshot_id,
            "query_terms": terms,
            "direct_matches": direct_ids.len(),
            "limits": {
                "max_tokens": limits.max_tokens,
                "max_files": limits.max_files,
                "max_entities": limits.max_entities,
                "max_diagnostics": limits.max_diagnostics,
                "max_depth": limits.max_depth,
            },
            "omitted": {
                "entities": snapshot.entities.len().saturating_sub(selected_entities.len()),
                "files": omitted_files,
                "diagnostics": snapshot.diagnostics.len().saturating_sub(selected_diagnostics.len()),
                "reason": "relevance_or_context_limits",
            },
            "estimated_tokens": estimate_tokens(&selected_entities, &selected_relations, &selected_diagnostics),
            "entities": selected_entities,
            "relations": selected_relations,
            "diagnostics": selected_diagnostics,
        }),
    }
}

fn try_select_entity(
    snapshot: &CanonicalSnapshot,
    id: athanor_domain::EntityId,
    selected_ids: &mut Vec<athanor_domain::EntityId>,
    selected_files: &mut BTreeSet<String>,
    limits: ContextLimits,
) -> bool {
    if selected_ids.contains(&id) || selected_ids.len() >= limits.max_entities {
        return false;
    }
    let Some(entity) = snapshot.entities.iter().find(|entity| entity.id == id) else {
        return false;
    };
    let entity_files = entity_files(entity).collect::<BTreeSet<_>>();
    let additional_files = entity_files.difference(selected_files).count();
    if selected_files.len() + additional_files > limits.max_files {
        return false;
    }

    let mut candidate_ids = selected_ids.clone();
    candidate_ids.push(id.clone());
    if estimate_selection_tokens(snapshot, &candidate_ids, limits.max_diagnostics)
        > limits.max_tokens
    {
        return false;
    }

    selected_ids.push(id);
    selected_files.extend(entity_files);
    true
}

fn estimate_selection_tokens(
    snapshot: &CanonicalSnapshot,
    selected_ids: &[athanor_domain::EntityId],
    max_diagnostics: usize,
) -> usize {
    let selected_id_set = selected_ids.iter().collect::<HashSet<_>>();
    let entities = snapshot
        .entities
        .iter()
        .filter(|entity| selected_id_set.contains(&entity.id))
        .collect::<Vec<_>>();
    let relations = snapshot
        .relations
        .iter()
        .filter(|relation| {
            selected_id_set.contains(&relation.from) && selected_id_set.contains(&relation.to)
        })
        .collect::<Vec<_>>();
    let diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .entities
                .iter()
                .any(|entity| selected_id_set.contains(entity))
        })
        .take(max_diagnostics)
        .collect::<Vec<_>>();
    estimate_tokens(&entities, &relations, &diagnostics)
}

fn entity_files(entity: &Entity) -> impl Iterator<Item = String> + '_ {
    entity
        .source
        .iter()
        .map(|source| source.path.clone())
        .chain(
            entity
                .ownership
                .iter()
                .map(|ownership| ownership.source_file.clone()),
        )
}

fn estimate_tokens<T: serde::Serialize, U: serde::Serialize, V: serde::Serialize>(
    entities: &[T],
    relations: &[U],
    diagnostics: &[V],
) -> usize {
    serde_json::to_vec(&(entities, relations, diagnostics))
        .map_or(0, |bytes| bytes.len().div_ceil(4))
}

fn entity_score(entity: &Entity, terms: &[String]) -> usize {
    let text = entity_text(entity);

    terms
        .iter()
        .map(|term| {
            let mut score = 0;
            if entity.name.to_lowercase() == *term {
                score += 8;
            }
            if entity
                .title
                .as_ref()
                .is_some_and(|title| title.to_lowercase() == *term)
            {
                score += 8;
            }
            if text.contains(term) {
                score += 1;
            }
            score
        })
        .sum()
}

fn entity_text(entity: &Entity) -> String {
    let mut parts = vec![entity.name.as_str(), entity.stable_key.0.as_str()];
    if let Some(title) = &entity.title {
        parts.push(title);
    }
    if let Some(source) = &entity.source {
        parts.push(&source.path);
    }
    parts.extend(entity.aliases.iter().map(String::as_str));
    parts.join(" ").to_lowercase()
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use athanor_core::CanonicalSnapshot;
    use athanor_domain::{
        Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, EntityId, EntityKind, Relation,
        RelationId, RelationKind, RelationStatus, Severity, SnapshotId, SourceLocation, StableKey,
    };

    use super::*;

    #[test]
    fn selects_direct_matches_relational_neighbors_and_diagnostics() {
        let file = entity("ent_file", "file://docs/auth.md", "auth.md", "docs/auth.md");
        let section = entity(
            "ent_login",
            "test://tests/login.rs",
            "Login test",
            "tests/login.rs",
        );
        let unrelated = entity("ent_other", "file://src/lib.rs", "lib.rs", "src/lib.rs");
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![file.clone(), section.clone(), unrelated],
            facts: Vec::new(),
            relations: vec![Relation {
                id: RelationId("rel_contains".to_string()),
                kind: RelationKind::Contains,
                from: file.id.clone(),
                to: section.id.clone(),
                status: RelationStatus::Verified,
                confidence: 1.0,
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_test".to_string()),
                payload: json!({}),
            }],
            diagnostics: vec![Diagnostic {
                id: DiagnosticId("diag_login".to_string()),
                kind: DiagnosticKind::MissingDocumentation,
                severity: Severity::Medium,
                status: DiagnosticStatus::Open,
                title: "Login documentation".to_string(),
                message: "Test diagnostic".to_string(),
                entities: vec![section.id.clone()],
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_test".to_string()),
                suggested_fix: None,
                payload: json!({}),
            }],
        };

        let pack = generate_context_pack(
            &snapshot,
            "change auth",
            ContextLevel::Normal,
            ContextLimits::for_level(ContextLevel::Normal),
            None,
        );

        assert_eq!(pack.entities, vec![file.id, section.id]);
        assert_eq!(pack.files, vec!["docs/auth.md", "tests/login.rs"]);
        assert_eq!(
            pack.diagnostics,
            vec![DiagnosticId("diag_login".to_string())]
        );
        assert_eq!(pack.payload["snapshot"], "snap_test");
        assert!(pack.confidence > 0.0);
    }

    #[test]
    fn returns_empty_pack_when_nothing_matches() {
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![entity(
                "ent_file",
                "file://README.md",
                "README.md",
                "README.md",
            )],
            ..CanonicalSnapshot::default()
        };

        let pack = generate_context_pack(
            &snapshot,
            "authentication",
            ContextLevel::Normal,
            ContextLimits::for_level(ContextLevel::Normal),
            None,
        );

        assert!(pack.entities.is_empty());
        assert_eq!(pack.confidence, 0.0);
        assert!(pack.summary.starts_with("No canonical entities matched"));
    }

    #[test]
    fn direct_selection_without_fallback_returns_empty_pack() {
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![entity(
                "ent_file",
                "file://docs/auth.md",
                "auth",
                "docs/auth.md",
            )],
            ..CanonicalSnapshot::default()
        };

        let pack = generate_context_pack_internal(
            &snapshot,
            "auth",
            ContextLevel::Normal,
            ContextLimits::for_level(ContextLevel::Normal),
            Some(Vec::new()),
            false,
        );

        assert!(pack.entities.is_empty());
        assert!(pack.files.is_empty());
        assert_eq!(pack.payload["direct_matches"], 0);
    }

    #[test]
    fn summary_level_does_not_expand_relations() {
        let file = entity("ent_file", "file://docs/auth.md", "auth", "docs/auth.md");
        let neighbor = entity("ent_neighbor", "file://src/code.rs", "code", "src/code.rs");
        let snapshot = CanonicalSnapshot {
            entities: vec![file.clone(), neighbor.clone()],
            relations: vec![Relation {
                id: RelationId("rel_contains".to_string()),
                kind: RelationKind::Contains,
                from: file.id.clone(),
                to: neighbor.id.clone(),
                status: RelationStatus::Verified,
                confidence: 1.0,
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_test".to_string()),
                payload: json!({}),
            }],
            ..CanonicalSnapshot::default()
        };

        let pack = generate_context_pack(
            &snapshot,
            "auth",
            ContextLevel::Summary,
            ContextLimits::for_level(ContextLevel::Summary),
            None,
        );

        assert_eq!(pack.level, ContextLevel::Summary);
        assert_eq!(pack.entities, vec![file.id]);
        assert_eq!(pack.payload["limits"]["max_depth"], 0);
    }

    #[test]
    fn explicit_limits_bound_files_entities_and_diagnostics() {
        let first = entity("ent_first", "file://docs/first.md", "auth", "docs/first.md");
        let second = entity(
            "ent_second",
            "file://docs/second.md",
            "auth",
            "docs/second.md",
        );
        let diagnostic = |id: &str, entity: EntityId| Diagnostic {
            id: DiagnosticId(id.to_string()),
            kind: DiagnosticKind::MissingDocumentation,
            severity: Severity::Medium,
            status: DiagnosticStatus::Open,
            title: id.to_string(),
            message: id.to_string(),
            entities: vec![entity],
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            suggested_fix: None,
            payload: json!({}),
        };
        let snapshot = CanonicalSnapshot {
            entities: vec![first.clone(), second.clone()],
            diagnostics: vec![
                diagnostic("diag_first", first.id.clone()),
                diagnostic("diag_second", first.id.clone()),
            ],
            ..CanonicalSnapshot::default()
        };
        let limits = ContextLimits {
            max_tokens: 2_000,
            max_files: 1,
            max_entities: 1,
            max_diagnostics: 1,
            max_depth: 0,
        };

        let pack = generate_context_pack(&snapshot, "auth", ContextLevel::Normal, limits, None);

        assert_eq!(pack.entities.len(), 1);
        assert_eq!(pack.files.len(), 1);
        assert_eq!(pack.diagnostics.len(), 1);
        assert!(pack.payload["estimated_tokens"].as_u64().unwrap() <= 2_000);
        assert_eq!(
            pack.payload["omitted"]["reason"],
            "relevance_or_context_limits"
        );
    }

    fn entity(id: &str, stable_key: &str, name: &str, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind: EntityKind::File,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: None,
                line_end: None,
            }),
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }
}
