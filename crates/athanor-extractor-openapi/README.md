# athanor-extractor-openapi

OpenAPI 3.0/3.1 extractor adapter.

Implements: `Extractor`

## What It Emits

- `EntityKind::ApiEndpoint` for operations under `paths`
- `EntityKind::ApiSchema` for entries under `components.schemas`
- `FactKind::RouteDeclared` for operations
- `FactKind::Other("api_schema_declared")` for component schemas

Operation payloads include normalized request and response schema uses. Each use preserves the
media type and local or external `$ref`; response uses also preserve the status code. References
nested under arrays or composition keywords are collected recursively.

Every emitted object has source-file ownership. Facts include source evidence and point to the canonical file entity.

## Inputs

UTF-8 YAML or JSON `SourceFile` values whose name is `openapi.*`, `swagger.*`, or `*.openapi.*`, or whose content contains a root OpenAPI marker.

Only OpenAPI 3.x documents are accepted.

Parser dispatch is version-aware and private to this adapter:

- OpenAPI 3.1.x uses typed `oas3` 0.22 parsing.
- OpenAPI 3.0.x uses the legacy normalized-value parser backed by `serde_yaml_ng` for YAML.

Both parsers produce the same private normalized document shape before canonical extraction.
Canonical endpoint/schema payloads record `parser_backend` for troubleshooting, but no third-party
parser types leave this crate.

## Stable Keys

```text
api://POST:/login
api-schema://openapi.yaml#LoginRequest
```

## Side Effects

None. The adapter runs in-process without commands or network access.

## Limitations

- `$ref` values are preserved; same-document `components.schemas` references are resolved later by
  the API linker, while external references are not resolved.
- Path-level parameters are counted separately and not merged into operation payloads.
- Multiple specification files declaring the same method/path currently produce the same canonical endpoint key and should be consolidated by a future API linker.
- Swagger 2.x is not supported.
- OpenAPI 3.2.x is not accepted until a parser contract and fixture corpus are added.
- OpenAPI 3.1 YAML is preflighted through the maintained generic YAML backend before typed `oas3`
  parsing, so parser compatibility is tested against both paths.

## Test

```bash
cargo test -p athanor-extractor-openapi
```
