---
id: doc://docs/development/roadmap-status.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Roadmap Status

This compact ledger describes current `main`. Detailed evidence lives in
`athanor_implementation_plan_ru.md`; long-range work lives in `start.md`.

## Status Rules

- **Implemented** means code, documentation, and source regressions are present.
- **Verified** means the required format/build/test/Clippy/smoke/security/feature/coverage matrix
  succeeded on one exact source commit.
- Documentation metadata is not execution evidence.

## Current Architecture

### Exact Evidence Update

Slices 8F–8H are now **verified** on exact source commit `e246a71bcb094cb3553bf6770954a4617dc1a376`.
The focused gate is green across Verification Matrix `34577131919`, AppSec `34577131939`, Store Conformance
`34577131986`, and Athanor self-documentation evaluation `34577131980`. The exact self-evaluation reports
`685/731` processed files (`9370` bps); YAML is `15/16`. Rustok evaluation and sanitized probe are also green.

### Current Semantic Gap

Post-gate completeness shows the only remaining YAML gap is `crates/athanor-extractor-openapi/tests/fixtures/basic.openapi.yaml`.
The other 46 unprocessed files are generic JSON/test fixtures, OpenAPI fixture variants, `Cargo.lock`, or
non-semantic repository files and remain explicitly out of scope unless product semantics justify them.

## Active Work

### `DOCGEN-001` — Evidence-Backed Documentation Generation

- [x] Slices 0A–1C: contracts, architecture profile, immutable publication, exact Store, CLI/inspection;
- [x] repaired aggregate citation selection, exact Rustok evaluation, and relation disclosure;
- [x] Slices 2A–5C: module/API/operations/onboarding source surfaces and publication flows;
- [x] Slices 6A–6B: pure completeness plus exact Store/read-only CLI/versioned JSON transport;
- [x] Slices 7A–7C: bounded built-in Next.js, Axum, and Express route projections;
- [x] Slices 8A–8E source implementation plus exact completeness confirmation;
- [x] Slice 8F source implementation for `install.sh` and `verify_release_version.py`;
- [x] Slice 8G source implementation for the first-party `install.ps1` installer;
- [x] Slice 8H source implementation and hardening for bounded GitHub Issue Forms;
- [x] focused exact verification for Slices 8F–8H on `e246a71bcb094cb3553bf6770954a4617dc1a376`;
- [ ] focused verification for earlier profile/framework slices remains pending.

`DOCGEN-001` remains in progress. Slices 8F–8H are promoted to `Verified` by the exact-commit gate.
The next slice must be selected from the post-gate completeness artifact and should prefer useful first-party
product semantics over coverage-only targets.

## Product Backlog

- [x] focused verification for Slices 8F–8H and exact completeness/self-evaluation;
- [ ] select the next bounded semantic slice from the exact post-gate artifact;
- [ ] keep generic JSON/test fixtures out of scope unless independently justified by product semantics;
- [ ] OpenAPI fixture YAML remains excluded unless a separate evidence-backed product contract justifies it;
- [ ] keep Next.js/Axum/Express schemas/auth/middleware and route-composition expansion evidence-driven;
- Dart/Flutter remains blocked on a portable DartScope dependency boundary rather than a local-only path dependency;
- optional i18n, semantic/vector retrieval, provider, daemon, and MCP integration after quality gates.
