use std::collections::{BTreeMap, BTreeSet};

use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityKind, Evidence,
    EvidenceStatus, Severity, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::{Value, json};

use super::normalize_endpoint_name;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Shape {
    type_family: Option<String>,
    required: bool,
}

#[derive(Debug, Default)]
struct ShapeDrift {
    missing_in_graphql: Vec<String>,
    missing_in_openapi: Vec<String>,
    type_mismatches: Vec<Value>,
    required_mismatches: Vec<Value>,
}

impl ShapeDrift {
    fn is_empty(&self) -> bool {
        self.missing_in_graphql.is_empty()
            && self.missing_in_openapi.is_empty()
            && self.type_mismatches.is_empty()
            && self.required_mismatches.is_empty()
    }
}

#[derive(Debug)]
struct OpenApiResponse<'a> {
    schema: &'a Value,
    reference: Option<&'a str>,
    status_code: &'a str,
    media_type: &'a str,
    resolved_name: Option<&'a str>,
}

#[derive(Debug)]
struct GraphQlResponse<'a> {
    schema: Option<&'a Entity>,
    root_field: &'a str,
    root_type: &'a str,
}

pub(super) fn detect_openapi_graphql_contract_drift(
    endpoints: &[&Entity],
    schemas: &[&Entity],
    snapshot: &SnapshotId,
    checker: &str,
) -> Vec<Diagnostic> {
    let openapi = endpoints
        .iter()
        .copied()
        .filter(|endpoint| endpoint_protocol(endpoint) == Some("openapi"))
        .collect::<Vec<_>>();
    let graphql = endpoints
        .iter()
        .copied()
        .filter(|endpoint| endpoint_protocol(endpoint) == Some("graphql"))
        .collect::<Vec<_>>();

    let mut diagnostics = Vec::new();
    for openapi_endpoint in openapi {
        let name = normalize_endpoint_name(openapi_endpoint);
        if name.is_empty() {
            continue;
        }
        for graphql_endpoint in &graphql {
            if normalize_endpoint_name(graphql_endpoint) != name {
                continue;
            }
            if let Some(diagnostic) = external_request_diagnostic(
                openapi_endpoint,
                graphql_endpoint,
                schemas,
                snapshot,
                checker,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) =
                parameter_diagnostic(openapi_endpoint, graphql_endpoint, snapshot, checker)
            {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = response_diagnostic(
                openapi_endpoint,
                graphql_endpoint,
                schemas,
                snapshot,
                checker,
            ) {
                diagnostics.push(diagnostic);
            }
        }
    }
    diagnostics
}

fn endpoint_protocol(endpoint: &Entity) -> Option<&str> {
    endpoint.payload.get("protocol").and_then(Value::as_str)
}

fn external_request_diagnostic(
    openapi: &Entity,
    graphql: &Entity,
    schemas: &[&Entity],
    snapshot: &SnapshotId,
    checker: &str,
) -> Option<Diagnostic> {
    let external_schemas = openapi
        .payload
        .get("request_schemas")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|schema_use| {
            let reference = schema_use.get("reference")?.as_str()?;
            let (document, _) = reference.split_once('#')?;
            if document.is_empty() || document.contains("://") {
                return None;
            }
            resolve_openapi_schema_reference(openapi, reference, schemas)
                .map(|schema| (reference, schema))
        })
        .collect::<Vec<_>>();
    if external_schemas.is_empty() {
        return None;
    }

    let graphql_variables = graphql_variable_shapes(graphql);
    for (reference, openapi_schema) in &external_schemas {
        let expected_family = named_family(&openapi_schema.name);
        if !graphql_variables
            .values()
            .any(|shape| shape.type_family.as_deref() == Some(expected_family.as_str()))
        {
            continue;
        }
        let graphql_schema = find_graphql_schema(schemas, &openapi_schema.name)?;
        let drift = compare_shapes(
            openapi_schema_shapes(openapi_schema),
            graphql_schema_shapes(graphql_schema),
            true,
        );
        if drift.is_empty() {
            return None;
        }
        return Some(contract_diagnostic(
            "api_openapi_graphql_external_request_drift",
            "OpenAPI external request schema and GraphQL input drift",
            "Reconcile the repository-owned external OpenAPI request schema with the matching GraphQL input object.",
            openapi,
            graphql,
            snapshot,
            checker,
            json!({
                "comparison_mode": "named_external_input",
                "reference": reference,
                "input_type": openapi_schema.name.as_str(),
                "missing_in_graphql": drift.missing_in_graphql,
                "missing_in_openapi": drift.missing_in_openapi,
                "type_mismatches": drift.type_mismatches,
                "required_mismatches": drift.required_mismatches,
            }),
        ));
    }

    let parameter_names = openapi_parameter_shapes(openapi)
        .into_keys()
        .collect::<BTreeSet<_>>();
    let graphql_body_variables = graphql_variables
        .into_iter()
        .filter(|(name, _)| !parameter_names.contains(name))
        .collect::<BTreeMap<_, _>>();
    let openapi_body = external_schemas
        .iter()
        .flat_map(|(_, schema)| openapi_schema_shapes(schema))
        .collect::<BTreeMap<_, _>>();
    let drift = compare_shapes(openapi_body, graphql_body_variables, true);
    if drift.is_empty() {
        return None;
    }

    Some(contract_diagnostic(
        "api_openapi_graphql_external_request_drift",
        "OpenAPI external request schema and GraphQL variables drift",
        "Reconcile the repository-owned external OpenAPI request schema with GraphQL variables.",
        openapi,
        graphql,
        snapshot,
        checker,
        json!({
            "comparison_mode": "flattened_external_request",
            "references": external_schemas
                .iter()
                .map(|(reference, _)| *reference)
                .collect::<Vec<_>>(),
            "missing_in_graphql": drift.missing_in_graphql,
            "missing_in_openapi": drift.missing_in_openapi,
            "type_mismatches": drift.type_mismatches,
            "required_mismatches": drift.required_mismatches,
        }),
    ))
}

fn parameter_diagnostic(
    openapi: &Entity,
    graphql: &Entity,
    snapshot: &SnapshotId,
    checker: &str,
) -> Option<Diagnostic> {
    let openapi_parameters = openapi_parameter_shapes(openapi);
    if openapi_parameters.is_empty() {
        return None;
    }
    let graphql_variables = graphql_variable_shapes(graphql);
    let locations = openapi
        .payload
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|parameter| {
            let name = parameter.get("name")?.as_str()?.to_string();
            let location = parameter.get("location")?.as_str()?.to_string();
            matches!(location.as_str(), "path" | "query" | "header").then_some((name, location))
        })
        .collect::<BTreeMap<_, _>>();
    let drift = compare_shapes(openapi_parameters, graphql_variables, false);
    if drift.is_empty() {
        return None;
    }

    Some(contract_diagnostic(
        "api_openapi_graphql_parameter_drift",
        "OpenAPI parameters and GraphQL variables drift",
        "Reconcile OpenAPI path, query, and header parameters with GraphQL operation variables.",
        openapi,
        graphql,
        snapshot,
        checker,
        json!({
            "parameter_locations": locations,
            "missing_in_graphql": drift.missing_in_graphql,
            "type_mismatches": drift.type_mismatches,
            "required_mismatches": drift.required_mismatches,
        }),
    ))
}

fn response_diagnostic(
    openapi: &Entity,
    graphql: &Entity,
    schemas: &[&Entity],
    snapshot: &SnapshotId,
    checker: &str,
) -> Option<Diagnostic> {
    let openapi_response = resolve_openapi_response(openapi, schemas)?;
    let graphql_response = resolve_graphql_response(graphql, schemas)?;

    let openapi_root = openapi_type_family(openapi_response.schema)
        .or_else(|| openapi_response.resolved_name.map(named_family));
    let graphql_root = Some(graphql_type_family(graphql_response.root_type));
    let root_type_mismatch = match (openapi_root.as_deref(), graphql_root.as_deref()) {
        (Some(openapi_type), Some(graphql_type)) => {
            !root_types_compatible(openapi_type, graphql_type)
        }
        _ => false,
    };

    let openapi_fields = openapi_response_shapes(openapi_response.schema, schemas, openapi);
    let graphql_fields = graphql_response
        .schema
        .map(graphql_schema_shapes)
        .unwrap_or_default();
    let drift = compare_shapes(openapi_fields, graphql_fields, true);
    if !root_type_mismatch && drift.is_empty() {
        return None;
    }

    Some(contract_diagnostic(
        "api_openapi_graphql_response_schema_drift",
        "OpenAPI and GraphQL response schema drift",
        "Reconcile response container, field types, and required/nullability semantics between OpenAPI and GraphQL.",
        openapi,
        graphql,
        snapshot,
        checker,
        json!({
            "openapi_status_code": openapi_response.status_code,
            "openapi_media_type": openapi_response.media_type,
            "openapi_reference": openapi_response.reference,
            "openapi_root_type": openapi_root,
            "graphql_root_field": graphql_response.root_field,
            "graphql_root_type": graphql_response.root_type,
            "graphql_schema": graphql_response.schema.map(|schema| schema.name.as_str()),
            "root_type_mismatch": root_type_mismatch,
            "missing_in_graphql": drift.missing_in_graphql,
            "missing_in_openapi": drift.missing_in_openapi,
            "type_mismatches": drift.type_mismatches,
            "required_mismatches": drift.required_mismatches,
        }),
    ))
}

fn compare_shapes(
    openapi: BTreeMap<String, Shape>,
    graphql: BTreeMap<String, Shape>,
    report_graphql_extras: bool,
) -> ShapeDrift {
    let openapi_names = openapi.keys().cloned().collect::<BTreeSet<_>>();
    let graphql_names = graphql.keys().cloned().collect::<BTreeSet<_>>();
    let missing_in_graphql = openapi_names.difference(&graphql_names).cloned().collect();
    let missing_in_openapi = if report_graphql_extras {
        graphql_names.difference(&openapi_names).cloned().collect()
    } else {
        Vec::new()
    };
    let mut type_mismatches = Vec::new();
    let mut required_mismatches = Vec::new();

    for name in openapi_names.intersection(&graphql_names) {
        let openapi_shape = openapi.get(name).expect("OpenAPI shape");
        let graphql_shape = graphql.get(name).expect("GraphQL shape");
        if let (Some(openapi_type), Some(graphql_type)) = (
            openapi_shape.type_family.as_deref(),
            graphql_shape.type_family.as_deref(),
        ) && !root_types_compatible(openapi_type, graphql_type)
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

    ShapeDrift {
        missing_in_graphql,
        missing_in_openapi,
        type_mismatches,
        required_mismatches,
    }
}

fn openapi_parameter_shapes(endpoint: &Entity) -> BTreeMap<String, Shape> {
    endpoint
        .payload
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|parameter| {
            let name = parameter.get("name")?.as_str()?.to_string();
            let location = parameter.get("location")?.as_str()?;
            if !matches!(location, "path" | "query" | "header") {
                return None;
            }
            let required = location == "path"
                || parameter
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            let type_family = parameter
                .get("schema")
                .filter(|schema| !schema.is_null())
                .and_then(openapi_type_family);
            Some((
                name,
                Shape {
                    type_family,
                    required,
                },
            ))
        })
        .collect()
}

fn graphql_variable_shapes(endpoint: &Entity) -> BTreeMap<String, Shape> {
    endpoint
        .payload
        .get("variable_definitions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|definition| {
            let name = definition.get("name")?.as_str()?.to_string();
            let type_name = definition.get("type")?.as_str()?;
            Some((
                name,
                Shape {
                    type_family: Some(graphql_type_family(type_name)),
                    required: type_name.trim().ends_with('!'),
                },
            ))
        })
        .collect()
}

fn openapi_schema_shapes(schema: &Entity) -> BTreeMap<String, Shape> {
    schema
        .payload
        .get("schema")
        .map(raw_openapi_schema_shapes)
        .unwrap_or_default()
}

fn raw_openapi_schema_shapes(schema: &Value) -> BTreeMap<String, Shape> {
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    schema
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, property)| {
            (
                name.clone(),
                Shape {
                    type_family: openapi_type_family(property),
                    required: required.contains(name.as_str()),
                },
            )
        })
        .collect()
}

fn graphql_schema_shapes(schema: &Entity) -> BTreeMap<String, Shape> {
    schema
        .payload
        .get("member_types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|member| {
            let name = member.get("name")?.as_str()?.to_string();
            let type_name = member.get("type")?.as_str()?;
            Some((
                name,
                Shape {
                    type_family: Some(graphql_type_family(type_name)),
                    required: type_name.trim().ends_with('!'),
                },
            ))
        })
        .collect()
}

fn resolve_openapi_response<'a>(
    endpoint: &'a Entity,
    schemas: &[&'a Entity],
) -> Option<OpenApiResponse<'a>> {
    let uses = endpoint
        .payload
        .get("response_schemas")
        .and_then(Value::as_array)?;
    for prefer_success in [true, false] {
        for schema_use in uses {
            let status_code = schema_use
                .get("status_code")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if prefer_success != status_code.starts_with('2') {
                continue;
            }
            let media_type = schema_use
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let reference = schema_use.get("reference").and_then(Value::as_str);
            let resolved = reference.and_then(|reference| {
                resolve_openapi_schema_reference(endpoint, reference, schemas)
            });
            if reference.is_some() && resolved.is_none() {
                continue;
            }
            let schema = schema_use
                .get("schema")
                .filter(|schema| !schema.is_null())
                .or_else(|| resolved.and_then(|schema| schema.payload.get("schema")))?;
            return Some(OpenApiResponse {
                schema,
                reference,
                status_code,
                media_type,
                resolved_name: resolved.map(|schema| schema.name.as_str()),
            });
        }
    }
    None
}

fn openapi_response_shapes(
    schema: &Value,
    schemas: &[&Entity],
    endpoint: &Entity,
) -> BTreeMap<String, Shape> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
        && let Some(resolved) = resolve_openapi_schema_reference(endpoint, reference, schemas)
    {
        return openapi_schema_shapes(resolved);
    }
    if schema.get("type").and_then(Value::as_str) == Some("array")
        && let Some(items) = schema.get("items")
    {
        if let Some(reference) = items.get("$ref").and_then(Value::as_str)
            && let Some(resolved) = resolve_openapi_schema_reference(endpoint, reference, schemas)
        {
            return openapi_schema_shapes(resolved);
        }
        return raw_openapi_schema_shapes(items);
    }
    raw_openapi_schema_shapes(schema)
}

fn resolve_graphql_response<'a>(
    endpoint: &'a Entity,
    schemas: &[&'a Entity],
) -> Option<GraphQlResponse<'a>> {
    let operation_type = endpoint
        .payload
        .get("operation_type")
        .and_then(Value::as_str)?;
    let selections = endpoint
        .payload
        .get("selection_roots")
        .and_then(Value::as_array)?;
    if selections.len() != 1 {
        return None;
    }
    let root_field = selections.first()?.as_str()?;
    let root_name = custom_graphql_root_name(operation_type, schemas)
        .unwrap_or_else(|| default_graphql_root_name(operation_type).to_string());
    let root_schema = find_graphql_schema(schemas, &root_name)?;
    let root_type = root_schema
        .payload
        .get("member_types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|member| {
            if member.get("name").and_then(Value::as_str) == Some(root_field) {
                member.get("type").and_then(Value::as_str)
            } else {
                None
            }
        })?;
    let response_name = graphql_named_type(root_type);
    let schema = if is_graphql_scalar(response_name) {
        None
    } else {
        find_graphql_schema(schemas, response_name)
    };
    Some(GraphQlResponse {
        schema,
        root_field,
        root_type,
    })
}

fn custom_graphql_root_name(operation_type: &str, schemas: &[&Entity]) -> Option<String> {
    schemas
        .iter()
        .copied()
        .filter(|schema| endpoint_protocol(schema) == Some("graphql"))
        .filter(|schema| {
            schema.payload.get("schema_kind").and_then(Value::as_str) == Some("schema")
        })
        .find_map(|schema| {
            schema
                .payload
                .get("member_types")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find_map(|member| {
                    if member.get("name").and_then(Value::as_str) == Some(operation_type) {
                        member
                            .get("type")
                            .and_then(Value::as_str)
                            .map(graphql_named_type)
                            .map(str::to_string)
                    } else {
                        None
                    }
                })
        })
}

fn default_graphql_root_name(operation_type: &str) -> &str {
    match operation_type {
        "mutation" => "Mutation",
        "subscription" => "Subscription",
        _ => "Query",
    }
}

fn find_graphql_schema<'a>(schemas: &[&'a Entity], name: &str) -> Option<&'a Entity> {
    schemas.iter().copied().find(|schema| {
        schema.kind == EntityKind::ApiSchema
            && endpoint_protocol(schema) == Some("graphql")
            && normalize_type_name(&schema.name) == normalize_type_name(name)
    })
}

fn resolve_openapi_schema_reference<'a>(
    endpoint: &Entity,
    reference: &str,
    schemas: &[&'a Entity],
) -> Option<&'a Entity> {
    let (document, fragment) = reference.split_once('#')?;
    let name = fragment.strip_prefix("/components/schemas/")?;
    let source_path = endpoint.source.as_ref()?.path.as_str();
    let target_path = if document.is_empty() {
        normalize_repo_path(source_path)?
    } else {
        resolve_relative_repo_path(source_path, document)?
    };
    schemas.iter().copied().find(|schema| {
        schema.kind == EntityKind::ApiSchema
            && schema.name == name
            && schema
                .source
                .as_ref()
                .and_then(|source| normalize_repo_path(&source.path))
                .as_deref()
                == Some(target_path.as_str())
    })
}

fn resolve_relative_repo_path(source_path: &str, reference_path: &str) -> Option<String> {
    if reference_path.contains("://") || reference_path.starts_with('/') {
        return None;
    }
    let source = source_path.replace('\\', "/");
    let mut parts = source
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    parts.pop();
    for part in reference_path.replace('\\', "/").split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            segment => parts.push(segment.to_string()),
        }
    }
    Some(parts.join("/"))
}

fn normalize_repo_path(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    if path.contains("://") || path.starts_with('/') {
        return None;
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            segment => parts.push(segment),
        }
    }
    Some(parts.join("/"))
}

fn graphql_named_type(type_name: &str) -> &str {
    let mut value = type_name.trim();
    loop {
        value = value.trim_end_matches('!').trim();
        if let Some(inner) = value
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            value = inner.trim();
            continue;
        }
        return value;
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

fn root_types_compatible(openapi: &str, graphql: &str) -> bool {
    if openapi == graphql {
        return true;
    }
    match (
        openapi
            .strip_prefix("list<")
            .and_then(|value| value.strip_suffix('>')),
        graphql
            .strip_prefix("list<")
            .and_then(|value| value.strip_suffix('>')),
    ) {
        (Some(openapi_item), Some(graphql_item)) => {
            root_types_compatible(openapi_item, graphql_item)
        }
        (None, None) => {
            (openapi == "object" && graphql.starts_with("named:"))
                || (graphql == "object" && openapi.starts_with("named:"))
        }
        _ => false,
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

fn is_graphql_scalar(name: &str) -> bool {
    matches!(name, "Int" | "Float" | "String" | "ID" | "Boolean")
}

#[allow(clippy::too_many_arguments)]
fn contract_diagnostic(
    kind: &str,
    title: &str,
    suggested_fix: &str,
    openapi: &Entity,
    graphql: &Entity,
    snapshot: &SnapshotId,
    checker: &str,
    payload: Value,
) -> Diagnostic {
    let id_material = format!("{kind}\0{}\0{}", openapi.stable_key.0, graphql.stable_key.0);
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
        kind: DiagnosticKind::Other(kind.to_string()),
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: title.to_string(),
        message: format!(
            "OpenAPI endpoint `{}` and GraphQL operation `{}` share a normalized name but have incompatible contract shapes",
            openapi.stable_key.0, graphql.stable_key.0
        ),
        entities: vec![openapi.id.clone(), graphql.id.clone()],
        evidence,
        ownership,
        snapshot: snapshot.clone(),
        suggested_fix: Some(suggested_fix.to_string()),
        payload,
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
    fn accepts_external_named_input_and_parameter_parity() {
        let openapi_input = openapi_schema(
            "UserInput",
            "schemas/common.openapi.yaml",
            json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": {"type": "string"}
                }
            }),
        );
        let graphql_input = graphql_schema(
            "UserInput",
            "input",
            "schema.graphql",
            json!([
                {"name": "name", "type": "String!"}
            ]),
        );
        let openapi = endpoint(
            "ent_openapi",
            "api://POST:/get-user",
            "specs/service.openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "request_schemas": [{
                    "reference": "../schemas/common.openapi.yaml#/components/schemas/UserInput",
                    "schema": {"$ref": "../schemas/common.openapi.yaml#/components/schemas/UserInput"}
                }],
                "parameters": [
                    {"name": "id", "location": "path", "required": true, "schema": {"type": "string"}},
                    {"name": "trace", "location": "header", "required": false, "schema": {"type": "string"}}
                ]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_QUERY:GetUser",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "variable_definitions": [
                    {"name": "input", "type": "UserInput!"},
                    {"name": "id", "type": "ID!"},
                    {"name": "trace", "type": "String"}
                ]
            }),
        );
        let endpoints = [&openapi, &graphql];
        let schemas = [&openapi_input, &graphql_input];
        let diagnostics = detect_openapi_graphql_contract_drift(
            &endpoints,
            &schemas,
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_openapi_parameter_type_and_required_drift() {
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/get-user",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "parameters": [
                    {"name": "id", "location": "path", "required": true, "schema": {"type": "string"}},
                    {"name": "limit", "location": "query", "required": false, "schema": {"type": "integer"}}
                ]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_QUERY:GetUser",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "variable_definitions": [
                    {"name": "id", "type": "ID"},
                    {"name": "limit", "type": "String"}
                ]
            }),
        );
        let endpoints = [&openapi, &graphql];
        let diagnostics = detect_openapi_graphql_contract_drift(
            &endpoints,
            &[],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        let parameter = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.kind
                    == DiagnosticKind::Other("api_openapi_graphql_parameter_drift".to_string())
            })
            .expect("parameter diagnostic");
        assert_eq!(parameter.payload["parameter_locations"]["id"], "path");
        assert_eq!(parameter.payload["type_mismatches"][0]["name"], "limit");
        assert_eq!(parameter.payload["required_mismatches"][0]["name"], "id");
    }

    #[test]
    fn accepts_external_response_schema_compatibility() {
        let openapi_user = openapi_schema(
            "User",
            "schemas/common.openapi.yaml",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": {"type": "string"},
                    "name": {"type": "string"}
                }
            }),
        );
        let graphql_query = graphql_schema(
            "Query",
            "type",
            "schema.graphql",
            json!([{"name": "user", "type": "User!"}]),
        );
        let graphql_user = graphql_schema(
            "User",
            "type",
            "schema.graphql",
            json!([
                {"name": "id", "type": "ID!"},
                {"name": "name", "type": "String"}
            ]),
        );
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/get-user",
            "specs/service.openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "response_schemas": [{
                    "status_code": "200",
                    "media_type": "application/json",
                    "reference": "../schemas/common.openapi.yaml#/components/schemas/User",
                    "schema": {"$ref": "../schemas/common.openapi.yaml#/components/schemas/User"}
                }]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_QUERY:GetUser",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "selection_roots": ["user"]
            }),
        );
        let endpoints = [&openapi, &graphql];
        let schemas = [&openapi_user, &graphql_query, &graphql_user];
        let diagnostics = detect_openapi_graphql_contract_drift(
            &endpoints,
            &schemas,
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_response_schema_field_type_and_nullability_drift() {
        let openapi_user = openapi_schema(
            "User",
            "openapi.yaml",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": {"type": "string"},
                    "name": {"type": "string"},
                    "email": {"type": "string"}
                }
            }),
        );
        let graphql_query = graphql_schema(
            "Query",
            "type",
            "schema.graphql",
            json!([{"name": "user", "type": "User!"}]),
        );
        let graphql_user = graphql_schema(
            "User",
            "type",
            "schema.graphql",
            json!([
                {"name": "id", "type": "Int!"},
                {"name": "name", "type": "String!"},
                {"name": "age", "type": "String"}
            ]),
        );
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/get-user",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "response_schemas": [{
                    "status_code": "200",
                    "media_type": "application/json",
                    "reference": "#/components/schemas/User",
                    "schema": {"$ref": "#/components/schemas/User"}
                }]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_QUERY:GetUser",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "selection_roots": ["user"]
            }),
        );
        let endpoints = [&openapi, &graphql];
        let schemas = [&openapi_user, &graphql_query, &graphql_user];
        let diagnostics = detect_openapi_graphql_contract_drift(
            &endpoints,
            &schemas,
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        let response = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.kind
                    == DiagnosticKind::Other(
                        "api_openapi_graphql_response_schema_drift".to_string(),
                    )
            })
            .expect("response schema diagnostic");
        assert_eq!(response.payload["missing_in_graphql"], json!(["email"]));
        assert_eq!(response.payload["missing_in_openapi"], json!(["age"]));
        assert_eq!(response.payload["type_mismatches"][0]["name"], "id");
        assert_eq!(response.payload["required_mismatches"][0]["name"], "name");
    }

    fn endpoint(id: &str, stable_key: &str, path: &str, payload: Value) -> Entity {
        entity(
            id,
            stable_key,
            stable_key,
            EntityKind::ApiEndpoint,
            path,
            payload,
        )
    }

    fn openapi_schema(name: &str, path: &str, schema: Value) -> Entity {
        entity(
            &format!("ent_openapi_{name}_{path}"),
            &format!("api-schema://{path}#{name}"),
            name,
            EntityKind::ApiSchema,
            path,
            json!({"schema": schema}),
        )
    }

    fn graphql_schema(name: &str, schema_kind: &str, path: &str, member_types: Value) -> Entity {
        entity(
            &format!("ent_graphql_{name}_{path}"),
            &format!("api-schema://graphql:{path}#{name}"),
            name,
            EntityKind::ApiSchema,
            path,
            json!({
                "protocol": "graphql",
                "schema_kind": schema_kind,
                "member_types": member_types,
            }),
        )
    }

    fn entity(
        id: &str,
        stable_key: &str,
        name: &str,
        kind: EntityKind,
        path: &str,
        payload: Value,
    ) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: name.to_string(),
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
