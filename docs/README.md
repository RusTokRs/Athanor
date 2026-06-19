# Athanor Documentation Map

This is the documentation index for Athanor.

Use this file to find the right document before changing code, adapters, plugins, pipeline behavior, or generated artifacts.

## Entry Points

- [Agent entrypoint](../AGENTS.md): required first read for agents.
- [Full architectural plan](../start.md): long-form product and architecture plan.
- [Agent workflow](development/agent-workflow.md): required implementation workflow.
- [Roadmap status](development/roadmap-status.md): what is implemented, in progress, and next.

## Architecture

- [Indexing pipeline](architecture/pipeline.md): source discovery, extraction, linking, checking, storage, and JSONL export.
- [Adapter architecture](architecture/adapters.md): adapter-first boundaries and current adapter map.

## Development

- [Agent workflow](development/agent-workflow.md): read-before-coding, planning, documentation, verification, completion notes.
- [Definition of done](development/definition-of-done.md): required checks and documentation expectations.
- [Roadmap status](development/roadmap-status.md): current verified implementation status and next recommended task.

## Adapter Documentation

High-level adapter docs:

- [Basic extractor](adapters/extractor-basic.md)
- [Markdown extractor](adapters/extractor-markdown.md)
- [Markdown linker](adapters/linker-markdown.md)
- [Markdown checker](adapters/checker-markdown.md)

Crate-local adapter docs:

- [`athanor-extractor-basic`](../crates/athanor-extractor-basic/README.md)
- [`athanor-extractor-markdown`](../crates/athanor-extractor-markdown/README.md)
- [`athanor-linker-markdown`](../crates/athanor-linker-markdown/README.md)
- [`athanor-checker-markdown`](../crates/athanor-checker-markdown/README.md)

## Current Generated Read Model

The current CLI uses `JsonlReadModelWriter` to write generated JSONL read models to:

```text
.athanor/generated/current/jsonl/
  entities.jsonl
  facts.jsonl
  relations.jsonl
  diagnostics.jsonl
  manifest.json
```

Generated files are not source documentation. They are disposable read models.

## Documentation Rule

When code changes, documentation must be updated in the same task.

Typical mapping:

| Change | Update |
| --- | --- |
| Pipeline behavior | `architecture/pipeline.md`, `development/roadmap-status.md` |
| Adapter/plugin behavior | `architecture/adapters.md`, `docs/adapters/*.md`, crate `README.md` |
| CLI behavior | `development/roadmap-status.md`, relevant architecture doc |
| Definition of done or workflow | `development/agent-workflow.md`, `development/definition-of-done.md`, `AGENTS.md` |
| Roadmap progress | `development/roadmap-status.md` |

If documentation is added, renamed, removed, or its purpose changes, update this index in the same task.
