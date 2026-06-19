# Roadmap Status

This file tracks what has actually been implemented. It is intentionally separate from `start.md`, which is the full architectural plan.

Agent entrypoint: read `AGENTS.md` and `docs/development/agent-workflow.md` before implementation work.

## Implemented

### Workspace Skeleton

Status: verified.

Crates:

- `athanor-domain`
- `athanor-core`
- `athanor-app`
- `athanor-source-fs`
- `athanor-store-memory`
- `athanor-extractor-basic`
- `athanor-extractor-markdown`
- `athanor-linker-markdown`
- `athanor-checker-markdown`
- `apps/ath`

### CLI

Status: verified.

Implemented commands:

```bash
ath
ath --version
ath init
ath index
```

### Indexing Vertical Slice

Status: verified.

Implemented flow:

```text
local files
  -> file and Markdown extraction
  -> Markdown containment links
  -> Markdown structure diagnostics
  -> memory store
  -> JSONL export
```

Current runtime check:

```bash
cargo run -p ath --quiet -- index .
```

Recent observed output:

```text
files_indexed: 30
entities: 169
facts: 168
relations: 139
diagnostics: 0
```

## In Progress

None.

## Next

Recommended next task:

```text
Introduce IndexPipeline or RuntimeBuilder.
```

Why:

- `athanor-app/src/index.rs` currently assembles sources, extractors, linkers, and checkers directly.
- The list of adapters should become configurable and reusable.
- Future CLI, daemon, and tests should share the same pipeline assembly.

## Verification Commands

Run before marking implementation work as verified:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```
