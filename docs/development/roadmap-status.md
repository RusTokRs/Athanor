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

### Documentation Generation Contracts

`DocumentationGenerationRequest` and `DocumentationGenerationManifest` bind future publication to an
exact snapshot, supported profile, bounded limits, omitted counts, portable paths, and checksums.
`DocumentationOutline`, `DocumentationContext`, `DocumentationCitation`, `DocumentationDraft`, and
`DocumentationValidationReport` define the bounded evidence flow, data-handling policy, citation and
diagram traceability, deterministic quality metrics, and provider-cost accounting. No runtime
documentation generator or provider dependency is present yet.

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

Current verified product and release evidence covers commit
`609027eb02caa05346ebfea8538552c42b588c31`: CI run `29995959544`, AppSec run `29995960063`, and
Store Conformance run `29995959512` all completed successfully. Annotated tag `v0.2.1` points to that
commit; Release run `29996579628` and installation-smoke run `29998347890` also succeeded.

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

### `REL-001` — Release Readiness

1. [x] reject tags that do not match both binary package versions or Semantic Versioning;
2. [x] require a valid ISO calendar date and publish the maintained changelog notes;
3. [x] package the changelog and document the supported artifacts, checklist, and recovery policy;
4. [x] preserve immutable historical tags and publish annotated `v0.2.1` on the exact verified commit;
5. [x] reject duplicate sections, decorative-only notes, and invalid CycloneDX inventory content;
6. [x] complete contract, Linux/Windows build, SBOM, signature, provenance, verify, and publish jobs;
7. [x] verify clean Linux and Windows installations from the published archives.

`REL-001` is verified. Release run `29996579628` published `v0.2.1` with the complete supported asset
set. Installation-smoke run `29998347890` downloaded the public archives, verified their outer and
internal checksums, installed into clean isolated prefixes, and executed both `ath` and `athd` on Linux
and Windows. The historical `v0.1.0` and failed-attempt `v0.2.0` tags remain immutable.

## Active Work

### `DOCGEN-001` — Evidence-Backed Documentation Generation

- [x] Slice 0A: strict versioned request and manifest contracts;
- [x] fixture-backed round-trip, schema-drift, unknown-field, output-path, checksum, and alignment
  regressions;
- [x] schema ownership in the public JSON contract inventory;
- [x] Slice 0B: outline, bounded context, structured draft, citation, validation-report, data policy,
  quality metrics, minimal fixture repository, and Rustok evaluation corpus contracts;
- [ ] Slice 1: deterministic architecture profile and immutable generated publication.

The package is implemented through Slice 0B. It has no current exact-commit verification claim.

## Product Backlog

- broader relationship and framework adapters;
- richer analysis completeness reporting;
- evidence-backed documentation generation beyond the active bounded slice;
- internationalization, concepts, and optional semantic/vector retrieval;
- additional Rustok, community-module, language, and framework integrations.

## History

Earlier roadmap revisions mixed snapshot metadata with current execution evidence. Historical detail
remains available in Git history, feature documentation, and `start.md`.
