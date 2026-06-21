# athanor-extractor-markdown

Markdown documentation extractor adapter.

Implements: `Extractor`

## What It Emits

- `EntityKind::DocumentationPage`
- `EntityKind::DocumentationSection`
- `FactKind::DocSectionFound`

All emitted objects include ownership metadata for the Markdown source file path.

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

Heading structure is parsed with `pulldown-cmark` 0.13 using explicit CommonMark/GFM options.
ATX and setext headings are supported, inline formatting is normalized into heading text, and
heading-like content inside fenced code blocks is ignored. Parser byte offsets are converted to
source evidence lines without exposing parser types outside this crate.

## Side Effects

None.

This adapter does not run commands, does not use the network, and does not modify project files.

## Test

```bash
cargo test -p athanor-extractor-markdown
```
