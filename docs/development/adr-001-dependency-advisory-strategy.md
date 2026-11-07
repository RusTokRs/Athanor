---
id: doc://docs/development/adr-001-dependency-advisory-strategy.md
kind: adr
language: en
source_language: en
status: draft
---

# ADR-001: Dependency and advisory remediation strategy

Status: proposed

## Context

The workspace pins a Rust 1.95 toolchain and runs `cargo-deny check` (advisories, licenses,
bans, sources) in CI on every commit. As of 2026-10-07 the repository tracks seven open
RUSTSEC advisory issues in transitive dependencies (#83–#88): `memmap2`
(RUSTSEC-2026-0186), `LruCache` (RUSTSEC-2026-0253), `bincode` (RUSTSEC-2025-0141,
unmaintained), `atomic-polyfill` (RUSTSEC-2023-0089, unmaintained), `event-listener`
(RUSTSEC-2026-0221), and a Stacked Borrows violation in an `IterMut` API
(RUSTSEC-2026-0002). The main sources of these transitives are the optional
`store-surreal` feature (`surrealdb` 2.6.5, BUSL-1.1 with a `deny.toml` license
exception) and the default-on `athanor-search-tantivy` (`tantivy` → `memmap2`).

Dependabot opens weekly cargo and github-actions update PRs, but there is no automerge,
so roughly ten dependency-bump PRs sit open at any time. `deny.toml` currently ignores a
single advisory (RUSTSEC-2026-0235, unused optional `rkyv` via `rust_decimal`) without a
recorded review date or strategy. Without a written policy, every new advisory blocks CI
while nobody owns the decision, and ignored advisories accumulate silently.

Constraints:

- The default CLI/daemon build must stay free of known-vulnerable transitives.
- `surrealdb` remains optional behind `store-surreal` and environment-backed; promoting
  or replacing it is a product decision, not a hygiene shortcut.
- All CI verification uses `--locked`, so dependency changes must land together with an
  updated `Cargo.lock` produced by `cargo update` on a machine with a Rust toolchain.
- The release workflow is gated on `cargo-deny` plus AppSec (CodeQL, Zizmor, Gitleaks);
  regressions there block signed releases.

## Decision

1. **Upgrade first.** Prefer updating the direct dependency that pulls a vulnerable
   transitive (`tantivy`, `surrealdb`, `tree-sitter`, `oxc_*`) within semver-compatible
   ranges. Major bumps (e.g. `surrealdb` 2 → 3) require their own reviewed PR with a
   conformance-suite run, not a hygiene drive-by.
2. **Ignore deliberately, visibly, and temporarily.** When no upstream fix exists, add
   the advisory to `deny.toml` `advisories.ignore` with a comment linking the tracking
   issue and a review date (at most one quarter out). Ignored advisories are re-reviewed
   at every release and every quarter; an ignore without a date is a CI failure in review.
3. **Automate safe dependency PRs.** Dependabot patch and minor update PRs (cargo and
   github-actions ecosystems) are auto-merged after a green required-checks run via
   `.github/workflows/automerge.yml`. Major dependency bumps and workflow changes stay
   manual.
4. **Keep the default build clean.** If an advisory only reaches the default build
   through an optional feature (today: none — `memmap2` arrives via the default-on
   Tantivy search), the owning adapter either upgrades, replaces the transitive, or the
   feature becomes non-default with a recorded rationale. The default `ath`/`athd`
   binaries must pass the target-aware `cargo-deny` advisory check with no unignored advisories.
5. **Weekly hygiene gate.** The existing `Security & License Checks` CI job remains the
   enforcement point; failing `cargo-deny` output is attached as a workflow artifact for review.
   The scheduled Security Audit uses the same native-target graph and reports failures through issues.

## Consequences

- Benefits: advisories get an owner and a clock; dependency PR backlog stops growing;
  the default build has a stated, checkable security invariant; `deny.toml` ignores
  become auditable instead of silent.
- Tradeoffs: the automerge workflow adds a maintainer responsibility to review
  auto-merged bumps via commit history; ignored advisories need quarterly discipline.
- Compatibility: no public API or contract changes. `Cargo.lock` changes are expected
  only alongside dependency PRs produced by `cargo update`.
- Migration plan: (1) merge the automerge workflow; (2) work through issues #83–#88 by
  upgrading `tantivy`/`surrealdb`/`tree-sitter`/`oxc_*` or recording dated ignores;
  (3) re-run `cargo-deny check`, the feature matrix, and the store conformance suite;
  (4) record exact CI run ids in the verification ledger.
- Verification: `cargo deny check` green with zero unignored advisories for supported native
  targets, `cargo test --workspace --locked` green on Linux, Windows, and macOS.

## Follow-up: SurrealDB security upgrade (2026-11-07)

The optional store adapter now targets SurrealDB 4.0.0 and a matching remote test server instead
of 2.6.5. This removes the vulnerable `surrealdb-core`/SurrealKV versions and the `reblessive`
transitive without excluding the adapter from advisory checks or adding advisory ignores. The
major SDK change requires typed database record IDs and explicit conversions at the adapter
boundary; the canonical Athanor JSON representation remains unchanged. Persistent SurrealDB 2
files must be backed up and migrated with a supported procedure before upgrading the engine.

SurrealDB 4 also depends on `jsonwebtoken/rust_crypto` **only on WebAssembly**. That feature adds
`rsa 0.9.10` (RUSTSEC-2023-0071, no patched stable release) and the older `crypto-bigint` (a
separate timing advisory) to the cross-platform lockfile. Athanor does not ship or test WebAssembly
and uses the native `aws_lc_rs` JWT backend. `deny.toml` therefore selects the native Linux,
Windows, and macOS target graphs for *all* cargo-deny checks; no advisory is ignored for a
supported target. Native-target coverage must be extended before shipping any additional target,
notably WebAssembly. Lockfile-only `cargo audit` does not distinguish target dependencies and
cannot substitute for this target-aware policy. The reviewed BUSL-1.1 exceptions for SurrealDB 4
are license-specific and version-pinned; they do not change advisory enforcement.
