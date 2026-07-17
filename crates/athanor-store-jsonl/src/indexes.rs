use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use athanor_core::{CoreError, CoreResult};
use athanor_domain::{Diagnostic, Entity, Fact, Relation, SnapshotId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PathIndexEntry {
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathIndex {
    pub schema: String,
    pub snapshot: String,
    pub entries: HashMap<String, PathIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableKeyIndex {
    pub schema: String,
    pub snapshot: String,
    pub entries: HashMap<String, String>,
}

pub(crate) fn write_indexes(
    snapshot_dir: &Path,
    snapshot: &SnapshotId,
    entities: &[Entity],
    facts: &[Fact],
    relations: &[Relation],
    diagnostics: &[Diagnostic],
) -> CoreResult<()> {
    let stable_key_index = StableKeyIndex {
        schema: "athanor.stable_key_index.v1".to_string(),
        snapshot: snapshot.0.clone(),
        entries: entities
            .iter()
            .map(|entity| (entity.stable_key.0.clone(), entity.id.0.clone()))
            .collect(),
    };

    let mut path_entries = HashMap::<String, PathIndexEntry>::new();
    for entity in entities {
        for path in entity_paths(entity) {
            path_entries
                .entry(path)
                .or_default()
                .entities
                .push(entity.id.0.clone());
        }
    }
    for fact in facts {
        for path in fact_paths(fact) {
            path_entries
                .entry(path)
                .or_default()
                .facts
                .push(fact.id.0.clone());
        }
    }
    for relation in relations {
        for path in relation_paths(relation) {
            path_entries
                .entry(path)
                .or_default()
                .relations
                .push(relation.id.0.clone());
        }
    }
    for diagnostic in diagnostics {
        for path in diagnostic_paths(diagnostic) {
            path_entries
                .entry(path)
                .or_default()
                .diagnostics
                .push(diagnostic.id.0.clone());
        }
    }

    let path_index = PathIndex {
        schema: "athanor.path_index.v1".to_string(),
        snapshot: snapshot.0.clone(),
        entries: path_entries,
    };

    write_json(
        &snapshot_dir.join("stable_key_index.json"),
        &stable_key_index,
        "stable key index",
    )?;
    write_json(
        &snapshot_dir.join("path_index.json"),
        &path_index,
        "path index",
    )
}

fn write_json<T: Serialize>(path: &Path, value: &T, subject: &str) -> CoreResult<()> {
    let content = serde_json::to_vec_pretty(value)
        .map_err(|error| CoreError::Adapter(format!("failed to serialize {subject}: {error}")))?;
    fs::write(path, content)
        .map_err(|error| CoreError::Adapter(format!("failed to write {subject}: {error}")))
}

fn entity_paths(entity: &Entity) -> Vec<String> {
    let mut paths = entity
        .ownership
        .iter()
        .map(|ownership| ownership.source_file.clone())
        .collect::<HashSet<_>>();
    if let Some(source) = &entity.source {
        paths.insert(source.path.clone());
    }
    paths.into_iter().collect()
}

fn fact_paths(fact: &Fact) -> Vec<String> {
    let mut paths = fact
        .ownership
        .iter()
        .map(|ownership| ownership.source_file.clone())
        .collect::<HashSet<_>>();
    paths.extend(
        fact.evidence
            .iter()
            .filter_map(|evidence| evidence.source_file.clone()),
    );
    paths.into_iter().collect()
}

fn relation_paths(relation: &Relation) -> Vec<String> {
    let mut paths = relation
        .ownership
        .iter()
        .map(|ownership| ownership.source_file.clone())
        .collect::<HashSet<_>>();
    paths.extend(
        relation
            .evidence
            .iter()
            .filter_map(|evidence| evidence.source_file.clone()),
    );
    paths.into_iter().collect()
}

fn diagnostic_paths(diagnostic: &Diagnostic) -> Vec<String> {
    let mut paths = diagnostic
        .ownership
        .iter()
        .map(|ownership| ownership.source_file.clone())
        .collect::<HashSet<_>>();
    paths.extend(
        diagnostic
            .evidence
            .iter()
            .filter_map(|evidence| evidence.source_file.clone()),
    );
    paths.into_iter().collect()
}
