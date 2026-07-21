use std::collections::{BTreeMap, BTreeSet};

use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, Evidence, EvidenceStatus,
    Severity, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::{Value, json};

use super::normalize_endpoint_name;

const AUTH_DIRECTIVES: &[&str] = &[
    "auth",
    "authenticated",
    "authorize",
    "authorization",
    "protected",
    "requiresauth",
];

const PERMISSION_DIRECTIVES: &[&str] = &[
    "auth",
    "authorize",
    "authorization",
    "permission",
    "permissions",
    "require",
    "requires",
    "requirerole",
    "requiresrole",
    "role",
    "roles",
    "scope",
    "scopes",
];

const PERMISSION_ARGUMENTS: &[&str] = &[
    "permission",
    "permissions",
    "require",
    "requires",
    "role",
    "roles",
    "scope",
    "scopes",
];

const AUTH_FAMILY_ARGUMENTS: &[&str] = &["authentication", "provider", "scheme", "type"];
const MAPPING_DIRECTIVES: &[&str] = &["athanorsecurity", "athanorsecuritymapping"];

#[derive(Debug, Clone)]
struct DirectiveMapping {
    authentication_directives: BTreeSet<String>,
    permission_directives: BTreeSet<String>,
    permission_arguments: BTreeSet<String>,
    authentication_family_arguments: BTreeSet<String>,
}

impl Default for DirectiveMapping {
    fn default() -> Self {
        Self {
            authentication_directives: AUTH_DIRECTIVES
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            permission_directives: PERMISSION_DIRECTIVES
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            permission_arguments: PERMISSION_ARGUMENTS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            authentication_family_arguments: AUTH_FAMILY_ARGUMENTS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct SecurityAlternative {
    index: usize,
    families: BTreeSet<String>,
    permissions: BTreeSet<String>,
}

pub(super) fn detect_openapi_graphql_security_drift(
    endpoints: &[&Entity],
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
        let normalized = normalize_endpoint_name(openapi_endpoint);
        if normalized.is_empty() {
            continue;
        }
        for graphql_endpoint in &graphql {
            if normalize_endpoint_name(graphql_endpoint) != normalized {
                continue;
            }
            let mapping = directive_mapping(graphql_endpoint);
            if let Some(diagnostic) =
                status_code_diagnostic(openapi_endpoint, graphql_endpoint, snapshot, checker)
            {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = authentication_diagnostic(
                openapi_endpoint,
                graphql_endpoint,
                &mapping,
                snapshot,
                checker,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = permission_diagnostic(
                openapi_endpoint,
                graphql_endpoint,
                &mapping,
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

fn status_code_diagnostic(
    openapi: &Entity,
    graphql: &Entity,
    snapshot: &SnapshotId,
    checker: &str,
) -> Option<Diagnostic> {
    let documented = string_set(openapi.payload.get("responses"));
    if documented.is_empty() {
        return None;
    }
    let operation_type = graphql
        .payload
        .get("operation_type")
        .and_then(Value::as_str)
        .unwrap_or("query")
        .to_ascii_lowercase();

    let expected_success: &[&str] = match operation_type.as_str() {
        "mutation" => &["200", "201", "202", "204"],
        "subscription" => &["101", "200"],
        _ => &["200"],
    };
    let success_compatible = documented.iter().any(|status| {
        status == "default"
            || status.eq_ignore_ascii_case("2XX")
            || expected_success.iter().any(|expected| status == expected)
    });

    let authentication_required = openapi_authentication_required(openapi);
    let missing_auth_status_codes = if authentication_required {
        ["401", "403"]
            .into_iter()
            .filter(|status| {
                !documented.contains(*status)
                    && !documented.contains("default")
                    && !documented.contains("4XX")
            })
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if success_compatible && missing_auth_status_codes.is_empty() {
        return None;
    }

    Some(parity_diagnostic(
        "api_openapi_graphql_status_code_drift",
        "OpenAPI response status policy is incompatible with the GraphQL operation",
        "Document a compatible success response and the required authentication failure responses for the matched operation.",
        openapi,
        graphql,
        snapshot,
        checker,
        json!({
            "operation_type": operation_type,
            "documented_status_codes": documented,
            "expected_success_codes": expected_success,
            "success_compatible": success_compatible,
            "missing_auth_status_codes": missing_auth_status_codes,
        }),
    ))
}

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
                || !alternative.families.is_disjoint(&graphql_families)
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
                || !alternative.families.is_disjoint(&graphql_families)
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

#[allow(clippy::too_many_arguments)]
fn parity_diagnostic(
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
            "OpenAPI endpoint `{}` and GraphQL operation `{}` share a normalized name but have incompatible status or security contracts",
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
    use athanor_domain::{EntityId, EntityKind, LanguageCode, SourceLocation, StableKey};

    use super::*;

    #[test]
    fn accepts_matching_status_authentication_and_permissions() {
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/get-user",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "responses": ["200", "401", "403"],
                "authentication_required": true,
                "security_requirements": [{
                    "alternative": 0,
                    "scheme_name": "oauth",
                    "kind": "oauth2",
                    "scheme": null,
                    "scopes": ["users:read"]
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
                "directive_applications": [{
                    "name": "auth",
                    "arguments": [
                        {"name": "type", "value": "oauth2"},
                        {"name": "scopes", "value": ["users:read"]}
                    ]
                }]
            }),
        );
        let endpoints = [&openapi, &graphql];
        assert!(
            detect_openapi_graphql_security_drift(
                &endpoints,
                &SnapshotId("snap".to_string()),
                "api-consistency",
            )
            .is_empty()
        );
    }

    #[test]
    fn accepts_one_matching_security_alternative_without_unioning_scopes() {
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/get-user",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "responses": ["200", "401", "403"],
                "authentication_required": true,
                "security_requirements": [
                    {"alternative": 0, "kind": "oauth2", "scheme": null, "scopes": ["users:read"]},
                    {"alternative": 1, "kind": "apiKey", "scheme": null, "scopes": []}
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
                "directive_applications": [{
                    "name": "auth",
                    "arguments": [{"name": "type", "value": "apiKey"}]
                }]
            }),
        );
        let diagnostics = detect_openapi_graphql_security_drift(
            &[&openapi, &graphql],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert!(!has_kind(
            &diagnostics,
            "api_openapi_graphql_authentication_drift"
        ));
        assert!(!has_kind(
            &diagnostics,
            "api_openapi_graphql_permission_drift"
        ));
    }

    #[test]
    fn operation_mapping_directive_configures_custom_security_vocabulary() {
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/get-user",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "responses": ["200", "401", "403"],
                "authentication_required": true,
                "security_requirements": [{
                    "alternative": 0,
                    "kind": "oauth2",
                    "scheme": null,
                    "scopes": ["users:read"]
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
                "directive_applications": [
                    {
                        "name": "athanorSecurity",
                        "arguments": [
                            {"name": "authenticationDirectives", "value": ["secured"]},
                            {"name": "permissionDirectives", "value": ["secured"]},
                            {"name": "permissionArguments", "value": ["policy"]},
                            {"name": "authenticationFamilyArguments", "value": ["provider"]}
                        ]
                    },
                    {
                        "name": "secured",
                        "arguments": [
                            {"name": "provider", "value": "oauth2"},
                            {"name": "policy", "value": ["users:read"]}
                        ]
                    }
                ]
            }),
        );
        let diagnostics = detect_openapi_graphql_security_drift(
            &[&openapi, &graphql],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert!(!has_kind(
            &diagnostics,
            "api_openapi_graphql_authentication_drift"
        ));
        assert!(!has_kind(
            &diagnostics,
            "api_openapi_graphql_permission_drift"
        ));
    }

    #[test]
    fn reports_status_authentication_and_permission_drift() {
        let openapi = endpoint(
            "ent_openapi",
            "api://POST:/update-user",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "updateUser",
                "responses": ["400"],
                "authentication_required": true,
                "security_requirements": [{
                    "alternative": 0,
                    "scheme_name": "oauth",
                    "kind": "oauth2",
                    "scheme": null,
                    "scopes": ["users:write"]
                }]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_MUTATION:UpdateUser",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_type": "mutation",
                "operation_name": "UpdateUser",
                "directive_applications": [{
                    "name": "auth",
                    "arguments": [
                        {"name": "type", "value": "bearer"},
                        {"name": "roles", "value": ["admin"]}
                    ]
                }]
            }),
        );
        let diagnostics = detect_openapi_graphql_security_drift(
            &[&openapi, &graphql],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert_eq!(diagnostics.len(), 3);
        assert!(has_kind(
            &diagnostics,
            "api_openapi_graphql_status_code_drift"
        ));
        assert!(has_kind(
            &diagnostics,
            "api_openapi_graphql_authentication_drift"
        ));
        let permission = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.kind
                    == DiagnosticKind::Other("api_openapi_graphql_permission_drift".to_string())
            })
            .expect("permission diagnostic");
        assert_eq!(
            permission.payload["missing_in_graphql"],
            json!(["users:write"])
        );
        assert_eq!(permission.payload["missing_in_openapi"], json!(["admin"]));
    }

    #[test]
    fn reports_authentication_presence_drift_without_scope_noise() {
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/health",
            "openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "health",
                "responses": ["200"],
                "authentication_required": false,
                "security_requirements": []
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_QUERY:Health",
            "schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "Health",
                "directive_applications": [{
                    "name": "authenticated",
                    "arguments": []
                }]
            }),
        );
        let diagnostics = detect_openapi_graphql_security_drift(
            &[&openapi, &graphql],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].kind,
            DiagnosticKind::Other("api_openapi_graphql_authentication_drift".to_string())
        );
    }

    fn has_kind(diagnostics: &[Diagnostic], kind: &str) -> bool {
        diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::Other(kind.to_string())
        })
    }

    fn endpoint(id: &str, stable_key: &str, path: &str, payload: Value) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind: EntityKind::ApiEndpoint,
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
