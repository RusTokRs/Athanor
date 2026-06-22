---
id: doc://docs/adapters/linker-api.md
kind: module_documentation
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000030
status: verified
---

# API Knowledge Linker

Crate: `athanor-linker-api`

Port: `Linker`

This linker creates the first cross-source API graph between OpenAPI operations, Rust functions, and Markdown pages or sections.

It emits `implemented_by`, `documents_operation`, and `documents_api` inferred relations. Matches are deterministic and lexical: exact normalized `operationId` for Rust handlers at confidence 0.7, and operation id, final static path segment, or tags for documentation at confidence 0.5. It also resolves same-document OpenAPI component `$ref` values into verified `schema_for_request` and `schema_for_response` relations at confidence 1.0, and links `ApiExample` entities to their declaring endpoint with verified `example_for` relations. Schema relation payloads retain media type and response status metadata. Every relation contains evidence and ownership from both sides and respects the affected subset during incremental indexing.

The linker has no commands, network access, or file side effects. Framework route inference,
call-graph analysis, external `$ref` resolution, and links from OpenAPI schemas to Rust types are
deferred.

Test with:

```bash
cargo test -p athanor-linker-api
```
