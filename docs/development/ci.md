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

The job uploads `coverage/lcov.info`, `coverage/summary.json`, and the HTML report as the `rust-source-coverage` artifact for 14 days. This first measurement intentionally does not enforce a percentage floor: the measured hosted result must be reviewed and committed as the baseline before a no-regression threshold is enabled.

## Nightly Security Audit Workflow

A separate nightly `Security Audit` workflow runs every day at midnight and scans the locked dependency tree for newly discovered vulnerabilities.

## Principles & Permissions

The workflow has read-only repository permissions, disables persisted checkout credentials, cancels superseded runs for the same ref, and keeps matrix failures visible.
