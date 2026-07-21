fn authentication_diagnostic(
    openapi: &Entity,
    graphql: &Entity,
    mapping: &DirectiveMapping,
    snapshot: &SnapshotId,
    checker: &str,
) -> Option<Diagnostic> {
    let openapi_required = openapi_authentication_required(openapi);
    let graphql_required = graphql_authentication_required(graphql, mapping);
    let alternatives = openapi_security_alternatives(openapi);
    let graphql_families = graphql_auth_families(graphql, mapping);
    let comparable_families = openapi_required
        && graphql_required
        && !graphql_families.is_empty()
        && alternatives
            .iter()
            .any(|alternative| !alternative.families.is_empty());
    let family_mismatch = comparable_families
        && !alternatives.iter().any(|alternative| {
            alternative.families.is_empty()
                || alternative.families.is_subset(&graphql_families)
        });

    if openapi_required == graphql_required && !family_mismatch {
        return None;
    }

    Some(parity_diagnostic(
        "api_openapi_graphql_authentication_drift",
        "OpenAPI and GraphQL authentication requirements drift",
        "Align authentication presence and ensure at least one OpenAPI security alternative is compatible with the configured GraphQL authentication directive family.",
        openapi,
        graphql,
        snapshot,
        checker,
        json!({
            "openapi_authentication_required": openapi_required,
            "graphql_authentication_required": graphql_required,
            "openapi_security_alternatives": alternatives.iter().map(security_alternative_json).collect::<Vec<_>>(),
            "graphql_authentication_families": graphql_families,
            "authentication_family_mismatch": family_mismatch,
        }),
    ))
}

fn permission_diagnostic(
    openapi: &Entity,
    graphql: &Entity,
    mapping: &DirectiveMapping,
    snapshot: &SnapshotId,
    checker: &str,
) -> Option<Diagnostic> {
    let openapi_required = openapi_authentication_required(openapi);
    let graphql_required = graphql_authentication_required(graphql, mapping);
    if openapi_required != graphql_required || (!openapi_required && !graphql_required) {
        return None;
    }

    let alternatives = openapi_security_alternatives(openapi);
    let graphql_families = graphql_auth_families(graphql, mapping);
    let graphql_permissions = graphql_permissions(graphql, mapping);
    let mut candidates = alternatives
        .iter()
        .filter(|alternative| {
            graphql_families.is_empty()
                || alternative.families.is_empty()
                || alternative.families.is_subset(&graphql_families)
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        candidates = alternatives.iter().collect();
    }
    if candidates
        .iter()
        .any(|alternative| alternative.permissions == graphql_permissions)
    {
        return None;
    }
    if candidates.is_empty() && graphql_permissions.is_empty() {
        return None;
    }

    let closest = candidates.into_iter().min_by_key(|alternative| {
        alternative
            .permissions
            .symmetric_difference(&graphql_permissions)
            .count()
    });
    let openapi_permissions = closest
        .map(|alternative| alternative.permissions.clone())
        .unwrap_or_default();
    let missing_in_graphql = openapi_permissions
        .difference(&graphql_permissions)
        .cloned()
        .collect::<Vec<_>>();
    let missing_in_openapi = graphql_permissions
        .difference(&openapi_permissions)
        .cloned()
        .collect::<Vec<_>>();
    if missing_in_graphql.is_empty() && missing_in_openapi.is_empty() {
        return None;
    }

    Some(parity_diagnostic(
        "api_openapi_graphql_permission_drift",
        "OpenAPI security scopes and GraphQL permissions drift",
        "Align GraphQL permissions with one compatible OpenAPI security alternative rather than the union of mutually exclusive alternatives.",
        openapi,
        graphql,
        snapshot,
        checker,
        json!({
            "selected_openapi_alternative": closest.map(|alternative| alternative.index),
            "openapi_permissions": openapi_permissions,
            "graphql_permissions": graphql_permissions,
            "missing_in_graphql": missing_in_graphql,
            "missing_in_openapi": missing_in_openapi,
            "openapi_security_alternatives": alternatives.iter().map(security_alternative_json).collect::<Vec<_>>(),
        }),
    ))
}

fn openapi_authentication_required(endpoint: &Entity) -> bool {
    endpoint
        .payload
        .get("authentication_required")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            endpoint
                .payload
                .get("security_requirements")
                .and_then(Value::as_array)
                .is_some_and(|requirements| !requirements.is_empty())
        })
}

fn openapi_security_alternatives(endpoint: &Entity) -> Vec<SecurityAlternative> {
    let mut alternatives = BTreeMap::<usize, SecurityAlternative>::new();
    for requirement in endpoint
        .payload
        .get("security_requirements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let index = requirement
            .get("alternative")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or_default();
        let alternative = alternatives.entry(index).or_insert_with(|| SecurityAlternative {
            index,
            ..SecurityAlternative::default()
        });
        if let Some(family) = normalize_auth_family(
            requirement.get("kind").and_then(Value::as_str),
            requirement.get("scheme").and_then(Value::as_str),
        ) {
            alternative.families.insert(family);
        }
        alternative.permissions.extend(
            requirement
                .get("scopes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(normalize_permission)
                .filter(|value| !value.is_empty()),
        );
    }
    alternatives.into_values().collect()
}

fn security_alternative_json(alternative: &SecurityAlternative) -> Value {
    json!({
        "alternative": alternative.index,
        "families": &alternative.families,
        "permissions": &alternative.permissions,
    })
}

fn directive_mapping(endpoint: &Entity) -> DirectiveMapping {
    let mut mapping = DirectiveMapping::default();
    if let Some(config) = endpoint
        .payload
        .get("security_directive_mapping")
        .and_then(Value::as_object)
    {
        apply_mapping_object(&mut mapping, config);
    }
    for application in directive_applications(endpoint) {
        let Some(name) = application.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !MAPPING_DIRECTIVES.contains(&normalize_token(name).as_str()) {
            continue;
        }
        if let Some(arguments) = application.get("arguments").and_then(Value::as_array) {
            apply_mapping_arguments(&mut mapping, arguments);
        }
    }
    mapping
}

fn apply_mapping_object(mapping: &mut DirectiveMapping, object: &serde_json::Map<String, Value>) {
    replace_mapping_set(
        &mut mapping.authentication_directives,
        object.get("authentication_directives"),
    );
    replace_mapping_set(
        &mut mapping.permission_directives,
        object.get("permission_directives"),
    );
    replace_mapping_set(
        &mut mapping.permission_arguments,
        object.get("permission_arguments"),
    );
    replace_mapping_set(
        &mut mapping.authentication_family_arguments,
        object.get("authentication_family_arguments"),
    );
}

fn apply_mapping_arguments(mapping: &mut DirectiveMapping, arguments: &[Value]) {
    for argument in arguments {
        let Some(name) = argument.get("name").and_then(Value::as_str) else {
            continue;
        };
        let target = match normalize_token(name).as_str() {
            "authenticationdirectives" => Some(&mut mapping.authentication_directives),
            "permissiondirectives" => Some(&mut mapping.permission_directives),
            "permissionarguments" => Some(&mut mapping.permission_arguments),
            "authenticationfamilyarguments" => {
                Some(&mut mapping.authentication_family_arguments)
            }
            _ => None,
        };
        if let Some(target) = target {
            replace_mapping_set(target, argument.get("value"));
        }
    }
}

fn replace_mapping_set(target: &mut BTreeSet<String>, value: Option<&Value>) {
    let replacement = flatten_values(value)
        .into_iter()
        .map(|value| normalize_token(&value))
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if !replacement.is_empty() {
        *target = replacement;
    }
}

fn graphql_authentication_required(endpoint: &Entity, mapping: &DirectiveMapping) -> bool {
    directive_applications(endpoint).any(|application| {
        application
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| {
                mapping
                    .authentication_directives
                    .contains(&normalize_token(name))
            })
    })
}

fn graphql_auth_families(
    endpoint: &Entity,
    mapping: &DirectiveMapping,
) -> BTreeSet<String> {
    directive_applications(endpoint)
        .filter(|application| {
            application
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| {
                    mapping
                        .authentication_directives
                        .contains(&normalize_token(name))
                })
        })
        .flat_map(|application| {
            application
                .get("arguments")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|argument| {
            argument
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| {
                    mapping
                        .authentication_family_arguments
                        .contains(&normalize_token(name))
                })
        })
        .flat_map(|argument| flatten_values(argument.get("value")))
        .filter_map(|value| normalize_auth_family(Some(&value), Some(&value)))
        .collect()
}

fn graphql_permissions(endpoint: &Entity, mapping: &DirectiveMapping) -> BTreeSet<String> {
    directive_applications(endpoint)
        .filter(|application| {
            application
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| {
                    mapping
                        .permission_directives
                        .contains(&normalize_token(name))
                })
        })
        .flat_map(|application| {
            application
                .get("arguments")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|argument| {
            argument
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| {
                    mapping
                        .permission_arguments
                        .contains(&normalize_token(name))
                })
        })
        .flat_map(|argument| flatten_values(argument.get("value")))
        .map(|value| normalize_permission(&value))
        .filter(|value| !value.is_empty())
        .collect()
}

fn directive_applications(endpoint: &Entity) -> impl Iterator<Item = &Value> {
    endpoint
        .payload
        .get("directive_applications")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn flatten_values(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(value)) => vec![value.clone()],
        Some(Value::Array(values)) => values
            .iter()
            .flat_map(|value| flatten_values(Some(value)))
            .collect(),
        Some(Value::Bool(value)) => vec![value.to_string()],
        Some(Value::Number(value)) => vec![value.to_string()],
        _ => Vec::new(),
    }
}

fn normalize_auth_family(kind: Option<&str>, scheme: Option<&str>) -> Option<String> {
    let kind = kind.map(normalize_token).unwrap_or_default();
    let scheme = scheme.map(normalize_token).unwrap_or_default();
    let value = if scheme.contains("bearer")
        || scheme.contains("jwt")
        || kind.contains("bearer")
        || kind.contains("jwt")
    {
        "bearer"
    } else if scheme.contains("basic") || kind.contains("basic") {
        "basic"
    } else if kind.contains("oauth") || scheme.contains("oauth") {
        "oauth2"
    } else if kind.contains("openid") || scheme.contains("openid") {
        "openid"
    } else if kind.contains("apikey") || scheme.contains("apikey") {
        "api_key"
    } else if !scheme.is_empty() {
        scheme.as_str()
    } else if !kind.is_empty() {
        kind.as_str()
    } else {
        return None;
    };
    Some(value.to_string())
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn normalize_token(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn normalize_permission(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, ':' | '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}
