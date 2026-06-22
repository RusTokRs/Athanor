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
the corresponding schema relation. It also validates extracted `ApiExample` values against their
declared schemas and emits `api_example_invalid` diagnostics. It consumes canonical entities and
relations only; it does not parse source files itself.

Documentation is satisfied by `documents_api`, `documents_operation`, or a verified generic
`documents` relation such as an exact Markdown frontmatter declaration.

### 2. Environment Variables Documentation Checker (`EnvDocsChecker`)
Reports environment variables used in the codebase but missing from the project's documentation. It checks `EntityKind::EnvVar` entities and flags them with `DiagnosticKind::MissingEnvVar` if they lack a `RelationKind::Documents` relation from a Markdown page or section.

Diagnostics include evidence and ownership. Relevant function, documentation, and relation changes trigger reevaluation. File additions and removals force a full rebuild at the pipeline level to keep absence diagnostics correct.

Example validation uses adapter-private `jsonschema` 0.46.5 with Draft 4 for OpenAPI 3.0 and Draft
2020-12 for OpenAPI 3.1. Same-document component schemas are assembled into an in-memory validation
document. Default resolver features are disabled, so validation cannot read files or use the
network. Compiled validators are cached by normalized schema during one checker run. External
schema references are skipped, and OpenAPI 3.0 keywords beyond Draft 4 compatibility remain a
documented limitation.

The checker is local and side-effect free. Deeper schema, status-code, authentication, permission, and breaking-change checks are deferred.

## Configuration & Policy Enforcement

When checking API consistency via `ath check api` (or with the `--strict` flag), policy parameters from `athanor.toml` (loaded under `[api]` or `[docs.api]`) dynamically filter the active diagnostics:

```toml
[api]
enabled = true
source_of_truth = "hybrid" # Options: "hybrid", "openapi_first", "code_first"
strict = true
fail_on_missing_docs = true
fail_on_openapi_mismatch = true
fail_on_undocumented_status_code = true
```

- `source_of_truth`:
  - `hybrid`: Enforces full API and documentation compliance.
  - `openapi_first`: Treats OpenAPI as the truth. Ignores implemented endpoints missing documentation, unless `fail_on_missing_docs` is explicitly set to `true`.
  - `code_first`: Treats code as the truth. Ignores documented endpoints missing implementations.
- `strict`: Enforces CI gate failure (non-zero process exit) if any filtered API consistency diagnostics or contract breaking changes are found.

Test with:

```bash
cargo test -p athanor-checker-api
```
