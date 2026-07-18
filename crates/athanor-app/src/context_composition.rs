use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore, SearchQuery};
use athanor_domain::{ContextLevel, ContextPack, ContextPackId, Entity, EntityId};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::hash::stable_hash;
use crate::index_state::IndexStateStore;
use crate::local_source::discover_source_files;
use crate::project_path::normalize_canonical_path;
use crate::search::get_or_build_search_index_with_factory;

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
    pub(crate) fn apply(&self, limits: &mut ContextLimits) {
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

/// Builds an agent context pack with explicitly supplied runtime dependencies.
pub async fn context_project_with_composition(
    options: ContextOptions,
    composition: &RuntimeComposition,
) -> Result<ContextPack> {
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
    let store = composition.init_store(&root, &config).await?;
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
        .map(|snapshot| snapshot.0.clone())
        .unwrap_or_else(|| "unknown".to_string());

    if options.diff {
        let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
        let previous_state = state_store.load().context("failed to load index state")?;
        let current_files = discover_source_files(&root)
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
    let direct_matches = match get_or_build_search_index_with_factory(
        &snapshot,
        &snapshot_id,
        &index_dir,
        |directory, documents| composition.build_search_index(directory, documents),
    ) {
        Ok(index) => index
            .search(SearchQuery {
                query: options.task.clone(),
                limit: limits.max_entities,
            })
            .await
            .ok()
            .map(|results| {
                results
                    .into_iter()
                    .map(|result| EntityId(result.id))
                    .collect::<Vec<_>>()
            }),
        Err(_) => None,
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
    direct_matches: Option<Vec<EntityId>>,
) -> ContextPack {
    generate_context_pack_internal(snapshot, task, level, limits, direct_matches, true)
}

fn generate_context_pack_internal(
    snapshot: &CanonicalSnapshot,
    task: &str,
    level: ContextLevel,
    limits: ContextLimits,
    direct_matches: Option<Vec<EntityId>>,
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
    id: EntityId,
    selected_ids: &mut Vec<EntityId>,
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
    selected_ids: &[EntityId],
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

fn estimate_tokens<T: Serialize, U: Serialize, V: Serialize>(
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
