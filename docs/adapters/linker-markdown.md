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

The linker receives the full extracted context plus an `AffectedSubset`. It emits containment relations only for documentation paths represented in the affected entities while still using full-context file/page/section entities to build valid relations.

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
