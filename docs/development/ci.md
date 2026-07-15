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

The Ubuntu compatibility matrix uses `fail-fast: false` and covers default/no-default,
`store-surreal`, `js-ts-precision`, and `--all-features`. Each slice runs locked check, tests, and
Clippy. Remote SurrealDB integration tests remain `#[ignore]` and are executed only by the dedicated
server job.

## Store conformance

The configured store matrix runs:

```bash
cargo test -p athanor-store-memory --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-store-surrealdb --locked
```

Memory, JSONL, and embedded SurrealDB invoke the same three reusable contracts:

- committed query and fact filtering;
- prepared/commit/abort lifecycle;
- atomic complete-batch plus committed-marker publication.

The atomic contract stages partial data, requires it to remain uncommitted, publishes a complete
replacement batch, compares exact and latest reads, and rejects republish and abort after commit.
Backend-focused regressions then exercise filesystem marker corruption and SurrealDB transaction
rollback.

Focused checks:

```bash
cargo test -p athanor-core fact_query --locked
cargo test -p athanor-app fact_query --locked
cargo test -p athanor-store-memory --test fact_query --locked
cargo test -p athanor-store-jsonl --test fact_query --locked
cargo test -p athanor-store-surrealdb --test fact_query --locked

cargo test -p athanor-store-memory --test atomic_publication --locked
cargo test -p athanor-store-jsonl --test atomic_publication --locked
cargo test -p athanor-store-surrealdb --test atomic_publication --locked

cargo test -p athanor-store-memory --test conformance --locked
cargo test -p athanor-store-jsonl --test conformance --locked
cargo test -p athanor-store-surrealdb --test conformance --locked
```

### JSONL marker policy

An atomic JSONL manifest declares `commit_marker_schema: athanor.canonical_commit.v1`. Exact and
latest loads require a parseable marker with the same schema and snapshot identity. Missing,
malformed, wrong-schema, and foreign-snapshot markers fail closed. A legacy manifest without the
declaration remains readable; an undeclared marker, when present, is still validated.

The JSONL suite also forces `latest.json` finalization failure after exact directory publication. The
exact generation must remain readable and non-abortable because data and marker were already
committed by the directory rename.

### SurrealDB transaction policy

The SurrealDB atomic boundary deletes old rows, inserts the complete replacement batch, and updates
the snapshot record to `prepared = true, committed = true` inside one SurrealQL transaction. A
duplicate-record statement failure must roll back both rows and marker, leave the snapshot
uncommitted, and permit a later valid atomic retry.

The public facade retries the complete boundary only when the backend message is classified as a
confirmed transaction conflict/`Busy`. Data and serialization failures are not retried.

Typed compatibility and deferred-facade checks:

```bash
cargo test -p athanor-core prepared_publication --locked
cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-app --test deferred_canonical_buffer --locked
cargo test -p athanor-store-memory --test prepared_publication --locked
cargo test -p athanor-store-surrealdb --test prepared_publication --locked
```

`deferred_canonical_buffer` proves both the facade and real deferred `IndexPipeline` path. A
context-aware full batch and prepare must not create an exact JSONL generation or prepared directory.
The explicit coordinator batch must be the first canonical data published and must replace any
process-local pending contents.

## Production atomic index coordinator

Production `index_publication` now resolves to `index_publication_atomic.rs`. The compatibility guard
and inner coordinator remain compiled for journal types and legacy fault fixtures, with intentional
legacy dead code explicitly allowed during the transition.

Run the complete application publication suite with:

```bash
cargo test -p athanor-app index_runtime_tests --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-app index_publication_finalize_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p athanor-app index_publication_content_tests --locked
cargo test -p athanor-app index_publication_combined_error_tests --locked
cargo test -p athanor-app --test deferred_canonical_buffer --locked
```

The active coordinator writes the durable journal, stages read-model/state artifacts, builds a
complete `SnapshotBatch` from pipeline output, and calls `AtomicSnapshotPublication`. If the backend
returns an error, it probes the journal snapshot by exact canonical id:

- exact committed: retain journal and application artifacts for committed recovery; never abort;
- exact absent/uncommitted: roll application artifacts back, clear journal, and abort;
- exact probe failure: preserve the original publish error with probe context.

Recovery uses the same exact probe instead of relying only on `LatestCommitted`. The JSONL regression
blocks `latest.json` after exact directory publication and requires the coordinator to preserve the
exact committed generation, journal, read-model manifest, and index state. After removing the
pointer fault, recovery clears stale application backups and the journal without reverting the
committed generation. Repeated recovery is a no-op.

Finalize sabotage hooks were moved from `commit_snapshot_with_context` to the atomic capability so
read-model finalize, index-state finalize, and journal-clear fault tests still inject failures after
the canonical commit. Combined publish/rollback/abort diagnostics are also preserved through the new
boundary.

The application facade now buffers context-aware full batches in process memory. The deferred
pipeline still invokes its compatibility `put_snapshot_with_context` and prepare methods, but those
calls do not mutate backend rows or a prepared marker. The durable journal and application staging
therefore precede the first canonical data publication. Snapshot allocation itself still precedes the
journal and remains the next crash-recovery slice, especially for durable uncommitted SurrealDB
snapshot records.

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

The configured suite checks two-client allocation uniqueness and atomic cross-client visibility. One
connection publishes rows and marker through `AtomicSnapshotPublication`; the independent reader
loads the entity/fact batch and retrieves the fact through `FactQueryStore`. It is not evidence of
remote behavior until a hosted run succeeds, and it does not force a real write conflict.

## Retry and cancellation checks

```bash
cargo test -p athanor-core cancellation --locked
cargo test -p athanor-app cancellation --locked
cargo test -p athanor-store-surrealdb cancellation_stops_retry_before_backoff --locked
```

Context-aware SurrealDB writes retry only `CoreError::Busy` with bounded delays of 10, 25, 50, and
100 milliseconds. Cancellation polling prevents the next attempt and interrupts backoff, but does
not force-abort an SDK request already executing. Atomic publication does not perform a post-success
cancellation check because durable commit has already happened.

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
- A declared JSONL marker that is missing, malformed, or identifies another snapshot must fail exact
  and latest reads.
- A latest-pointer error that makes the exact JSONL generation abortable violates the durable
  data-plus-marker boundary.
- A failed SurrealDB atomic transaction that exposes inserted rows or a committed marker violates
  rollback semantics.
- An atomic coordinator error that rolls back application artifacts after exact commit violates the
  exact canonical probe.
- A committed exact recovery that depends on `LatestCommitted` violates pointer-failure safety.
- A deferred pipeline that creates an exact generation or prepared directory before the coordinator
  atomic boundary bypasses the application pending-batch barrier.
- A coordinator publication exposing the pending compatibility batch instead of `IndexPipelineOutput`
  violates complete-batch replacement.
- A successful publication leaving `index-publication.json` means finalization did not complete.
- A malformed application artifact changing current state means recovery preflight was bypassed.
- A second recovery call changing state after journal cleanup violates idempotence.
- A combined failure that omits the original publish cause or cleanup cause loses diagnostic context.
- A remote test running during normal `--all-features` means its isolation boundary was removed.
- Empty hosted statuses must not be interpreted as successful checks.
