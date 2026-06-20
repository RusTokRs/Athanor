# API Consistency Checker

Crate: `athanor-checker-api`

Port: `Checker`

The checker reports OpenAPI operations without linked Rust implementations and implemented operations without linked Markdown documentation. It consumes canonical entities and relations only; it does not parse source files itself.

Diagnostics include OpenAPI evidence and ownership covering the endpoint plus current candidate source files. Relevant function, documentation, and relation changes trigger reevaluation. File additions and removals force a full rebuild at the pipeline level to keep absence diagnostics correct.

The checker is local and side-effect free. Deeper schema, status-code, authentication, permission, and breaking-change checks are deferred.

Test with:

```bash
cargo test -p athanor-checker-api
```
