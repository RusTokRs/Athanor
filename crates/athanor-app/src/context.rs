use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{ContextLevel, ContextPack, ContextPackId, Entity};
use athanor_extractor_basic::stable_hash;
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde_json::json;

use crate::project_path::normalize_canonical_path;

const DEFAULT_MAX_ENTITIES: usize = 20;

#[derive(Debug, Clone)]
pub struct ContextOptions {
    pub root: PathBuf,
    pub task: String,
}

pub async fn context_project(options: ContextOptions) -> Result<ContextPack> {
    if options.task.trim().is_empty() {
        bail!("context task must not be empty");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
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

    Ok(generate_context_pack(
        &snapshot,
        &options.task,
        DEFAULT_MAX_ENTITIES,
    ))
}

pub fn generate_context_pack(
    snapshot: &CanonicalSnapshot,
    task: &str,
    max_entities: usize,
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

    let direct_ids = ranked
        .iter()
        .take(max_entities)
        .map(|(_, entity)| entity.id.clone())
        .collect::<Vec<_>>();
    let direct_id_set = direct_ids.iter().cloned().collect::<HashSet<_>>();
    let mut selected_ids = direct_ids.clone();

    for relation in &snapshot.relations {
        let neighbor = if direct_id_set.contains(&relation.from) {
            Some(&relation.to)
        } else if direct_id_set.contains(&relation.to) {
            Some(&relation.from)
        } else {
            None
        };

        if let Some(neighbor) = neighbor
            && !selected_ids.contains(neighbor)
            && selected_ids.len() < max_entities
        {
            selected_ids.push(neighbor.clone());
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
    let id_material = format!("{snapshot_id}\0{task}\0normal\0{max_entities}");
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

    ContextPack {
        id: ContextPackId(format!("ctx_{:016x}", stable_hash(id_material.as_bytes()))),
        task: task.to_string(),
        scope,
        level: ContextLevel::Normal,
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
            "entities": selected_entities,
            "relations": selected_relations,
            "diagnostics": selected_diagnostics,
        }),
    }
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

fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let mut components = path.components();

        if let Some(Component::Prefix(prefix)) = components.next() {
            match prefix.kind() {
                Prefix::VerbatimDisk(disk) => {
                    let drive = char::from(disk);
                    return PathBuf::from(format!("{drive}:\\")).join(components.as_path());
                }
                Prefix::VerbatimUNC(server, share) => {
                    return PathBuf::from(format!(
                        "\\\\{}\\{}",
                        server.to_string_lossy(),
                        share.to_string_lossy()
                    ))
                    .join(components.as_path());
                }
                _ => {}
            }
        }
    }

    path
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

        let pack = generate_context_pack(&snapshot, "change auth", 20);

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

        let pack = generate_context_pack(&snapshot, "authentication", 20);

        assert!(pack.entities.is_empty());
        assert_eq!(pack.confidence, 0.0);
        assert!(pack.summary.starts_with("No canonical entities matched"));
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
