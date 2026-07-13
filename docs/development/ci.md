---
id: doc://docs/development/ci.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Continuous Integration

Athanor uses GitHub Actions for continuous quality, compatibility, security, and source-coverage checks.

## Quality & Compatibility Pipeline

The main `CI` workflow runs on pushes to `main`, pull requests, and manual dispatches.

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

This gate exists to prevent optional Cargo feature regressions from reaching the main branch. The matrix remains in the pull-request workflow unless hosted duration demonstrates that the `store-surreal` or aggregate slice must move to a scheduled full-feature run.

### Rust Source Coverage

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
4. **Workflow security audit** installs pinned `zizmor 1.26.1` and audits workflows, local actions, and Dependabot configuration in offline, strict-collection mode. The initial rollout emits high-severity/high-confidence annotations without failing the workflow. After the first hosted report is reviewed and any justified exceptions are documented, `--no-exit-codes` must be removed to make the audit blocking.

Local workflow audit:

```bash
cargo install zizmor --version 1.26.1 --locked
zizmor --offline --strict-collection --min-severity high --min-confidence high .
```

All `uses:` references in the current CI, production, audit, release, and AppSec workflows are pinned to immutable commit SHAs. Human-readable version comments are retained next to the SHA, and the `github-actions` Dependabot ecosystem remains enabled so those pins can receive reviewed updates.

## Nightly Security Audit Workflow

A separate nightly `Security Audit` workflow runs every day at midnight and scans the locked dependency tree for newly discovered vulnerabilities. It also enforces the workspace `unsafe_code` denial policy.

## Principles & Permissions

Read-oriented workflows use repository `contents: read`, disable persisted checkout credentials, and cancel superseded runs where appropriate. CodeQL receives `security-events: write` only in its own job. Release-only signing and publishing permissions remain confined to the tag-triggered release workflow.
