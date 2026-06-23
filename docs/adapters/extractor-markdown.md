---
id: doc://docs/adapters/extractor-markdown.md
kind: module_documentation
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000030
status: verified
---

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
- `EntityKind::Runbook` when frontmatter `kind` is `runbook` or `operations_runbook`

Facts:

- `FactKind::DocSectionFound`

Stable keys:

```text
doc://path/to/file.md
doc://path/to/file.md#section-slug
runbook://path/to/file.md
```

An explicit frontmatter `id` replaces the path-derived page key. It must be a non-empty,
fragment-free `doc://` stable key. Section keys then use that id as their prefix, so a document can
move without changing canonical identity.

Non-ASCII heading slugs are percent-encoded to keep stable keys portable across terminals and platforms.

The adapter uses `pulldown-cmark` 0.13 internally. It recognizes ATX and setext headings, reduces
inline formatting to canonical heading text, ignores heading syntax inside fenced code blocks, and
maps parser source offsets to evidence lines. `pulldown-cmark` types remain private to the adapter.

## YAML Frontmatter

The adapter recognizes one optional YAML block at the start of the file:

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

Supported fields:

| Field | Canonical effect |
| --- | --- |
| `id` | Overrides the path-derived documentation page stable key. |
| `language` | Sets page and section `Entity.language`. |
| `documentation_layer` | Classifies the document as `editable` or `generated`. |
| `frontmatter_fields` | Lists explicitly declared frontmatter keys for app-layer completeness checks. |
| `kind` | Records the documentation kind in page and section payloads. |
| `source_language` | Records the source language for future translation workflows. |
| `concepts` | Records declared concept stable keys in the page payload. |
| `entities` | Records declared canonical entity stable keys in the page payload. |
| `last_verified_snapshot` | Records the snapshot against which the document was verified. |
| `status` | Records the author-declared documentation status. |

Markdown without frontmatter remains compatible: page identity is path-derived, language remains
`markdown`, and the documentation layer defaults to `editable`. Paths under
`.athanor/generated/` default to `generated` when such a source is explicitly supplied.

When `kind` is `runbook` or `operations_runbook`, the adapter also emits a `Runbook` entity with a
`runbook://` stable key derived from the page identity. Its payload records the source
documentation page and copies frontmatter `entities` into `operation_targets` so checker adapters
can validate whether the runbook is tied to known operational knowledge.

The frontmatter bytes are excluded from CommonMark parsing while full-file line offsets are
preserved. YAML keys therefore cannot become false setext headings. `serde_yaml_ng` remains private
to the adapter. Malformed YAML, a missing closing delimiter, invalid language, or invalid explicit
page identity fails extraction with an input error.

## Evidence

Each section fact includes:

- source file
- heading line
- extractor name
- confidence
- verified status

## Ownership

Emitted page entities, section entities, runbook entities, and section facts are owned by the Markdown source file path.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Does not modify project files.

## Limitations

- Only heading structure is materialized; paragraphs, links, and code blocks are not emitted as
  separate canonical entities yet.
- Runbook extraction recognizes the page-level runbook identity and operation target declarations;
  individual operation steps are not materialized yet.
- Frontmatter concept/entity references use exact stable-key resolution; aliases and fuzzy matching
  are not supported.
- Stable slugs continue to use Athanor's existing slug algorithm rather than a renderer-specific
  anchor algorithm.

## Tests

```bash
cargo test -p athanor-extractor-markdown
```
