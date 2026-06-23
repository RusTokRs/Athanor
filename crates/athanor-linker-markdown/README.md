# athanor-linker-markdown

Markdown containment and explicit-reference linker adapter.

Implements: `Linker`

## What It Emits

- `RelationKind::Contains`
- `RelationKind::Documents`

Relations include ownership copied from the related file/page/section/runbook/step entities.

Current relations:

```text
file contains documentation_page
documentation_page contains documentation_section
documentation_page contains runbook
runbook contains operation_step
documentation_page documents declared entity/concept
```

`documents` relations come from exact stable keys declared in Markdown frontmatter `entities` and
`concepts` lists. They are verified, include declaration evidence, and are owned by both the page
and target source paths.

## Inputs

Entities emitted by all extractors. The adapter receives full extracted context plus an
`AffectedSubset`, emits containment relations for affected documentation paths, and rebuilds an
explicit relation when either its page or target is affected.

## Side Effects

None.

This adapter does not run commands, does not use the network, and does not modify project files.

## Test

```bash
cargo test -p athanor-linker-markdown
```
