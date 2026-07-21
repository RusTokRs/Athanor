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
    fn requires_all_schemes_within_one_security_alternative() {
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
                    {"alternative": 0, "kind": "oauth2", "scheme": null, "scopes": []},
                    {"alternative": 0, "kind": "apiKey", "scheme": null, "scopes": []}
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
                    "arguments": [{"name": "type", "value": "oauth2"}]
                }]
            }),
        );
        let diagnostics = detect_openapi_graphql_security_drift(
            &[&openapi, &graphql],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert!(has_kind(
            &diagnostics,
            "api_openapi_graphql_authentication_drift"
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
