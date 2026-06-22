---
id: doc://docs/development/ci.md
kind: developer_guide
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000030
status: verified
---

# Continuous Integration

The `CI` GitHub Actions workflow runs on pushes to `main`, pull requests, and manual dispatches.
It uses Rust 1.95 across Linux, Windows, and macOS.

Each matrix job runs:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

The locked dependency graph makes CI fail when `Cargo.lock` is inconsistent. The indexing command
builds a disposable local snapshot on the runner, and the documentation gate checks that snapshot
against the editable-documentation completeness policy.

The workflow has read-only repository permissions, disables persisted checkout credentials, cancels
superseded runs for the same ref, and allows every operating-system matrix entry to finish when one
entry fails.
