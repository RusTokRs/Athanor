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

### API Protocol Consistency

GraphQL operations use canonical `protocol = graphql`; OpenAPI operations use the symmetric
`protocol = openapi` boundary. The verified `API-001` package compares request bodies, parameters,
response schemas, status policy, authentication families, and permission scopes. Repository-owned
external references, OpenAPI security-alternative semantics, configurable GraphQL security mappings,
and compatible multi-root response selection are covered by source-level regressions.

### Documentation Lifecycle Policy

Required structural fields are `id`, `kind`, `language`, `source_language`, and `status`. Accepted
lifecycle values are `active`, `implemented`, `planned`, `draft`, and `verified`. Snapshot freshness
remains a separate drift concern.

### MCP Control Plane

Notifications bypass ordinary request admission. Inline reader-loop responses use nonblocking bounded
admission. Full response queues do not stop cancellation processing, and EOF cancels registered
operations before request-task drain.

### Exact Verification Evidence

The push workflows publish `athanor/verification-matrix`, `athanor/appsec`, and
`athanor/store-conformance` on the exact `main` SHA. `docs/development/verification-evidence.json`
records the CI identity. Workflow source without successful exact results remains implementation
evidence only.

Current verified product evidence covers commit
`f976239c0aa8b58abaf9222485bcf717a50c1ddf`: CI run `29943452118`, AppSec run `29943452179`, and
Store Conformance run `29943452289` all completed successfully.

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

- exact CI, AppSec, Store Conformance, and JSON evidence channels;
- repository-owned Rust 1.95 setup;
- cross-platform workspace, feature, coverage, installer, index, and docs verification;
- architecture and product packages verified on identified source commits.

### `API-001`

- canonical GraphQL/OpenAPI protocol identity;
- request, parameter, response, status, authentication, and permission comparison;
- repository-local external reference resolution and explicit remote-reference boundary;
- exact successful CI, AppSec, and Store Conformance evidence on the promoted `main` commit.

## Active Work

### `REL-001` — Release Readiness

1. [x] reject tags that do not match both binary package versions or Semantic Versioning;
2. [x] require a valid ISO calendar date and publish the maintained changelog notes;
3. [x] package the changelog and document the supported artifacts, checklist, and recovery policy;
4. [x] freeze the `0.1.0` changelog section with the intentional candidate date;
5. [x] reject duplicate matching version sections and heading-only release notes;
6. [ ] verify the first release candidate from one exact tag commit.

The static contract, calendar-valid date guard, unambiguous changelog selection, source-level
regressions, and dated candidate notes are implemented in `main`. `REL-001` remains active until an
intentional tag run proves the complete build, SBOM, signature, provenance, verification, and
publication chain.

## Product Backlog

- broader relationship and framework adapters;
- richer analysis completeness reporting;
- evidence-backed documentation generation;
- internationalization, concepts, and optional semantic/vector retrieval;
- additional Rustok, community-module, language, and framework integrations.

## History

Earlier roadmap revisions mixed snapshot metadata with current execution evidence. Historical detail
remains available in Git history, feature documentation, and `start.md`.
