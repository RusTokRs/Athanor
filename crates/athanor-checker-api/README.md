# athanor-checker-api

Basic API consistency checker adapter.

Implements: `Checker`

## What It Emits

- `ApiEndpointDocumentedButNotImplemented` when an OpenAPI endpoint has no `implemented_by` relation
- `ApiEndpointImplementedButNotDocumented` when an implemented endpoint has no generic `documents`, `documents_api`, or `documents_operation` relation
- `ApiRequestSchemaMismatch` when a local request schema `$ref` has no `schema_for_request` relation
- `ApiResponseSchemaMismatch` when a local response schema `$ref` has no `schema_for_response` relation

Diagnostics are open and deterministic. Implementation/documentation absence diagnostics are owned
by the endpoint plus current Rust or Markdown candidates. Schema diagnostics are owned by the
OpenAPI source file. Evidence points to the OpenAPI operation.

## Incremental Behavior

The checker evaluates affected endpoints and reevaluates all endpoints when relevant Rust functions, Markdown entities, or API relations change. The pipeline performs a safe full rebuild when files are added or removed so absence diagnostics cannot remain stale across source-set changes.

## Side Effects

None. The checker does not run commands, use the network, or modify files.

## Limitations

- Consistency accepts lexical API links and verified generic links declared through Markdown frontmatter; framework routes and unresolved links can still produce false positives.
- Schema checks currently validate only resolution of same-document component `$ref` values; they do
  not compare schema structure with Rust types or validate inline/external schemas.
- Status-code, auth, and permission checks are deferred.

## Test

```bash
cargo test -p athanor-checker-api
```
