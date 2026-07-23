---
id: doc://docs/development/roadmap-status.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Roadmap Status

This document is the compact status ledger for current `main`. Detailed evidence lives in
`athanor_implementation_plan_ru.md`; long-range product ideas live in `start.md`.

## Status Rules

- **Implemented means** code, documentation, and source-level regressions are present in `main`.
- **Verified means** the required formatting, build, test, Clippy, smoke, security, feature, and
  coverage matrix succeeded on one identified source commit.
- Documentation lifecycle metadata is not execution evidence.
- A failed matrix is diagnostic evidence and never promotes a package.

## Current Architecture

### Explicit Runtime Composition

Production application services receive `RuntimeComposition` explicitly. Store, search, projector,
extractor, and transport factories are selected by the `ath`, `athd`, or MCP composition root.
Process-global installers and public Store initialization have been removed.

### Bounded Application Owners

Check, API, Graph, Change Map, Overview, Capabilities, Impact, Coverage, Docs, Repair, documentation
architecture, publication, and daemon work use focused owners. Extraction and protocol consistency
remain adapter-owned.

### Transactional Index Publication

Indexing uses bounded phases and staged read-model/state publication. Pre-commit failure rolls back;
durable post-commit success is not masked by late cancellation.

### JSON Contract Inventory

Public, general non-public, adapter, and automation registries are disjoint. Production Rust sources
are scanned recursively. CLI, daemon, and MCP Index serialize one typed `IndexReport`.

### Evidence-Backed Documentation Generation

Strict request, manifest, outline, context, citation, draft, and validation contracts bind output to
one canonical snapshot and hard limits. The deterministic architecture profile emits cited Markdown,
relation-backed Mermaid source, omission disclosure, and evidence footnotes. The app-layer publisher
writes immutable generations and atomically replaces `documentation/current.json` only after exact
artifact and checksum validation.

No CLI, daemon, MCP, provider, or store-loading entrypoint exposes this profile yet.

### Documentation Lifecycle Policy

Required document fields are `id`, `kind`, `language`, `source_language`, and `status`. Accepted values
are `active`, `implemented`, `planned`, `draft`, and `verified`. Snapshot freshness is a separate concern.

### MCP Control Plane

Notifications bypass ordinary admission. Inline reader responses use nonblocking bounded admission.
Full response queues do not stop cancellation, and EOF cancels registered operations before drain.

### Exact Verification Evidence

Push workflows publish `athanor/verification-matrix`, `athanor/appsec`, and
`athanor/store-conformance` for an exact source SHA. `docs/development/verification-evidence.json`
records the CI identity. Workflow source without successful exact results remains implementation
evidence only.

Current released product evidence remains commit `609027eb02caa05346ebfea8538552c42b588c31` with CI
`29995959544`, AppSec `29995960063`, Store `29995959512`, release `29996579628`, and clean-install
smoke `29998347890`.

Current documentation-generation source evidence covers commit
`0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`: Verification Matrix `30013208011`, AppSec
`30013208197`, and Store Conformance `30013208312` succeeded.

## Implemented Architecture Packages

### `COMP-003` / `COMP-003C2B2C2B`

- explicit runtime dependency composition;
- composition-only read/write services;
- bounded application owners;
- compatibility execution owners removed.

### `MCP-007`

- cancellation-safe pre-commit rollback;
- exact-snapshot commit-race reconciliation;
- durable post-commit success preservation;
- CLI, daemon, and MCP report parity.

### `JSON-003`

- recursive schema inventory;
- lifecycle separation and legacy normalization;
- persisted, generated, interchange, and automation fixtures;
- typed public report parity.

### `DOC-001` / `DOC-002`

- stale aggregate claims removed;
- current owners replace deleted paths;
- current, target, and history documentation separated;
- status and line-budget inventories enforced.

### `MCP-004`

- control input ordered before ordinary task reaping;
- notifications bypass ordinary admission;
- saturated response queues cannot stop cancellation;
- EOF cancellation regressions.

### `VERIFY-001`

- exact CI, AppSec, Store, and JSON evidence channels;
- repository-owned Rust 1.95 setup;
- cross-platform workspace, feature, installer, index, docs, and coverage verification.

### `API-001`

- canonical GraphQL/OpenAPI identity;
- request, response, status, authentication, and permission consistency;
- repository-local external reference resolution;
- exact successful CI/AppSec/Store evidence.

### `REL-001`

- immutable version tags and changelog gates;
- Linux/Windows archives, checksums, signatures, provenance, and CycloneDX SBOM;
- clean Linux and Windows installation verification for `v0.2.1`.

## Active Work

### `DOCGEN-001` — Evidence-Backed Documentation Generation

- [x] Slice 0A: strict request and manifest contracts;
- [x] Slice 0B: bounded evidence flow, data policy, quality metrics, fixture repository, and Rustok
  evaluation corpus;
- [x] Slice 1A: deterministic architecture outline/context/draft/Markdown/validation composition;
- [x] Slice 1B: immutable application-layer generation directories, exact artifact reuse, atomic current
  pointer, force, tamper recovery, and cancellation regressions;
- [x] Slices 1A–1B exact evidence on `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`: CI
  `30013208011`, AppSec `30013208197`, Store `30013208312`;
- [ ] Slice 1C: exact committed-snapshot loading and bounded CLI generation/inspection surface.

Slices 0A–1B are execution-confirmed. `DOCGEN-001` remains in progress because no supported user-facing
command loads and publishes the profile from a committed project snapshot yet.

## Product Backlog

- broader relationship and framework adapters;
- richer analysis completeness reporting;
- additional documentation profiles after Slice 1C;
- internationalization, concepts, and optional semantic/vector retrieval;
- additional Rustok and community integrations.

## History

Earlier revisions mixed snapshot metadata with current execution evidence. Historical detail remains
in Git history, feature documentation, and `start.md`.
