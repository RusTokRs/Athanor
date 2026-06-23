# athanor-checker-api

API, environment, script, deployment, and runbook checker adapter.

Implements: `Checker`

## What It Emits

- `ApiEndpointDocumentedButNotImplemented` when an OpenAPI endpoint has no `implemented_by` relation
- `ApiEndpointImplementedButNotDocumented` when an implemented endpoint has no generic `documents`, `documents_api`, or `documents_operation` relation
- `ApiRequestSchemaMismatch` when a local request schema `$ref` has no `schema_for_request` relation
- `ApiResponseSchemaMismatch` when a local response schema `$ref` has no `schema_for_response` relation
- `ApiExampleInvalid` when an extracted request or response example violates its declared schema
- `MissingEnvVar` when an environment variable entity has no Markdown `documents` relation
- `MissingDocumentation` with payload scope `env` when a runtime configuration key has no Markdown
  `documents` relation
- `MissingDocumentation` with payload scope `scripts` when a script command entity has no Markdown
  `documents` relation
- `MissingDocumentation` with payload scope `deployment` when a deployment/service entity has no
  Markdown `documents` relation
- `StaleDocumentation` with payload scope `runbooks` when a runbook does not reference any known
  operational entity target
- `StaleDocumentation` with payload scope `runbooks` when a runbook has no extracted operation
  steps
- `StaleDocumentation` with payload scope `runbooks` when extracted operation steps do not mention
  any declared operational target stable key, name, title, or alias
- `StaleDocumentation` with payload scope `runbooks` when some declared operational targets are not
  mentioned by any extracted operation step

Diagnostics are open and deterministic. Implementation/documentation absence diagnostics are owned
by the endpoint plus current Rust or Markdown candidates. Schema diagnostics are owned by the
OpenAPI source file. Evidence points to the OpenAPI operation.

Example validation uses adapter-private `jsonschema` 0.46.5 with Draft 4 for OpenAPI 3.0 and Draft
2020-12 for OpenAPI 3.1. Default resolver features are disabled, so validation never reads files or
uses the network. Validators are cached by normalized schema during one checker run.

## Incremental Behavior

The API checker evaluates affected endpoints and reevaluates all endpoints when relevant Rust
functions, Markdown entities, or API relations change. The environment, script, and deployment
documentation checkers evaluate affected environment/config/script/deployment entities and
reevaluate when Markdown entities change. The runbook consistency checker evaluates affected
runbooks and runbooks whose declared operation targets changed.
The pipeline performs a safe full rebuild when files are added or removed so absence diagnostics
cannot remain stale across source-set changes.

## Side Effects

None. The checker does not run commands, use the network, or modify files.

## Limitations

- Consistency accepts lexical API links and verified generic links declared through Markdown frontmatter; framework routes and unresolved links can still produce false positives.
- Schema checks currently validate only resolution of same-document component `$ref` values; they do
  not compare schema structure with Rust types or validate inline/external schemas.
- Status-code, auth, and permission checks are deferred.
- External schema references are skipped; OpenAPI 3.0-specific keywords outside JSON Schema Draft 4
  are not interpreted yet.
- Environment, runtime configuration, script, and deployment documentation checks require explicit
  Markdown frontmatter entity links; lexical mentions do not satisfy them.
- Script checks cover `ScriptCommand` entities, and deployment checks cover `DockerService`
  entities.
- Runbook consistency currently validates that a runbook declares at least one known operational
  stable key target, contains at least one ordered-list operation step, and explicitly references a
  declared target from extracted steps. Every declared operational target must be mentioned by at
  least one extracted step. Step dependency validation is deferred.

## Test

```bash
cargo test -p athanor-checker-api
```
