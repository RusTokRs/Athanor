---
id: doc://docs/development/ci.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Continuous Integration

Athanor defines workflows for quality, optional features, store conformance, coverage, AppSec,
installers, and releases.

## Hosted status

Workflow files are present on `main`, but the public Actions page currently shows onboarding instead
of runs. The connector also returns no push-run or commit-status evidence. Hosted checks, artifacts,
and enforcement remain unverified until Actions are enabled or made visible.

Never mark a hosted item complete from workflow YAML alone.

## Workspace quality

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

The quality matrix is configured for Linux, Windows, and macOS with Rust `1.95.0`. Optional Linux
slices cover default/no-default, `store-surreal`, `js-ts-precision`, and `--all-features` with
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
publication journal. The active coordinator stages read model/state, publishes the explicit complete
batch through `AtomicSnapshotPublication`, and exact-probes the journal snapshot after errors.

## JSONL policy

Atomic JSONL manifests declare `commit_marker_schema: athanor.canonical_commit.v1`. Exact/latest loads
require a parseable marker with matching schema and snapshot identity. Legacy manifests without the
declaration remain readable. A failed `latest.json` update after exact rename must leave the exact
generation readable and non-abortable.

## SurrealDB policy

SurrealDB atomic publication deletes old rows, inserts the complete replacement batch, and updates
`prepared = true, committed = true` in one transaction. Statement failure must roll back rows and
marker. Only classified conflicts map to retryable `Busy`.

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

Coverage remains observational until the first hosted artifact establishes a baseline. Do not invent
a threshold.

## AppSec and release integrity

Configured checks include `cargo-deny`, dependency review, CodeQL Rust security-extended,
full-history Gitleaks, blocking Zizmor, CycloneDX SBOM, checksums, Sigstore signing, provenance, and
release verification before publish. All workflow `uses:` references are immutable SHA pins.

```bash
cargo install zizmor --version 1.26.1 --locked
zizmor --offline --strict-collection --min-severity high --min-confidence high .
```

Repository settings such as secret push protection, Actions enablement, branch protection, and
required checks still need explicit verification.
