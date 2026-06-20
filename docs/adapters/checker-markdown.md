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

Current rules:

- A Markdown page should have a top-level title.
- A Markdown page should expose at least one heading section.

## Evidence

Diagnostics include:

- affected entity
- source file
- source line when known
- checker name
- verified evidence status
- ownership copied from the diagnosed documentation page entity

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Does not modify project files.

## Tests

```bash
cargo test -p athanor-checker-markdown
```
