use std::collections::{BTreeMap, BTreeSet};

use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityKind, Evidence,
    EvidenceStatus, Severity, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::{Value, json};

use super::normalize_endpoint_name;

#[derive(Debug, Clone, PartialEq, Eq)]
struct InputShape {
    type_family: Option<String>,
    required: bool,
}

#[derive(Debug)]
struct RequestDrift {
    mode: &'static str,
    input_type: Option<String>,
    missing_in_graphql: Vec<String>,
    missing_in_openapi: Vec<String>,
    type_mismatches: Vec<Value>,
    required_mismatches: Vec<Value>,
}

impl RequestDrift {
    fn is_empty(&self) -> bool {
        self.missing_in_graphql.is_empty()
            && self.missing_in_openapi.is_empty()
            && self.type_mismatches.is_empty()
            && self.required_mismatches.is_empty()
    }
}

pub(super) fn detect_openapi_graphql_request_drift(
    endpoints: &[&Entity],
    schemas: &[&Entity],
    snapshot: &SnapshotId,
    checker: &str,
) -> Vec<Diagnostic> {
    let openapi_endpoints = endpoints
        .iter()
        .copied()
        .filter(|endpoint| endpoint_protocol(endpoint) == Some("openapi"))
        .collect::<Vec<_>>();
    let graphql_endpoints = endpoints
        .iter()
        .copied()
        .filter(|endpoint| endpoint_protocol(endpoint) == Some("graphql"))
        .collect::<Vec<_>>();

    let mut diagnostics = Vec::new();
    for openapi in openapi_endpoints {
        let openapi_name = normalize_endpoint_name(openapi);
        if openapi_name.is_empty() {
            continue;
        }
        for graphql in &graphql_endpoints {
            if normalize_endpoint_name(graphql) != openapi_name {
                continue;
            }
            let Some(drift) = compare_request_inputs(openapi, graphql, schemas) else {
                continue;
            };
            if drift.is_empty() {
                continue;
            }
            diagnostics.push(request_drift_diagnostic(
                openapi, graphql, drift, snapshot, checker,
            ));
        }
    }
    diagnostics
}

fn endpoint_protocol(endpoint: &Entity) -> Option<&str> {
    endpoint.payload.get("protocol").and_then(Value::as_str)
}

fn compare_request_inputs(
    openapi: &Entity,
    graphql: &Entity,
    schemas: &[&Entity],
) -> Option<RequestDrift> {
    let openapi_schemas = openapi_request_schemas(openapi, schemas);
    if openapi_schemas.is_empty() {
        return None;
    }
    let graphql_variables = graphql_variable_shapes(graphql);

    for openapi_schema in &openapi_schemas {
        let input_name = openapi_schema.name.as_str();
        let referenced = graphql_variables.values().any(|shape| {
            shape
                .type_family
                .as_deref()
                .is_some_and(|family| family == named_family(input_name))
        });
        if !referenced {
            continue;
        }
        let graphql_schema = schemas.iter().copied().find(|schema| {
            schema.kind == EntityKind::ApiSchema
                && endpoint_protocol(schema) == Some("graphql")
                && normalize_type_name(&schema.name) == normalize_type_name(input_name)
        })?;
        return Some(compare_shapes(
            "named_input",
            Some(input_name.to_string()),
            openapi_schema_shapes(openapi_schema),
            graphql_schema_shapes(graphql_schema),
        ));
    }

    let openapi_arguments = openapi_schemas
        .into_iter()
        .flat_map(openapi_schema_shapes)
        .collect::<BTreeMap<_, _>>();
    if openapi_arguments.is_empty() {
        return None;
    }
    Some(compare_shapes(
        "flattened_arguments",
        None,
        openapi_arguments,
        graphql_variables,
    ))
}

fn compare_shapes(
    mode: &'static str,
    input_type: Option<String>,
    openapi: BTreeMap<String, InputShape>,
    graphql: BTreeMap<String, InputShape>,
) -> RequestDrift {
    let openapi_names = openapi.keys().cloned().collect::<BTreeSet<_>>();
    let graphql_names = graphql.keys().cloned().collect::<BTreeSet<_>>();
    let missing_in_graphql = openapi_names.difference(&graphql_names).cloned().collect();
    let missing_in_openapi = graphql_names.difference(&openapi_names).cloned().collect();
    let mut type_mismatches = Vec::new();
    let mut required_mismatches = Vec::new();

    for name in openapi_names.intersection(&graphql_names) {
        let openapi_shape = openapi.get(name).expect("OpenAPI shape");
        let graphql_shape = graphql.get(name).expect("GraphQL shape");
        if let (Some(openapi_type), Some(graphql_type)) = (
            openapi_shape.type_family.as_deref(),
            graphql_shape.type_family.as_deref(),
        ) && openapi_type != graphql_type
        {
            type_mismatches.push(json!({
                "name": name,
                "openapi": openapi_type,
                "graphql": graphql_type,
            }));
        }
        if openapi_shape.required != graphql_shape.required {
            required_mismatches.push(json!({
                "name": name,
                "openapi_required": openapi_shape.required,
                "graphql_required": graphql_shape.required,
            }));
        }
    }

    RequestDrift {
        mode,
        input_type,
        missing_in_graphql,
        missing_in_openapi,
        type_mismatches,
        required_mismatches,
    }
}

fn openapi_request_schemas<'a>(endpoint: &Entity, schemas: &[&'a Entity]) -> Vec<&'a Entity> {
    let source_path = endpoint.source.as_ref().map(|source| source.path.as_str());
    endpoint
        .payload
        .get("request_schemas")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|schema_use| schema_use.get("reference").and_then(Value::as_str))
        .filter_map(|reference| reference.strip_prefix("#/components/schemas/"))
        .filter_map(|name| {
            schemas.iter().copied().find(|schema| {
                schema.kind == EntityKind::ApiSchema
                    && schema.name == name
                    && schema.source.as_ref().map(|source| source.path.as_str()) == source_path
            })
        })
        .collect()
}

fn openapi_schema_shapes(schema: &Entity) -> BTreeMap<String, InputShape> {
    let Some(schema_value) = schema.payload.get("schema") else {
        return BTreeMap::new();
    };
    let required = schema_value
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    schema_value
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, property)| {
            (
                name.clone(),
                InputShape {
                    type_family: openapi_type_family(property),
                    required: required.contains(name.as_str()),
                },
            )
        })
        .collect()
}

fn graphql_variable_shapes(endpoint: &Entity) -> BTreeMap<String, InputShape> {
    endpoint
        .payload
        .get("variable_definitions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|definition| {
            let name = definition.get("name")?.as_str()?.to_string();
            let type_name = definition.get("type")?.as_str()?;
            Some((name, graphql_input_shape(type_name)))
        })
        .collect()
}

fn graphql_schema_shapes(schema: &Entity) -> BTreeMap<String, InputShape> {
    schema
        .payload
        .get("member_types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|member| {
            let name = member.get("name")?.as_str()?.to_string();
            let type_name = member.get("type")?.as_str()?;
            Some((name, graphql_input_shape(type_name)))
        })
        .collect()
}

fn graphql_input_shape(type_name: &str) -> InputShape {
    InputShape {
        type_family: Some(graphql_type_family(type_name)),
        required: type_name.trim().ends_with('!'),
    }
}

fn graphql_type_family(type_name: &str) -> String {
    let trimmed = type_name.trim().trim_end_matches('!');
    if let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return format!("list<{}>", graphql_type_family(inner));
    }
    match trimmed {
        "Int" => "integer".to_string(),
        "Float" => "number".to_string(),
        "String" | "ID" => "string".to_string(),
        "Boolean" => "boolean".to_string(),
        other => named_family(other),
    }
}

fn openapi_type_family(schema: &Value) -> Option<String> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
        && let Some(name) = reference.rsplit('/').next()
    {
        return Some(named_family(name));
    }
    let type_name = schema.get("type").and_then(|value| {
        value.as_str().or_else(|| {
            value
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .find(|candidate| *candidate != "null")
        })
    })?;
    match type_name {
        "array" => schema
            .get("items")
            .and_then(openapi_type_family)
            .map(|item| format!("list<{item}>")),
        "integer" | "number" | "string" | "boolean" | "object" => Some(type_name.to_string()),
        other => Some(other.to_string()),
    }
}

fn named_family(name: &str) -> String {
    format!("named:{}", normalize_type_name(name))
}

fn normalize_type_name(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn request_drift_diagnostic(
    openapi: &Entity,
    graphql: &Entity,
    drift: RequestDrift,
    snapshot: &SnapshotId,
    checker: &str,
) -> Diagnostic {
    let id_material = format!(
        "api_openapi_graphql_request_drift\0{}\0{}\0{}",
        openapi.stable_key.0, graphql.stable_key.0, drift.mode
    );
    let mut evidence = vec![entity_evidence(openapi, checker)];
    if graphql.source.is_some() {
        evidence.push(entity_evidence(graphql, checker));
    }
    let mut ownership = openapi.ownership.clone();
    for owner in &graphql.ownership {
        if !ownership
            .iter()
            .any(|existing| existing.source_file == owner.source_file)
        {
            ownership.push(owner.clone());
        }
    }
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_api_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: DiagnosticKind::Other("api_openapi_graphql_request_drift".to_string()),
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "OpenAPI and GraphQL request input drift".to_string(),
        message: format!(
            "OpenAPI endpoint `{}` and GraphQL operation `{}` share a normalized name but have incompatible request inputs",
            openapi.stable_key.0, graphql.stable_key.0
        ),
        entities: vec![openapi.id.clone(), graphql.id.clone()],
        evidence,
        ownership,
        snapshot: snapshot.clone(),
        suggested_fix: Some(format!(
            "Reconcile request arguments and input types between `{}` and `{}`",
            openapi.stable_key.0, graphql.stable_key.0
        )),
        payload: json!({
            "openapi_endpoint": openapi.stable_key.0,
            "graphql_endpoint": graphql.stable_key.0,
            "comparison_mode": drift.mode,
            "input_type": drift.input_type,
            "missing_in_graphql": drift.missing_in_graphql,
            "missing_in_openapi": drift.missing_in_openapi,
            "type_mismatches": drift.type_mismatches,
            "required_mismatches": drift.required_mismatches,
        }),
    }
}

fn entity_evidence(entity: &Entity, checker: &str) -> Evidence {
    Evidence {
        source_file: entity.source.as_ref().map(|source| source.path.clone()),
        line_start: entity.source.as_ref().and_then(|source| source.line_start),
        line_end: entity.source.as_ref().and_then(|source| source.line_end),
        extractor: Some(checker.to_string()),
        commit_hash: None,
        confidence: 0.8,
        status: EvidenceStatus::Conflicting,
    }
}

#[cfg(test)]
mod tests {
    use athanor_domain::{EntityId, LanguageCode, SourceLocation, StableKey};

    use super::*;

    #[test]
    fn reports_flattened_request_argument_type_and_required_drift() {
        let openapi_schema = schema(
            "UserInput",
            "openapi.yaml",
            json!({
                "schema": {
                    "type": "object",
                    "required": ["id"],
                    "properties": {
                        "id": {"type": "string"},
                        "limit": {"type": "integer"},
                        "includeEmail": {"type": "boolean"}
                    }
                }
            }),
        );
        let openapi = endpoint(
            "ent_openapi",
            "api://POST:/get-user",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "request_schemas": [{"reference": "#/components/schemas/UserInput"}]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_QUERY:GetUser",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_name": "GetUser",
                "variable_definitions": [
                    {"name": "id", "type": "ID"},
                    {"name": "limit", "type": "String"},
                    {"name": "extra", "type": "Boolean"}
                ]
            }),
        );
        let entities = [&openapi, &graphql];
        let schemas = [&openapi_schema];
        let diagnostics = detect_openapi_graphql_request_drift(
            &entities,
            &schemas,
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert_eq!(diagnostics.len(), 1);
        let payload = &diagnostics[0].payload;
        assert_eq!(payload["comparison_mode"], "flattened_arguments");
        assert_eq!(payload["missing_in_graphql"], json!(["includeEmail"]));
        assert_eq!(payload["missing_in_openapi"], json!(["extra"]));
        assert_eq!(payload["type_mismatches"][0]["name"], "limit");
        assert_eq!(payload["required_mismatches"][0]["name"], "id");
    }

    #[test]
    fn accepts_equivalent_named_input_object_shapes() {
        let openapi_schema = schema(
            "UpdateUserInput",
            "openapi.yaml",
            json!({
                "schema": {
                    "type": "object",
                    "required": ["id", "tags"],
                    "properties": {
                        "id": {"type": "string"},
                        "tags": {"type": "array", "items": {"type": "string"}},
                        "score": {"type": "number"}
                    }
                }
            }),
        );
        let graphql_schema = schema(
            "UpdateUserInput",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "schema_kind": "input",
                "member_types": [
                    {"name": "id", "type": "ID!"},
                    {"name": "tags", "type": "[String]!"},
                    {"name": "score", "type": "Float"}
                ]
            }),
        );
        let openapi = endpoint(
            "ent_openapi",
            "api://POST:/update-user",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "updateUser",
                "request_schemas": [{"reference": "#/components/schemas/UpdateUserInput"}]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_MUTATION:UpdateUser",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_name": "UpdateUser",
                "variable_definitions": [{"name": "input", "type": "UpdateUserInput!"}]
            }),
        );
        let entities = [&openapi, &graphql];
        let schemas = [&openapi_schema, &graphql_schema];
        let diagnostics = detect_openapi_graphql_request_drift(
            &entities,
            &schemas,
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_named_input_member_drift() {
        let openapi_schema = schema(
            "UpdateUserInput",
            "openapi.yaml",
            json!({
                "schema": {
                    "type": "object",
                    "required": ["id"],
                    "properties": {
                        "id": {"type": "string"},
                        "email": {"type": "string"}
                    }
                }
            }),
        );
        let graphql_schema = schema(
            "UpdateUserInput",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "schema_kind": "input",
                "member_types": [
                    {"name": "id", "type": "Int!"},
                    {"name": "name", "type": "String"}
                ]
            }),
        );
        let openapi = endpoint(
            "ent_openapi",
            "api://POST:/update-user",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "updateUser",
                "request_schemas": [{"reference": "#/components/schemas/UpdateUserInput"}]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_MUTATION:UpdateUser",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_name": "UpdateUser",
                "variable_definitions": [{"name": "input", "type": "UpdateUserInput!"}]
            }),
        );
        let entities = [&openapi, &graphql];
        let schemas = [&openapi_schema, &graphql_schema];
        let diagnostics = detect_openapi_graphql_request_drift(
            &entities,
            &schemas,
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert_eq!(diagnostics.len(), 1);
        let payload = &diagnostics[0].payload;
        assert_eq!(payload["comparison_mode"], "named_input");
        assert_eq!(payload["input_type"], "UpdateUserInput");
        assert_eq!(payload["missing_in_graphql"], json!(["email"]));
        assert_eq!(payload["missing_in_openapi"], json!(["name"]));
        assert_eq!(payload["type_mismatches"][0]["name"], "id");
    }

    fn endpoint(id: &str, stable_key: &str, path: &str, payload: Value) -> Entity {
        entity(id, stable_key, EntityKind::ApiEndpoint, path, payload)
    }

    fn schema(name: &str, path: &str, payload: Value) -> Entity {
        let mut entity = entity(
            &format!("ent_schema_{name}_{path}"),
            &format!("api-schema://{path}#{name}"),
            EntityKind::ApiSchema,
            path,
            payload,
        );
        entity.name = name.to_string();
        entity
    }

    fn entity(id: &str, stable_key: &str, kind: EntityKind, path: &str, payload: Value) -> Entity {
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
            ownership: Vec::new(),
            payload,
        }
    }
}
