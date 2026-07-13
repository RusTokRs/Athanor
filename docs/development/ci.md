---
id: doc://docs/development/ci.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Continuous Integration

Athanor uses GitHub Actions for continuous quality, compatibility, and security checks.

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

This gate exists to prevent optional Cargo feature regressions from reaching the main branch.

## Nightly Security Audit Workflow

A separate nightly `Security Audit` workflow runs every day at midnight and scans the locked dependency tree for newly discovered vulnerabilities.

## Principles & Permissions

The workflow has read-only repository permissions, disables persisted checkout credentials, cancels superseded runs for the same ref, and keeps matrix failures visible.