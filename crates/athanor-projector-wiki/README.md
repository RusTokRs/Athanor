# Markdown Wiki Projector Adapter

Crate: `athanor-projector-wiki`

Port: `Projector`

## Purpose

Builds a disposable, agent-readable Markdown wiki from canonical snapshot objects. The projector writes a neutral index plus one page per entity and open diagnostic.

## Input

Receives `ProjectInput` with an `athanor.wiki_projection.v1` payload containing canonical entities, facts, relations, and diagnostics from one snapshot.

## Output

Writes this replaceable read model below the requested target:

```text
index.md
manifest.json
entities/<entity-id>.md
diagnostics/<diagnostic-id>.md
```

Pages contain YAML frontmatter, source/evidence locations, canonical facts, related entities, and diagnostics where applicable. Output ordering and file names are deterministic. A complete staging directory is renamed into place so readers do not observe a partially written generation; on platforms that cannot replace a non-empty directory in one operation, the target can be briefly absent during the swap.

The adapter also exposes cooperative cancellation checkpoints while building index, entity, and
diagnostic files. Cancellation removes the staging directory and leaves the previously published
wiki unchanged.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Writes only to the configured target and temporary sibling directories used for replacement.

## Limitations

- The first slice emits neutral pages only; localized concept pages are deferred.
- It rebuilds the complete wiki rather than projecting only affected pages.
- It does not copy source documentation bodies or render HTML.

## Tests

```bash
cargo test -p athanor-projector-wiki
```
