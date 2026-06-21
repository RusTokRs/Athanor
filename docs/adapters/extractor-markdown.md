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

The adapter uses `pulldown-cmark` 0.13 internally. It recognizes ATX and setext headings, reduces
inline formatting to canonical heading text, ignores heading syntax inside fenced code blocks, and
maps parser source offsets to evidence lines. `pulldown-cmark` types remain private to the adapter.

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

## Limitations

- Only heading structure is materialized; paragraphs, links, code blocks, and frontmatter are not
  emitted as separate canonical entities yet.
- Stable slugs continue to use Athanor's existing slug algorithm rather than a renderer-specific
  anchor algorithm.

## Tests

```bash
cargo test -p athanor-extractor-markdown
```
