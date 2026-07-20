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

- **Implemented means** the code, documentation, and source-level regressions are present in `main`.
- **Verified** means the required formatting, build, test, Clippy, smoke, security, feature, and
  coverage matrix was executed successfully on one identified commit.
- Documentation lifecycle status and historical snapshot metadata are not execution evidence.
- A failed matrix is useful diagnostic evidence, but it does not promote implementation to verified.

## Current Architecture

### Explicit Runtime Composition

Production application services receive `RuntimeComposition` explicitly. Store, search, projector,
and adapter factories are selected through the composition root owned by `ath`, `athd`, or the MCP
host. Process-global installer APIs and public `store::init_store` have been removed.

Composition-only execution covers Index, Generation, Wiki, HTML, Search, Context, Explain, Change Map,
Overview, Capabilities, Impact, Coverage, Check, Graph, API contracts, Repair, Docs, and daemon work.

### Bounded Application Owners

- Change Map: `crates/athanor-app/src/change_map/`;
- Check: `crates/athanor-app/src/check/`;
- API contracts: `crates/athanor-app/src/api/`;
- Graph contracts and pure adapters: `crates/athanor-app/src/graph/`;
- Overview, Capabilities, Impact, and Coverage in focused module trees.

Graph contracts live in `graph/model.rs`, standard operations in `graph/standard.rs`, RusTok adapters
in `graph/rustok.rs`, and root regressions in `graph/tests.rs`. Traversal-heavy algorithms remain in
cooperative algorithm owners.

### Transactional Index Publication

Indexing runs through composition-aware entry points in `index_runtime.rs` and bounded pipeline phase
owners: `pipeline_source.rs`, `pipeline_extract.rs`, `pipeline_link.rs`, `pipeline_check.rs`,
`pipeline_merge.rs`, `pipeline_ownership.rs`, `pipeline_metrics.rs`, and `pipeline_support.rs`.

`index_publication.rs` and `index_publication_snapshot.rs` own staged read-model/state publication,
recovery journals, exact-snapshot commit probing, rollback before durable commit, and post-commit
success preservation. MCP does not apply a transport postflight after durable Index completion.

### JSON Contract Inventory

Current JSON ownership is divided into mutually disjoint public, general non-public, adapter, and
automation registries. Production Rust sources are scanned recursively. A new quoted `athanor.*`
schema literal must be registered or classified in the same change. CLI, daemon, and MCP Index
serialize one typed `IndexReport` contract.

### Documentation Lifecycle Policy

The root `athanor.toml`, `ProjectConfig::default`, and `ath init` use one lifecycle-aware completeness
policy. Required structural fields are `id`, `kind`, `language`, `source_language`, and `status`.
Accepted lifecycle values are `active`, `implemented`, `planned`, `draft`, and `verified`.

`last_verified_snapshot` is not a default completeness field. Snapshot freshness remains the separate
`ath docs drift` concern. The `ath init` template is parsed as the current `ProjectConfig`, and golden
config contracts reflect the same defaults.

### MCP Control Plane

The MCP stdin loop remains available while ordinary request slots are full. Notifications are handled
before ordinary admission. Inline reader-loop responses use nonblocking bounded admission. Ordinary
request tasks retain bounded response backpressure. EOF cancels registered operations before drain.

### Exact Verification Evidence

The main CI publishes legacy status `athanor/verification-matrix` on the exact push SHA after
aggregating security, quality, feature-matrix, and coverage jobs. A successful run may additionally
publish `docs/development/verification-evidence.json` with the same exact identity. Workflow source
without a successful exact result remains implementation evidence only.

### Workflow Toolchain Ownership

The workspace MSRV is Rust `1.95`. CI, production, and release now use the repository-owned
`.github/actions/setup-rust/action.yml`, which installs exact Rust `1.95.0` with optional components and
targets. Active workflows no longer use the version-encoded `dtolnay/rust-toolchain` commit that
silently selected Rust `1.100.0`.

Run `29701756503` on commit `9fda772436f50b55b2e4d9b11b18b7ec1e43a091` recorded failure before
project compilation: all quality/feature/coverage jobs stopped at the old toolchain setup. Cargo-deny
now emits a concise log and uploads `cargo-deny-diagnostics` when the security check fails.

## Implemented Architecture Packages

### `COMP-003` / `COMP-003C2B2C2B`

- explicit runtime dependency composition;
- composition-only read and write services;
- bounded Check, API, Graph, and related owners;
- removal of public Store initializer and legacy execution owners;
- source inventories preventing compatibility API reintroduction.

### `MCP-007`

- pre-commit cancellation and rollback;
- exact-snapshot reconciliation for commit-race terminal errors;
- post-commit durable success preservation;
- MCP cancellation registration without transport postflight;
- CLI, daemon, and MCP `IndexReport` parity.

### `JSON-003`

- recursive workspace schema inventory;
- public/general/adapter/automation lifecycle separation;
- qualified schema validation and legacy-input normalization;
- persisted/generated/interchange fixtures;
- typed public report transport parity.

### `DOC-001` / `DOC-002`

- snapshot-era aggregate verification claims removed;
- deleted monolith references replaced with bounded owner paths;
- pipeline current architecture, target work, and history separated;
- roadmap, pipeline guide, and implementation plan synchronized;
- status/path/alignment inventories added.

### `MCP-004`

- stdin/control input ordered before ordinary task reaping;
- notifications bypass ordinary admission;
- inline responses use nonblocking bounded admission;
- saturated response queue cannot stop cancellation processing;
- EOF cancellation and saturation regressions added.

### `VERIFY-001A`

- exact JSON and legacy commit-status evidence channels;
- successful status requires all required job results to succeed;
- evidence-only commits cannot recursively trigger CI;
- workflow, schema, and status contracts are source-enforced.

### `VERIFY-001B`

- completeness lifecycle separated from snapshot verification age;
- root/default/init policies share one current contract;
- `ath init` emits a parseable current configuration;
- golden config reports reflect lifecycle-aware defaults.

### `VERIFY-001C`

- repository-owned Rust 1.95 setup shared by CI, production, and release;
- explicit components/targets and bounded install retry;
- version-encoded third-party toolchain setup removed;
- cargo-deny failure diagnostics retained as an artifact;
- `workflow_toolchain_inventory` prevents regression.

## Active Work

### `VERIFY-001`

The remaining architecture task is one exact successful matrix:

1. a new final push-CI reaches the actual Rust checks;
2. any remaining failure is resolved from exact job logs or diagnostic artifacts;
3. `athanor/verification-matrix` reports success, or valid JSON evidence is published;
4. only packages covered by that exact SHA are promoted to verified.

Until a successful exact result exists, the architecture remains implemented, not verified.

## Product Backlog

- deeper GraphQL and cross-protocol API consistency;
- broader relationship and framework adapters;
- richer analysis completeness reporting;
- evidence-backed documentation generation;
- release-readiness consolidation;
- internationalization, concepts, and optional semantic/vector retrieval;
- additional Rustok, community-module, language, and framework integrations.

## History

Earlier roadmap revisions mixed per-feature snapshot metadata with current execution evidence and
referenced owners that were later decomposed or deleted. This ledger records current architecture and
directs historical detail to Git history, feature documentation, and `start.md`.
