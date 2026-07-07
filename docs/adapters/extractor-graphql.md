---
id: doc://docs/adapters/extractor-graphql.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: draft
---
# GraphQL Extractor

Crate: `athanor-extractor-graphql`

Port: `Extractor`

The adapter extracts first-class GraphQL contract knowledge from standalone `.graphql` and `.gql`
files, GraphQL introspection JSON, plus sources with `language_hint = graphql`. It emits canonical
API operation and schema entities without adding GraphQL-specific types to `athanor-domain` or
`athanor-core`.

GraphQL `query`, `mutation`, and `subscription` declarations become `EntityKind::ApiEndpoint`
entities with `protocol = graphql` payloads. SDL `schema`, `type`, `input`, `interface`, `enum`,
`scalar`, and `union` declarations become `EntityKind::ApiSchema` entities. Introspection JSON root
operation types and schema `types` entries also become `EntityKind::ApiSchema` entities, skipping
built-in `__*` types. Fragment
declarations become `EntityKind::Other("graphql_fragment")` entities with type-condition and
selection-root payloads. SDL and introspection directive definitions become
`EntityKind::Other("graphql_directive")` entities with location and argument payloads. Operation
declarations emit `FactKind::RouteDeclared`; schema declarations emit
`FactKind::Other("api_schema_declared")`; fragments emit
`FactKind::Other("graphql_fragment_declared")`; directive definitions emit
`FactKind::Other("graphql_directive_declared")`.

Because operations use the shared `ApiEndpoint` kind, the API consistency checker can report
missing GraphQL resolver/implementation links and missing operation documentation through the same
bounded diagnostics used for OpenAPI contract operations. The diagnostics remain protocol-aware;
OpenAPI-only example validation and deeper OpenAPI/GraphQL drift checks are still separate slices.

Explicit GraphQL inputs also emit bounded diagnostics with source evidence and ownership when the
adapter cannot extract useful contract knowledge: invalid introspection JSON, introspection JSON
without root operation types, non-built-in schema types, or directive definitions, or `.graphql`/`.gql`
content without supported top-level operation/fragment/directive/SDL declarations.

Stable keys use the shared API contract namespace:

```text
api://GRAPHQL_QUERY:GetUser
api://GRAPHQL_MUTATION:UpdateUser
api-schema://graphql:schema.graphql#User
api-directive://graphql:schema.graphql#auth
```

Every emitted object is owned by the GraphQL source file. Facts include source evidence and link
back to the canonical file entity.

The first implementation slice uses a small adapter-local recognizer for top-level declarations,
bounded operation variable names and variable type definitions, fragment type conditions, fragment-spread names, inline-fragment
type conditions, bounded operation/field argument names, directive names, directive-definition
locations, directive-definition argument names, root schema operation names, and bounded field/member names, plus `serde_json`
and field/member type names, plus `serde_json` handling for standard introspection root operation
type, `types`, and `directives` shapes. It is side-effect free,
performs no network or command execution, and does not expose parser-library types across adapter
boundaries. SDL `@deprecated` directives and introspection
`isDeprecated`/`deprecationReason` fields are preserved as payload metadata for later drift checks.

Limitations:

- GraphQL syntax validation, directive semantics, argument/variable usage validation beyond captured names and types, captured
  fragment-spread and inline type-condition validation, deprecated-usage checking,
  resolver/callsite linking, and stale-operation checks are deferred.
- Embedded GraphQL in JavaScript/TypeScript sources is deferred to a later JS/TS-aware slice.
- Field extraction is best-effort for straightforward SDL bodies and is not a replacement for a
  full GraphQL parser contract.

Test with:

```bash
cargo test -p athanor-extractor-graphql
```
