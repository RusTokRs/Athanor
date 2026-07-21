from __future__ import annotations

from pathlib import Path


def replace_exact(path: str, old: str, new: str, *, count: int = 1) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    actual = text.count(old)
    if actual != count:
        raise SystemExit(f"{path}: expected {count} occurrences, found {actual}: {old[:80]!r}")
    target.write_text(text.replace(old, new), encoding="utf-8")


def insert_before(path: str, marker: str, addition: str) -> None:
    replace_exact(path, marker, addition + marker)


OPENAPI = "crates/athanor-extractor-openapi/src/implementation.rs"
GRAPHQL = "crates/athanor-extractor-graphql/src/lib.rs"
CHECKER = "crates/athanor-checker-api/src/lib.rs"
CHECKER_DOC = "docs/adapters/checker-api.md"
ROADMAP = "docs/development/roadmap-status.md"
PLAN = "athanor_implementation_plan_ru.md"

replace_exact(
    OPENAPI,
    '''        let component_parameters = root
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("parameters"))
            .and_then(Value::as_object);
''',
    '''        let components = root.get("components").and_then(Value::as_object);
        let component_parameters = components
            .and_then(|components| components.get("parameters"))
            .and_then(Value::as_object);
        let security_schemes = components
            .and_then(|components| components.get("securitySchemes"))
            .and_then(Value::as_object);
        let root_security = root.get("security");
''',
)

replace_exact(
    OPENAPI,
    '''                        component_parameters,
                        line,
''',
    '''                        component_parameters,
                        root_security,
                        security_schemes,
                        line,
''',
)

replace_exact(
    OPENAPI,
    '''    component_parameters: Option<&Map<String, Value>>,
    line: Option<u32>,
''',
    '''    component_parameters: Option<&Map<String, Value>>,
    root_security: Option<&Value>,
    security_schemes: Option<&Map<String, Value>>,
    line: Option<u32>,
''',
)

replace_exact(
    OPENAPI,
    '''    let parameters = operation_parameters(path_parameters, operation, component_parameters);
    let path_parameter_count = array_len(path_parameters);
''',
    '''    let parameters = operation_parameters(path_parameters, operation, component_parameters);
    let path_parameter_count = array_len(path_parameters);
    let effective_security = operation.get("security").or(root_security);
    let security_requirements =
        normalize_security_requirements(effective_security, security_schemes);
    let authentication_required = authentication_required(effective_security);
''',
)

replace_exact(
    OPENAPI,
    '''            "response_schemas": response_schemas,
            "security": operation.get("security").cloned(),
''',
    '''            "response_schemas": response_schemas,
            "security": effective_security.cloned(),
            "security_requirements": security_requirements,
            "authentication_required": authentication_required,
''',
)

insert_before(
    OPENAPI,
    "fn request_schema_references(operation: &Map<String, Value>) -> Vec<Value> {\n",
    r'''fn authentication_required(security: Option<&Value>) -> bool {
    security
        .and_then(Value::as_array)
        .is_some_and(|requirements| {
            !requirements.is_empty()
                && requirements.iter().all(|requirement| {
                    requirement
                        .as_object()
                        .is_some_and(|requirement| !requirement.is_empty())
                })
        })
}

fn normalize_security_requirements(
    security: Option<&Value>,
    security_schemes: Option<&Map<String, Value>>,
) -> Vec<Value> {
    let mut output = Vec::new();
    for (alternative, requirement) in security
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let Some(requirement) = requirement.as_object() else {
            continue;
        };
        for (scheme_name, scopes) in requirement {
            let definition = security_schemes.and_then(|schemes| schemes.get(scheme_name));
            let kind = definition
                .and_then(|definition| definition.get("type"))
                .and_then(Value::as_str);
            let scheme = definition
                .and_then(|definition| definition.get("scheme"))
                .and_then(Value::as_str);
            let bearer_format = definition
                .and_then(|definition| definition.get("bearerFormat"))
                .and_then(Value::as_str);
            let mut scopes = scopes
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            scopes.sort();
            scopes.dedup();
            output.push(json!({
                "alternative": alternative,
                "scheme_name": scheme_name,
                "kind": kind,
                "scheme": scheme,
                "bearer_format": bearer_format,
                "scopes": scopes,
            }));
        }
    }
    output
}

''',
)

insert_before(
    OPENAPI,
    "    #[tokio::test]\n    async fn extracts_json_openapi_documents() {\n",
    r'''    #[tokio::test]
    async fn extracts_effective_security_requirements_and_scopes() {
        let output = OpenApiExtractor
            .extract(input(
                "openapi.yaml",
                r#"openapi: 3.1.0
info: { title: Users, version: '1' }
security:
  - oauth: [users:read]
paths:
  /users:
    get:
      operationId: getUsers
      responses:
        '200': { description: ok }
    post:
      operationId: createUser
      security: []
      responses:
        '201': { description: created }
components:
  securitySchemes:
    oauth:
      type: oauth2
      flows:
        clientCredentials:
          tokenUrl: /token
          scopes:
            users:read: read users
"#,
            ))
            .await
            .unwrap();

        let get_users = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "api://GET:/users")
            .unwrap();
        assert_eq!(get_users.payload["authentication_required"], true);
        assert_eq!(
            get_users.payload["security_requirements"],
            json!([{
                "alternative": 0,
                "scheme_name": "oauth",
                "kind": "oauth2",
                "scheme": null,
                "bearer_format": null,
                "scopes": ["users:read"]
            }])
        );

        let create_user = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "api://POST:/users")
            .unwrap();
        assert_eq!(create_user.payload["authentication_required"], false);
        assert_eq!(create_user.payload["security_requirements"], json!([]));
    }

''',
)

replace_exact(
    GRAPHQL,
    '''                    declaration.arguments.clone(),
                    declaration.directives.clone(),
                ),
''',
    '''                    declaration.arguments.clone(),
                    declaration.directives.clone(),
                    declaration.directive_applications.clone(),
                ),
''',
)

replace_exact(
    GRAPHQL,
    '''    directives: Vec<String>,
    deprecation_reason: Option<String>,
''',
    '''    directives: Vec<String>,
    directive_applications: Vec<Value>,
    deprecation_reason: Option<String>,
''',
)

replace_exact(
    GRAPHQL,
    '''                directives: graphql_directive_names(line),
                deprecation_reason:''',
    '''                directives: graphql_directive_names(line),
                directive_applications: graphql_directive_application_values(line),
                deprecation_reason:''',
    count=3,
)

replace_exact(
    GRAPHQL,
    '''                inline_type_conditions: Vec::new(),
                directives: Vec::new(),
                deprecation_reason: None,
''',
    '''                inline_type_conditions: Vec::new(),
                directives: Vec::new(),
                directive_applications: Vec::new(),
                deprecation_reason: None,
''',
)

replace_exact(
    GRAPHQL,
    '''    arguments: Vec<String>,
    directives: Vec<String>,
) -> Entity {
''',
    '''    arguments: Vec<String>,
    directives: Vec<String>,
    directive_applications: Vec<Value>,
) -> Entity {
''',
)

replace_exact(
    GRAPHQL,
    '''            "inline_type_conditions": inline_type_conditions,
            "directives": directives,
''',
    '''            "inline_type_conditions": inline_type_conditions,
            "directives": directives,
            "directive_applications": directive_applications,
''',
)

insert_before(
    GRAPHQL,
    "fn graphql_directive_applications(line: &str) -> Vec<(String, Vec<String>)> {\n",
    r'''fn graphql_directive_application_values(line: &str) -> Vec<Value> {
    line.split('@')
        .skip(1)
        .filter_map(|after_at| {
            let trimmed = after_at.trim_start();
            let name = leading_graphql_name(trimmed)?;
            let rest = trimmed[name.len()..].trim_start();
            let arguments = rest
                .strip_prefix('(')
                .and_then(|after_open| after_open.split_once(')'))
                .map(|(inside, _)| graphql_directive_argument_values(inside))
                .unwrap_or_default();
            Some(json!({
                "name": name,
                "arguments": arguments,
            }))
        })
        .take(64)
        .collect()
}

fn graphql_directive_argument_values(input: &str) -> Vec<Value> {
    split_graphql_value_list(input)
        .into_iter()
        .filter_map(|argument| {
            let (name, value) = argument.split_once(':')?;
            let name = leading_graphql_name(name.trim())?;
            Some(json!({
                "name": name,
                "value": graphql_directive_value(value.trim()),
            }))
        })
        .take(64)
        .collect()
}

fn graphql_directive_value(value: &str) -> Value {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return Value::Array(
            split_graphql_value_list(inner)
                .into_iter()
                .map(|value| graphql_directive_value(value.trim()))
                .collect(),
        );
    }
    if let Some(value) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Value::String(value.to_string());
    }
    match value {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(value.to_string()),
    }
}

fn split_graphql_value_list(input: &str) -> Vec<&str> {
    let mut output = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in input.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
                continue;
            }
            match character {
                '\\' => escaped = true,
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match character {
            '"' => quoted = true,
            '[' | '{' | '(' => depth += 1,
            ']' | '}' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let value = input[start..index].trim();
                if !value.is_empty() {
                    output.push(value);
                }
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    let value = input[start..].trim();
    if !value.is_empty() {
        output.push(value);
    }
    output
}

''',
)

replace_exact(
    GRAPHQL,
    '''                && entity.payload["directives"] == json!(["auth"])
                && entity.payload["fragment_spreads"] == json!(["UserFields"])
''',
    '''                && entity.payload["directives"] == json!(["auth"])
                && entity.payload["directive_applications"]
                    == json!([{"name": "auth", "arguments": []}])
                && entity.payload["fragment_spreads"] == json!(["UserFields"])
''',
)

insert_before(
    GRAPHQL,
    "    #[tokio::test]\n    async fn extracts_graphql_introspection_schema_types() {\n",
    r'''    #[tokio::test]
    async fn extracts_graphql_directive_argument_values() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"query GetUser @auth(type: OAUTH2, scopes: ["users:read", "users:write"], role: ADMIN) {
  user { id }
}"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let operation = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::ApiEndpoint)
            .unwrap();
        assert_eq!(
            operation.payload["directive_applications"],
            json!([{
                "name": "auth",
                "arguments": [
                    {"name": "type", "value": "OAUTH2"},
                    {"name": "scopes", "value": ["users:read", "users:write"]},
                    {"name": "role", "value": "ADMIN"}
                ]
            }])
        );
    }

''',
)

replace_exact(
    CHECKER,
    "mod request_parity;\n",
    "mod request_parity;\nmod security_parity;\n",
)

replace_exact(
    CHECKER,
    '''        diagnostics.extend(contract_parity::detect_openapi_graphql_contract_drift(
            &endpoints,
            &schemas,
            &input.snapshot,
            self.name(),
        ));

        Ok(diagnostics)
''',
    '''        diagnostics.extend(contract_parity::detect_openapi_graphql_contract_drift(
            &endpoints,
            &schemas,
            &input.snapshot,
            self.name(),
        ));
        diagnostics.extend(security_parity::detect_openapi_graphql_security_drift(
            &endpoints,
            &input.snapshot,
            self.name(),
        ));

        Ok(diagnostics)
''',
)

replace_exact(
    CHECKER_DOC,
    '''URL references, unresolved external parameter references, and multi-root GraphQL selections remain
explicitly deferred.

Documentation is satisfied''',
    '''URL references, unresolved external parameter references, and multi-root GraphQL selections remain
explicitly deferred.

The fourth slice publishes effective OpenAPI security requirements after operation-level override or
root inheritance, including scheme kind, HTTP scheme, bearer format, and required scopes. GraphQL
operations preserve directive applications and scalar/list argument values. The checker emits
`api_openapi_graphql_status_code_drift`, `api_openapi_graphql_authentication_drift`, and
`api_openapi_graphql_permission_drift`. Status compatibility is policy-based by GraphQL operation
type; authentication directives and permission argument names use a documented bounded vocabulary.
Custom directive semantics outside that vocabulary remain deferred.

Documentation is satisfied''',
)

replace_exact(
    CHECKER_DOC,
    '''The checker is local and side-effect free. External request references, OpenAPI parameter parity,
response schema compatibility, status-code, authentication, permission, breaking-change, rollout,
and step dependency checks are deferred.
''',
    '''The checker is local and side-effect free. Remote references, unresolved external parameter
references, custom GraphQL authentication directive semantics, multi-root response selection,
breaking-change, rollout, and step dependency checks are deferred.
''',
)

replace_exact(
    ROADMAP,
    '''GraphQL operations use canonical `protocol = graphql`; OpenAPI operations use the symmetric
`protocol = openapi` boundary. The verified first `API-001` slice lets response-field drift consume
real canonical entities. The active second slice compares request-body properties with GraphQL
variables or matching named input objects after scalar, list, and required/nullability normalization.
''',
    '''GraphQL operations use canonical `protocol = graphql`; OpenAPI operations use the symmetric
`protocol = openapi` boundary. Request bodies, parameters, repository-owned external references, and
response schema shapes are implemented. The active fourth `API-001` slice adds effective OpenAPI
security requirements, GraphQL directive argument values, and status/authentication/permission drift.
''',
)

replace_exact(
    PLAN,
    '''- [-] Четвёртый bounded slice активен: status-code policy, effective authentication requirements и permission scope drift.
''',
    '''- [-] Четвёртый bounded slice реализует status-code policy, effective OpenAPI authentication requirements, GraphQL directive argument values и permission scope drift; targeted matrix pending.
''',
)

replace_exact(
    PLAN,
    '''| `API-001` | P1 | `[-] in progress` | Contract shapes implemented; status/auth/permission slice active |
''',
    '''| `API-001` | P1 | `[-] in progress` | Contract shapes and security parity implemented; targeted/standard evidence pending |
''',
)

replace_exact(
    PLAN,
    '''Завершить четвёртый bounded `API-001` slice: опубликовать effective OpenAPI security requirements,
GraphQL directive argument values и deterministic status/auth/permission diagnostics.''',
    '''Подтвердить четвёртый bounded `API-001` slice targeted и standard matrix, затем оценить
полный Definition of Done пакета `API-001`.''',
)
