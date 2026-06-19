# Markdown Linker Adapter

Crate: `athanor-linker-markdown`

Port: `Linker`

## Purpose

Creates containment relations for Markdown documentation knowledge.

## Inputs

Reads entities emitted by extractors:

- `EntityKind::File`
- `EntityKind::DocumentationPage`
- `EntityKind::DocumentationSection`

## Outputs

Relations:

- `RelationKind::Contains`

Current relation patterns:

```text
file contains documentation_page
documentation_page contains documentation_section
```

Each relation has:

- `status = verified`
- `confidence = 1.0`
- evidence pointing to the relevant source file/line when available

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Does not modify project files.

## Tests

```bash
cargo test -p athanor-linker-markdown
```
