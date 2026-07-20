---
id: doc://docs/development/ci.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Continuous Integration

Athanor defines workflows for quality, optional features, store conformance, coverage, AppSec,
installers, and releases.

## Evidence rules

Workflow YAML is implementation evidence, not execution evidence. A package may be described as
verified only when one completed successful CI run identifies the exact commit that was checked.

The `CI` workflow runs on pushes to `main`, pull requests, and manual dispatch. Its final
`verification-status` job uses `if: always()` and depends on security, quality, feature-matrix, and
coverage. For a push to `main`, it publishes the legacy commit status
`athanor/verification-matrix` on the exact `GITHUB_SHA`:

- `success` only when every required job result is `success`;
- `failure` when any required result failed, was cancelled, or was skipped;
- a target URL pointing to the exact workflow run.

The status job has only `contents: read` and `statuses: write`. This channel is visible through the
commit-status API even when check-run listing is unavailable to a client.

After a successful push run for `main`, `.github/workflows/verification-evidence.yml` also records:

- schema `athanor.verification_evidence.v1`;
- the exact CI `head_sha`;
- workflow run id and URL;
- conclusion and completion timestamp;
- the matrix represented by that run.

The evidence is written to `docs/development/verification-evidence.json` by a dedicated
`workflow_run` job with `contents: write`. Only successful `push` runs whose `head_branch` is `main`
are eligible. Pull-request runs and failed or cancelled runs cannot publish evidence.

The evidence-only commit changes only that JSON file. The main CI workflow ignores that path, which
prevents an evidence commit from recursively creating another CI run. The evidence file proves the
recorded `head_sha`; it does not automatically prove unrelated later changes.

A remediation workflow may publish a validated source commit through the repository `GITHUB_TOKEN`
without recursively starting a second CI run. That source commit remains implemented, not verified,
until an explicit dispatch or a subsequent non-recursive evidence/documentation push runs the full
matrix on a descendant HEAD containing the same source. A successful run for the workflow commit
must never be attributed to the later bot-published source commit.

A verified claim must cite an exact SHA with either a successful `athanor/verification-matrix` status
or a valid versioned evidence file for the same successful CI run. If neither channel is present and
valid, the current architecture status remains implemented, not verified.

## Toolchain ownership and failure diagnostics

The workspace MSRV is Rust `1.95`. CI, production, and release install it through the repository-owned
`.github/actions/setup-rust/action.yml`. The action installs exactly `1.95.0`, accepts optional rustup
components and targets, retries transient installation failures, and prints the installed Rust and
Cargo versions.

Run `29701756503` for commit `9fda772436f50b55b2e4d9b11b18b7ec1e43a091` recorded a failed
`athanor/verification-matrix`. Quality, feature-matrix, and coverage jobs stopped at toolchain setup
before project compilation. The previous immutable `dtolnay/rust-toolchain` commit encoded toolchain
`1.100.0`; its action contract did not expose the workflow's `toolchain: 1.95.0` field as an input, so
that field was ignored. The version-encoded action is no longer used by active workflows.

Cargo-deny is downloaded directly as the pinned `0.20.2` release binary. Its output is concise, and a
failed check uploads `cargo-deny-diagnostics` for retrieval instead of burying the result in a Docker
build log. `workflow_toolchain_inventory.rs` enforces the local setup, workflow coverage, diagnostic
artifact, and owner line budgets.

## Workspace quality

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

The quality matrix is configured for Linux, Windows, and macOS with Rust `1.95.0`. Optional Linux
slices cover default, `store-surreal`, `js-ts-precision`, and `--all-features` with
`fail-fast: false`.

## Store conformance

```bash
cargo test -p athanor-store-memory --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-store-surrealdb --locked

cargo test -p athanor-store-memory --test conformance --locked
cargo test -p athanor-store-jsonl --test conformance --locked
cargo test -p athanor-store-surrealdb --test conformance --locked

cargo test -p athanor-store-memory --test atomic_publication --locked
cargo test -p athanor-store-jsonl --test atomic_publication --locked
cargo test -p athanor-store-surrealdb --test atomic_publication --locked
cargo test -p athanor-store-surrealdb --test allocation_recovery --locked
```

The shared contracts cover committed query isolation, prepared lifecycle, and complete-batch plus
marker publication. Backend regressions additionally cover JSONL marker corruption/latest-pointer
failure, SurrealDB transaction rollback, and SurrealDB allocation recovery.

### Generation identity checks

```bash
cargo test -p athanor-domain generation --locked
cargo test -p athanor-app index_publication_journal --locked
cargo test -p athanor-app read_model --locked
cargo test -p athanor-app index_state --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-store-jsonl --test atomic_publication --locked
```

Every new publication derives `GenerationId` deterministically from `SnapshotId`. JSONL canonical
marker v2, SurrealDB committed snapshot record, read-model manifest, index state, and recovery journal
v3 must carry the same identity. JSONL marker v1, journal v1/v2, and index state without generation
remain readable during the migration window. A present mismatched generation fails closed.

### Allocation recovery expectations

The SurrealDB allocation suite requires:

- context-owned allocation metadata is persisted with operation id and Unix timestamp;
- a cutoff below allocation time removes nothing;
- an explicit stale cutoff removes the record;
- cleanup is bounded by the requested limit and the backend cap of 128;
- destructive delete repeats repo, age, committed, and prepared predicates;
- prepared and committed records survive cleanup;
- repeated cleanup is a no-op.

The automatic production sweep uses a minimum age of 24 hours and runs before the next context-aware
allocation for the same repo. Legacy records without allocation metadata are not automatically
removed.

## Deferred data and publication coordinator

```bash
cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-app --test deferred_canonical_buffer --locked
cargo test -p athanor-app index_runtime_tests --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-app index_publication_finalize_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p athanor-app index_publication_content_tests --locked
cargo test -p athanor-app index_publication_combined_error_tests --locked
```

The application facade buffers context-aware full batches process-locally. Deferred execution must not
create canonical rows, an exact JSONL generation, or a prepared directory before the durable
publication journal. The active coordinator writes journal v3, stages generation-bearing read
model/state, publishes the explicit complete batch through `AtomicSnapshotPublication`, and
exact-probes the journal snapshot after errors.

## JSONL policy

Atomic JSONL manifests declare `commit_marker_schema: athanor.canonical_commit.v2`. Exact/latest loads
require a parseable marker with matching schema, snapshot, and generation identity. Legacy v1 markers
and manifests without the declaration remain readable during the migration window. A failed
`latest.json` update after exact rename must leave the exact generation readable and non-abortable.

## SurrealDB policy

SurrealDB atomic publication deletes old rows, inserts the complete replacement batch, and updates
`prepared = true`, `committed = true`, and deterministic `generation` in one transaction. Statement
failure must roll back rows and marker. Only classified conflicts map to retryable `Busy`.

Remote tests remain isolated:

```bash
ATHANOR_SURREAL_REMOTE_URI=ws://127.0.0.1:8000 \
  cargo test -p athanor-store-surrealdb \
  --features remote --test remote --locked -- --ignored
```

A configured remote test is not evidence until a hosted run succeeds.

## Retry and cancellation

```bash
cargo test -p athanor-core cancellation --locked
cargo test -p athanor-app cancellation --locked
cargo test -p athanor-store-surrealdb cancellation_stops_retry_before_backoff --locked
```

Context-aware retries use bounded delays of 10, 25, 50, and 100 milliseconds. Cancellation/deadline
are checked before attempts and during backoff, not after a successful durable commit. Rollback and
recovery remain cancellation-independent.

## Coverage

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.8.7 --locked
mkdir -p coverage
cargo llvm-cov --workspace --locked --lcov --output-path coverage/lcov.info
cargo llvm-cov report --json --summary-only --output-path coverage/summary.json
cargo llvm-cov report --html --output-dir coverage/html
```

Coverage remains observational until a hosted artifact establishes a baseline. Do not invent a
threshold.

## AppSec and release integrity

Configured checks include `cargo-deny`, dependency review, CodeQL Rust security-extended,
full-history Gitleaks, blocking Zizmor, CycloneDX SBOM, checksums, Sigstore signing, provenance, and
release verification before publish. All external workflow `uses:` references are immutable SHA pins;
the Rust setup is a repository-owned local composite action.

```bash
cargo install zizmor --version 1.26.1 --locked
zizmor --offline --strict-collection --min-severity high --min-confidence high .
```

Repository settings such as secret push protection, Actions enablement, branch protection, and
required checks still need explicit verification.
