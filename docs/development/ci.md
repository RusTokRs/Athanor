---
id: doc://docs/development/ci.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Continuous Integration

Athanor uses GitHub Actions for continuous quality, compatibility, store-contract, security, and source-coverage checks.

## Quality & Compatibility Pipeline

The main `CI` workflow runs on pushes to `main`, pull requests, and manual dispatches.

All uses of the SHA-pinned `dtolnay/rust-toolchain` action pass `toolchain: 1.95.0` explicitly. A full action commit SHA does not encode the Rust channel, so omitting this input would leave CI, production, and release jobs without the repository's declared MSRV toolchain.

### Security & License Checks

Runs `cargo-deny` on `ubuntu-latest` to verify dependency licenses, advisories, bans, and sources.

### Code Quality & Formatting

The default quality matrix runs across Linux (`ubuntu-latest`), Windows (`windows-latest`), and macOS (`macos-latest`).

Each matrix job runs:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

The Linux and Windows entries also exercise the packaged installer integrity contract. Each smoke test
creates two disposable binaries and a valid `SHA256SUMS`, verifies a successful installation, then
modifies one binary and requires the installer to fail before writing a second installation directory.
The Linux script is also syntax-checked and exercised locally with the same positive/tamper sequence
when release tooling changes.

### Optional Feature Matrix

The CI workflow also runs an Ubuntu feature compatibility matrix with `fail-fast: false`.

Supported slices:

- default feature graph
- `store-surreal` optional backend
- `js-ts-precision` precision mode
- `--all-features` aggregate validation

Each slice runs:

```bash
cargo check --workspace <features> --locked
cargo test --workspace --quiet <features> --locked
cargo clippy --workspace --all-targets <features> --locked -- -D warnings
```

The default workspace features are empty, so the default slice is also the supported `--no-default-features` boundary. A separate spelling is not run because it would duplicate the same Cargo graph.

The SurrealDB crate feature `remote` enables only the WebSocket client protocol. Its server-dependent integration tests are explicitly ignored during normal workspace and `--all-features` runs, so the aggregate feature graph remains self-contained.

This gate exists to prevent optional Cargo feature regressions from reaching the main branch. The matrix remains in the pull-request workflow unless hosted duration demonstrates that the `store-surreal` or aggregate slice must move to a scheduled full-feature run.

## Store Conformance Workflow

`.github/workflows/store-conformance.yml` runs a dedicated Ubuntu matrix for the Memory, JSONL, and embedded SurrealDB backends. Each entry executes its package tests against the locked dependency graph:

```bash
cargo test -p athanor-store-memory --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-store-surrealdb --locked
```

The reusable suite checks committed snapshot selection, stable-key and ID-based queries, prepared-snapshot invisibility, commit/abort behavior, and preservation of `LatestCommitted` after an abort. The batch fixture includes entities, facts, relations, and diagnostics. The current public store query contract independently verifies entity, relation, and diagnostic visibility; fact visibility remains part of the canonical-snapshot contract because `KnowledgeStore` has no fact query method.

`athanor-core` owns the backend-neutral `PreparedSnapshot` and `PreparedSnapshotPublication` extension beside `KnowledgeStore`. `athanor-app` preserves the existing application API through a compatibility re-export. Real backend regressions exercise the same typed protocol for Memory, JSONL, and embedded SurrealDB:

```bash
cargo test -p athanor-core prepared_publication --locked
cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-store-memory --test prepared_publication --locked
cargo test -p athanor-store-surrealdb --test prepared_publication --locked
```

Each backend requires `prepare_publication` followed by `publish_prepared` to advance the latest committed view, and requires `abort_prepared` on a later snapshot to preserve the previously published generation. The Memory regression also verifies that prepared data remains invisible through `SnapshotSelector::LatestCommitted`. JSONL and SurrealDB verify their canonical latest-snapshot representation.

The application regression additionally covers context forwarding and cancellation racing immediately after a successful backend prepare: the prepared handle must still be returned so cleanup can run outside the cancelled request budget. The race fixture retains the registered cancellation lease for the duration of prepare; a temporary handle would be dropped before the assertion and would not model an active operation correctly.

The embedded SurrealDB package additionally verifies:

- a complete `SnapshotBatch` is submitted through one `BEGIN`/bulk-`INSERT`/`COMMIT` transaction;
- response-level statement errors are checked and reported;
- a duplicate-ID failure rolls back completely, proven by a successful clean retry with the same ID;
- prepared snapshots reject late writes;
- 32 concurrent allocations using separate process-local writer gates produce unique snapshot IDs;
- two independent `connect()` calls to one persistent `surrealkv://` directory enforce exclusive ownership;
- the rejected second connection is classified as `CoreError::Busy` and is retryable;
- confirmed transaction-conflict messages are retryable, while data and statement failures remain non-retryable.

The allocation test proves backend atomicity over one shared embedded client without relying on the wrapper mutex. The persistent-path test proves that embedded SurrealKV fails closed when a second owner attempts to open the same directory. Embedded storage is therefore a single-owner process model, not a multi-writer database.

### Remote SurrealDB Job

The `surrealdb-remote` job starts the exact server line used by the locked SDK:

```bash
docker run --detach --rm \
  --name athanor-surrealdb \
  --publish 8000:8000 \
  surrealdb/surrealdb:v2.6.5 \
  start --log warn --unauthenticated --bind 0.0.0.0:8000 memory
```

After the `/health` endpoint responds, CI runs the ignored remote tests explicitly:

```bash
ATHANOR_SURREAL_REMOTE_URI=ws://127.0.0.1:8000 \
  cargo test -p athanor-store-surrealdb \
  --features remote --test remote --locked -- --ignored
```

The configured contract uses two independent SDK connections. It requires 32 concurrent allocations to produce unique snapshot IDs, then writes and commits a batch through one connection and loads its entity and fact records through the other. Server logs are printed and the container is removed even when the test fails.

This job proves remote shared-server visibility only after a successful hosted run. It does not yet deterministically reproduce a transaction write conflict.

### Retry and Cancellation Contract

Context-aware SurrealDB write and publication methods retry only `CoreError::Busy`. Backoff is bounded to 10, 25, 50, and 100 milliseconds. Every attempt checks both deadline and process-local cancellation state; when the remaining deadline cannot fit the next delay, the current `Busy` error is returned without sleeping past the budget.

`OperationContextCancellation::cancellation_handle()` registers one live cancellation authority for a stable operation id. Callers clone that returned handle when several owners participate in the same operation. A second registration while any clone is alive fails with `CoreError::Conflict` instead of merging independent contexts into one global flag; after every clone is dropped, the operation id may be registered again. Cancellation state remains process-local and is not serialized. The core unit tests verify clone propagation, duplicate-id rejection, identity reuse after drop, stable `Cancelled` mapping, unchanged JSON wire shape, and rejection of anonymous cancellation handles.

`CancellationToken::bind_operation()` is idempotent for the same token and operation id: it reuses the handle already held by the token instead of attempting a second core registration. Binding a different operation id to that token still fails, and binding an independent token to the same active operation id still fails closed with `Conflict`.

During retry backoff, cancellation is polled at intervals no larger than five milliseconds. A cancellation request therefore prevents the next retry and interrupts the wait with bounded latency. It does not force-abort a backend request that has already entered the SurrealDB SDK.

Daemon write jobs use an operation-aware scheduler. Before a token is inserted into the daemon cancellation registry, the application `CancellationToken` is bound to the same core handle carried by the job's `OperationContext`. The registry clone and running-task clone therefore cancel the same state. Index, generate, wiki, and HTML report jobs use this path; index forwards the bound context through `begin_snapshot`, `put_snapshot`, `prepare_snapshot`, and `commit_snapshot`. Repeating the same binding is safe and idempotent; binding another independent token to the same active operation id fails closed.

Relevant local checks:

```bash
cargo test -p athanor-core cancellation --locked
cargo test -p athanor-core prepared_publication --locked
cargo test -p athanor-app cancellation --locked
cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-store-memory --test prepared_publication --locked
cargo test -p athanor-store-surrealdb --test prepared_publication --locked
cargo test -p athanor-store-surrealdb cancellation_stops_retry_before_backoff --locked
```

Plain methods do not retry. Data, duplicate-ID, serialization, and other statement failures fail immediately. Rollback after an index publication error deliberately uses the plain abort path so cleanup can complete even when the originating operation was cancelled. Read-only daemon commands, CLI, and MCP cancellation propagation remain separate E2E work.

Troubleshooting:

- a duplicate snapshot ID indicates a broken atomic counter boundary;
- a clean retry failing after the intentional duplicate-ID batch suggests partial transaction leakage;
- a write accepted after `prepare_snapshot` violates snapshot immutability;
- a prepared snapshot visible to a query violates snapshot isolation;
- an aborted snapshot selected by `LatestCommitted` violates publication semantics;
- a backend-specific typed lifecycle test that does not preserve the previous latest snapshot after abort violates the shared publication contract;
- a successful backend prepare that returns cancellation instead of a typed handle can strand prepared data without a cleanup authority;
- a cancellation race fixture that creates and immediately drops its only handle does not retain a live cancellation state;
- a second persistent embedded connection succeeding indicates that exclusive ownership is broken;
- lock contention returned as `AdapterExecution` instead of `Busy` indicates a retry-classification regression;
- a cancelled retry issuing a second backend attempt indicates that `check_active()` was bypassed;
- a daemon registry token that does not cancel the bound `OperationContext` indicates scheduler binding was bypassed;
- a second live registration of the same operation id succeeding indicates that independent cancellation contexts can still be merged;
- repeating the same token/id binding and receiving a conflict indicates that application binding is not idempotent;
- rebinding one application token to a different operation id must fail;
- cancellation state appearing in serialized `OperationContext` is a wire-compatibility regression;
- remote tests running during a normal `--all-features` job indicate that their `#[ignore]` boundary was removed;
- a remote job failure before tests should be diagnosed from the Docker health check and captured server logs;
- an embedded or remote visibility pass must not be used as evidence that a real write conflict was reproduced.

## Rust Source Coverage

The Linux coverage job installs `cargo-llvm-cov` at the pinned version `0.8.7` and runs the workspace tests with the locked dependency graph.

Local equivalent:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.8.7 --locked
mkdir -p coverage
cargo llvm-cov --workspace --locked --lcov --output-path coverage/lcov.info
cargo llvm-cov report --json --summary-only --output-path coverage/summary.json
cargo llvm-cov report --html --output-dir coverage/html
```

The job uploads `coverage/lcov.info`, `coverage/summary.json`, and the HTML report as the `rust-source-coverage` artifact for 14 days. Coverage remains an observation job until a successful hosted artifact establishes a real baseline and branch protection makes a blocking threshold enforceable. A percentage floor must not be guessed from repository size or test count.

## AppSec Workflow

`.github/workflows/appsec.yml` runs on pushes to `main`, pull requests, a weekly schedule, and manual dispatches.

It contains four independent checks:

1. **Dependency review** runs only for pull requests and rejects newly introduced dependencies with known vulnerabilities of moderate severity or higher. Snapshot warnings are retried because the dependency graph can be updated asynchronously.
2. **CodeQL / Rust** initializes the official CodeQL Rust extractor, builds the locked workspace with all features, runs the `security-extended` query suite, and uploads results to code scanning.
3. **Secret scan** checks the complete Git history with Gitleaks. GitHub also provides automatic secret scanning for this public repository; repository push-protection remains a platform setting that must be confirmed separately.
4. **Workflow security audit** installs pinned `zizmor 1.26.1` and audits workflows, local actions, and Dependabot configuration in offline, strict-collection mode. High-severity/high-confidence findings now return a failing exit status and block the workflow; justified exceptions must be documented rather than globally suppressing exit codes.

Local workflow audit:

```bash
cargo install zizmor --version 1.26.1 --locked
zizmor --offline --strict-collection --min-severity high --min-confidence high .
```

All `uses:` references in the current CI, production, audit, release, AppSec, and store-conformance workflows are pinned to immutable commit SHAs. Human-readable version comments are retained next to the SHA, and the `github-actions` Dependabot ecosystem remains enabled so those pins can receive reviewed updates.

## Nightly Security Audit Workflow

A separate nightly `Security Audit` workflow runs every day at midnight and scans the locked dependency tree for newly discovered vulnerabilities. It also enforces the workspace `unsafe_code` denial policy.

## Principles & Permissions

Read-oriented workflows use repository `contents: read`, disable persisted checkout credentials, and cancel superseded runs where appropriate. CodeQL receives `security-events: write` only in its own job. Release-only signing and publishing permissions remain confined to the tag-triggered release workflow.
