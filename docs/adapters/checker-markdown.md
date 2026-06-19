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

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Does not modify project files.

## Tests

```bash
cargo test -p athanor-checker-markdown
```
