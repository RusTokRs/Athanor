---
id: doc://docs/adapters/checker-api.md
kind: module_documentation
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000030
status: verified
---

# API Consistency & Environment Checker

Crate: `athanor-checker-api`

Port: `Checker`

This crate houses two checkers:

### 1. API Consistency Checker (`ApiConsistencyChecker`)
Reports OpenAPI operations without linked Rust implementations, implemented operations
without linked Markdown documentation, and local request/response component `$ref` values without
the corresponding schema relation. It consumes canonical entities and relations only; it does not
parse source files itself.

Documentation is satisfied by `documents_api`, `documents_operation`, or a verified generic
`documents` relation such as an exact Markdown frontmatter declaration.

### 2. Environment Variables Documentation Checker (`EnvDocsChecker`)
Reports environment variables used in the codebase but missing from the project's documentation. It checks `EntityKind::EnvVar` entities and flags them with `DiagnosticKind::MissingEnvVar` if they lack a `RelationKind::Documents` relation from a Markdown page or section.

Diagnostics include evidence and ownership. Relevant function, documentation, and relation changes trigger reevaluation. File additions and removals force a full rebuild at the pipeline level to keep absence diagnostics correct.

The schema checks are intentionally structural: they verify that a same-document component
reference resolves to an extracted `ApiSchema`. They do not compare OpenAPI schema fields with Rust
types or validate inline and external schemas.

The checker is local and side-effect free. Deeper schema, status-code, authentication, permission, and breaking-change checks are deferred.

Test with:

```bash
cargo test -p athanor-checker-api
```
