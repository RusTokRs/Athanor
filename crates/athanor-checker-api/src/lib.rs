use std::collections::{HashMap, HashSet, hash_map::Entry};

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
        let examples = entities_of_kind(&input.entities, EntityKind::ApiExample);
        let schemas = entities_of_kind(&input.entities, EntityKind::ApiSchema);
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
                    RelationKind::Documents
                        | RelationKind::DocumentsApi
                        | RelationKind::DocumentsOperation
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
                            | RelationKind::Documents
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

        let examples_affected = input
            .affected
            .entities
            .iter()
            .any(|entity| entity.kind == EntityKind::ApiExample);
        if examples_affected || schemas_affected {
            diagnostics.extend(validate_examples(
                &examples,
                &schemas,
                &affected_ids,
                schemas_affected,
                self.name(),
                &input.snapshot,
            ));
        }

        Ok(diagnostics)
    }
}

fn validate_examples(
    examples: &[&Entity],
    schemas: &[&Entity],
    affected_ids: &HashSet<EntityId>,
    schemas_affected: bool,
    checker: &str,
    snapshot: &SnapshotId,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut validators = HashMap::<String, jsonschema::Validator>::new();

    for example in examples {
        if !schemas_affected && !affected_ids.contains(&example.id) {
            continue;
        }
        let Some(instance) = example.payload.get("value") else {
            continue;
        };
        let Some(schema) = example
            .payload
            .get("schema")
            .filter(|schema| !schema.is_null())
        else {
            continue;
        };
        if example
            .payload
            .get("schema_reference")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|reference| !reference.starts_with("#/components/schemas/"))
        {
            continue;
        }

        let validation_schema = validation_schema(example, schema, schemas);
        let cache_key = serde_json::to_string(&validation_schema).unwrap_or_default();
        let validator = match validators.entry(cache_key) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let version = example
                    .payload
                    .get("openapi_version")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("3.0.0");
                let draft = if version.starts_with("3.1.") {
                    jsonschema::Draft::Draft202012
                } else {
                    jsonschema::Draft::Draft4
                };
                match jsonschema::options()
                    .with_draft(draft)
                    .build(&validation_schema)
                {
                    Ok(validator) => entry.insert(validator),
                    Err(error) => {
                        diagnostics.push(invalid_example_diagnostic(
                            example,
                            checker,
                            snapshot,
                            vec![format!("schema compilation failed: {error}")],
                        ));
                        continue;
                    }
                }
            }
        };
        let errors = validator
            .iter_errors(instance)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            diagnostics.push(invalid_example_diagnostic(
                example, checker, snapshot, errors,
            ));
        }
    }
    diagnostics
}

fn validation_schema(
    example: &Entity,
    schema: &serde_json::Value,
    schemas: &[&Entity],
) -> serde_json::Value {
    let source_path = example.source.as_ref().map(|source| source.path.as_str());
    let components = schemas
        .iter()
        .filter(|candidate| {
            candidate.source.as_ref().map(|source| source.path.as_str()) == source_path
        })
        .filter_map(|candidate| {
            candidate
                .payload
                .get("schema")
                .cloned()
                .map(|schema| (candidate.name.clone(), schema))
        })
        .collect::<serde_json::Map<_, _>>();
    json!({
        "components": { "schemas": components },
        "allOf": [schema],
    })
}

fn invalid_example_diagnostic(
    example: &Entity,
    checker: &str,
    snapshot: &SnapshotId,
    errors: Vec<String>,
) -> Diagnostic {
    let id_material = format!("api_example_invalid\0{}", example.stable_key.0);
    let source = example.source.as_ref();
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_api_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: DiagnosticKind::ApiExampleInvalid,
        severity: Severity::High,
        status: DiagnosticStatus::Open,
        title: "OpenAPI example does not match its schema".to_string(),
        message: errors.join("; "),
        entities: vec![example.id.clone()],
        evidence: vec![Evidence {
            source_file: source.map(|source| source.path.clone()),
            line_start: source.and_then(|source| source.line_start),
            line_end: source.and_then(|source| source.line_end),
            extractor: Some(checker.to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Conflicting,
        }],
        ownership: example.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some("Update the example value or its declared OpenAPI schema.".to_string()),
        payload: json!({
            "example": example.stable_key.0,
            "endpoint": example.payload.get("endpoint"),
            "errors": errors,
        }),
    }
}

#[derive(Debug, Clone, Default)]
pub struct EnvDocsChecker;

#[async_trait]
impl Checker for EnvDocsChecker {
    fn name(&self) -> &'static str {
        "env-docs"
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let env_vars = entities_of_kind(&input.entities, EntityKind::EnvVar);
        let docs_affected = input.affected.entities.iter().any(|entity| {
            matches!(
                entity.kind,
                EntityKind::DocumentationPage | EntityKind::DocumentationSection
            )
        });
        let affected_ids = input
            .affected
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect::<HashSet<_>>();

        let mut diagnostics = Vec::new();

        for env_var in env_vars {
            let env_var_affected = affected_ids.contains(&env_var.id);
            if env_var_affected || docs_affected {
                let documented = input.relations.iter().any(|relation| {
                    relation.kind == RelationKind::Documents && relation.to == env_var.id
                });
                if !documented {
                    diagnostics.push(missing_env_var_diagnostic(
                        env_var,
                        self.name(),
                        &input.snapshot,
                    ));
                }
            }
        }

        Ok(diagnostics)
    }
}

fn missing_env_var_diagnostic(
    env_var: &Entity,
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    let kind = DiagnosticKind::MissingEnvVar;
    let kind_slug = "missing_env_var";
    let id_material = format!("{kind_slug}\0{}", env_var.stable_key.0);
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_env_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Environment variable is not documented".to_string(),
        message: format!(
            "Environment variable `{}` is used in the codebase but not documented.",
            env_var.stable_key.0
        ),
        entities: vec![env_var.id.clone()],
        evidence: vec![Evidence {
            source_file: env_var.source.as_ref().map(|s| s.path.clone()),
            line_start: env_var.source.as_ref().and_then(|s| s.line_start),
            line_end: env_var.source.as_ref().and_then(|s| s.line_end),
            extractor: Some(checker.to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Missing,
        }],
        ownership: env_var.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(format!(
            "Document `{}` in your Markdown documentation frontmatter by adding it to the `entities` list.",
            env_var.stable_key.0
        )),
        payload: json!({
            "env_var": env_var.stable_key.0,
            "missing_relation": "documents",
        }),
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
        DiagnosticKind::ApiExampleInvalid => "api_example_invalid",
        DiagnosticKind::MissingEnvVar => "missing_env_var",
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
            RelationKind::Documents,
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

    #[tokio::test]
    async fn reports_undocumented_env_var_and_accepts_documented_one() {
        let env_var = entity(
            "ent_env_db",
            "env://DATABASE_URL",
            EntityKind::EnvVar,
            "src/main.rs",
        );
        let doc_page = entity(
            "ent_doc_page",
            "doc://docs/config.md",
            EntityKind::DocumentationPage,
            "docs/config.md",
        );

        // Scenario 1: Undocumented env var
        let diagnostics = EnvDocsChecker
            .check(input(
                vec![env_var.clone()],
                Vec::new(),
                vec![env_var.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::MissingEnvVar);
        assert_eq!(diagnostics[0].entities[0], env_var.id);

        // Scenario 2: Documented env var
        let documents_rel = relation("rel_doc_env", RelationKind::Documents, &doc_page, &env_var);

        let diagnostics2 = EnvDocsChecker
            .check(input(
                vec![env_var.clone(), doc_page],
                vec![documents_rel.clone()],
                vec![env_var],
                vec![documents_rel],
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics2.len(), 0);
    }

    #[tokio::test]
    async fn reports_invalid_openapi_examples_and_accepts_valid_ones() {
        let schema = api_schema(
            "User",
            json!({
                "type": "object",
                "required": ["name"],
                "properties": {"name": {"type": "string"}}
            }),
        );
        let valid = api_example("ent_valid", json!({"name": "Alice"}));
        let invalid = api_example("ent_invalid", json!({"name": 42}));

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![schema.clone(), valid.clone(), invalid.clone()],
                Vec::new(),
                vec![schema, valid, invalid.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        let invalid_diagnostics = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.kind == DiagnosticKind::ApiExampleInvalid)
            .collect::<Vec<_>>();
        assert_eq!(invalid_diagnostics.len(), 1);
        assert_eq!(invalid_diagnostics[0].entities, vec![invalid.id]);
        assert!(!invalid_diagnostics[0].evidence.is_empty());
    }

    fn api_schema(name: &str, schema: serde_json::Value) -> Entity {
        let mut entity = entity(
            "ent_schema",
            &format!("api-schema://openapi.yaml#{name}"),
            EntityKind::ApiSchema,
            "openapi.yaml",
        );
        entity.name = name.to_string();
        entity.payload = json!({"schema": schema});
        entity
    }

    fn api_example(id: &str, value: serde_json::Value) -> Entity {
        let mut entity = entity(
            id,
            &format!("api-example://openapi.yaml#{id}"),
            EntityKind::ApiExample,
            "openapi.yaml",
        );
        entity.payload = json!({
            "openapi_version": "3.0.3",
            "endpoint": "api://POST:/users",
            "value": value,
            "schema": {"$ref": "#/components/schemas/User"},
            "schema_reference": "#/components/schemas/User"
        });
        entity
    }

    fn input(
        entities: Vec<Entity>,
        relations: Vec<Relation>,
        affected_entities: Vec<Entity>,
        affected_relations: Vec<Relation>,
    ) -> CheckInput {
        CheckInput {
            snapshot: SnapshotId("snap_test".to_string()),
            entities: entities.into(),
            facts: Vec::new().into(),
            relations: relations.into(),
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
