use std::collections::HashSet;

use async_trait::async_trait;
use athanor_core::{CoreResult, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityId, EntityKind, Evidence, EvidenceStatus, Ownership, Relation, RelationId,
    RelationKind, RelationStatus, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct ApiKnowledgeLinker;

#[async_trait]
impl Linker for ApiKnowledgeLinker {
    fn name(&self) -> &'static str {
        "api-knowledge"
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
        let affected = input
            .affected
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect::<HashSet<_>>();
        let endpoints = entities_of_kind(&input.entities, EntityKind::ApiEndpoint);
        let schemas = entities_of_kind(&input.entities, EntityKind::ApiSchema);
        let functions = entities_of_kind(&input.entities, EntityKind::Function);
        let documents = input
            .entities
            .iter()
            .filter(|entity| {
                matches!(
                    entity.kind,
                    EntityKind::DocumentationPage | EntityKind::DocumentationSection
                )
            })
            .collect::<Vec<_>>();
        let mut relations = Vec::new();
        let mut relation_ids = HashSet::new();

        for endpoint in endpoints {
            for (payload_key, kind) in [
                ("request_schemas", RelationKind::SchemaForRequest),
                ("response_schemas", RelationKind::SchemaForResponse),
            ] {
                for schema_use in endpoint_schema_uses(endpoint, payload_key) {
                    let Some(reference) =
                        schema_use.get("reference").and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    let Some(component_name) = local_schema_name(reference) else {
                        continue;
                    };
                    let Some(schema) = schemas.iter().find(|schema| {
                        schema.name == component_name && same_source_file(endpoint, schema)
                    }) else {
                        continue;
                    };
                    if either_affected(endpoint, schema, &affected) {
                        push_unique(
                            &mut relations,
                            &mut relation_ids,
                            schema_relation(
                                &input.snapshot,
                                endpoint,
                                schema,
                                kind.clone(),
                                self.name(),
                                schema_use,
                            ),
                        );
                    }
                }
            }

            if let Some(operation_id) = endpoint_operation_id(endpoint) {
                let normalized_operation_id = normalize(operation_id);
                if !normalized_operation_id.is_empty() {
                    for function in &functions {
                        if normalize(&function.name) == normalized_operation_id
                            && either_affected(endpoint, function, &affected)
                        {
                            push_unique(
                                &mut relations,
                                &mut relation_ids,
                                relation(
                                    &input.snapshot,
                                    endpoint,
                                    function,
                                    RelationKind::ImplementedBy,
                                    self.name(),
                                    "operation_id_matches_rust_function",
                                    operation_id,
                                    0.7,
                                ),
                            );
                        }
                    }
                }
            }

            for document in &documents {
                let Some((reason, matched_value)) = documentation_match(document, endpoint) else {
                    continue;
                };
                if !either_affected(document, endpoint, &affected) {
                    continue;
                }
                let kind = if document.kind == EntityKind::DocumentationSection {
                    RelationKind::DocumentsOperation
                } else {
                    RelationKind::DocumentsApi
                };
                push_unique(
                    &mut relations,
                    &mut relation_ids,
                    relation(
                        &input.snapshot,
                        document,
                        endpoint,
                        kind,
                        self.name(),
                        reason,
                        &matched_value,
                        0.5,
                    ),
                );
            }
        }

        Ok(relations)
    }
}

fn entities_of_kind(entities: &[Entity], kind: EntityKind) -> Vec<&Entity> {
    entities
        .iter()
        .filter(|entity| entity.kind == kind)
        .collect()
}

fn endpoint_operation_id(endpoint: &Entity) -> Option<&str> {
    endpoint
        .payload
        .get("operation_id")
        .and_then(serde_json::Value::as_str)
}

fn endpoint_schema_uses<'a>(
    endpoint: &'a Entity,
    payload_key: &str,
) -> Vec<&'a serde_json::Map<String, serde_json::Value>> {
    endpoint
        .payload
        .get(payload_key)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_object)
        .collect()
}

fn local_schema_name(reference: &str) -> Option<&str> {
    reference
        .strip_prefix("#/components/schemas/")
        .filter(|name| !name.is_empty() && !name.contains('/'))
}

fn same_source_file(left: &Entity, right: &Entity) -> bool {
    left.source
        .as_ref()
        .zip(right.source.as_ref())
        .is_some_and(|(left, right)| left.path == right.path)
}

fn documentation_match(document: &Entity, endpoint: &Entity) -> Option<(&'static str, String)> {
    let document_text = normalize(
        &[
            document.name.as_str(),
            document.title.as_deref().unwrap_or_default(),
            document.stable_key.0.as_str(),
            &document.aliases.join(" "),
        ]
        .join(" "),
    );

    if let Some(operation_id) = endpoint_operation_id(endpoint)
        && candidate_matches(&document_text, operation_id)
    {
        return Some((
            "documentation_mentions_operation_id",
            operation_id.to_string(),
        ));
    }

    if let Some(path_segment) = endpoint
        .payload
        .get("path")
        .and_then(serde_json::Value::as_str)
        .and_then(final_static_path_segment)
        && candidate_matches(&document_text, path_segment)
    {
        return Some((
            "documentation_mentions_path_segment",
            path_segment.to_string(),
        ));
    }

    endpoint
        .payload
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .find(|tag| candidate_matches(&document_text, tag))
        .map(|tag| ("documentation_mentions_api_tag", tag.to_string()))
}

fn candidate_matches(document_text: &str, candidate: &str) -> bool {
    let candidate = normalize(candidate);
    candidate.len() >= 3 && document_text.contains(&candidate)
}

fn final_static_path_segment(path: &str) -> Option<&str> {
    path.split('/')
        .rev()
        .find(|segment| !segment.is_empty() && !segment.starts_with('{'))
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn either_affected(left: &Entity, right: &Entity, affected: &HashSet<EntityId>) -> bool {
    affected.contains(&left.id) || affected.contains(&right.id)
}

#[allow(clippy::too_many_arguments)]
fn relation(
    snapshot: &SnapshotId,
    from: &Entity,
    to: &Entity,
    kind: RelationKind,
    linker: &str,
    reason: &str,
    matched_value: &str,
    confidence: f32,
) -> Relation {
    let kind_name = relation_kind_name(&kind);
    let id_material = format!("{kind_name}\0{}\0{}", from.stable_key.0, to.stable_key.0);
    Relation {
        id: RelationId(format!(
            "rel_api_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        from: from.id.clone(),
        to: to.id.clone(),
        status: RelationStatus::Inferred,
        confidence,
        evidence: evidence_for_entities(from, to, linker, confidence),
        ownership: ownership_for_entities(from, to),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": from.stable_key.0,
            "to": to.stable_key.0,
            "reason": reason,
            "matched_value": matched_value,
        }),
    }
}

fn relation_kind_name(kind: &RelationKind) -> &'static str {
    match kind {
        RelationKind::ImplementedBy => "implemented_by",
        RelationKind::DocumentsOperation => "documents_operation",
        RelationKind::DocumentsApi => "documents_api",
        RelationKind::SchemaForRequest => "schema_for_request",
        RelationKind::SchemaForResponse => "schema_for_response",
        _ => "api_relation",
    }
}

fn schema_relation(
    snapshot: &SnapshotId,
    endpoint: &Entity,
    schema: &Entity,
    kind: RelationKind,
    linker: &str,
    schema_use: &serde_json::Map<String, serde_json::Value>,
) -> Relation {
    let reference = schema_use
        .get("reference")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let mut relation = relation(
        snapshot,
        endpoint,
        schema,
        kind,
        linker,
        "openapi_component_schema_reference",
        reference,
        1.0,
    );
    relation.status = RelationStatus::Verified;
    for evidence in &mut relation.evidence {
        evidence.status = EvidenceStatus::Verified;
    }
    relation.payload = json!({
        "from": endpoint.stable_key.0,
        "to": schema.stable_key.0,
        "reason": "openapi_component_schema_reference",
        "schema_use": schema_use,
    });
    relation
}

fn evidence_for_entities(
    left: &Entity,
    right: &Entity,
    linker: &str,
    confidence: f32,
) -> Vec<Evidence> {
    [left, right]
        .into_iter()
        .filter_map(|entity| entity.source.as_ref())
        .map(|source| Evidence {
            source_file: Some(source.path.clone()),
            line_start: source.line_start,
            line_end: source.line_end,
            extractor: Some(linker.to_string()),
            commit_hash: None,
            confidence,
            status: EvidenceStatus::Inferred,
        })
        .collect()
}

fn ownership_for_entities(left: &Entity, right: &Entity) -> Vec<Ownership> {
    let mut ownership = left.ownership.clone();
    for owner in &right.ownership {
        if !ownership
            .iter()
            .any(|existing| existing.source_file == owner.source_file)
        {
            ownership.push(owner.clone());
        }
    }
    ownership
}

fn push_unique(
    relations: &mut Vec<Relation>,
    relation_ids: &mut HashSet<RelationId>,
    relation: Relation,
) {
    if relation_ids.insert(relation.id.clone()) {
        relations.push(relation);
    }
}

#[cfg(test)]
mod tests {
    use athanor_core::AffectedSubset;
    use athanor_domain::{LanguageCode, SourceLocation, StableKey};

    use super::*;

    #[tokio::test]
    async fn links_openapi_operation_to_rust_and_markdown() {
        let endpoint = endpoint();
        let function = entity(
            "ent_function",
            "symbol://rust:auth::login",
            EntityKind::Function,
            "login",
            "src/auth.rs",
            json!({}),
        );
        let section = entity(
            "ent_section",
            "doc://docs/auth.md#login-flow",
            EntityKind::DocumentationSection,
            "Login flow",
            "docs/auth.md",
            json!({}),
        );
        let entities = vec![endpoint.clone(), function.clone(), section.clone()];

        let relations = ApiKnowledgeLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: entities.clone(),
                facts: Vec::new(),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 2);
        let implementation = relations
            .iter()
            .find(|relation| relation.kind == RelationKind::ImplementedBy)
            .unwrap();
        assert_eq!(implementation.from, endpoint.id);
        assert_eq!(implementation.to, function.id);
        assert_eq!(implementation.evidence.len(), 2);
        assert_eq!(implementation.ownership.len(), 2);
        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::DocumentsOperation
                && relation.from == section.id
                && relation.to == endpoint.id
        }));
    }

    #[tokio::test]
    async fn links_request_and_response_component_schemas() {
        let endpoint = endpoint();
        let request_schema = entity(
            "ent_request_schema",
            "api-schema://openapi.yaml#LoginRequest",
            EntityKind::ApiSchema,
            "LoginRequest",
            "openapi.yaml",
            json!({}),
        );
        let response_schema = entity(
            "ent_response_schema",
            "api-schema://openapi.yaml#LoginResponse",
            EntityKind::ApiSchema,
            "LoginResponse",
            "openapi.yaml",
            json!({}),
        );
        let entities = vec![endpoint, request_schema, response_schema];

        let relations = ApiKnowledgeLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: entities.clone(),
                facts: Vec::new(),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 2);
        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::SchemaForRequest
                && relation.status == RelationStatus::Verified
        }));
        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::SchemaForResponse
                && relation.payload["schema_use"]["status_code"] == "200"
        }));
    }

    #[tokio::test]
    async fn incremental_linking_requires_an_affected_side() {
        let endpoint = endpoint();
        let function = entity(
            "ent_function",
            "symbol://rust:auth::login",
            EntityKind::Function,
            "login",
            "src/auth.rs",
            json!({}),
        );
        let section = entity(
            "ent_section",
            "doc://docs/auth.md#login",
            EntityKind::DocumentationSection,
            "Login",
            "docs/auth.md",
            json!({}),
        );

        let relations = ApiKnowledgeLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![endpoint, function.clone(), section],
                facts: Vec::new(),
                affected: AffectedSubset::from_extracted(vec![function], Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].kind, RelationKind::ImplementedBy);
    }

    #[tokio::test]
    async fn does_not_link_unrelated_entities() {
        let endpoint = endpoint();
        let function = entity(
            "ent_function",
            "symbol://rust:auth::logout",
            EntityKind::Function,
            "logout",
            "src/auth.rs",
            json!({}),
        );
        let page = entity(
            "ent_page",
            "doc://docs/billing.md",
            EntityKind::DocumentationPage,
            "Billing",
            "docs/billing.md",
            json!({}),
        );
        let entities = vec![endpoint, function, page];

        let relations = ApiKnowledgeLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: entities.clone(),
                facts: Vec::new(),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(relations.is_empty());
    }

    fn endpoint() -> Entity {
        entity(
            "ent_endpoint",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "login",
            "openapi.yaml",
            json!({
                "operation_id": "login",
                "method": "POST",
                "path": "/login",
                "tags": ["auth"],
                "request_schemas": [{
                    "media_type": "application/json",
                    "reference": "#/components/schemas/LoginRequest"
                }],
                "response_schemas": [{
                    "status_code": "200",
                    "media_type": "application/json",
                    "reference": "#/components/schemas/LoginResponse"
                }],
            }),
        )
    }

    fn entity(
        id: &str,
        stable_key: &str,
        kind: EntityKind,
        name: &str,
        path: &str,
        payload: serde_json::Value,
    ) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: name.to_string(),
            title: Some(name.to_string()),
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("test".to_string())),
            aliases: Vec::new(),
            ownership: athanor_extractor_basic::ownership_for_file(path),
            payload,
        }
    }
}
