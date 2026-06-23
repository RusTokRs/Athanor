---
id: doc://docs/adapters/linker-markdown.md
kind: module_documentation
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000030
status: verified
---

# Markdown Linker Adapter

Crate: `athanor-linker-markdown`

Port: `Linker`

## Purpose

Creates containment and explicit documentation relations for Markdown knowledge.

## Inputs

Reads entities emitted by extractors:

- `EntityKind::File`
- `EntityKind::DocumentationPage`
- `EntityKind::DocumentationSection`
- `EntityKind::Runbook`
- any canonical entity referenced by exact stable key from page payload `entities` or `concepts`

The linker receives the full extracted context plus an `AffectedSubset`. It emits containment relations only for documentation paths represented in the affected entities while still using full-context file/page/section entities to build valid relations.

## Outputs

Relations:

- `RelationKind::Contains`
- `RelationKind::Documents`

Current relation patterns:

```text
file contains documentation_page
documentation_page contains documentation_section
documentation_page contains runbook
documentation_page documents frontmatter-declared entity/concept
```

Each relation has:

- `status = verified`
- `confidence = 1.0`
- evidence pointing to the relevant source file/line when available
- ownership copied from the related file/page/section entities

Explicit `documents` relations additionally include:

- `reason = markdown_frontmatter_reference`
- the source frontmatter list in `reference_type`
- evidence pointing to the declaring Markdown page
- ownership from both the page and resolved target

Resolution is an exact canonical stable-key match. Ambiguous keys with more than one target do not
produce a relation and are handled by the Markdown checker. An explicit relation is recalculated
when either the declaring page or resolved target appears in the affected subset.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Does not modify project files.

## Tests

```bash
cargo test -p athanor-linker-markdown
```
