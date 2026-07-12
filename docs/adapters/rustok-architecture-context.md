---
id: doc://docs/adapters/rustok-architecture-context.md
kind: adapter
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# RusTok Architecture Context

The RusTok architecture context is a bounded application read model over Athanor's latest canonical
snapshot. It does not add another source-of-truth registry and does not parse RusTok files directly.
It combines generic Rust, JavaScript/TypeScript, API, documentation, and test knowledge with opt-in
RusTok FFA/FBA entities and relations.

The primary agent interface is the MCP tool:

```text
rustok_architecture_context
```

The equivalent human and CI inspection command is:

```bash
ath rustok architecture context "catalog search facets" --module search --path D:\RusTok --json
```

The stable response schema is `athanor.rustok_architecture_context.v1`. The report contains:

- an explicit `resolved`, `ambiguous`, or `unresolved` ownership resolution;
- bounded module candidates and evidence reasons;
- public FBA contracts, ports, operations, FFA surfaces, and indexed API contracts;
- declared FBA consumer/provider interactions;
- linked or source-owned tests and open diagnostics;
- bounded source evidence and omitted counts;
- short implementation guidance that tells an agent to reuse a public owner contract or resolve
  ownership before adding domain behavior.

An explicit module selector anchors the result. Without one, comparable candidate scores produce an
`ambiguous` result rather than guessing ownership. Empty limits and requests without either intent or
module fail with actionable input errors.

The read model is implemented in `crates/athanor-app/src/rustok_architecture.rs`. CLI formatting
stays in `apps/ath`; MCP schema and transport mapping stay in `athanor-transport-mcp`. The feature
works without MCP through the CLI and works offline from the latest committed canonical snapshot.

## Project Setup

RusTok repositories should opt in to existing FFA, FBA, and Page Builder adapters through tracked
`.athanor/adapters/*.json` manifests. Generated snapshots, state, and read models under `.athanor`
remain ignored and rebuildable.

Run a writable index before the first context query:

```bash
ath index D:\RusTok
ath rustok architecture context "catalog search facets" --path D:\RusTok --json
```

## Limitations

- Semantic ownership that is neither declared nor evidenced by canonical relations remains
  ambiguous.
- The first slice materializes declared FBA dependency edges; event publisher/subscriber and
  integration-runtime evidence need canonical extractor/linker support before this read model can
  report them as verified interactions.
- Ranking is deterministic lexical and graph-based context selection, not model inference.

## Verification

```bash
cargo test -p athanor-app rustok_architecture --quiet
cargo test -p athanor-transport-mcp --quiet
cargo check -p ath --quiet
ath index D:\RusTok
ath rustok architecture context "catalog search facets" --path D:\RusTok --json
```
