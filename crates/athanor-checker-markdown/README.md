# athanor-checker-markdown

Markdown structure checker adapter.

Implements: `Checker`

## What It Emits

- `DiagnosticKind::DocumentationPageMissingTitle`
- `DiagnosticKind::EmptyDocumentationPage`

## Inputs

Canonical entities and relations from the current pipeline. The adapter receives full extracted context plus an `AffectedSubset` and evaluates only affected documentation pages and affected containment relations.

This checker does not read files directly.

## Side Effects

None.

This adapter does not run commands, does not use the network, and does not modify project files.

## Test

```bash
cargo test -p athanor-checker-markdown
```
