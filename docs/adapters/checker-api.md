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
Reports shared API contract operations, including OpenAPI and GraphQL `ApiEndpoint` entities,
without linked Rust implementations, routes, or resolvers; implemented operations without linked
Markdown documentation; and local request/response component `$ref` values without the
corresponding schema relation. Diagnostics are protocol-aware and include the inferred protocol in
their payloads, while still consuming canonical entities and relations only. The checker does not
parse source files itself.

It also validates extracted OpenAPI `ApiExample` values against their declared schemas and emits
`api_example_invalid` diagnostics.

When both OpenAPI and GraphQL endpoints are present, the checker detects response field drift
between operations that share a normalized name (e.g., `getUser` REST endpoint and `GetUser`
GraphQL query). It emits `api_openapi_graphql_drift` diagnostics with evidence from both endpoints
and the specific fields missing in each protocol.

The second `API-001` slice also emits `api_openapi_graphql_request_drift`. It compares OpenAPI
request-body component properties with GraphQL operation variables. When a GraphQL variable refers
to a named input object matching the OpenAPI component name, it compares the two input schemas
instead of flattening the body. Scalar families, list structure, required properties, and GraphQL
non-null markers are normalized before comparison.

The third slice adds canonical OpenAPI parameter metadata and repository-owned contract resolution.
Relative request and response schema references can resolve to `ApiSchema` entities emitted from
other OpenAPI documents in the repository. `api_openapi_graphql_parameter_drift` compares path,
query, and header parameters with GraphQL variables; `api_openapi_graphql_external_request_drift`
covers external request schemas; and `api_openapi_graphql_response_schema_drift` compares response
containers, fields, scalar/list families, and required/nullability semantics.

The fourth slice publishes effective OpenAPI security requirements after operation-level override or
root inheritance, including scheme kind, HTTP scheme, bearer format, alternatives, and required
scopes. GraphQL operations preserve directive applications and scalar/list argument values. The
checker emits `api_openapi_graphql_status_code_drift`,
`api_openapi_graphql_authentication_drift`, and `api_openapi_graphql_permission_drift`.

The fifth slice extends repository-local contract resolution and corrects security alternative
semantics:

- OpenAPI `components.parameters` entries are emitted as canonical `ApiSchema` entities, including
  partial repository-owned component documents that are not complete OpenAPI roots;
- relative external parameter references are resolved by normalized repository path, while remote
  URL references remain outside the local checker boundary;
- multi-root GraphQL operations are compared against every resolvable top-level response selection,
  and no response diagnostic is emitted when one candidate is fully compatible;
- OpenAPI security alternatives remain alternatives: permission scopes are compared against one
  compatible alternative instead of a union across mutually exclusive alternatives;
- custom GraphQL security vocabulary can be supplied through canonical
  `security_directive_mapping` metadata or an operation-level `@athanorSecurity` /
  `@athanorSecurityMapping` directive.

The mapping directive accepts list or scalar values for `authenticationDirectives`,
`permissionDirectives`, `permissionArguments`, and `authenticationFamilyArguments`. For example:

```graphql
query GetUser @athanorSecurity(
  authenticationDirectives: ["secured"]
  permissionDirectives: ["secured"]
  permissionArguments: ["policy"]
  authenticationFamilyArguments: ["provider"]
) @secured(provider: "oauth2", policy: ["users:read"]) {
  getUser { id }
}
```

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
configuration, script command, deployment, runbook, operation-target, and relation changes trigger
reevaluation. File additions and removals force a full rebuild at the pipeline level to keep absence
diagnostics correct.

Example validation is currently OpenAPI-specific. It uses adapter-private `jsonschema` 0.46.5 with
Draft 4 for OpenAPI 3.0 and Draft 2020-12 for OpenAPI 3.1. Same-document component schemas are
assembled into an in-memory validation document. Default resolver features are disabled, so
validation cannot read files or use the network. Compiled validators are cached by normalized schema
during one checker run. External schema references are skipped, and OpenAPI 3.0 keywords beyond
Draft 4 compatibility remain a documented limitation.

The checker is local and side-effect free. Remote URL references, unresolved repository references,
unmapped custom GraphQL security semantics, breaking-change, rollout, and step dependency checks
remain deferred. The fifth slice implementation remains pending exact successful `main` verification.

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
