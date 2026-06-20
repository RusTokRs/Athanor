# Markdown Extractor Adapter

Crate: `athanor-extractor-markdown`

Port: `Extractor`

## Purpose

Extracts documentation knowledge from Markdown files.

## Inputs

Supports `SourceFile` when:

```text
language_hint == "markdown"
```

Requires text content in `SourceFile.content`.

## Outputs

Entities:

- `EntityKind::DocumentationPage`
- `EntityKind::DocumentationSection`

Facts:

- `FactKind::DocSectionFound`

Stable keys:

```text
doc://path/to/file.md
doc://path/to/file.md#section-slug
```

Non-ASCII heading slugs are percent-encoded to keep stable keys portable across terminals and platforms.

## Evidence

Each section fact includes:

- source file
- heading line
- extractor name
- confidence
- verified status

## Ownership

Emitted page entities, section entities, and section facts are owned by the Markdown source file path.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Does not modify project files.

## Tests

```bash
cargo test -p athanor-extractor-markdown
```
