# OpenAPI Extractor

Crate: `athanor-extractor-openapi`

Port: `Extractor`

The adapter parses OpenAPI 3.0.x and 3.1.x YAML and JSON documents. It emits canonical API endpoint and component-schema entities, plus declaration facts connected to the canonical source-file entity.

Parsing is replaceable behind a private `OpenApiDocumentParser` boundary. OpenAPI 3.1 uses `oas3`
0.22; OpenAPI 3.0 uses a normalized-value fallback with `serde_yaml_ng` for YAML. Both feed a
private `NormalizedOpenApiDocument`, and no parser-library types cross into domain/core or process
adapter contracts. Entity payloads record the selected parser backend.

Endpoint keys use `api://METHOD:/path`. Schema keys include the specification path to prevent component-name collisions between independent specifications. Facts carry source evidence and every object is owned by its specification file.

Endpoint payloads normalize request and response schema `$ref` uses from OpenAPI media content.
Request uses retain media type; response uses retain status code and media type. Nested schema
references are collected recursively for later linking and checking.

The adapter is local and side-effect free. It preserves `$ref` values but leaves resolution to the
API linker. It does not resolve external references, merge specifications, infer handlers, or
compare OpenAPI operations with code and documentation.

The contract corpus covers OpenAPI 3.0.3, 3.1.0, and 3.1.1 in both YAML and JSON. Swagger 2.x and
OpenAPI 3.2 are not supported.

Test with:

```bash
cargo test -p athanor-extractor-openapi
```
