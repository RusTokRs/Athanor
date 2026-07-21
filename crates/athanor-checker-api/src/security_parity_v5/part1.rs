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
