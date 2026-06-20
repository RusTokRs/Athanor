# athanor-checker-api

Basic API consistency checker adapter.

Implements: `Checker`

## What It Emits

- `ApiEndpointDocumentedButNotImplemented` when an OpenAPI endpoint has no `implemented_by` relation
- `ApiEndpointImplementedButNotDocumented` when an implemented endpoint has no `documents_api` or `documents_operation` relation

Diagnostics are open, deterministic, and owned by the endpoint plus current Rust or Markdown candidates that can invalidate the absence result. Evidence points to the OpenAPI operation.

## Incremental Behavior

The checker evaluates affected endpoints and reevaluates all endpoints when relevant Rust functions, Markdown entities, or API relations change. The pipeline performs a safe full rebuild when files are added or removed so absence diagnostics cannot remain stale across source-set changes.

## Side Effects

None. The checker does not run commands, use the network, or modify files.

## Limitations

- Consistency depends on the current lexical API linker; framework routes and unresolved links can produce false positives.
- Request/response schema, status-code, auth, and permission checks are deferred.

## Test

```bash
cargo test -p athanor-checker-api
```
