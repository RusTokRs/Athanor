# athanor-linker-api

Cross-source API knowledge linker.

Implements: `Linker`

## What It Emits

```text
api_endpoint implemented_by rust_function
documentation_section documents_operation api_endpoint
documentation_page documents_api api_endpoint
api_endpoint schema_for_request api_schema
api_endpoint schema_for_response api_schema
```

Implementation links require an exact normalized match between OpenAPI `operationId` and a Rust function or method name. Documentation links use an operation id, final static path segment, or OpenAPI tag found in the documentation entity's title, name, aliases, or stable key.

Relations are inferred, include evidence from both sources, and are owned by the union of both source files. Exact operation-id/function-name links use confidence 0.7; lexical documentation links use confidence 0.5. Incremental runs emit a relation only when at least one side is affected.

Schema relations resolve same-document `#/components/schemas/<name>` references. They are verified
at confidence 1.0 and preserve request media type or response status/media metadata in the relation
payload.

## Side Effects

None. The linker does not run commands, use the network, or modify files.

## Limitations

- Handler matching does not inspect framework route macros or call graphs.
- Documentation matching is lexical and intentionally conservative.
- External `$ref` targets and inline schemas are not materialized as schema relations.
- OpenAPI schemas are not linked to Rust request/response types yet.

## Test

```bash
cargo test -p athanor-linker-api
```
