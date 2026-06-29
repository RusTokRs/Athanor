---
id: doc://docs/adapters/checker-api.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# API Consistency, Environment, Script, Deployment, And Runbook Checker

Crate: `athanor-checker-api`

Port: `Checker`

This crate houses five checkers:

### 1. API Consistency Checker (`ApiConsistencyChecker`)
Reports OpenAPI operations without linked Rust implementations, implemented operations
without linked Markdown documentation, and local request/response component `$ref` values without
the corresponding schema relation. It also validates extracted `ApiExample` values against their
declared schemas and emits `api_example_invalid` diagnostics. It consumes canonical entities and
relations only; it does not parse source files itself.

Documentation is satisfied by `documents_api`, `documents_operation`, or a verified generic
`documents` relation such as an exact Markdown frontmatter declaration.

### 2. Environment And Runtime Configuration Documentation Checker (`EnvDocsChecker`)
Reports environment variables and runtime configuration keys used in the codebase but missing from
the project's documentation. It checks `EntityKind::EnvVar` entities and flags them with
`DiagnosticKind::MissingEnvVar` if they lack a `RelationKind::Documents` relation from a Markdown
page or section. It also checks runtime configuration `EntityKind::Feature` entities whose payload
has `feature_kind = "runtime_config_key"` and emits `DiagnosticKind::MissingDocumentation` with
payload `scope = "env"` when they are not documented.

The findings are exposed separately through:

```bash
cargo run -p ath --quiet -- check env
cargo run -p ath --quiet -- check env --json
```

### 3. Script Command Documentation Checker (`ScriptDocsChecker`)
Reports operational script commands that are known to the graph but missing from editable
documentation. It checks `EntityKind::ScriptCommand` entities and emits
`DiagnosticKind::MissingDocumentation` with payload `scope = "scripts"` when the command lacks a
generic `RelationKind::Documents` relation from Markdown frontmatter.

The findings are exposed separately through:

```bash
cargo run -p ath --quiet -- check scripts
cargo run -p ath --quiet -- check scripts --json
```

### 4. Deployment Documentation Checker (`DeploymentDocsChecker`)
Reports deployment and service resources that are known to the graph but missing from editable
documentation. It checks `EntityKind::DockerService` entities and emits
`DiagnosticKind::MissingDocumentation` with payload `scope = "deployment"` when the resource lacks a
generic `RelationKind::Documents` relation from Markdown frontmatter.

The findings are exposed separately through:

```bash
cargo run -p ath --quiet -- check deployment
cargo run -p ath --quiet -- check deployment --json
```

### 5. Runbook Consistency Checker (`RunbookConsistencyChecker`)
Reports runbook pages that are not tied to known operational knowledge. It checks
`EntityKind::Runbook` entities emitted from Markdown frontmatter `kind = "runbook"` or
`kind = "operations_runbook"` and emits `DiagnosticKind::StaleDocumentation` with payload
`scope = "runbooks"` when the runbook has no declared operation targets or none of its declared
targets resolve to operational entities such as script commands, deployment resources,
environment variables, runtime configuration keys, migrations, packages, or dependencies. It also
emits a scoped stale-documentation diagnostic when a runbook has no extracted ordered-list
operation steps, or when the extracted operation steps do not mention any declared operational
target stable key, name, title, or alias. When some targets are mentioned but others are not, it
also reports the uncovered operational targets so a runbook can describe deploy, rollback,
verification, and related procedures explicitly.

The findings are exposed separately through:

```bash
cargo run -p ath --quiet -- check runbooks
cargo run -p ath --quiet -- check runbooks --json
```

Diagnostics include evidence and ownership. Relevant function, documentation, environment, runtime
configuration, script command, deployment, runbook, operation-target, and relation changes trigger reevaluation. File
additions and removals force a full rebuild at the pipeline level to keep absence diagnostics
correct.

Example validation uses adapter-private `jsonschema` 0.46.5 with Draft 4 for OpenAPI 3.0 and Draft
2020-12 for OpenAPI 3.1. Same-document component schemas are assembled into an in-memory validation
document. Default resolver features are disabled, so validation cannot read files or use the
network. Compiled validators are cached by normalized schema during one checker run. External
schema references are skipped, and OpenAPI 3.0 keywords beyond Draft 4 compatibility remain a
documented limitation.

The checker is local and side-effect free. Deeper schema, status-code, authentication, permission,
breaking-change, rollout, and step dependency checks are deferred.

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

[api.retention]
auto_cleanup = false
keep_snapshots = 2
keep_diffs = 2
```

- `source_of_truth`:
  - `hybrid`: Enforces full API and documentation compliance.
  - `openapi_first`: Treats OpenAPI as the truth. Ignores implemented endpoints missing documentation, unless `fail_on_missing_docs` is explicitly set to `true`.
  - `code_first`: Treats code as the truth. Ignores documented endpoints missing implementations.
- `strict`: Enforces CI gate failure (non-zero process exit) if any filtered API consistency diagnostics or contract breaking changes are found.
- `api.retention`: Controls optional cleanup of API contract snapshots and diff artifacts after successful `ath api snapshot` and `ath api diff` runs. The default leaves automatic cleanup disabled; manual `ath api cleanup` remains available.

Test with:

```bash
cargo test -p athanor-checker-api
```
