---
id: doc://docs/adapters/checker-markdown.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Markdown Checker Adapter

Crate: `athanor-checker-markdown`

Port: `Checker`

## Purpose

Diagnoses basic Markdown documentation structure issues.

## Inputs

Reads canonical objects from the current pipeline:

- documentation page entities
- documentation section entities
- containment relations

It does not read files directly.

The checker receives the full extracted context plus an `AffectedSubset`. It evaluates only affected documentation page entities while still using affected containment relations to determine whether a page has sections.

## Outputs

Diagnostics:

- `DiagnosticKind::DocumentationPageMissingTitle`
- `DiagnosticKind::EmptyDocumentationPage`
- `DiagnosticKind::DocumentationReferenceUnresolved`
- `DiagnosticKind::DuplicateDocumentationId`

Current rules:

- A Markdown page should have a top-level title.
- A Markdown page should expose at least one heading section.
- Every `entities` or `concepts` frontmatter stable key should resolve to exactly one canonical entity.
- Explicit documentation page ids must be unique across Markdown sources.

## Evidence

Diagnostics include:

- affected entity
- source file
- source line when known
- checker name
- verified evidence status
- ownership copied from the diagnosed documentation page entity

Structure diagnostics retain page ownership. Unresolved-reference and duplicate-id diagnostics use
candidate-aware ownership across current entity source paths because any changed source can add,
remove, or rename a matching stable key. Duplicate-id diagnostics include conflicting evidence from
every declaring page.

The checker recalculates full-context frontmatter rules only when the affected subset is non-empty;
on a no-change incremental run, compatible diagnostics are carried forward by the pipeline.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Does not modify project files.

## Tests

```bash
cargo test -p athanor-checker-markdown
```
