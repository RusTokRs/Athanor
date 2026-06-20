# athanor-linker-markdown

Markdown containment linker adapter.

Implements: `Linker`

## What It Emits

- `RelationKind::Contains`

Relations include ownership copied from the related file/page/section entities.

Current relations:

```text
file contains documentation_page
documentation_page contains documentation_section
```

## Inputs

Entities emitted by file and Markdown extractors. The adapter receives full extracted context plus an `AffectedSubset` and emits containment relations only for affected documentation paths.

## Side Effects

None.

This adapter does not run commands, does not use the network, and does not modify project files.

## Test

```bash
cargo test -p athanor-linker-markdown
```
