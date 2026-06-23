# athanor-extractor-markdown

Markdown documentation extractor adapter.

Implements: `Extractor`

## What It Emits

- `EntityKind::DocumentationPage`
- `EntityKind::DocumentationSection`
- `EntityKind::Runbook` when frontmatter `kind` is `runbook` or `operations_runbook`
- `EntityKind::OperationStep` for ordered-list items in runbook documents
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

When YAML frontmatter contains an explicit `id`, that `doc://` page key replaces the path-derived
page key and becomes the prefix for section keys. This keeps documentation identity stable when a
source file moves.

Non-ASCII heading text is percent-encoded in slugs.

Heading structure is parsed with `pulldown-cmark` 0.13 using explicit CommonMark/GFM options.
ATX and setext headings are supported, inline formatting is normalized into heading text, and
heading-like content inside fenced code blocks is ignored. Parser byte offsets are converted to
source evidence lines without exposing parser types outside this crate.

## Frontmatter

An optional leading YAML block supports:

```yaml
---
id: doc://product/authentication
kind: api_documentation
language: en
source_language: ru
documentation_layer: editable
concepts:
  - concept://authentication
entities:
  - api://POST:/login
last_verified_snapshot: snap_reference
status: verified
---
```

`documentation_layer` accepts `editable` or `generated`. Source Markdown defaults to `editable`.
The page payload records `frontmatter_fields`, preserving which keys were explicitly declared so
app-layer completeness policies can distinguish defaults from authored metadata.
Frontmatter metadata is stored in the page payload and identity, language, layer, and kind are
inherited by section entities. Malformed YAML, unclosed frontmatter, invalid language values, and
page ids that are not fragment-free `doc://` keys fail extraction.
Runbook frontmatter emits a separate `runbook://...` entity whose payload records the source
documentation page and operation targets from the page `entities` list.
Runbook ordered-list items emit `operation_step` entities with `runbook://...#step-N` stable keys,
source lines, sequence numbers, and normalized item text.

## Side Effects

None.

This adapter does not run commands, does not use the network, and does not modify project files.

## Test

```bash
cargo test -p athanor-extractor-markdown
```
