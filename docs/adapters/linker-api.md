# API Knowledge Linker

Crate: `athanor-linker-api`

Port: `Linker`

This linker creates the first cross-source API graph between OpenAPI operations, Rust functions, and Markdown pages or sections.

It emits `implemented_by`, `documents_operation`, and `documents_api` inferred relations. Matches are deterministic and lexical: exact normalized `operationId` for Rust handlers at confidence 0.7, and operation id, final static path segment, or tags for documentation at confidence 0.5. Every relation contains evidence and ownership from both sides and respects the affected subset during incremental indexing.

The linker has no commands, network access, or file side effects. Framework route inference, call-graph analysis, and request/response schema linking are deferred.

Test with:

```bash
cargo test -p athanor-linker-api
```
