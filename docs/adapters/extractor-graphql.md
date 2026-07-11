---
id: doc://docs/adapters/extractor-graphql.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
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

The extractor validates same-file GraphQL declarations and emits additional diagnostics:

- **Unresolved fragment spreads** (`graphql_unresolved_fragment_spread`): reported at `Severity::Medium`
  when an operation or fragment uses `...FragmentName` but no fragment with that name is declared in
  the same file.
- **Unresolved inline type conditions** (`graphql_unresolved_type_condition`): reported at
  `Severity::Medium` when an operation or fragment uses `... on TypeName` but no schema type with
  that name is declared in the same file.
- **Deprecated field usage** (`graphql_deprecated_field_used`): reported at `Severity::Low` when an
  operation selects a field that is marked `@deprecated` in a schema type declared in the same file.
- **Invalid directive location** (`graphql_invalid_directive_location`): reported at `Severity::Medium`
  when a directive is used at a location not matching its SDL declaration (e.g., `@auth` declared on
  `FIELD_DEFINITION` used on a `QUERY` operation).
- **Undeclared variable reference** (`graphql_undeclared_variable_reference`): reported at `Severity::Medium`
  when a variable is used in an operation body but not declared in the operation's variable definitions.
- **Unused variable** (`graphql_unused_variable`): reported at `Severity::Low` when a variable is
  declared in an operation's variable definitions but never used in the operation body.
- **Variable type not found** (`graphql_variable_type_not_found`): reported at `Severity::Medium`
  when a variable definition references a type that is not a declared schema type or built-in scalar
  (`Int`, `Float`, `String`, `Boolean`, `ID`).
- **Invalid directive argument** (`graphql_invalid_directive_argument`): reported at `Severity::Medium`
  when a directive usage on a schema field/member passes an argument that is not defined in the
  directive's SDL declaration (e.g., `@auth(unknownArg: 1)` when `@auth` only declares `role`).

All validation diagnostics include a `suggested_fix` with actionable remediation guidance.

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
locations, directive-definition argument names (used for usage validation), root schema operation names, and bounded field/member names, plus `serde_json`
and field/member type names, plus `serde_json` handling for standard introspection root operation
type, `types`, and `directives` shapes. It is side-effect free,
performs no network or command execution, and does not expose parser-library types across adapter
boundaries. SDL `@deprecated` directives and introspection
`isDeprecated`/`deprecationReason` fields are preserved as payload metadata for later drift checks.

Limitations:

- GraphQL syntax validation, directive semantics, and stale-operation checks are deferred.
- Cross-file fragment-spread and type-condition validation requires linker-level resolution and is deferred.
- Resolver linking is handled by the API linker through `operation_name` matching against Rust functions.
- Embedded GraphQL in JavaScript/TypeScript sources is deferred to a later JS/TS-aware slice.
- Field extraction is best-effort for straightforward SDL bodies and is not a replacement for a
  full GraphQL parser contract.

Test with:

```bash
cargo test -p athanor-extractor-graphql
```
