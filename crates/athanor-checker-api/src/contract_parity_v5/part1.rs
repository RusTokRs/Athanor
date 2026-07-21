use std::collections::{BTreeMap, BTreeSet};

use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityKind, Evidence,
    EvidenceStatus, Severity, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::{Value, json};

use super::normalize_endpoint_name;

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
            if let Some(diagnostic) = parameter_diagnostic(
                openapi_endpoint,
                graphql_endpoint,
                schemas,
                snapshot,
                checker,
            ) {
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
        return Some(parity_diagnostic(
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

    let parameter_names = resolved_openapi_parameters(openapi, schemas)
        .into_iter()
        .map(|parameter| parameter.name)
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

    Some(parity_diagnostic(
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct Shape {
    type_family: Option<String>,
    required: bool,
}

#[derive(Debug, Clone, Default)]
struct ShapeDrift {
    missing_in_graphql: Vec<String>,
    missing_in_openapi: Vec<String>,
    type_mismatches: Vec<Value>,
    required_mismatches: Vec<Value>,
}

impl ShapeDrift {
    fn is_empty(&self) -> bool {
        self.mismatch_count() == 0
    }

    fn mismatch_count(&self) -> usize {
        self.missing_in_graphql.len()
            + self.missing_in_openapi.len()
            + self.type_mismatches.len()
            + self.required_mismatches.len()
    }
}

#[derive(Debug, Clone)]
struct ResolvedParameter {
    name: String,
    location: String,
    required: bool,
    type_family: Option<String>,
    reference: Option<String>,
}

fn parameter_diagnostic(
    openapi: &Entity,
    graphql: &Entity,
    schemas: &[&Entity],
    snapshot: &SnapshotId,
    checker: &str,
) -> Option<Diagnostic> {
    let parameters = resolved_openapi_parameters(openapi, schemas);
    if parameters.is_empty() {
        return None;
    }
    let openapi_shapes = parameters
        .iter()
        .map(|parameter| {
            (
                parameter.name.clone(),
                Shape {
                    type_family: parameter.type_family.clone(),
                    required: parameter.required,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let graphql_shapes = graphql_variable_shapes(graphql);
    let drift = compare_shapes(openapi_shapes, graphql_shapes, false);
    if drift.is_empty() {
        return None;
    }

    Some(parity_diagnostic(
        "api_openapi_graphql_parameter_drift",
        "OpenAPI parameters and GraphQL variables drift",
        "Reconcile OpenAPI path, query, and header parameters, including repository-owned external parameter components, with GraphQL operation variables.",
        openapi,
        graphql,
        snapshot,
        checker,
        json!({
            "parameter_locations": parameters.iter().map(|parameter| {
                (parameter.name.clone(), parameter.location.clone())
            }).collect::<BTreeMap<_, _>>(),
            "parameters": parameters.iter().map(|parameter| json!({
                "name": &parameter.name,
                "location": &parameter.location,
                "required": parameter.required,
                "type_family": &parameter.type_family,
                "reference": &parameter.reference,
            })).collect::<Vec<_>>(),
            "missing_in_graphql": drift.missing_in_graphql,
            "type_mismatches": drift.type_mismatches,
            "required_mismatches": drift.required_mismatches,
        }),
    ))
}

fn resolved_openapi_parameters(endpoint: &Entity, schemas: &[&Entity]) -> Vec<ResolvedParameter> {
    endpoint
        .payload
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|parameter| resolve_parameter(endpoint, parameter, schemas))
        .filter(|parameter| matches!(parameter.location.as_str(), "path" | "query" | "header"))
        .collect()
}

fn resolve_parameter(
    endpoint: &Entity,
    parameter: &Value,
    schemas: &[&Entity],
) -> Option<ResolvedParameter> {
    let reference = parameter
        .get("reference")
        .and_then(Value::as_str)
        .map(str::to_string);
    let payload = if parameter.get("name").is_some() && parameter.get("location").is_some() {
        parameter
    } else {
        let reference = reference.as_deref()?;
        resolve_openapi_parameter_reference(endpoint, reference, schemas)?
            .payload
            .get("parameter")?
    };
    let name = payload.get("name")?.as_str()?.to_string();
    let location = payload.get("location")?.as_str()?.to_ascii_lowercase();
    let required = location == "path"
        || payload
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let type_family = payload
        .get("schema")
        .filter(|schema| !schema.is_null())
        .and_then(openapi_type_family);
    Some(ResolvedParameter {
        name,
        location,
        required,
        type_family,
        reference,
    })
}

fn resolve_openapi_parameter_reference<'a>(
    endpoint: &Entity,
    reference: &str,
    schemas: &[&'a Entity],
) -> Option<&'a Entity> {
    let (document, fragment) = reference.split_once('#')?;
    let name = fragment.strip_prefix("/components/parameters/")?;
    let source_path = endpoint.source.as_ref()?.path.as_str();
    let target_path = if document.is_empty() {
        normalize_repo_path(source_path)?
    } else {
        resolve_relative_repo_path(source_path, document)?
    };
    schemas.iter().copied().find(|schema| {
        schema.kind == EntityKind::ApiSchema
            && schema.name == name
            && schema.payload.get("schema_kind").and_then(Value::as_str) == Some("parameter")
            && schema
                .source
                .as_ref()
                .and_then(|source| normalize_repo_path(&source.path))
                .as_deref()
                == Some(target_path.as_str())
    })
}
