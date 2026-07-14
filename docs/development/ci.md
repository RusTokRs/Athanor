---
id: doc://docs/development/ci.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Continuous Integration

Athanor defines GitHub Actions workflows for quality, optional features, store conformance, source
coverage, AppSec, installers, and releases.

## Hosted status

The workflow files are present on `main`, but the public Actions page currently shows onboarding
instead of workflow runs. The connector also returns no push-run or commit-status evidence. Until
Actions are enabled or made visible in repository/organization settings, hosted checks, artifacts,
and enforcement remain unverified.

Do not mark a hosted item complete based only on workflow YAML.

## Quality and compatibility

The main `CI` workflow is configured for pushes to `main`, pull requests, and manual dispatches.
SHA-pinned `dtolnay/rust-toolchain` steps pass `toolchain: 1.95.0` explicitly.

The Linux, Windows, and macOS quality matrix runs:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

Linux and Windows also exercise fail-closed installer checksum behavior with a valid installation and
a tampered binary.

## Optional feature matrix

The Ubuntu compatibility matrix uses `fail-fast: false` and covers:

- default/no-default feature graph;
- `store-surreal`;
- `js-ts-precision`;
- `--all-features`.

Each slice runs locked `cargo check`, tests, and Clippy. Remote SurrealDB integration tests remain
`#[ignore]` and are executed only by the dedicated server job, so `--all-features` stays
self-contained.

## Store conformance and fact queries

The configured store matrix runs:

```bash
cargo test -p athanor-store-memory --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-store-surrealdb --locked
```

The shared contract covers committed selection, stable-key entity queries, backend-neutral fact
queries, relation/diagnostic filtering, prepared invisibility, commit/abort semantics, and
preservation of `LatestCommitted` after abort.

`FactQuery` filters by `subject`, `object`, serialized kind name, extractor, and limit. The
`FactQueryStore` blanket implementation operates on `CanonicalSnapshotStore`, so Memory, JSONL,
SurrealDB, and `AthanorStore` share one committed-only implementation.

Focused checks:

```bash
cargo test -p athanor-core fact_query --locked
cargo test -p athanor-app fact_query --locked
cargo test -p athanor-store-memory --test fact_query --locked
cargo test -p athanor-store-jsonl --test fact_query --locked
cargo test -p athanor-store-surrealdb --test fact_query --locked
```

Typed backend publication checks:

```bash
cargo test -p athanor-core prepared_publication --locked
cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-store-memory --test prepared_publication --locked
cargo test -p athanor-store-surrealdb --test prepared_publication --locked
```

## Typed index runtime and recovery

Production indexing uses `index_runtime.rs` and the guarded typed publication coordinator. The broad
incremental, validation, cancellation, and fresh-index equivalence regressions have been moved to
focused modules; the former monolithic `crates/athanor-app/src/index.rs` has been deleted.

Run the complete application publication suite with:

```bash
cargo test -p athanor-app index_runtime_tests --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-app index_publication_finalize_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p athanor-app index_publication_content_tests --locked
cargo test -p athanor-app index_publication_combined_error_tests --locked
```

The fault matrix covers journal/prepare/publish/finalize/clear failures, malformed artifact types,
schema and identity mismatches, mixed backup generations, absent backups, repeated recovery, and
simultaneous publish/rollback/abort failures.

Recovery content preflight is fail-closed before destructive mutation:

- manifests and state documents must be parseable and contain a non-empty snapshot identity;
- active current/staged artifacts use exact current schemas;
- historical index-state backups may use a validated numeric `athanor.index_state.vN` schema;
- current/staged snapshot identities agree with the journal when recovery would mutate them;
- read-model and state backups identify the same previous generation.

A second recovery call after successful committed or uncommitted cleanup must be a no-op.

## Remote SurrealDB

The dedicated job starts:

```bash
docker run --detach --rm \
  --name athanor-surrealdb \
  --publish 8000:8000 \
  surrealdb/surrealdb:v2.6.5 \
  start --log warn --unauthenticated --bind 0.0.0.0:8000 memory
```

After the health endpoint responds:

```bash
ATHANOR_SURREAL_REMOTE_URI=ws://127.0.0.1:8000 \
  cargo test -p athanor-store-surrealdb \
  --features remote --test remote --locked -- --ignored
```

The configured suite checks two-client allocation uniqueness, canonical cross-client visibility, and
fact retrieval through public `FactQueryStore` from the independent reader. It is not evidence of
remote behavior until a hosted run succeeds, and it does not yet force a real write conflict.

## Retry and cancellation checks

```bash
cargo test -p athanor-core cancellation --locked
cargo test -p athanor-app cancellation --locked
cargo test -p athanor-store-surrealdb cancellation_stops_retry_before_backoff --locked
```

Context-aware SurrealDB writes retry only `CoreError::Busy` with bounded delays of 10, 25, 50, and
100 milliseconds. Cancellation polling prevents the next attempt and interrupts backoff, but does
not force-abort an SDK request already executing.

One live `operation_id` owns one process-local cancellation authority. Clone the registered handle
for additional owners. Independent duplicate registration fails with `CoreError::Conflict`, while
repeating the same application token/id binding is idempotent.

## Rust source coverage

The coverage job installs pinned `cargo-llvm-cov 0.8.7` and uploads LCOV, JSON summary, and HTML
artifacts.

Local equivalent:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.8.7 --locked
mkdir -p coverage
cargo llvm-cov --workspace --locked --lcov --output-path coverage/lcov.info
cargo llvm-cov report --json --summary-only --output-path coverage/summary.json
cargo llvm-cov report --html --output-dir coverage/html
```

Coverage remains observational until the first hosted artifact establishes a real baseline. Do not
invent a percentage floor.

## AppSec and release integrity

Configured checks include `cargo-deny`, dependency review, CodeQL Rust `security-extended`,
full-history Gitleaks, blocking Zizmor, nightly dependency audit, CycloneDX SBOM, checksums, Sigstore
signing, provenance, and release verification before publish.

All workflow `uses:` references are pinned to immutable commit SHAs. Platform settings such as secret
push protection and required checks still need explicit verification.

Local workflow audit:

```bash
cargo install zizmor --version 1.26.1 --locked
zizmor --offline --strict-collection --min-severity high --min-confidence high .
```

## Troubleshooting

- Different canonical, read-model, and index-state snapshot ids mean the typed coordinator was
  bypassed.
- A fact returned from an uncommitted/prepared snapshot violates query isolation.
- Different fact filters or limit behavior between backends means the canonical blanket contract was
  bypassed.
- A successful publication leaving `index-publication.json` means finalization did not complete.
- A journal-write failure leaving prepared canonical data means the pre-journal cleanup guard failed.
- A malformed type, schema, or snapshot identity changing current artifacts means recovery preflight
  was bypassed.
- Mixed read-model/state backup generations must fail before either current artifact is replaced.
- A second recovery call changing state after journal cleanup violates idempotence.
- A combined failure that omits the original publish cause or cleanup cause loses diagnostic context.
- A remote test running during normal `--all-features` means its isolation boundary was removed.
- Empty hosted statuses must not be interpreted as successful checks.
