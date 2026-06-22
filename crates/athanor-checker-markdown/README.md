# athanor-checker-markdown

Markdown structure checker adapter.

Implements: `Checker`

## What It Emits

- `DiagnosticKind::DocumentationPageMissingTitle`
- `DiagnosticKind::EmptyDocumentationPage`
- `DiagnosticKind::DocumentationReferenceUnresolved`
- `DiagnosticKind::DuplicateDocumentationId`

Diagnostics include ownership copied from the diagnosed documentation page entity.

## Inputs

Canonical entities and relations from the current pipeline. Structure rules evaluate affected
documentation pages. Frontmatter reference and identity rules use full canonical context whenever
the affected subset is non-empty.

Unresolved and duplicate diagnostics use candidate-aware ownership across the current entity
registry. This invalidates them when a source changes that could introduce, remove, or rename the
referenced target.

This checker does not read files directly.

## Side Effects

None.

This adapter does not run commands, does not use the network, and does not modify project files.

## Test

```bash
cargo test -p athanor-checker-markdown
```
