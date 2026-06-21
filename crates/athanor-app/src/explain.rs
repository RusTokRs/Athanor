use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{Diagnostic, Entity, Fact, Relation};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde::Serialize;

use crate::project_path::normalize_canonical_path;

#[derive(Debug, Clone)]
pub struct ExplainOptions {
    pub root: PathBuf,
    pub stable_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainedRelation {
    pub relation: Relation,
    pub related_entity: Option<Entity>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityExplanation {
    pub schema: String,
    pub snapshot: String,
    pub entity: Entity,
    pub facts: Vec<Fact>,
    pub outgoing_relations: Vec<ExplainedRelation>,
    pub incoming_relations: Vec<ExplainedRelation>,
    pub diagnostics: Vec<Diagnostic>,
}

pub async fn explain_project(options: ExplainOptions) -> Result<EntityExplanation> {
    if options.stable_key.trim().is_empty() {
        bail!("entity stable key must not be empty");
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

    explain_snapshot(&snapshot, options.stable_key.trim())
}

pub fn explain_snapshot(
    snapshot: &CanonicalSnapshot,
    stable_key: &str,
) -> Result<EntityExplanation> {
    let entity = snapshot
        .entities
        .iter()
        .find(|entity| entity.stable_key.0 == stable_key)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("canonical entity not found: {stable_key}"))?;

    let mut facts = snapshot
        .facts
        .iter()
        .filter(|fact| {
            fact.subject == entity.id || fact.object.as_ref().is_some_and(|id| *id == entity.id)
        })
        .cloned()
        .collect::<Vec<_>>();
    facts.sort_by(|left, right| left.id.0.cmp(&right.id.0));

    let mut outgoing_relations = snapshot
        .relations
        .iter()
        .filter(|relation| relation.from == entity.id)
        .map(|relation| explained_relation(snapshot, relation, &relation.to))
        .collect::<Vec<_>>();
    outgoing_relations.sort_by(|left, right| left.relation.id.0.cmp(&right.relation.id.0));

    let mut incoming_relations = snapshot
        .relations
        .iter()
        .filter(|relation| relation.to == entity.id)
        .map(|relation| explained_relation(snapshot, relation, &relation.from))
        .collect::<Vec<_>>();
    incoming_relations.sort_by(|left, right| left.relation.id.0.cmp(&right.relation.id.0));

    let mut diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.entities.contains(&entity.id))
        .cloned()
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));

    Ok(EntityExplanation {
        schema: "athanor.entity_explanation.v1".to_string(),
        snapshot: snapshot
            .snapshot
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
        entity,
        facts,
        outgoing_relations,
        incoming_relations,
        diagnostics,
    })
}

fn explained_relation(
    snapshot: &CanonicalSnapshot,
    relation: &Relation,
    related_id: &athanor_domain::EntityId,
) -> ExplainedRelation {
    ExplainedRelation {
        relation: relation.clone(),
        related_entity: snapshot
            .entities
            .iter()
            .find(|entity| entity.id == *related_id)
            .cloned(),
    }
}

#[cfg(test)]
mod tests {
    use athanor_domain::{
        DiagnosticId, DiagnosticKind, DiagnosticStatus, EntityId, EntityKind, EvidenceStatus,
        FactId, FactKind, RelationId, RelationKind, RelationStatus, Severity, SnapshotId,
        StableKey,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn explains_entity_evidence_relations_and_diagnostics() {
        let endpoint = entity("ent_endpoint", "api://POST:/login", EntityKind::ApiEndpoint);
        let schema = entity(
            "ent_schema",
            "api-schema://openapi.yaml#LoginRequest",
            EntityKind::ApiSchema,
        );
        let document = entity(
            "ent_doc",
            "doc://docs/login.md#login",
            EntityKind::DocumentationSection,
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![endpoint.clone(), schema.clone(), document.clone()],
            facts: vec![Fact {
                id: FactId("fact_route".to_string()),
                kind: FactKind::RouteDeclared,
                subject: endpoint.id.clone(),
                object: None,
                value: json!({}),
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_test".to_string()),
                extractor: "test".to_string(),
                confidence: 1.0,
            }],
            relations: vec![
                relation(
                    "rel_schema",
                    RelationKind::SchemaForRequest,
                    &endpoint,
                    &schema,
                ),
                relation(
                    "rel_docs",
                    RelationKind::DocumentsOperation,
                    &document,
                    &endpoint,
                ),
            ],
            diagnostics: vec![Diagnostic {
                id: DiagnosticId("diag_docs".to_string()),
                kind: DiagnosticKind::ApiEndpointImplementedButNotDocumented,
                severity: Severity::Medium,
                status: DiagnosticStatus::Open,
                title: "Missing docs".to_string(),
                message: "Test".to_string(),
                entities: vec![endpoint.id.clone()],
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_test".to_string()),
                suggested_fix: None,
                payload: json!({}),
            }],
        };

        let explanation = explain_snapshot(&snapshot, "api://POST:/login").unwrap();

        assert_eq!(explanation.schema, "athanor.entity_explanation.v1");
        assert_eq!(explanation.snapshot, "snap_test");
        assert_eq!(explanation.facts.len(), 1);
        assert_eq!(explanation.outgoing_relations.len(), 1);
        assert_eq!(
            explanation.outgoing_relations[0]
                .related_entity
                .as_ref()
                .unwrap()
                .stable_key,
            schema.stable_key
        );
        assert_eq!(explanation.incoming_relations.len(), 1);
        assert_eq!(
            explanation.incoming_relations[0]
                .related_entity
                .as_ref()
                .unwrap()
                .stable_key,
            document.stable_key
        );
        assert_eq!(explanation.diagnostics.len(), 1);
    }

    #[test]
    fn rejects_unknown_stable_key() {
        let error =
            explain_snapshot(&CanonicalSnapshot::default(), "api://GET:/missing").unwrap_err();

        assert_eq!(
            error.to_string(),
            "canonical entity not found: api://GET:/missing"
        );
    }

    fn entity(id: &str, stable_key: &str, kind: EntityKind) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: stable_key.to_string(),
            title: None,
            source: None,
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
            evidence: vec![athanor_domain::Evidence {
                source_file: None,
                line_start: None,
                line_end: None,
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
