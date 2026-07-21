use std::collections::{HashMap, HashSet, hash_map::Entry};

use async_trait::async_trait;
use athanor_core::{CheckInput, Checker, CoreResult};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, Ownership, Relation, RelationKind, Severity, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::json;

mod request_parity;

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

        for endpoint in &endpoints {
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

        diagnostics.extend(detect_openapi_graphql_drift(
            &endpoints,
            &schemas,
            &input.snapshot,
            self.name(),
        ));
        diagnostics.extend(request_parity::detect_openapi_graphql_request_drift(
            &endpoints,
            &schemas,
            &input.snapshot,
            self.name(),
        ));

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

fn detect_openapi_graphql_drift(
    endpoints: &[&Entity],
    schemas: &[&Entity],
    snapshot: &SnapshotId,
    checker: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let rest_endpoints: Vec<&Entity> = endpoints
        .iter()
        .filter(|ep| {
            ep.payload
                .get("protocol")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|p| p == "openapi")
        })
        .copied()
        .collect();

    let graphql_endpoints: Vec<&Entity> = endpoints
        .iter()
        .filter(|ep| {
            ep.payload
                .get("protocol")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|p| p == "graphql")
        })
        .copied()
        .collect();

    if rest_endpoints.is_empty() || graphql_endpoints.is_empty() {
        return diagnostics;
    }

    let schemas_by_name: HashMap<&str, &Entity> =
        schemas.iter().map(|s| (s.name.as_str(), *s)).collect();

    for rest_ep in &rest_endpoints {
        let rest_name = normalize_endpoint_name(rest_ep);
        if rest_name.is_empty() {
            continue;
        }
        for graphql_ep in &graphql_endpoints {
            let graphql_name = normalize_endpoint_name(graphql_ep);
            if graphql_name.is_empty() || rest_name != graphql_name {
                continue;
            }
            let rest_fields = endpoint_response_field_names(rest_ep, &schemas_by_name);
            let graphql_fields = endpoint_response_field_names(graphql_ep, &schemas_by_name);
            if rest_fields.is_empty() || graphql_fields.is_empty() {
                continue;
            }
            let missing_in_graphql: Vec<&str> = rest_fields
                .iter()
                .filter(|f| !graphql_fields.contains(f))
                .copied()
                .collect();
            let missing_in_rest: Vec<&str> = graphql_fields
                .iter()
                .filter(|f| !rest_fields.contains(f))
                .copied()
                .collect();
            if missing_in_graphql.is_empty() && missing_in_rest.is_empty() {
                continue;
            }
            let id_material = format!(
                "api_openapi_graphql_drift\0{}\0{}",
                rest_ep.stable_key.0, graphql_ep.stable_key.0
            );
            let mut evidence = vec![Evidence {
                source_file: rest_ep.source.as_ref().map(|source| source.path.clone()),
                line_start: rest_ep.source.as_ref().and_then(|s| s.line_start),
                line_end: rest_ep.source.as_ref().and_then(|s| s.line_end),
                extractor: Some(checker.to_string()),
                commit_hash: None,
                confidence: 0.8,
                status: EvidenceStatus::Conflicting,
            }];
            if let Some(source) = &graphql_ep.source {
                evidence.push(Evidence {
                    source_file: Some(source.path.clone()),
                    line_start: source.line_start,
                    line_end: source.line_end,
                    extractor: Some(checker.to_string()),
                    commit_hash: None,
                    confidence: 0.8,
                    status: EvidenceStatus::Conflicting,
                });
            }
            let mut ownership = rest_ep.ownership.clone();
            for owner in &graphql_ep.ownership {
                if !ownership
                    .iter()
                    .any(|existing| existing.source_file == owner.source_file)
                {
                    ownership.push(owner.clone());
                }
            }
            diagnostics.push(Diagnostic {
                id: DiagnosticId(format!(
                    "diag_api_{:016x}",
                    stable_hash(id_material.as_bytes())
                )),
                kind: DiagnosticKind::Other("api_openapi_graphql_drift".to_string()),
                severity: Severity::Medium,
                status: DiagnosticStatus::Open,
                title: "OpenAPI and GraphQL response field drift".to_string(),
                message: format!(
                    "REST endpoint `{}` and GraphQL operation `{}` share a normalized name but have different response fields",
                    rest_ep.stable_key.0, graphql_ep.stable_key.0
                ),
                entities: vec![rest_ep.id.clone(), graphql_ep.id.clone()],
                evidence,
                ownership,
                snapshot: snapshot.clone(),
                suggested_fix: Some(format!(
                    "Reconcile response fields between `{}` and `{}`",
                    rest_ep.stable_key.0, graphql_ep.stable_key.0
                )),
                payload: json!({
                    "rest_endpoint": rest_ep.stable_key.0,
                    "graphql_endpoint": graphql_ep.stable_key.0,
                    "rest_protocol": rest_ep.payload.get("protocol"),
                    "graphql_operation_type": graphql_ep.payload.get("operation_type"),
                    "missing_in_graphql": missing_in_graphql,
                    "missing_in_rest": missing_in_rest,
                }),
            });
        }
    }

    diagnostics
}

fn normalize_endpoint_name(endpoint: &Entity) -> String {
    if let Some(operation_id) = endpoint
        .payload
        .get("operation_id")
        .and_then(serde_json::Value::as_str)
    {
        return normalize(operation_id);
    }
    if let Some(operation_name) = endpoint
        .payload
        .get("operation_name")
        .and_then(serde_json::Value::as_str)
    {
        return normalize(operation_name);
    }
    if let Some(path) = endpoint
        .payload
        .get("path")
        .and_then(serde_json::Value::as_str)
    {
        let last_segment = path
            .split('/')
            .rev()
            .find(|s| !s.is_empty() && !s.starts_with('{'));
        if let Some(segment) = last_segment {
            return normalize(segment);
        }
    }
    normalize(&endpoint.name)
}

fn endpoint_response_field_names<'a>(
    endpoint: &'a Entity,
    schemas_by_name: &HashMap<&'a str, &'a Entity>,
) -> Vec<&'a str> {
    let mut fields = Vec::new();
    if let Some(response_schemas) = endpoint
        .payload
        .get("response_schemas")
        .and_then(serde_json::Value::as_array)
    {
        for schema_use in response_schemas {
            if let Some(reference) = schema_use
                .get("reference")
                .and_then(serde_json::Value::as_str)
                && let Some(name) = reference.strip_prefix("#/components/schemas/")
                && let Some(schema) = schemas_by_name.get(name)
            {
                if let Some(member_types) = schema
                    .payload
                    .get("member_types")
                    .and_then(serde_json::Value::as_array)
                {
                    for member in member_types {
                        if let Some(field_name) =
                            member.get("name").and_then(serde_json::Value::as_str)
                        {
                            fields.push(field_name);
                        }
                    }
                }
                if let Some(properties) = schema
                    .payload
                    .get("schema")
                    .and_then(|s| s.get("properties"))
                    .and_then(serde_json::Value::as_object)
                {
                    for field_name in properties.keys() {
                        if !fields.contains(&field_name.as_str()) {
                            fields.push(field_name);
                        }
                    }
                }
            }
        }
    }
    if let Some(member_types) = endpoint
        .payload
        .get("member_types")
        .and_then(serde_json::Value::as_array)
    {
        for member in member_types {
            if let Some(field_name) = member.get("name").and_then(serde_json::Value::as_str)
                && !fields.contains(&field_name)
            {
                fields.push(field_name);
            }
        }
    }
    fields
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct EnvDocsChecker;

#[async_trait]
impl Checker for EnvDocsChecker {
    fn name(&self) -> &'static str {
        "env-docs"
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let env_vars = unique_entities_of_kind(&input.entities, EntityKind::EnvVar);
        let runtime_config_keys = input
            .entities
            .iter()
            .filter(|entity| is_runtime_config_key(entity))
            .collect::<Vec<_>>();
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

        for config_key in runtime_config_keys {
            let config_key_affected = affected_ids.contains(&config_key.id);
            if config_key_affected || docs_affected {
                let documented = input.relations.iter().any(|relation| {
                    relation.kind == RelationKind::Documents && relation.to == config_key.id
                });
                if !documented {
                    diagnostics.push(missing_runtime_config_key_documentation_diagnostic(
                        config_key,
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

fn missing_runtime_config_key_documentation_diagnostic(
    config_key: &Entity,
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    let kind = DiagnosticKind::MissingDocumentation;
    let kind_slug = "missing_runtime_config_key_documentation";
    let id_material = format!("{kind_slug}\0{}", config_key.stable_key.0);
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_env_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Runtime configuration key is not documented".to_string(),
        message: format!(
            "Runtime configuration key `{}` is known to the operations graph but not documented.",
            config_key.stable_key.0
        ),
        entities: vec![config_key.id.clone()],
        evidence: vec![Evidence {
            source_file: config_key.source.as_ref().map(|s| s.path.clone()),
            line_start: config_key.source.as_ref().and_then(|s| s.line_start),
            line_end: config_key.source.as_ref().and_then(|s| s.line_end),
            extractor: Some(checker.to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Missing,
        }],
        ownership: config_key.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(format!(
            "Document `{}` in Markdown frontmatter by adding it to an `entities` list.",
            config_key.stable_key.0
        )),
        payload: json!({
            "config_key": config_key.stable_key.0,
            "missing_relation": "documents",
            "scope": "env",
        }),
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScriptDocsChecker;

#[async_trait]
impl Checker for ScriptDocsChecker {
    fn name(&self) -> &'static str {
        "script-docs"
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let script_commands = unique_entities_of_kind(&input.entities, EntityKind::ScriptCommand);
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
        for script_command in script_commands {
            if !affected_ids.contains(&script_command.id) && !docs_affected {
                continue;
            }
            let documented = input.relations.iter().any(|relation| {
                relation.kind == RelationKind::Documents && relation.to == script_command.id
            });
            if !documented {
                diagnostics.push(missing_script_documentation_diagnostic(
                    script_command,
                    self.name(),
                    &input.snapshot,
                ));
            }
        }

        Ok(diagnostics)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeploymentDocsChecker;

#[async_trait]
impl Checker for DeploymentDocsChecker {
    fn name(&self) -> &'static str {
        "deployment-docs"
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let deployments = unique_entities_of_kind(&input.entities, EntityKind::DockerService);
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
        for deployment in deployments {
            if !affected_ids.contains(&deployment.id) && !docs_affected {
                continue;
            }
            let documented = input.relations.iter().any(|relation| {
                relation.kind == RelationKind::Documents && relation.to == deployment.id
            });
            if !documented {
                diagnostics.push(missing_deployment_documentation_diagnostic(
                    deployment,
                    self.name(),
                    &input.snapshot,
                ));
            }
        }

        Ok(diagnostics)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RunbookConsistencyChecker;

#[async_trait]
impl Checker for RunbookConsistencyChecker {
    fn name(&self) -> &'static str {
        "runbook-consistency"
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let runbooks = unique_entities_of_kind(&input.entities, EntityKind::Runbook);
        let affected_ids = input
            .affected
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect::<HashSet<_>>();
        let affected_stable_keys = input
            .affected
            .entities
            .iter()
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<HashSet<_>>();
        let entities_by_stable_key = input
            .entities
            .iter()
            .map(|entity| (entity.stable_key.0.as_str(), entity))
            .collect::<HashMap<_, _>>();
        let operation_steps = input
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::OperationStep)
            .collect::<Vec<_>>();

        let mut diagnostics = Vec::new();
        for runbook in runbooks {
            let targets = runbook_operation_targets(runbook);
            let target_affected = targets
                .iter()
                .any(|target| affected_stable_keys.contains(target.as_str()));
            let step_affected = input.affected.entities.iter().any(|entity| {
                entity.kind == EntityKind::OperationStep
                    && entity
                        .payload
                        .get("runbook")
                        .and_then(serde_json::Value::as_str)
                        == Some(runbook.stable_key.0.as_str())
            });
            if !affected_ids.contains(&runbook.id) && !target_affected && !step_affected {
                continue;
            }

            let operational_targets = targets
                .iter()
                .filter_map(|target| entities_by_stable_key.get(target.as_str()).copied())
                .filter(|entity| is_operational_runbook_target(&entity.kind))
                .collect::<Vec<_>>();

            if operational_targets.is_empty() {
                diagnostics.push(runbook_without_operational_target_diagnostic(
                    runbook,
                    &targets,
                    self.name(),
                    &input.snapshot,
                ));
            }

            let steps = runbook_operation_steps(runbook, &operation_steps);
            if steps.is_empty() {
                diagnostics.push(runbook_without_operation_steps_diagnostic(
                    runbook,
                    self.name(),
                    &input.snapshot,
                ));
            } else if !runbook_steps_reference_targets(&steps, &operational_targets) {
                diagnostics.push(runbook_steps_without_target_reference_diagnostic(
                    runbook,
                    &operational_targets,
                    self.name(),
                    &input.snapshot,
                ));
            } else {
                let uncovered_targets =
                    runbook_targets_without_step_references(&steps, &operational_targets);
                if !uncovered_targets.is_empty() {
                    diagnostics.push(runbook_steps_missing_target_coverage_diagnostic(
                        runbook,
                        &uncovered_targets,
                        self.name(),
                        &input.snapshot,
                    ));
                }
            }
        }

        Ok(diagnostics)
    }
}

fn missing_deployment_documentation_diagnostic(
    deployment: &Entity,
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    let kind = DiagnosticKind::MissingDocumentation;
    let kind_slug = "missing_deployment_documentation";
    let id_material = format!("{kind_slug}\0{}", deployment.stable_key.0);
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_deployment_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Deployment resource is not documented".to_string(),
        message: format!(
            "Deployment resource `{}` is known to the operations graph but not documented.",
            deployment.stable_key.0
        ),
        entities: vec![deployment.id.clone()],
        evidence: vec![Evidence {
            source_file: deployment.source.as_ref().map(|source| source.path.clone()),
            line_start: deployment
                .source
                .as_ref()
                .and_then(|source| source.line_start),
            line_end: deployment
                .source
                .as_ref()
                .and_then(|source| source.line_end),
            extractor: Some(checker.to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Missing,
        }],
        ownership: deployment.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(format!(
            "Document `{}` in Markdown frontmatter by adding it to an `entities` list.",
            deployment.stable_key.0
        )),
        payload: json!({
            "deployment": deployment.stable_key.0,
            "missing_relation": "documents",
            "scope": "deployment",
        }),
    }
}

fn runbook_without_operational_target_diagnostic(
    runbook: &Entity,
    targets: &[String],
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    let kind = DiagnosticKind::StaleDocumentation;
    let kind_slug = "runbook_missing_operational_target";
    let id_material = format!("{kind_slug}\0{}", runbook.stable_key.0);
    let evidence_status = if targets.is_empty() {
        EvidenceStatus::Missing
    } else {
        EvidenceStatus::Stale
    };
    let message = if targets.is_empty() {
        format!(
            "Runbook `{}` does not declare any operational entity targets.",
            runbook.stable_key.0
        )
    } else {
        format!(
            "Runbook `{}` does not reference any known operational entity.",
            runbook.stable_key.0
        )
    };

    Diagnostic {
        id: DiagnosticId(format!(
            "diag_runbook_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Runbook is not tied to operational knowledge".to_string(),
        message,
        entities: vec![runbook.id.clone()],
        evidence: vec![Evidence {
            source_file: runbook.source.as_ref().map(|source| source.path.clone()),
            line_start: runbook.source.as_ref().and_then(|source| source.line_start),
            line_end: runbook.source.as_ref().and_then(|source| source.line_end),
            extractor: Some(checker.to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: evidence_status,
        }],
        ownership: runbook.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(
            "Add at least one known operational stable key to the runbook frontmatter `entities` list."
                .to_string(),
        ),
        payload: json!({
            "runbook": runbook.stable_key.0,
            "operation_targets": targets,
            "scope": "runbooks",
        }),
    }
}

fn runbook_without_operation_steps_diagnostic(
    runbook: &Entity,
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    let kind = DiagnosticKind::StaleDocumentation;
    let kind_slug = "runbook_missing_operation_steps";
    let id_material = format!("{kind_slug}\0{}", runbook.stable_key.0);

    Diagnostic {
        id: DiagnosticId(format!(
            "diag_runbook_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Runbook has no operation steps".to_string(),
        message: format!(
            "Runbook `{}` does not contain any extracted operation steps.",
            runbook.stable_key.0
        ),
        entities: vec![runbook.id.clone()],
        evidence: vec![Evidence {
            source_file: runbook.source.as_ref().map(|source| source.path.clone()),
            line_start: runbook.source.as_ref().and_then(|source| source.line_start),
            line_end: runbook.source.as_ref().and_then(|source| source.line_end),
            extractor: Some(checker.to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Missing,
        }],
        ownership: runbook.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(
            "Add ordered list items to the runbook body so Athanor can extract operation steps."
                .to_string(),
        ),
        payload: json!({
            "runbook": runbook.stable_key.0,
            "missing": "operation_steps",
            "scope": "runbooks",
        }),
    }
}

fn runbook_steps_without_target_reference_diagnostic(
    runbook: &Entity,
    targets: &[&Entity],
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    let kind = DiagnosticKind::StaleDocumentation;
    let kind_slug = "runbook_steps_without_target_reference";
    let id_material = format!("{kind_slug}\0{}", runbook.stable_key.0);
    let target_keys = targets
        .iter()
        .map(|target| target.stable_key.0.clone())
        .collect::<Vec<_>>();

    Diagnostic {
        id: DiagnosticId(format!(
            "diag_runbook_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Runbook steps do not reference operational targets".to_string(),
        message: format!(
            "Runbook `{}` declares operational targets, but its extracted operation steps do not reference them.",
            runbook.stable_key.0
        ),
        entities: vec![runbook.id.clone()],
        evidence: vec![Evidence {
            source_file: runbook.source.as_ref().map(|source| source.path.clone()),
            line_start: runbook.source.as_ref().and_then(|source| source.line_start),
            line_end: runbook.source.as_ref().and_then(|source| source.line_end),
            extractor: Some(checker.to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Stale,
        }],
        ownership: runbook.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(
            "Mention at least one declared operational target stable key or name in an ordered runbook step."
                .to_string(),
        ),
        payload: json!({
            "runbook": runbook.stable_key.0,
            "operation_targets": target_keys,
            "missing": "step_target_reference",
            "scope": "runbooks",
        }),
    }
}

fn runbook_steps_missing_target_coverage_diagnostic(
    runbook: &Entity,
    targets: &[&Entity],
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    let kind = DiagnosticKind::StaleDocumentation;
    let kind_slug = "runbook_steps_missing_target_coverage";
    let target_keys = targets
        .iter()
        .map(|target| target.stable_key.0.clone())
        .collect::<Vec<_>>();
    let id_material = format!(
        "{kind_slug}\0{}\0{}",
        runbook.stable_key.0,
        target_keys.join("\0")
    );

    Diagnostic {
        id: DiagnosticId(format!(
            "diag_runbook_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Runbook steps do not cover every operational target".to_string(),
        message: format!(
            "Runbook `{}` declares operational targets that are not referenced by any extracted operation step.",
            runbook.stable_key.0
        ),
        entities: vec![runbook.id.clone()],
        evidence: vec![Evidence {
            source_file: runbook.source.as_ref().map(|source| source.path.clone()),
            line_start: runbook.source.as_ref().and_then(|source| source.line_start),
            line_end: runbook.source.as_ref().and_then(|source| source.line_end),
            extractor: Some(checker.to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Stale,
        }],
        ownership: runbook.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(
            "Mention every declared operational target stable key or name in at least one ordered runbook step."
                .to_string(),
        ),
        payload: json!({
            "runbook": runbook.stable_key.0,
            "operation_targets": target_keys,
            "missing": "step_target_coverage",
            "scope": "runbooks",
        }),
    }
}

fn missing_script_documentation_diagnostic(
    script_command: &Entity,
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    let kind = DiagnosticKind::MissingDocumentation;
    let kind_slug = "missing_script_documentation";
    let id_material = format!("{kind_slug}\0{}", script_command.stable_key.0);
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_script_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Script command is not documented".to_string(),
        message: format!(
            "Script command `{}` is known to the operations graph but not documented.",
            script_command.stable_key.0
        ),
        entities: vec![script_command.id.clone()],
        evidence: vec![Evidence {
            source_file: script_command
                .source
                .as_ref()
                .map(|source| source.path.clone()),
            line_start: script_command
                .source
                .as_ref()
                .and_then(|source| source.line_start),
            line_end: script_command
                .source
                .as_ref()
                .and_then(|source| source.line_end),
            extractor: Some(checker.to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Missing,
        }],
        ownership: script_command.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(format!(
            "Document `{}` in Markdown frontmatter by adding it to an `entities` list.",
            script_command.stable_key.0
        )),
        payload: json!({
            "script_command": script_command.stable_key.0,
            "missing_relation": "documents",
            "scope": "scripts",
        }),
    }
}

fn entities_of_kind(entities: &[Entity], kind: EntityKind) -> Vec<&Entity> {
    entities
        .iter()
        .filter(|entity| entity.kind == kind)
        .collect()
}

fn unique_entities_of_kind(entities: &[Entity], kind: EntityKind) -> Vec<&Entity> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for entity in entities.iter().filter(|entity| entity.kind == kind) {
        if seen.insert(entity.stable_key.0.clone()) {
            unique.push(entity);
        }
    }
    unique
}

fn runbook_operation_targets(runbook: &Entity) -> Vec<String> {
    runbook
        .payload
        .get("operation_targets")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect()
}

fn runbook_operation_steps<'a>(
    runbook: &Entity,
    operation_steps: &[&'a Entity],
) -> Vec<&'a Entity> {
    operation_steps
        .iter()
        .copied()
        .filter(|step| {
            step.payload
                .get("runbook")
                .and_then(serde_json::Value::as_str)
                == Some(runbook.stable_key.0.as_str())
        })
        .collect()
}

fn runbook_steps_reference_targets(steps: &[&Entity], targets: &[&Entity]) -> bool {
    targets
        .iter()
        .any(|target| runbook_steps_reference_target(steps, target))
}

fn runbook_targets_without_step_references<'a>(
    steps: &[&Entity],
    targets: &[&'a Entity],
) -> Vec<&'a Entity> {
    targets
        .iter()
        .copied()
        .filter(|target| !runbook_steps_reference_target(steps, target))
        .collect()
}

fn runbook_steps_reference_target(steps: &[&Entity], target: &Entity) -> bool {
    steps.iter().any(|step| {
        let text = operation_step_text(step).to_ascii_lowercase();
        !text.is_empty()
            && target_reference_terms(target).iter().any(|term| {
                let term = term.to_ascii_lowercase();
                !term.is_empty() && text.contains(&term)
            })
    })
}

fn operation_step_text(step: &Entity) -> String {
    step.payload
        .get("text")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            step.title
                .as_deref()
                .unwrap_or(step.name.as_str())
                .to_string()
        })
}

fn target_reference_terms(target: &Entity) -> Vec<&str> {
    let mut terms = vec![target.stable_key.0.as_str(), target.name.as_str()];
    if let Some(title) = target.title.as_deref() {
        terms.push(title);
    }
    terms.extend(target.aliases.iter().map(String::as_str));
    terms
}

fn is_runtime_config_key(entity: &Entity) -> bool {
    entity.kind == EntityKind::Feature
        && entity
            .payload
            .get("feature_kind")
            .and_then(serde_json::Value::as_str)
            == Some("runtime_config_key")
}

fn is_operational_runbook_target(kind: &EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Script
            | EntityKind::ScriptCommand
            | EntityKind::EnvVar
            | EntityKind::DbTable
            | EntityKind::DbMigration
            | EntityKind::CiJob
            | EntityKind::DockerService
            | EntityKind::Feature
            | EntityKind::Package
            | EntityKind::Dependency
    )
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
    let protocol = api_protocol(endpoint);
    let message = format!(
        "The {protocol} operation is not linked to a Rust function, method, route, or resolver."
    );
    let suggested_fix = format!(
        "Add or rename the {protocol} implementation to match the operation, or add explicit route/resolver metadata."
    );
    diagnostic(
        endpoint,
        DiagnosticKind::ApiEndpointDocumentedButNotImplemented,
        Severity::High,
        "API endpoint has no linked implementation",
        &message,
        &suggested_fix,
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
    let protocol = api_protocol(endpoint);
    let message = format!(
        "The implemented {protocol} operation is not linked to a Markdown page or section."
    );
    let suggested_fix = format!(
        "Add API documentation mentioning the {protocol} operation id, path, query, mutation, subscription, or tag."
    );
    diagnostic(
        endpoint,
        DiagnosticKind::ApiEndpointImplementedButNotDocumented,
        Severity::Medium,
        "Implemented API endpoint has no linked documentation",
        &message,
        &suggested_fix,
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
    let protocol = api_protocol(endpoint);
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
            format!(
                "The {protocol} request body references a component schema that was not linked."
            ),
        ),
        _ => (
            Severity::Medium,
            "API response references an unresolved schema",
            format!("The {protocol} response references a component schema that was not linked."),
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
        message,
        entities: vec![endpoint.id.clone()],
        evidence: vec![evidence_for_endpoint(endpoint, checker)],
        ownership: endpoint.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: Some(format!(
            "Define {reference} in components.schemas or correct the local $ref."
        )),
        payload: json!({
            "endpoint": endpoint.stable_key.0,
            "protocol": protocol,
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
            "protocol": api_protocol(endpoint),
            "missing_relation": missing_relation,
        }),
    }
}

fn api_protocol(endpoint: &Entity) -> &'static str {
    match endpoint
        .payload
        .get("protocol")
        .and_then(serde_json::Value::as_str)
    {
        Some("graphql") => "GraphQL",
        Some("openapi") => "OpenAPI",
        Some("rest") => "REST",
        Some(_) => "API",
        None => {
            if endpoint.stable_key.0.starts_with("api://GRAPHQL_") {
                "GraphQL"
            } else if endpoint.payload.get("openapi_version").is_some()
                || endpoint
                    .source
                    .as_ref()
                    .is_some_and(|source| source.path.contains("openapi"))
            {
                "OpenAPI"
            } else {
                "API"
            }
        }
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
    async fn reports_graphql_endpoint_without_resolver_as_api_contract() {
        let mut endpoint = entity(
            "ent_graphql_endpoint",
            "api://GRAPHQL_MUTATION:UpdateUser",
            EntityKind::ApiEndpoint,
            "schema.graphql",
        );
        endpoint.payload = json!({
            "protocol": "graphql",
            "operation_type": "mutation",
            "operation_name": "UpdateUser"
        });

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![endpoint.clone()],
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
        assert!(diagnostics[0].message.contains("GraphQL operation"));
        assert!(
            diagnostics[0]
                .suggested_fix
                .as_ref()
                .is_some_and(|fix| { fix.contains("route/resolver metadata") })
        );
        assert_eq!(diagnostics[0].payload["protocol"], "GraphQL");
        assert_eq!(
            diagnostics[0].payload["endpoint"],
            "api://GRAPHQL_MUTATION:UpdateUser"
        );
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
    async fn reports_undocumented_runtime_config_key_and_accepts_documented_one() {
        let mut config_key = entity(
            "ent_config_server_port",
            "config://config/app.toml#server.port",
            EntityKind::Feature,
            "config/app.toml",
        );
        config_key.name = "server.port".to_string();
        config_key.payload = json!({
            "feature_kind": "runtime_config_key",
            "key": "server.port",
            "value_kind": "number",
            "value_redacted": true,
        });
        let doc_page = entity(
            "ent_doc_page",
            "doc://docs/operations/config.md",
            EntityKind::DocumentationPage,
            "docs/operations/config.md",
        );

        let diagnostics = EnvDocsChecker
            .check(input(
                vec![config_key.clone()],
                Vec::new(),
                vec![config_key.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::MissingDocumentation);
        assert_eq!(diagnostics[0].payload["scope"], "env");
        assert_eq!(
            diagnostics[0].payload["config_key"],
            "config://config/app.toml#server.port"
        );
        assert_eq!(diagnostics[0].entities[0], config_key.id);

        let documents_rel = relation(
            "rel_doc_config",
            RelationKind::Documents,
            &doc_page,
            &config_key,
        );
        let diagnostics = EnvDocsChecker
            .check(input(
                vec![config_key.clone(), doc_page],
                vec![documents_rel.clone()],
                vec![config_key],
                vec![documents_rel],
            ))
            .await
            .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[tokio::test]
    async fn reports_undocumented_script_command_and_accepts_documented_one() {
        let script = entity(
            "ent_script_deploy",
            "script-command://Makefile#target:deploy",
            EntityKind::ScriptCommand,
            "Makefile",
        );
        let doc_page = entity(
            "ent_doc_page",
            "doc://docs/operations/deploy.md",
            EntityKind::DocumentationPage,
            "docs/operations/deploy.md",
        );

        let diagnostics = ScriptDocsChecker
            .check(input(
                vec![script.clone()],
                Vec::new(),
                vec![script.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::MissingDocumentation);
        assert_eq!(diagnostics[0].payload["scope"], "scripts");
        assert_eq!(diagnostics[0].entities[0], script.id);

        let documents_rel = relation(
            "rel_doc_script",
            RelationKind::Documents,
            &doc_page,
            &script,
        );
        let diagnostics = ScriptDocsChecker
            .check(input(
                vec![script.clone(), doc_page],
                vec![documents_rel.clone()],
                vec![script],
                vec![documents_rel],
            ))
            .await
            .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[tokio::test]
    async fn reports_undocumented_deployment_and_accepts_documented_one() {
        let deployment = entity(
            "ent_deployment_api",
            "kubernetes://k8s/deployment.yaml#Deployment:api",
            EntityKind::DockerService,
            "k8s/deployment.yaml",
        );
        let doc_page = entity(
            "ent_doc_page",
            "doc://docs/operations/deploy.md",
            EntityKind::DocumentationPage,
            "docs/operations/deploy.md",
        );

        let diagnostics = DeploymentDocsChecker
            .check(input(
                vec![deployment.clone()],
                Vec::new(),
                vec![deployment.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::MissingDocumentation);
        assert_eq!(diagnostics[0].payload["scope"], "deployment");
        assert_eq!(diagnostics[0].entities[0], deployment.id);

        let documents_rel = relation(
            "rel_doc_deployment",
            RelationKind::Documents,
            &doc_page,
            &deployment,
        );
        let diagnostics = DeploymentDocsChecker
            .check(input(
                vec![deployment.clone(), doc_page],
                vec![documents_rel.clone()],
                vec![deployment],
                vec![documents_rel],
            ))
            .await
            .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[tokio::test]
    async fn reports_runbook_without_operational_targets_and_accepts_linked_one() {
        let mut runbook = entity(
            "ent_runbook_deploy",
            "runbook://docs/operations/deploy",
            EntityKind::Runbook,
            "docs/operations/deploy.md",
        );
        runbook.payload = json!({"operation_targets": []});

        let diagnostics = RunbookConsistencyChecker
            .check(input(
                vec![runbook.clone()],
                Vec::new(),
                vec![runbook.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::StaleDocumentation
                && diagnostic.payload["scope"] == "runbooks"
                && diagnostic.payload["operation_targets"].as_array().is_some()
                && diagnostic.entities[0] == runbook.id
        }));

        let script = entity(
            "ent_script_deploy",
            "script-command://Makefile#target:deploy",
            EntityKind::ScriptCommand,
            "Makefile",
        );
        let mut script = script;
        script.name = "deploy".to_string();
        let mut step = entity(
            "ent_step_deploy",
            "runbook://docs/operations/deploy#step-1",
            EntityKind::OperationStep,
            "docs/operations/deploy.md",
        );
        step.payload = json!({
            "runbook": "runbook://docs/operations/deploy",
            "sequence": 1,
            "text": "Run deploy",
        });
        runbook.payload = json!({
            "operation_targets": ["script-command://Makefile#target:deploy"]
        });

        let diagnostics = RunbookConsistencyChecker
            .check(input(
                vec![runbook.clone(), script, step],
                Vec::new(),
                vec![runbook],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[tokio::test]
    async fn reports_runbook_steps_without_target_references() {
        let mut runbook = entity(
            "ent_runbook_deploy",
            "runbook://docs/operations/deploy",
            EntityKind::Runbook,
            "docs/operations/deploy.md",
        );
        runbook.payload = json!({
            "operation_targets": ["script-command://Makefile#target:deploy"]
        });
        let mut script = entity(
            "ent_script_deploy",
            "script-command://Makefile#target:deploy",
            EntityKind::ScriptCommand,
            "Makefile",
        );
        script.name = "deploy".to_string();
        let mut step = entity(
            "ent_step_deploy",
            "runbook://docs/operations/deploy#step-1",
            EntityKind::OperationStep,
            "docs/operations/deploy.md",
        );
        step.payload = json!({
            "runbook": "runbook://docs/operations/deploy",
            "sequence": 1,
            "text": "Notify the team",
        });

        let diagnostics = RunbookConsistencyChecker
            .check(input(
                vec![runbook.clone(), script, step],
                Vec::new(),
                vec![runbook.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::StaleDocumentation);
        assert_eq!(diagnostics[0].payload["missing"], "step_target_reference");
        assert_eq!(diagnostics[0].payload["scope"], "runbooks");
        assert_eq!(diagnostics[0].entities[0], runbook.id);
    }

    #[tokio::test]
    async fn reports_runbook_steps_that_do_not_cover_every_target() {
        let mut runbook = entity(
            "ent_runbook_deploy",
            "runbook://docs/operations/deploy",
            EntityKind::Runbook,
            "docs/operations/deploy.md",
        );
        runbook.payload = json!({
            "operation_targets": [
                "script-command://Makefile#target:deploy",
                "script-command://Makefile#target:rollback"
            ]
        });
        let mut deploy = entity(
            "ent_script_deploy",
            "script-command://Makefile#target:deploy",
            EntityKind::ScriptCommand,
            "Makefile",
        );
        deploy.name = "deploy".to_string();
        let mut rollback = entity(
            "ent_script_rollback",
            "script-command://Makefile#target:rollback",
            EntityKind::ScriptCommand,
            "Makefile",
        );
        rollback.name = "rollback".to_string();
        let mut step = entity(
            "ent_step_deploy",
            "runbook://docs/operations/deploy#step-1",
            EntityKind::OperationStep,
            "docs/operations/deploy.md",
        );
        step.payload = json!({
            "runbook": "runbook://docs/operations/deploy",
            "sequence": 1,
            "text": "Run deploy",
        });

        let diagnostics = RunbookConsistencyChecker
            .check(input(
                vec![runbook.clone(), deploy, rollback, step],
                Vec::new(),
                vec![runbook.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::StaleDocumentation);
        assert_eq!(diagnostics[0].payload["missing"], "step_target_coverage");
        assert_eq!(
            diagnostics[0].payload["operation_targets"],
            json!(["script-command://Makefile#target:rollback"])
        );
        assert_eq!(diagnostics[0].entities[0], runbook.id);
    }

    #[tokio::test]
    async fn reports_runbook_without_operation_steps() {
        let mut runbook = entity(
            "ent_runbook_deploy",
            "runbook://docs/operations/deploy",
            EntityKind::Runbook,
            "docs/operations/deploy.md",
        );
        let script = entity(
            "ent_script_deploy",
            "script-command://Makefile#target:deploy",
            EntityKind::ScriptCommand,
            "Makefile",
        );
        runbook.payload = json!({
            "operation_targets": ["script-command://Makefile#target:deploy"]
        });

        let diagnostics = RunbookConsistencyChecker
            .check(input(
                vec![runbook.clone(), script],
                Vec::new(),
                vec![runbook.clone()],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::StaleDocumentation);
        assert_eq!(diagnostics[0].payload["missing"], "operation_steps");
        assert_eq!(diagnostics[0].entities[0], runbook.id);
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

    #[tokio::test]
    async fn detects_openapi_graphql_drift_when_fields_differ() {
        let schema = api_schema(
            "User",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "name": {"type": "string"},
                    "email": {"type": "string"}
                }
            }),
        );
        let rest_endpoint = {
            let mut ep = entity(
                "ent_rest_get_user",
                "api://GET:/users/{id}",
                EntityKind::ApiEndpoint,
                "openapi.yaml",
            );
            ep.name = "get_user".to_string();
            ep.payload = json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "method": "GET",
                "path": "/users/{id}",
                "response_schemas": [{
                    "status_code": "200",
                    "media_type": "application/json",
                    "reference": "#/components/schemas/User"
                }]
            });
            ep
        };
        let graphql_endpoint = {
            let mut ep = entity(
                "ent_gql_get_user",
                "api://GRAPHQL_QUERY:GetUser",
                EntityKind::ApiEndpoint,
                "schema.graphql",
            );
            ep.name = "QUERY GetUser".to_string();
            ep.payload = json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "member_types": [
                    {"name": "id", "type": "ID!"},
                    {"name": "name", "type": "String!"}
                ]
            });
            ep
        };

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![
                    schema.clone(),
                    rest_endpoint.clone(),
                    graphql_endpoint.clone(),
                ],
                Vec::new(),
                vec![schema, rest_endpoint, graphql_endpoint],
                Vec::new(),
            ))
            .await
            .unwrap();

        let drift = diagnostics
            .iter()
            .filter(|d| d.kind == DiagnosticKind::Other("api_openapi_graphql_drift".to_string()))
            .collect::<Vec<_>>();
        assert_eq!(
            drift.len(),
            1,
            "expected one drift diagnostic, got: {:?}",
            drift
        );
        assert!(drift[0].message.contains("different response fields"));
        assert!(
            drift[0].payload["missing_in_graphql"]
                .as_array()
                .unwrap()
                .contains(&json!("email"))
        );
        assert!(drift[0].entities.len() == 2);
        assert!(!drift[0].evidence.is_empty());
    }

    #[tokio::test]
    async fn no_drift_when_rest_and_graphql_have_same_fields() {
        let schema = api_schema(
            "User",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "name": {"type": "string"}
                }
            }),
        );
        let rest_endpoint = {
            let mut ep = entity(
                "ent_rest_get_user",
                "api://GET:/users/{id}",
                EntityKind::ApiEndpoint,
                "openapi.yaml",
            );
            ep.name = "get_user".to_string();
            ep.payload = json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "method": "GET",
                "path": "/users/{id}",
                "response_schemas": [{
                    "status_code": "200",
                    "media_type": "application/json",
                    "reference": "#/components/schemas/User"
                }]
            });
            ep
        };
        let graphql_endpoint = {
            let mut ep = entity(
                "ent_gql_get_user",
                "api://GRAPHQL_QUERY:GetUser",
                EntityKind::ApiEndpoint,
                "schema.graphql",
            );
            ep.name = "QUERY GetUser".to_string();
            ep.payload = json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "member_types": [
                    {"name": "id", "type": "ID!"},
                    {"name": "name", "type": "String!"}
                ]
            });
            ep
        };

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![
                    schema.clone(),
                    rest_endpoint.clone(),
                    graphql_endpoint.clone(),
                ],
                Vec::new(),
                vec![schema, rest_endpoint, graphql_endpoint],
                Vec::new(),
            ))
            .await
            .unwrap();

        let drift = diagnostics
            .iter()
            .filter(|d| d.kind == DiagnosticKind::Other("api_openapi_graphql_drift".to_string()))
            .collect::<Vec<_>>();
        assert!(drift.is_empty(), "expected no drift when fields match");
    }

    #[tokio::test]
    async fn no_drift_when_only_one_protocol_present() {
        let schema = api_schema(
            "User",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "name": {"type": "string"}
                }
            }),
        );
        let rest_endpoint = {
            let mut ep = entity(
                "ent_rest_get_user",
                "api://GET:/users/{id}",
                EntityKind::ApiEndpoint,
                "openapi.yaml",
            );
            ep.name = "get_user".to_string();
            ep.payload = json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "method": "GET",
                "path": "/users/{id}",
                "response_schemas": [{
                    "status_code": "200",
                    "media_type": "application/json",
                    "reference": "#/components/schemas/User"
                }]
            });
            ep
        };

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![schema.clone(), rest_endpoint.clone()],
                Vec::new(),
                vec![schema, rest_endpoint],
                Vec::new(),
            ))
            .await
            .unwrap();

        let drift = diagnostics
            .iter()
            .filter(|d| d.kind == DiagnosticKind::Other("api_openapi_graphql_drift".to_string()))
            .collect::<Vec<_>>();
        assert!(drift.is_empty(), "expected no drift with single protocol");
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
