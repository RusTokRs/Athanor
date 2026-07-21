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
host. Process-global installer APIs and public Store initialization have been removed.

### Bounded Application Owners

Check, API, Graph, Change Map, Overview, Capabilities, Impact, Coverage, Docs, Repair, and daemon work
use focused owners. API contracts live under `crates/athanor-app/src/api/`; extraction and consistency
remain adapter-owned.

### Transactional Index Publication

Indexing runs through composition-aware entry points and bounded pipeline phases. Staged read-model
and state publication preserve rollback before durable commit and successful results after commit.

### JSON Contract Inventory

Public, general non-public, adapter, and automation registries are mutually disjoint. Production Rust
sources are scanned recursively. CLI, daemon, and MCP Index serialize one typed `IndexReport`.

### API Protocol Identity

GraphQL operations use canonical `protocol = graphql`; OpenAPI operations use the symmetric
`protocol = openapi` boundary. Request bodies, parameters, repository-owned external references, and
response schema shapes are implemented. The active fourth `API-001` slice adds effective OpenAPI
security requirements, GraphQL directive argument values, and status/authentication/permission drift.

### Documentation Lifecycle Policy

Required structural fields are `id`, `kind`, `language`, `source_language`, and `status`. Accepted
lifecycle values are `active`, `implemented`, `planned`, `draft`, and `verified`. Snapshot freshness
remains a separate drift concern.

### MCP Control Plane

Notifications bypass ordinary request admission. Inline reader-loop responses use nonblocking bounded
admission. Full response queues do not stop cancellation processing, and EOF cancels registered
operations before request-task drain.

### Exact Verification Evidence

The main CI publishes `athanor/verification-matrix` on the exact push SHA after aggregating security,
quality, feature, and coverage jobs. `docs/development/verification-evidence.json` records the same
identity. Workflow source without a successful exact result remains implementation evidence only.

Current verified architecture evidence is run `29836572040` on commit
`c4a494f3a1c1af5dcbad4252c5eb69e00d558b3a`.

## Implemented Architecture Packages

### `COMP-003` / `COMP-003C2B2C2B`

- explicit runtime dependency composition;
- composition-only read and write services;
- bounded Check, API, Graph, and related owners;
- removal of compatibility execution owners.

### `MCP-007`

- pre-commit cancellation and rollback;
- exact-snapshot reconciliation for commit races;
- post-commit durable success preservation;
- CLI, daemon, and MCP report parity.

### `JSON-003`

- recursive workspace schema inventory;
- lifecycle separation and legacy-input normalization;
- persisted, generated, and interchange fixtures;
- typed public report parity.

### `DOC-001` / `DOC-002`

- stale aggregate claims removed;
- deleted owners replaced with current bounded paths;
- current, target, and history documentation separated;
- status and line-budget inventories added.

### `MCP-004`

- control input ordered before ordinary task reaping;
- notifications bypass ordinary admission;
- saturated response queues cannot stop cancellation;
- EOF cancellation regressions added.

### `VERIFY-001`

- exact status and JSON evidence channels;
- repository-owned Rust 1.95 setup;
- cross-platform workspace, feature, coverage, installer, index, and docs verification;
- architecture audit verified on one exact source commit.

## Active Work

### `API-001`

The architecture audit remains closed. Product development now advances GraphQL and cross-protocol API
consistency in bounded slices:

1. [x] normalize OpenAPI endpoint protocol identity at the adapter boundary;
2. [x] verify normalized-name response-field comparison on real canonical entities;
3. [x] compare request-body arguments and named input types;
4. [-] resolve repository-owned external schemas and compare OpenAPI parameters and response shapes;
5. [ ] extend compatibility to status codes, authentication, and permissions;
6. [ ] promote the package only after one exact successful matrix covers its complete Definition of Done.

## Product Backlog

- broader relationship and framework adapters;
- richer analysis completeness reporting;
- evidence-backed documentation generation;
- release-readiness consolidation;
- internationalization, concepts, and optional semantic/vector retrieval;
- additional Rustok, community-module, language, and framework integrations.

## History

Earlier roadmap revisions mixed snapshot metadata with current execution evidence. Historical detail
remains available in Git history, feature documentation, and `start.md`.
