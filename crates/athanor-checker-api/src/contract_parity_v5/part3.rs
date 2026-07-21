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
            "OpenAPI endpoint `{}` and GraphQL operation `{}` share a normalized name but have incompatible request or response contracts",
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
    fn selects_best_matching_graphql_root_from_multi_root_operation() {
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/get-user",
            "api/openapi.yaml",
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
            "api/schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "selection_roots": ["viewer", "getUser"]
            }),
        );
        let openapi_user = schema(
            "ent_openapi_user",
            "User",
            "api/openapi.yaml",
            json!({
                "protocol": "openapi",
                "schema": {
                    "type": "object",
                    "required": ["id", "name"],
                    "properties": {"id": {"type": "string"}, "name": {"type": "string"}}
                }
            }),
        );
        let query = schema(
            "ent_query",
            "Query",
            "api/schema.graphql",
            json!({
                "protocol": "graphql",
                "schema_kind": "type",
                "member_types": [
                    {"name": "viewer", "type": "Viewer"},
                    {"name": "getUser", "type": "User"}
                ]
            }),
        );
        let user = schema(
            "ent_user",
            "User",
            "api/schema.graphql",
            json!({
                "protocol": "graphql",
                "schema_kind": "type",
                "member_types": [
                    {"name": "id", "type": "String!"},
                    {"name": "name", "type": "String!"}
                ]
            }),
        );
        let viewer = schema(
            "ent_viewer",
            "Viewer",
            "api/schema.graphql",
            json!({
                "protocol": "graphql",
                "schema_kind": "type",
                "member_types": [{"name": "id", "type": "String!"}]
            }),
        );
        let endpoints = [&openapi, &graphql];
        let schemas = [&openapi_user, &query, &user, &viewer];
        let diagnostics = detect_openapi_graphql_contract_drift(
            &endpoints,
            &schemas,
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert!(!has_kind(
            &diagnostics,
            "api_openapi_graphql_response_schema_drift"
        ));
    }

    #[test]
    fn resolves_repository_owned_external_parameter_reference() {
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/get-user",
            "api/openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "parameters": [{"reference": "parameters.yaml#/components/parameters/UserId"}]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_QUERY:GetUser",
            "api/schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "variable_definitions": [{"name": "id", "type": "String!"}]
            }),
        );
        let parameter = schema(
            "ent_parameter",
            "UserId",
            "api/parameters.yaml",
            json!({
                "protocol": "openapi",
                "schema_kind": "parameter",
                "parameter": {
                    "name": "id",
                    "location": "path",
                    "required": true,
                    "schema": {"type": "integer"}
                }
            }),
        );
        let diagnostics = detect_openapi_graphql_contract_drift(
            &[&openapi, &graphql],
            &[&parameter],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| has_diagnostic_kind(diagnostic, "api_openapi_graphql_parameter_drift"))
            .expect("external parameter mismatch must be compared");
        assert_eq!(diagnostic.payload["type_mismatches"][0]["name"], "id");
        assert_eq!(
            diagnostic.payload["parameters"][0]["reference"],
            "parameters.yaml#/components/parameters/UserId"
        );
    }

    #[test]
    fn leaves_remote_parameter_reference_outside_local_checker_boundary() {
        let openapi = endpoint(
            "ent_openapi",
            "api://GET:/get-user",
            "api/openapi.yaml",
            json!({
                "protocol": "openapi",
                "operation_id": "getUser",
                "parameters": [{"reference": "https://example.com/parameters.yaml#/components/parameters/UserId"}]
            }),
        );
        let graphql = endpoint(
            "ent_graphql",
            "api://GRAPHQL_QUERY:GetUser",
            "api/schema.graphql",
            json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "variable_definitions": [{"name": "id", "type": "String!"}]
            }),
        );
        let diagnostics = detect_openapi_graphql_contract_drift(
            &[&openapi, &graphql],
            &[],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        assert!(!has_kind(
            &diagnostics,
            "api_openapi_graphql_parameter_drift"
        ));
    }

    #[test]
    fn accepts_external_named_input_and_parameter_parity() {
        let openapi_input = openapi_schema(
            "UserInput",
            "schemas/common.openapi.yaml",
            json!({
                "type": "object",
                "required": ["name"],
                "properties": {"name": {"type": "string"}}
            }),
        );
        let graphql_input = graphql_schema(
            "UserInput",
            "input",
            "schema.graphql",
            json!([{"name": "name", "type": "String!"}]),
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
        let diagnostics = detect_openapi_graphql_contract_drift(
            &[&openapi, &graphql],
            &[&openapi_input, &graphql_input],
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
        let diagnostics = detect_openapi_graphql_contract_drift(
            &[&openapi, &graphql],
            &[],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        let parameter = diagnostics
            .iter()
            .find(|diagnostic| has_diagnostic_kind(diagnostic, "api_openapi_graphql_parameter_drift"))
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
                "properties": {"id": {"type": "string"}, "name": {"type": "string"}}
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
        let diagnostics = detect_openapi_graphql_contract_drift(
            &[&openapi, &graphql],
            &[&openapi_user, &graphql_query, &graphql_user],
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
        let diagnostics = detect_openapi_graphql_contract_drift(
            &[&openapi, &graphql],
            &[&openapi_user, &graphql_query, &graphql_user],
            &SnapshotId("snap".to_string()),
            "api-consistency",
        );
        let response = diagnostics
            .iter()
            .find(|diagnostic| has_diagnostic_kind(diagnostic, "api_openapi_graphql_response_schema_drift"))
            .expect("response schema diagnostic");
        assert_eq!(response.payload["missing_in_graphql"], json!(["email"]));
        assert_eq!(response.payload["missing_in_openapi"], json!(["age"]));
        assert_eq!(response.payload["type_mismatches"][0]["name"], "id");
        assert_eq!(response.payload["required_mismatches"][0]["name"], "name");
    }

    fn has_kind(diagnostics: &[Diagnostic], kind: &str) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| has_diagnostic_kind(diagnostic, kind))
    }

    fn has_diagnostic_kind(diagnostic: &Diagnostic, kind: &str) -> bool {
        diagnostic.kind == DiagnosticKind::Other(kind.to_string())
    }

    fn endpoint(id: &str, stable_key: &str, path: &str, payload: Value) -> Entity {
        entity(id, stable_key, EntityKind::ApiEndpoint, stable_key, path, payload)
    }

    fn openapi_schema(name: &str, path: &str, schema_payload: Value) -> Entity {
        schema(
            &format!("ent_openapi_{name}_{path}"),
            name,
            path,
            json!({"protocol": "openapi", "schema": schema_payload}),
        )
    }

    fn graphql_schema(name: &str, schema_kind: &str, path: &str, member_types: Value) -> Entity {
        schema(
            &format!("ent_graphql_{name}_{path}"),
            name,
            path,
            json!({
                "protocol": "graphql",
                "schema_kind": schema_kind,
                "member_types": member_types,
            }),
        )
    }

    fn schema(id: &str, name: &str, path: &str, payload: Value) -> Entity {
        entity(
            id,
            &format!("api-schema://{path}#{name}"),
            EntityKind::ApiSchema,
            name,
            path,
            payload,
        )
    }

    fn entity(
        id: &str,
        stable_key: &str,
        kind: EntityKind,
        name: &str,
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
