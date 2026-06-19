# athanor-extractor-markdown

Markdown documentation extractor adapter.

Implements: `Extractor`

## What It Emits

- `EntityKind::DocumentationPage`
- `EntityKind::DocumentationSection`
- `FactKind::DocSectionFound`

## Inputs

`SourceFile` with:

```text
language_hint = markdown
content = UTF-8 Markdown text
```

## Stable Keys

```text
doc://docs/example.md
doc://docs/example.md#section
```

Non-ASCII heading text is percent-encoded in slugs.

## Side Effects

None.

This adapter does not run commands, does not use the network, and does not modify project files.

## Test

```bash
cargo test -p athanor-extractor-markdown
```
