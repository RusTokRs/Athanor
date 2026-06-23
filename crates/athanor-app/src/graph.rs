use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::load_config;
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;
use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{Entity, Relation};
use serde::Serialize;

pub const GRAPH_EXPORT_SCHEMA: &str = "athanor.graph_export.v1";

#[derive(Debug, Clone)]
pub struct GraphExportOptions {
    pub root: PathBuf,
    pub max_entities: usize,
    pub max_relations: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphExport {
    pub schema: String,
    pub snapshot: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub omitted: GraphOmitted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphNode {
    pub id: String,
    pub stable_key: String,
    pub kind: String,
    pub name: String,
    pub source: Option<String>,
    pub degree: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphEdge {
    pub id: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    pub status: String,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphOmitted {
    pub nodes: usize,
    pub edges: usize,
    pub reason: String,
}

pub async fn export_graph(options: GraphExportOptions) -> Result<GraphExport> {
    if options.max_entities == 0 || options.max_relations == 0 {
        bail!("graph export entity and relation limits must be greater than zero");
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

    Ok(build_graph_export(
        &snapshot,
        options.max_entities,
        options.max_relations,
    ))
}

pub fn build_graph_export(
    snapshot: &CanonicalSnapshot,
    max_entities: usize,
    max_relations: usize,
) -> GraphExport {
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());
    let degree_by_id = degree_by_id(snapshot);

    let mut entities = snapshot.entities.iter().collect::<Vec<_>>();
    entities.sort_by(|left, right| {
        degree_by_id
            .get(&right.id.0)
            .unwrap_or(&0)
            .cmp(degree_by_id.get(&left.id.0).unwrap_or(&0))
            .then_with(|| left.stable_key.0.cmp(&right.stable_key.0))
    });
    entities.truncate(max_entities);

    let selected_ids = entities
        .iter()
        .map(|entity| entity.id.0.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let nodes = entities
        .iter()
        .map(|entity| graph_node(entity, *degree_by_id.get(&entity.id.0).unwrap_or(&0)))
        .collect::<Vec<_>>();

    let mut relations = snapshot
        .relations
        .iter()
        .filter(|relation| {
            selected_ids.contains(&relation.from.0) && selected_ids.contains(&relation.to.0)
        })
        .collect::<Vec<_>>();
    relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    relations.truncate(max_relations);
    let edges = relations
        .iter()
        .map(|relation| graph_edge(relation))
        .collect::<Vec<_>>();
    let emitted_edges = edges.len();

    GraphExport {
        schema: GRAPH_EXPORT_SCHEMA.to_string(),
        snapshot: snapshot_id,
        nodes,
        edges,
        omitted: GraphOmitted {
            nodes: snapshot.entities.len().saturating_sub(selected_ids.len()),
            edges: snapshot.relations.len().saturating_sub(emitted_edges),
            reason: "graph_export_limits".to_string(),
        },
    }
}

fn degree_by_id(snapshot: &CanonicalSnapshot) -> HashMap<String, usize> {
    let mut degree_by_id = HashMap::new();
    for relation in &snapshot.relations {
        *degree_by_id.entry(relation.from.0.clone()).or_default() += 1;
        *degree_by_id.entry(relation.to.0.clone()).or_default() += 1;
    }
    degree_by_id
}

fn graph_node(entity: &Entity, degree: usize) -> GraphNode {
    GraphNode {
        id: entity.id.0.clone(),
        stable_key: entity.stable_key.0.clone(),
        kind: serialized_name(&entity.kind),
        name: entity.name.clone(),
        source: entity.source.as_ref().map(|source| {
            source.line_start.map_or_else(
                || source.path.clone(),
                |line| format!("{}:{line}", source.path),
            )
        }),
        degree,
    }
}

fn graph_edge(relation: &Relation) -> GraphEdge {
    GraphEdge {
        id: relation.id.0.clone(),
        kind: serialized_name(&relation.kind),
        from: relation.from.0.clone(),
        to: relation.to.0.clone(),
        status: serialized_name(&relation.status),
        confidence: relation.confidence,
        evidence: relation
            .evidence
            .iter()
            .filter_map(|evidence| {
                evidence.source_file.as_ref().map(|path| {
                    evidence
                        .line_start
                        .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
                })
            })
            .collect(),
    }
}

fn serialized_name(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use athanor_core::CanonicalSnapshot;
    use athanor_domain::{
        EntityId, EntityKind, Evidence, EvidenceStatus, RelationId, RelationKind, RelationStatus,
        SnapshotId, SourceLocation, StableKey,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn exports_bounded_graph_by_degree_then_stable_key() {
        let endpoint = entity(
            "ent_endpoint",
            "api://GET:/health",
            EntityKind::ApiEndpoint,
            "health",
        );
        let handler = entity(
            "ent_handler",
            "rust://src/lib.rs#health",
            EntityKind::Function,
            "health",
        );
        let doc = entity(
            "ent_doc",
            "doc://docs/api/health.md",
            EntityKind::DocumentationPage,
            "Health API",
        );
        let orphan = entity("ent_orphan", "file://orphan", EntityKind::File, "orphan");
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![handler.clone(), orphan, doc.clone(), endpoint.clone()],
            relations: vec![
                relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
                relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &handler),
            ],
            ..CanonicalSnapshot::default()
        };

        let export = build_graph_export(&snapshot, 3, 1);

        assert_eq!(export.schema, GRAPH_EXPORT_SCHEMA);
        assert_eq!(export.snapshot, "snap_test");
        assert_eq!(
            export
                .nodes
                .iter()
                .map(|node| node.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "api://GET:/health",
                "doc://docs/api/health.md",
                "rust://src/lib.rs#health"
            ]
        );
        assert_eq!(export.edges.len(), 1);
        assert_eq!(export.edges[0].id, "rel_docs");
        assert_eq!(export.edges[0].evidence, vec!["docs/api/health.md:1"]);
        assert_eq!(export.omitted.nodes, 1);
        assert_eq!(export.omitted.edges, 1);
    }

    fn entity(id: &str, stable_key: &str, kind: EntityKind, name: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: stable_key
                    .strip_prefix("doc://")
                    .unwrap_or("src/lib.rs")
                    .to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }

    fn relation(id: &str, kind: RelationKind, from: &Entity, to: &Entity) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind,
            from: from.id.clone(),
            to: to.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: vec![Evidence {
                source_file: Some("docs/api/health.md".to_string()),
                line_start: Some(1),
                line_end: Some(1),
                extractor: Some("test".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: EvidenceStatus::Verified,
            }],
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        }
    }
}
