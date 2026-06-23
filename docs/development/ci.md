---
id: doc://docs/development/ci.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000090
source_language: en
status: verified
---
# Continuous Integration

Athanor uses GitHub Actions for continuous quality, compatibility, and security checks.

## Quality & Compatibility Pipeline

The main `CI` workflow runs on pushes to `main`, pull requests, and manual dispatches. It contains two main jobs:

### 1. Security & License Checks
Runs `cargo-deny` on `ubuntu-latest` to verify:
- **Licenses**: Ensures all dependencies use approved open-source licenses (MIT, Apache-2.0, BSD-3-Clause, ISC, CC0-1.0, BSL-1.0, Unicode-DFS-2016, OpenSSL) and bans copyleft licenses (GPL, AGPL).
- **Advisories**: Fails the build if any dependency has known security vulnerabilities (CVEs).
- **Bans**: Detects and warns on duplicate dependency versions to track bloat.
- **Sources**: Ensures dependencies are fetched only from standard registries (e.g. crates.io) and bans untracked git repositories.

### 2. Code Quality & Formatting
Runs across a matrix of Linux (`ubuntu-latest`), Windows (`windows-latest`), and macOS (`macos-latest`).

Each matrix job runs:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## Nightly Security Audit Workflow

A separate nightly `Security Audit` workflow runs every day at midnight (and can be triggered manually). It executes `rustsec/audit-check` (`cargo-audit`) to scan the locked dependency tree for newly discovered vulnerabilities in the Rust advisory database.

## Principles & Permissions

The locked dependency graph makes CI fail when `Cargo.lock` is inconsistent. The indexing command
builds a disposable local snapshot on the runner, and the documentation gate checks that snapshot
against the editable-documentation completeness policy.

The workflow has read-only repository permissions, disables persisted checkout credentials, cancels
superseded runs for the same ref, and allows every operating-system matrix entry to finish when one
entry fails.
