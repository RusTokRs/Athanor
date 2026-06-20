# athanor-extractor-openapi

OpenAPI 3.x extractor adapter.

Implements: `Extractor`

## What It Emits

- `EntityKind::ApiEndpoint` for operations under `paths`
- `EntityKind::ApiSchema` for entries under `components.schemas`
- `FactKind::RouteDeclared` for operations
- `FactKind::Other("api_schema_declared")` for component schemas

Every emitted object has source-file ownership. Facts include source evidence and point to the canonical file entity.

## Inputs

UTF-8 YAML or JSON `SourceFile` values whose name is `openapi.*`, `swagger.*`, or `*.openapi.*`, or whose content contains a root OpenAPI marker.

Only OpenAPI 3.x documents are accepted.

## Stable Keys

```text
api://POST:/login
api-schema://openapi.yaml#LoginRequest
```

## Side Effects

None. The adapter runs in-process without commands or network access.

## Limitations

- `$ref` values are preserved but not resolved.
- Path-level parameters are counted separately and not merged into operation payloads.
- Multiple specification files declaring the same method/path currently produce the same canonical endpoint key and should be consolidated by a future API linker.
- Swagger 2.x is not supported.

## Test

```bash
cargo test -p athanor-extractor-openapi
```
