# OpenAPI Extractor

Crate: `athanor-extractor-openapi`

Port: `Extractor`

The adapter parses OpenAPI 3.x YAML and JSON documents. It emits canonical API endpoint and component-schema entities, plus declaration facts connected to the canonical source-file entity.

Endpoint keys use `api://METHOD:/path`. Schema keys include the specification path to prevent component-name collisions between independent specifications. Facts carry source evidence and every object is owned by its specification file.

The adapter is local and side-effect free. It does not resolve `$ref`, merge multiple specifications, infer handlers, or compare OpenAPI operations with code and documentation.

Test with:

```bash
cargo test -p athanor-extractor-openapi
```
