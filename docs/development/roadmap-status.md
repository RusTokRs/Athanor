---
id: doc://docs/development/roadmap-status.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Roadmap Status

This document is the compact status ledger for the current `main` branch. The detailed work log and
verification matrix live in `athanor_implementation_plan_ru.md`; long-range product ideas live in
`start.md`.

## Status Rules

- **Implemented** means the code, documentation, and source-level regressions are present in `main`.
- **Verified** means the required formatting, build, test, Clippy, and smoke matrix was executed on one
  identified commit.
- Historical snapshot metadata in individual documentation pages is content metadata. It is not
  execution evidence for the current repository commit.
- The current direct-to-main architecture work remains implemented, not verified, until one complete
  matrix succeeds on the same commit.

## Current Architecture

### Explicit Runtime Composition

Production application services receive `RuntimeComposition` explicitly. Store, search, projector,
and adapter factories are selected through the composition root owned by `ath`, `athd`, or the MCP
host. Process-global installer APIs and the public `store::init_store` compatibility initializer have
been removed.

Composition-only execution covers Index, Generation, Wiki, HTML reporting, Search, Context, Explain,
Change Map, Overview, Capabilities, Impact, Coverage, Check, Graph, API contracts, Repair, Docs, and
daemon work.

### Bounded Application Owners

The following former large owners have conventional module boundaries without forwarding facades:

- Change Map: `crates/athanor-app/src/change_map/`;
- Check: `crates/athanor-app/src/check/`;
- API contracts: `crates/athanor-app/src/api/`;
- Graph contracts and pure adapters: `crates/athanor-app/src/graph/`;
- Overview, Capabilities, Impact, and Coverage in their focused module trees.

The deleted Graph monolith is not an active source path. Graph contracts live in `graph/model.rs`,
standard operations in `graph/standard.rs`, RusTok adapters in `graph/rustok.rs`, and root regressions
in `graph/tests.rs`. Traversal-heavy algorithms remain in cooperative algorithm owners.

### Transactional Index Publication

Indexing runs through composition-aware entry points in `index_runtime.rs` and the bounded pipeline
phase owners:

- `pipeline_source.rs`;
- `pipeline_extract.rs`;
- `pipeline_link.rs`;
- `pipeline_check.rs`;
- `pipeline_merge.rs`, `pipeline_ownership.rs`, `pipeline_metrics.rs`, and `pipeline_support.rs`.

`index_publication.rs` and `index_publication_snapshot.rs` own staged read-model/state publication,
persisted recovery journals, exact-snapshot commit probing, rollback before durable commit, and
post-commit success preservation. MCP registers Index cancellation but does not apply a transport
postflight after the application future has crossed the durable commit boundary.

### JSON Contract Inventory

Current JSON ownership is divided into mutually disjoint registries:

- 60 public application and daemon contracts;
- 30 general persisted, generated, interchange, and embedded contracts;
- two current and two legacy-input adapter contracts;
- native schema-less MCP JSON-RPC and external-process protocol inventories.

Production Rust sources under `crates/*/src` and `apps/*/src` are scanned recursively. A new quoted
`athanor.*` schema literal must be registered or explicitly classified in the same change. CLI,
daemon, and MCP Index paths serialize one typed `IndexReport` contract.

### MCP Control Plane

The MCP stdin loop remains available while ordinary request slots are full. Notifications are handled
before ordinary admission. Inline reader-loop responses use nonblocking bounded admission so a full
response queue cannot terminate or suspend the only input reader. Ordinary request tasks retain
bounded response backpressure. EOF cancels registered operations before request-task drain.

## Implemented Architecture Packages

### `COMP-003` / `COMP-003C2B2C2B`

- explicit runtime dependency composition;
- removal of process-global runtime factory installation;
- composition-only read and write services;
- bounded Check, API, Graph, and related read owners;
- removal of the public Store initializer and legacy Graph/Docs execution owners;
- source inventories preventing compatibility API reintroduction.

### `MCP-007`

- pre-commit operation cancellation and deadline checks;
- rollback and snapshot abort before durable canonical commit;
- exact-snapshot reconciliation for commit-race terminal errors;
- post-commit durable success preservation;
- MCP cancellation registration without transport postflight;
- CLI, daemon, and MCP `IndexReport` payload parity guards.

### `JSON-003`

- recursive workspace schema emitter inventory;
- public/non-public/adapter lifecycle separation;
- qualified schema ID validation for feature-specific wire families;
- adapter legacy-input normalization;
- persisted/generated/interchange fixtures;
- schema-less external process protocol inventory;
- typed public report transport parity.

### `DOC-001` / `DOC-002`

- aggregate snapshot-era verification claims removed from the roadmap;
- deleted monolith references replaced with bounded owner paths;
- pipeline current architecture, target work, and history separated;
- roadmap, pipeline guide, and implementation plan synchronized;
- source inventory added for status, path, section, alignment, and line-budget invariants.

### `MCP-004`

- stdin/control input ordered before ordinary task reaping in the biased select loop;
- notifications bypass ordinary request admission and response production;
- inline parse, initialize, and overload responses use nonblocking bounded admission;
- a saturated response queue cannot stop cancellation notification processing;
- EOF cancels registered operations before saturated request-task drain;
- focused saturation, protocol-error, overload, and disconnect regressions added;
- source inventory enforces routing, response admission, cancellation, and line budgets.

## Active Work

### `VERIFY-001`

Run one complete matrix on one commit:

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run -p ath --quiet --locked -- --help
cargo run -p ath --quiet --locked -- index .
```

Focused architecture inventories listed in `athanor_implementation_plan_ru.md` must run in the same
matrix. Only then may implemented items be promoted to verified for that commit.

## Product Backlog

The remaining product backlog is intentionally separate from architecture-cleanup status:

- deeper GraphQL and cross-protocol API consistency;
- broader relationship and framework adapters;
- richer analysis completeness reporting;
- evidence-backed documentation generation;
- release-readiness consolidation;
- internationalization and concept mapping;
- optional semantic/vector retrieval;
- additional Rustok and community-module integrations;
- deeper language and framework support.

These items should not be described as current implementation gaps in the composition, publication,
or JSON-contract packages unless their source owners actually change.

## History

Earlier roadmap revisions accumulated hundreds of per-feature `Status: verified` entries tied to an
old canonical snapshot. Those entries mixed product history with current execution evidence and
referenced owners that were later decomposed or deleted. This ledger now records current architecture
and directs historical detail to Git history, feature documentation, and `start.md`.
