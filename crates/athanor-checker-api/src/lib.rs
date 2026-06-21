use std::collections::HashSet;

use async_trait::async_trait;
use athanor_core::{CheckInput, Checker, CoreResult};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, Ownership, Relation, RelationKind, Severity, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct ApiConsistencyChecker;

#[async_trait]
impl Checker for ApiConsistencyChecker {
    fn name(&self) -> &'static str {
        "api-consistency"
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let endpoints = entities_of_kind(&input.entities, EntityKind::ApiEndpoint);
        let functions = entities_of_kind(&input.entities, EntityKind::Function);
        let schemas_affected = input
            .affected
            .entities
            .iter()
            .any(|entity| entity.kind == EntityKind::ApiSchema);
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
        let affected_ids = input
            .affected
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect::<HashSet<_>>();
        let functions_affected = input
            .affected
            .entities
            .iter()
            .any(|entity| entity.kind == EntityKind::Function);
        let documents_affected = input.affected.entities.iter().any(|entity| {
            matches!(
                entity.kind,
                EntityKind::DocumentationPage | EntityKind::DocumentationSection
            )
        });
        let mut diagnostics = Vec::new();

        for endpoint in endpoints {
            let implemented = input.relations.iter().any(|relation| {
                relation.kind == RelationKind::ImplementedBy && relation.from == endpoint.id
            });
            let documented = input.relations.iter().any(|relation| {
                matches!(
                    relation.kind,
                    RelationKind::DocumentsApi | RelationKind::DocumentsOperation
                ) && relation.to == endpoint.id
            });
            let affected_relations = input
                .affected
                .relations
                .iter()
                .filter(|relation| relation_touches(relation, &endpoint.id))
                .collect::<Vec<_>>();
            let implementation_changed = functions_affected
                || affected_relations
                    .iter()
                    .any(|relation| relation.kind == RelationKind::ImplementedBy);
            let documentation_changed = documents_affected
                || affected_relations.iter().any(|relation| {
                    matches!(
                        relation.kind,
                        RelationKind::ImplementedBy
                            | RelationKind::DocumentsApi
                            | RelationKind::DocumentsOperation
                    )
                });
            let endpoint_affected = affected_ids.contains(&endpoint.id);
            let schema_links_changed = affected_relations.iter().any(|relation| {
                matches!(
                    relation.kind,
                    RelationKind::SchemaForRequest | RelationKind::SchemaForResponse
                )
            });

            if endpoint_affected || schemas_affected || schema_links_changed {
                for (payload_key, relation_kind, diagnostic_kind) in [
                    (
                        "request_schemas",
                        RelationKind::SchemaForRequest,
                        DiagnosticKind::ApiRequestSchemaMismatch,
                    ),
                    (
                        "response_schemas",
                        RelationKind::SchemaForResponse,
                        DiagnosticKind::ApiResponseSchemaMismatch,
                    ),
                ] {
                    for schema_use in endpoint_schema_uses(endpoint, payload_key) {
                        let Some(reference) = schema_use
                            .get("reference")
                            .and_then(serde_json::Value::as_str)
                        else {
                            continue;
                        };
                        if !is_local_component_schema_reference(reference) {
                            continue;
                        }
                        let resolved = input.relations.iter().any(|relation| {
                            relation.kind == relation_kind
                                && relation.from == endpoint.id
                                && relation_schema_reference(relation) == Some(reference)
                        });
                        if !resolved {
                            diagnostics.push(missing_schema_diagnostic(
                                endpoint,
                                diagnostic_kind.clone(),
                                schema_use,
                                self.name(),
                                &input.snapshot,
                            ));
                        }
                    }
                }
            }

            if !implemented && (endpoint_affected || implementation_changed) {
                diagnostics.push(missing_implementation_diagnostic(
                    endpoint,
                    &functions,
                    self.name(),
                    &input.snapshot,
                ));
            }

            if implemented && !documented && (endpoint_affected || documentation_changed) {
                diagnostics.push(missing_documentation_diagnostic(
                    endpoint,
                    &documents,
                    &functions,
                    self.name(),
                    &input.snapshot,
                ));
            }
        }

        Ok(diagnostics)
    }
}

fn entities_of_kind(entities: &[Entity], kind: EntityKind) -> Vec<&Entity> {
    entities
        .iter()
        .filter(|entity| entity.kind == kind)
        .collect()
}

fn relation_touches(relation: &Relation, entity: &EntityId) -> bool {
    relation.from == *entity || relation.to == *entity
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

fn is_local_component_schema_reference(reference: &str) -> bool {
    reference
        .strip_prefix("#/components/schemas/")
        .is_some_and(|name| !name.is_empty() && !name.contains('/'))
}

fn relation_schema_reference(relation: &Relation) -> Option<&str> {
    relation
        .payload
        .get("schema_use")
        .and_then(|schema_use| schema_use.get("reference"))
        .and_then(serde_json::Value::as_str)
}

fn missing_implementation_diagnostic(
    endpoint: &Entity,
    functions: &[&Entity],
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    diagnostic(
        endpoint,
        DiagnosticKind::ApiEndpointDocumentedButNotImplemented,
        Severity::High,
        "API endpoint has no linked implementation",
        "The OpenAPI operation is not linked to a Rust function or method.",
        "Add or rename the handler to match operationId, or add explicit route metadata.",
        checker,
        snapshot,
        ownership_with_candidates(endpoint, functions),
        "implemented_by",
    )
}

fn missing_documentation_diagnostic(
    endpoint: &Entity,
    documents: &[&Entity],
    functions: &[&Entity],
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    diagnostic(
        endpoint,
        DiagnosticKind::ApiEndpointImplementedButNotDocumented,
        Severity::Medium,
        "Implemented API endpoint has no linked documentation",
        "The implemented OpenAPI operation is not linked to a Markdown page or section.",
        "Add an API documentation heading mentioning the operation id, path, or tag.",
        checker,
        snapshot,
        ownership_with_candidate_groups(endpoint, documents, functions),
        "documents_api_or_operation",
    )
}

fn missing_schema_diagnostic(
    endpoint: &Entity,
    kind: DiagnosticKind,
    schema_use: &serde_json::Map<String, serde_json::Value>,
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    let reference = schema_use
        .get("reference")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let context = serde_json::to_string(schema_use).unwrap_or_default();
    let kind_slug = diagnostic_slug(&kind);
    let id_material = format!(
        "{kind_slug}\0{}\0{reference}\0{context}",
        endpoint.stable_key.0
    );
    let (severity, title, message) = match kind {
        DiagnosticKind::ApiRequestSchemaMismatch => (
            Severity::High,
            "API request references an unresolved schema",
            "The OpenAPI request body references a component schema that was not linked.",
        ),
        _ => (
            Severity::Medium,
            "API response references an unresolved schema",
            "The OpenAPI response references a component schema that was not linked.",
        ),
    };

    Diagnostic {
        id: DiagnosticId(format!(
            "diag_api_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity,
        status: DiagnosticStatus::Open,
        title: title.to_string(),
        message: message.to_string(),
        entities: vec![endpoint.id.clone()],
        evidence: vec![evidence_for_endpoint(endpoint, checker)],
        ownership: endpoint.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(format!(
            "Define {reference} in components.schemas or correct the local $ref."
        )),
        payload: json!({
            "endpoint": endpoint.stable_key.0,
            "schema_use": schema_use,
            "missing_relation": match kind_slug {
                "api_request_schema_mismatch" => "schema_for_request",
                _ => "schema_for_response",
            },
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn diagnostic(
    endpoint: &Entity,
    kind: DiagnosticKind,
    severity: Severity,
    title: &str,
    message: &str,
    suggested_fix: &str,
    checker: &str,
    snapshot: &SnapshotId,
    ownership: Vec<Ownership>,
    missing_relation: &str,
) -> Diagnostic {
    let kind_slug = diagnostic_slug(&kind);
    let id_material = format!("{kind_slug}\0{}", endpoint.stable_key.0);
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_api_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity,
        status: DiagnosticStatus::Open,
        title: title.to_string(),
        message: message.to_string(),
        entities: vec![endpoint.id.clone()],
        evidence: vec![evidence_for_endpoint(endpoint, checker)],
        ownership,
        snapshot: snapshot.clone(),
        suggested_fix: Some(suggested_fix.to_string()),
        payload: json!({
            "endpoint": endpoint.stable_key.0,
            "missing_relation": missing_relation,
        }),
    }
}

fn diagnostic_slug(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::ApiEndpointDocumentedButNotImplemented => "api_endpoint_not_implemented",
        DiagnosticKind::ApiEndpointImplementedButNotDocumented => "api_endpoint_not_documented",
        DiagnosticKind::ApiRequestSchemaMismatch => "api_request_schema_mismatch",
        DiagnosticKind::ApiResponseSchemaMismatch => "api_response_schema_mismatch",
        _ => "api_consistency",
    }
}

fn evidence_for_endpoint(endpoint: &Entity, checker: &str) -> Evidence {
    let source = endpoint.source.as_ref();
    Evidence {
        source_file: source.map(|source| source.path.clone()),
        line_start: source.and_then(|source| source.line_start),
        line_end: source.and_then(|source| source.line_end),
        extractor: Some(checker.to_string()),
        commit_hash: None,
        confidence: 1.0,
        status: EvidenceStatus::Missing,
    }
}

fn ownership_with_candidates(endpoint: &Entity, candidates: &[&Entity]) -> Vec<Ownership> {
    ownership_with_candidate_groups(endpoint, candidates, &[])
}

fn ownership_with_candidate_groups(
    endpoint: &Entity,
    first: &[&Entity],
    second: &[&Entity],
) -> Vec<Ownership> {
    let mut ownership = endpoint.ownership.clone();
    for owner in first
        .iter()
        .chain(second.iter())
        .flat_map(|candidate| candidate.ownership.iter())
    {
        if !ownership
            .iter()
            .any(|existing| existing.source_file == owner.source_file)
        {
            ownership.push(owner.clone());
        }
    }
    ownership
}

#[cfg(test)]
mod tests {
    use athanor_core::AffectedSubset;
    use athanor_domain::{LanguageCode, RelationId, RelationStatus, SourceLocation, StableKey};

    use super::*;

    #[tokio::test]
    async fn reports_endpoint_without_implementation() {
        let endpoint = entity(
            "ent_endpoint",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );
        let function = entity(
            "ent_function",
            "symbol://rust:auth::logout",
            EntityKind::Function,
            "src/auth.rs",
        );

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![endpoint.clone(), function],
                Vec::new(),
                vec![endpoint.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].kind,
            DiagnosticKind::ApiEndpointDocumentedButNotImplemented
        );
        assert_eq!(diagnostics[0].severity, Severity::High);
        assert_eq!(diagnostics[0].ownership.len(), 2);
        assert!(!diagnostics[0].evidence.is_empty());
    }

    #[tokio::test]
    async fn reports_implemented_endpoint_without_documentation() {
        let endpoint = entity(
            "ent_endpoint",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );
        let function = entity(
            "ent_function",
            "symbol://rust:auth::login",
            EntityKind::Function,
            "src/auth.rs",
        );
        let implementation = relation(
            "rel_implementation",
            RelationKind::ImplementedBy,
            &endpoint,
            &function,
        );

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![endpoint.clone(), function],
                vec![implementation.clone()],
                vec![endpoint],
                vec![implementation],
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].kind,
            DiagnosticKind::ApiEndpointImplementedButNotDocumented
        );
        assert_eq!(diagnostics[0].ownership.len(), 2);
    }

    #[tokio::test]
    async fn accepts_implemented_and_documented_endpoint() {
        let endpoint = entity(
            "ent_endpoint",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );
        let function = entity(
            "ent_function",
            "symbol://rust:auth::login",
            EntityKind::Function,
            "src/auth.rs",
        );
        let document = entity(
            "ent_document",
            "doc://docs/login.md#login",
            EntityKind::DocumentationSection,
            "docs/login.md",
        );
        let implementation = relation(
            "rel_implementation",
            RelationKind::ImplementedBy,
            &endpoint,
            &function,
        );
        let documentation = relation(
            "rel_documentation",
            RelationKind::DocumentsOperation,
            &document,
            &endpoint,
        );

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![endpoint.clone(), function, document],
                vec![implementation.clone(), documentation.clone()],
                vec![endpoint],
                vec![implementation, documentation],
            ))
            .await
            .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[tokio::test]
    async fn function_changes_reevaluate_unaffected_endpoints() {
        let endpoint = entity(
            "ent_endpoint",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );
        let function = entity(
            "ent_function",
            "symbol://rust:auth::logout",
            EntityKind::Function,
            "src/auth.rs",
        );

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![endpoint, function.clone()],
                Vec::new(),
                vec![function],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
    }

    #[tokio::test]
    async fn reports_unresolved_request_and_response_schema_references() {
        let mut endpoint = entity(
            "ent_endpoint",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );
        endpoint.payload = json!({
            "request_schemas": [{
                "media_type": "application/json",
                "reference": "#/components/schemas/MissingRequest"
            }],
            "response_schemas": [{
                "status_code": "200",
                "media_type": "application/json",
                "reference": "#/components/schemas/MissingResponse"
            }]
        });

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![endpoint.clone()],
                Vec::new(),
                vec![endpoint],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::ApiRequestSchemaMismatch
                && diagnostic.payload["schema_use"]["reference"]
                    == "#/components/schemas/MissingRequest"
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::ApiResponseSchemaMismatch
                && diagnostic.payload["schema_use"]["status_code"] == "200"
        }));
    }

    #[tokio::test]
    async fn accepts_linked_request_and_response_schemas() {
        let mut endpoint = entity(
            "ent_endpoint",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );
        endpoint.payload = json!({
            "request_schemas": [{"reference": "#/components/schemas/LoginRequest"}],
            "response_schemas": [{
                "status_code": "200",
                "reference": "#/components/schemas/LoginResponse"
            }]
        });
        let request = entity(
            "ent_request",
            "api-schema://openapi.yaml#LoginRequest",
            EntityKind::ApiSchema,
            "openapi.yaml",
        );
        let response = entity(
            "ent_response",
            "api-schema://openapi.yaml#LoginResponse",
            EntityKind::ApiSchema,
            "openapi.yaml",
        );
        let mut request_relation = relation(
            "rel_request",
            RelationKind::SchemaForRequest,
            &endpoint,
            &request,
        );
        request_relation.payload = json!({
            "schema_use": {"reference": "#/components/schemas/LoginRequest"}
        });
        let mut response_relation = relation(
            "rel_response",
            RelationKind::SchemaForResponse,
            &endpoint,
            &response,
        );
        response_relation.payload = json!({
            "schema_use": {"reference": "#/components/schemas/LoginResponse"}
        });

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![endpoint.clone(), request, response],
                vec![request_relation.clone(), response_relation.clone()],
                vec![endpoint],
                vec![request_relation, response_relation],
            ))
            .await
            .unwrap();

        assert!(!diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.kind,
            DiagnosticKind::ApiRequestSchemaMismatch | DiagnosticKind::ApiResponseSchemaMismatch
        )));
    }

    fn input(
        entities: Vec<Entity>,
        relations: Vec<Relation>,
        affected_entities: Vec<Entity>,
        affected_relations: Vec<Relation>,
    ) -> CheckInput {
        CheckInput {
            snapshot: SnapshotId("snap_test".to_string()),
            entities,
            facts: Vec::new(),
            relations,
            affected: AffectedSubset::from_extracted(affected_entities, Vec::new())
                .with_relations(affected_relations),
        }
    }

    fn relation(id: &str, kind: RelationKind, from: &Entity, to: &Entity) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind,
            from: from.id.clone(),
            to: to.id.clone(),
            status: RelationStatus::Inferred,
            confidence: 0.7,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        }
    }

    fn entity(id: &str, stable_key: &str, kind: EntityKind, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: stable_key.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("test".to_string())),
            aliases: Vec::new(),
            ownership: athanor_extractor_basic::ownership_for_file(path),
            payload: json!({}),
        }
    }
}
