#[derive(Debug, Clone)]
struct OpenApiResponse {
    fields: BTreeMap<String, Shape>,
    root_type: Option<String>,
    reference: Option<String>,
    status_code: String,
    media_type: String,
}

#[derive(Debug, Clone)]
struct GraphQlResponse {
    fields: BTreeMap<String, Shape>,
    root_field: String,
    root_type: String,
    schema_name: Option<String>,
}

fn response_diagnostic(
    openapi: &Entity,
    graphql: &Entity,
    schemas: &[&Entity],
    snapshot: &SnapshotId,
    checker: &str,
) -> Option<Diagnostic> {
    let openapi_response = resolve_openapi_response(openapi, schemas)?;
    let mut candidates = resolve_graphql_responses(graphql, schemas);
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by(|left, right| left.root_field.cmp(&right.root_field));

    let mut best: Option<(usize, bool, ShapeDrift, GraphQlResponse)> = None;
    for candidate in candidates {
        let graphql_root = graphql_type_family(&candidate.root_type);
        let root_mismatch = openapi_response
            .root_type
            .as_deref()
            .is_some_and(|openapi_root| !root_types_compatible(openapi_root, &graphql_root));
        let drift = compare_shapes(
            openapi_response.fields.clone(),
            candidate.fields.clone(),
            true,
        );
        let score = usize::from(root_mismatch) + drift.mismatch_count();
        if score == 0 {
            return None;
        }
        if best.as_ref().is_none_or(|(best_score, _, _, _)| score < *best_score) {
            best = Some((score, root_mismatch, drift, candidate));
        }
    }
    let (_, root_type_mismatch, drift, selected) = best?;

    Some(parity_diagnostic(
        "api_openapi_graphql_response_schema_drift",
        "OpenAPI and GraphQL response schema drift",
        "Reconcile response container, field types, and required/nullability semantics between OpenAPI and the best matching GraphQL top-level response selection.",
        openapi,
        graphql,
        snapshot,
        checker,
        json!({
            "openapi_status_code": openapi_response.status_code,
            "openapi_media_type": openapi_response.media_type,
            "openapi_reference": openapi_response.reference,
            "openapi_root_type": openapi_response.root_type,
            "graphql_root_field": selected.root_field,
            "graphql_root_type": selected.root_type,
            "graphql_schema": selected.schema_name,
            "graphql_selection_roots": graphql.payload.get("selection_roots"),
            "root_type_mismatch": root_type_mismatch,
            "missing_in_graphql": drift.missing_in_graphql,
            "missing_in_openapi": drift.missing_in_openapi,
            "type_mismatches": drift.type_mismatches,
            "required_mismatches": drift.required_mismatches,
        }),
    ))
}

fn resolve_openapi_response(endpoint: &Entity, schemas: &[&Entity]) -> Option<OpenApiResponse> {
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
            let root_type = openapi_type_family(schema).or_else(|| {
                resolved.map(|schema| named_family(&schema.name))
            });
            let fields = openapi_response_shapes(schema, schemas, endpoint);
            return Some(OpenApiResponse {
                fields,
                root_type,
                reference: reference.map(str::to_string),
                status_code: status_code.to_string(),
                media_type: media_type.to_string(),
            });
        }
    }
    None
}

fn resolve_graphql_responses(endpoint: &Entity, schemas: &[&Entity]) -> Vec<GraphQlResponse> {
    let Some(operation_type) = endpoint
        .payload
        .get("operation_type")
        .and_then(Value::as_str)
    else {
        return Vec::new();
    };
    let selections = endpoint
        .payload
        .get("selection_roots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let root_name = custom_graphql_root_name(operation_type, schemas)
        .unwrap_or_else(|| default_graphql_root_name(operation_type).to_string());
    let Some(root_schema) = find_graphql_schema(schemas, &root_name) else {
        return Vec::new();
    };
    let members = root_schema
        .payload
        .get("member_types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    selections
        .into_iter()
        .filter_map(|root_field| {
            let root_type = members.iter().find_map(|member| {
                (member.get("name").and_then(Value::as_str) == Some(root_field))
                    .then(|| member.get("type").and_then(Value::as_str))
                    .flatten()
            })?;
            let response_name = graphql_named_type(root_type);
            let schema = (!is_graphql_scalar(response_name))
                .then(|| find_graphql_schema(schemas, response_name))
                .flatten();
            Some(GraphQlResponse {
                fields: schema.map(graphql_schema_shapes).unwrap_or_default(),
                root_field: root_field.to_string(),
                root_type: root_type.to_string(),
                schema_name: schema.map(|schema| schema.name.clone()),
            })
        })
        .collect()
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
            && schema.payload.get("schema_kind").and_then(Value::as_str) != Some("parameter")
            && schema
                .source
                .as_ref()
                .and_then(|source| normalize_repo_path(&source.path))
                .as_deref()
                == Some(target_path.as_str())
    })
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

fn endpoint_protocol(endpoint: &Entity) -> Option<&str> {
    endpoint.payload.get("protocol").and_then(Value::as_str)
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
                    (member.get("name").and_then(Value::as_str) == Some(operation_type))
                        .then(|| {
                            member
                                .get("type")
                                .and_then(Value::as_str)
                                .map(graphql_named_type)
                                .map(str::to_string)
                        })
                        .flatten()
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

fn normalize_type_name(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
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
        "integer" | "number" | "string" | "boolean" | "object" => {
            Some(type_name.to_string())
        }
        other => Some(other.to_string()),
    }
}

fn named_family(name: &str) -> String {
    format!("named:{}", normalize_type_name(name))
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

fn is_graphql_scalar(name: &str) -> bool {
    matches!(name, "Int" | "Float" | "String" | "ID" | "Boolean")
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
