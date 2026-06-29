---
id: doc://docs/adapters/projector-wiki.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000255
source_language: en
status: verified
---
# Markdown Wiki Projector

Crate: `athanor-projector-wiki`

Port: `Projector`

## Purpose

Projects the latest canonical snapshot into a disposable neutral-language Markdown wiki for agents and developers. The generated wiki is a read model and never becomes canonical input.

## Input

`athanor-app::project_wiki` loads the latest snapshot from `CanonicalSnapshotStore` and passes a versioned `athanor.wiki_projection.v1` payload to `MarkdownWikiProjector`.

The payload contains:

- entities
- facts
- relations
- diagnostics
- the canonical snapshot id in `ProjectInput`

## Output

The default output is:

```text
.athanor/generated/current/wiki/
  index.md
  manifest.json
  entities/<entity-id>.md
  diagnostics/<diagnostic-id>.md
```

Only open diagnostics receive pages. Entity pages include matching facts, incoming and outgoing relations, and attached open diagnostics. Every page has YAML frontmatter with canonical identity, kind, neutral language, and snapshot id.

The projector uses `athanor-projector-support` to write a complete temporary sibling directory and rename it into place. Existing output is moved aside during the swap and removed only after the new wiki is installed. This prevents partial generations; on platforms that cannot replace a non-empty directory in one operation, the target can be briefly absent until the final rename completes.

## CLI

```bash
ath wiki
ath wiki <project-root>
ath wiki <project-root> --output <directory>
```

Relative output paths are resolved against the project root. The command does not index; it fails with a prompt to run `ath index` when no canonical snapshot exists.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Writes only below the selected output path and temporary sibling paths used for replacement.

## Limitations

- Emits neutral pages only; localized concept pages are deferred.
- Rebuilds the complete wiki on each invocation.
- Does not include source document bodies or HTML rendering.
- Projector plugin discovery and runtime registration are not implemented yet; the app service selects the built-in projector directly.

## Verification

```bash
cargo test -p athanor-projector-wiki
cargo run -p ath --quiet -- index .
cargo run -p ath --quiet -- wiki .
```
