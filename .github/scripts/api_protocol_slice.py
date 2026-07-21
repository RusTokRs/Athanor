#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file_path = Path(path)
    text = file_path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one target, found {count}: {old!r}")
    file_path.write_text(text.replace(old, new, 1), encoding="utf-8")


def patch_openapi_protocol() -> None:
    path = "crates/athanor-extractor-openapi/src/lib.rs"
    replace_once(
        path,
        '        payload: json!({\n            "openapi_version": source.version,\n            "parser_backend": source.parser_backend,\n            "method": method,',
        '        payload: json!({\n            "protocol": "openapi",\n            "openapi_version": source.version,\n            "parser_backend": source.parser_backend,\n            "method": method,',
    )
    replace_once(
        path,
        '        assert_eq!(endpoint.name, "login");\n        assert_eq!(endpoint.payload["responses"][0], "200");',
        '        assert_eq!(endpoint.name, "login");\n        assert_eq!(endpoint.payload["protocol"], "openapi");\n        assert_eq!(endpoint.payload["responses"][0], "200");',
    )
    replace_once(
        path,
        '        assert_eq!(output.entities[0].name, "GET /health");',
        '        assert_eq!(output.entities[0].name, "GET /health");\n        assert_eq!(output.entities[0].payload["protocol"], "openapi");',
    )
    replace_once(
        path,
        '            assert_eq!(endpoint.stable_key.0, "api://POST:/compat");\n            assert_eq!(endpoint.payload["openapi_version"], version);',
        '            assert_eq!(endpoint.stable_key.0, "api://POST:/compat");\n            assert_eq!(endpoint.payload["protocol"], "openapi");\n            assert_eq!(endpoint.payload["openapi_version"], version);',
    )


def patch_checker_dispatch() -> None:
    path = "crates/athanor-checker-api/src/lib.rs"
    replace_once(
        path,
        '                .is_some_and(|p| p != "graphql")',
        '                .is_some_and(|protocol| protocol == "openapi")',
    )
    anchor = "    fn api_schema(name: &str, schema: serde_json::Value) -> Entity {\n"
    regression = r'''    #[tokio::test]
    async fn ignores_unknown_protocols_in_openapi_graphql_drift() {
        let schema = api_schema(
            "User",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "email": {"type": "string"}
                }
            }),
        );
        let unknown_endpoint = {
            let mut endpoint = entity(
                "ent_grpc_get_user",
                "api://GRPC:GetUser",
                EntityKind::ApiEndpoint,
                "service.proto",
            );
            endpoint.name = "get_user".to_string();
            endpoint.payload = json!({
                "protocol": "grpc",
                "operation_id": "getUser",
                "response_schemas": [{
                    "reference": "#/components/schemas/User"
                }]
            });
            endpoint
        };
        let graphql_endpoint = {
            let mut endpoint = entity(
                "ent_gql_get_user",
                "api://GRAPHQL_QUERY:GetUser",
                EntityKind::ApiEndpoint,
                "schema.graphql",
            );
            endpoint.name = "QUERY GetUser".to_string();
            endpoint.payload = json!({
                "protocol": "graphql",
                "operation_type": "query",
                "operation_name": "GetUser",
                "member_types": [{"name": "id", "type": "ID!"}]
            });
            endpoint
        };

        let diagnostics = ApiConsistencyChecker
            .check(input(
                vec![
                    schema.clone(),
                    unknown_endpoint.clone(),
                    graphql_endpoint.clone(),
                ],
                Vec::new(),
                vec![schema, unknown_endpoint, graphql_endpoint],
                Vec::new(),
            ))
            .await
            .unwrap();

        assert!(
            diagnostics.iter().all(|diagnostic| diagnostic.kind
                != DiagnosticKind::Other("api_openapi_graphql_drift".to_string())),
            "unknown protocols must not be classified as OpenAPI"
        );
    }

    fn api_schema(name: &str, schema: serde_json::Value) -> Entity {
'''
    replace_once(path, anchor, regression)


def patch_docs() -> None:
    replace_once(
        "docs/adapters/extractor-graphql.md",
        "Because operations use the shared `ApiEndpoint` kind, the API consistency checker can report\nmissing GraphQL resolver/implementation links and missing operation documentation through the same\nbounded diagnostics used for OpenAPI contract operations. The diagnostics remain protocol-aware;\nOpenAPI-only example validation and deeper OpenAPI/GraphQL drift checks are still separate slices.",
        "Because operations use the shared `ApiEndpoint` kind, the API consistency checker can report\nmissing GraphQL resolver/implementation links and missing operation documentation through the same\nbounded diagnostics used for OpenAPI contract operations. OpenAPI endpoints carry `protocol = openapi`\nand GraphQL endpoints carry `protocol = graphql`; unknown or missing protocol identities are excluded\nfrom cross-protocol drift matching. The current drift slice compares response field sets for operations\nwith the same normalized name. Request/input types, authentication, status codes, and deeper schema\ncompatibility remain separate slices.",
    )
    replace_once(
        "docs/adapters/checker-api.md",
        "Reports shared API contract operations, including OpenAPI and GraphQL `ApiEndpoint` entities,\nwithout linked Rust implementations, routes, or resolvers; implemented operations without linked\nMarkdown documentation; and local request/response component `$ref` values without the\ncorresponding schema relation. Diagnostics are protocol-aware and include the inferred protocol in\ntheir payloads, while still consuming canonical entities and relations only. The checker does not\nparse source files itself.",
        "Reports shared API contract operations, including OpenAPI and GraphQL `ApiEndpoint` entities,\nwithout linked Rust implementations, routes, or resolvers; implemented operations without linked\nMarkdown documentation; and local request/response component `$ref` values without the\ncorresponding schema relation. Cross-protocol matching is fail closed: only canonical\n`protocol = openapi` and `protocol = graphql` endpoint payloads participate, while missing or unknown\nprotocol identities are not guessed from names or stable keys. Diagnostics remain protocol-aware and\nconsume canonical entities and relations only; the checker does not parse source files itself.",
    )
    replace_once(
        "docs/adapters/checker-api.md",
        "The checker is local and side-effect free. Deeper OpenAPI/GraphQL schema drift, status-code,\nauthentication, permission, breaking-change, rollout, and step dependency checks are deferred.",
        "The checker is local and side-effect free. Request/input argument and type drift, deeper schema\ncompatibility, status-code, authentication, permission, breaking-change, rollout, and step dependency\nchecks are deferred.",
    )


def patch_plan() -> None:
    path = "athanor_implementation_plan_ru.md"
    replace_once(
        path,
        "> Статус: architecture audit verified",
        "> Статус: architecture audit verified; product development active",
    )
    replace_once(
        path,
        "- `[x] implemented` — реализация опубликована, но отметка не заменяет execution evidence.\n- `[x] verified` — реализация подтверждена successful matrix на одном exact commit.",
        "- `[x] implemented` — реализация опубликована, но отметка не заменяет execution evidence.\n- `[-] in progress` — пакет начат, но его Definition of Done закрыт частично.\n- `[x] verified` — реализация подтверждена successful matrix на одном exact commit.",
    )
    old = """### 4.2 Product backlog

- [ ] deeper GraphQL/cross-protocol API consistency;
- [ ] broader relationship/framework adapters;
- [ ] richer analysis completeness reporting;
- [ ] evidence-backed documentation generation;
- [ ] release-readiness consolidation;
- [ ] i18n/concept mapping и optional semantic/vector retrieval.
"""
    new = """### 4.2 `API-001` — GraphQL and cross-protocol consistency

- [x] OpenAPI endpoints публикуют canonical `protocol = openapi`; GraphQL endpoints сохраняют `protocol = graphql`.
- [x] Cross-protocol checker принимает только explicit OpenAPI/GraphQL identities и не угадывает неизвестные protocols.
- [x] Response-field drift для совпадающих normalized operation names покрыт source regressions.
- [ ] сравнить request arguments и input types;
- [ ] расширить schema compatibility, status-code, authentication и permission drift;
- [ ] получить exact successful main matrix для полного `API-001` package.

### 4.3 Product backlog

- [ ] broader relationship/framework adapters;
- [ ] richer analysis completeness reporting;
- [ ] evidence-backed documentation generation;
- [ ] release-readiness consolidation;
- [ ] i18n/concept mapping и optional semantic/vector retrieval.
"""
    replace_once(path, old, new)
    replace_once(
        path,
        "| `VERIFY-001` | P1 | `[x] verified` | Run `29830669460` succeeded on `e9be33c01e51a0aca718ff3e9cacd0b76876e2ca` |",
        "| `VERIFY-001` | P1 | `[x] verified` | Run `29830669460` succeeded on `e9be33c01e51a0aca718ff3e9cacd0b76876e2ca` |\n| `API-001` | P1 | `[-] in progress` | Canonical protocol identity complete; deeper request/schema/auth drift pending |",
    )
    replace_once(
        path,
        "Архитектурный аудит закрыт и verified. Source fixes опубликованы, временная remediation\nинфраструктура удалена, exact CI evidence записан. Шесть planned-направлений относятся к следующему\nэтапу продуктового развития, а не к хвосту текущего аудита.",
        "Архитектурный аудит закрыт и verified. Source fixes опубликованы, временная remediation\nинфраструктура удалена, exact CI evidence записан. Product development продолжен пакетом `API-001`;\nостальные planned-направления относятся к следующим этапам, а не к хвосту архитектурного аудита.",
    )


def patch_roadmap() -> None:
    path = "docs/development/roadmap-status.md"
    old = """## Active Work

### `VERIFY-001`

The complete remediation gate is successful. The remaining architecture task is one exact successful
main verification matrix:

1. this ordinary push-CI reaches the required security, quality, feature, and coverage jobs;
2. any remaining failure is resolved from exact job logs or diagnostic artifacts;
3. `athanor/verification-matrix` reports success, or valid JSON evidence is published;
4. only packages covered by that exact SHA are promoted to verified.

Until a successful exact result exists, the architecture remains implemented, not verified.
"""
    new = """## Active Work

### `API-001`

The architecture audit is verified. Product development now continues with explicit GraphQL and
cross-protocol API consistency:

1. OpenAPI and GraphQL endpoint payloads carry canonical protocol identities;
2. cross-protocol matching fails closed for missing or unknown protocols;
3. response-field drift remains the first implemented comparison slice;
4. request/input types, schema compatibility, status codes, authentication, and permissions follow as
   bounded extensions with their own regressions and exact CI evidence.
"""
    replace_once(path, old, new)
    replace_once(
        path,
        "- deeper GraphQL and cross-protocol API consistency;\n- broader relationship and framework adapters;",
        "- broader relationship and framework adapters;",
    )


def patch_documentation_inventory() -> None:
    path = "crates/athanor-app/tests/documentation_status_inventory.rs"
    old = """    assert!(PLAN.contains("### 4.1 `VERIFY-001` — execution matrix"));

    let active_work = ROADMAP.find("## Active Work").expect("roadmap active work");
    for package in ["### `DOC-001` / `DOC-002`", "### `MCP-004`"] {
        let position = ROADMAP
            .find(package)
            .unwrap_or_else(|| panic!("roadmap omits {package}"));
        assert!(
            position < active_work,
            "completed package {package} remains active"
        );
    }
    assert!(ROADMAP[active_work..].contains("### `VERIFY-001`"));
"""
    new = """    assert!(PLAN.contains("### 4.1 `VERIFY-001` — execution matrix"));
    assert!(PLAN.contains("### 4.2 `API-001` — GraphQL and cross-protocol consistency"));

    let active_work = ROADMAP.find("## Active Work").expect("roadmap active work");
    for package in ["### `DOC-001` / `DOC-002`", "### `MCP-004`"] {
        let position = ROADMAP
            .find(package)
            .unwrap_or_else(|| panic!("roadmap omits {package}"));
        assert!(
            position < active_work,
            "completed package {package} remains active"
        );
    }
    assert!(ROADMAP[active_work..].contains("### `API-001`"));
    assert!(!ROADMAP[active_work..].contains("### `VERIFY-001`"));
"""
    replace_once(path, old, new)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--publish", action="store_true")
    args = parser.parse_args()

    patch_openapi_protocol()
    patch_checker_dispatch()
    patch_docs()
    patch_plan()
    patch_roadmap()
    patch_documentation_inventory()

    if args.publish:
        Path(".github/workflows/api-protocol-slice.yml").unlink()
        Path(".github/scripts/api_protocol_slice.py").unlink()


if __name__ == "__main__":
    main()
