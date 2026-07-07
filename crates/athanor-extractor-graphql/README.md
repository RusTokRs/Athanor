# athanor-extractor-graphql

GraphQL contract extractor adapter.

Implements: `Extractor`

## What It Emits

- `EntityKind::ApiEndpoint` for GraphQL `query`, `mutation`, and `subscription` operations.
- `EntityKind::ApiSchema` for SDL `schema`, `type`, `input`, `interface`, `enum`, `scalar`, and `union` definitions, plus GraphQL introspection JSON root schema and schema types.
- `EntityKind::Other("graphql_fragment")` for GraphQL fragment declarations and their type conditions.
- `EntityKind::Other("graphql_directive")` for SDL and introspection directive definitions.
- `FactKind::RouteDeclared` for operations.
- `FactKind::Other("api_schema_declared")` for schema definitions.
- `FactKind::Other("graphql_fragment_declared")` for fragment definitions.
- `FactKind::Other("graphql_directive_declared")` for directive definitions.
- `DiagnosticKind::Other("graphql_introspection_parse_error")` for invalid introspection JSON.
- `DiagnosticKind::Other("graphql_introspection_empty")` when introspection JSON has no extractable non-built-in schema types or directive definitions.
- `DiagnosticKind::Other("graphql_no_declarations")` when an explicit GraphQL source has no supported top-level operation, fragment, directive, or SDL declaration.

Every emitted object has source-file ownership. Facts include source evidence and point to the
canonical file entity. Diagnostics include source evidence and source-file ownership.

GraphQL operations use the shared API contract model, so the API consistency checker can report
missing resolver/implementation links and missing Markdown documentation for GraphQL endpoints with
protocol-aware diagnostics. OpenAPI example validation and deeper OpenAPI/GraphQL drift checks
remain separate slices.

## Inputs

UTF-8 `SourceFile` values with `.graphql` or `.gql` paths, or `language_hint = graphql`.
GraphQL introspection JSON is accepted for paths such as `.graphql.json`, `.gql.json`,
`graphql.schema.json`, `graphql-introspection.json`, and `introspection.json`, or JSON sources whose
content contains a `__schema.types`, `__schema.directives`, or root operation type marker.

The initial parser is intentionally adapter-local and dependency-free. It recognizes top-level SDL,
operation, fragment, directive, and root schema declarations, captures bounded operation variable names and variable type definitions, fragment type
conditions, fragment-spread names, inline-fragment type conditions, operation/field argument names,
directive names, field/member names, and field/member type names from simple brace bodies, and reads the standard
introspection root operation types plus `types` and `directives` arrays. SDL `@deprecated` directives and introspection
`isDeprecated`/`deprecationReason` fields are captured as payload metadata. Parser library types do
not cross into Athanor core/domain contracts.

## Stable Keys

```text
api://GRAPHQL_QUERY:GetUser
api://GRAPHQL_MUTATION:UpdateUser
api-schema://graphql:schema.graphql#User
api-directive://graphql:schema.graphql#auth
```

Anonymous operations are keyed by source path, operation type, line, and a stable hash.

## Side Effects

None. The adapter runs in-process without commands or network access.

## Limitations

- This first slice does not validate GraphQL syntax, resolve captured fragment spreads or inline
  type conditions, validate directive semantics, validate argument/variable usage beyond capturing
  names and variable types, validate deprecated usage, or link resolvers/callsites.
- Embedded frontend GraphQL strings remain outside this extractor and should be handled by a later
  JS/TS-aware slice.
- Field capture is best-effort for straightforward SDL bodies and intentionally bounded to avoid
  turning this dependency-free slice into a full parser.

## Test

```bash
cargo test -p athanor-extractor-graphql
```
