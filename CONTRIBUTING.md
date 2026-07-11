# Contributing to Athanor

Thank you for improving Athanor. Start with [AGENTS.md](AGENTS.md), the development workflow,
and the coding standards. Changes must preserve adapter-first boundaries and include evidence-backed
tests and documentation where applicable.

## Pull requests

Keep each pull request focused. Include the motivation, compatibility impact, verification commands,
and any deferred limitations. Do not combine architecture migration with unrelated extractor features.

Before requesting review, run:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```

## Commit messages

Use Conventional Commits with a non-empty scope when it improves clarity:

```text
feat(core): add bounded snapshot query
fix(jsonl): publish canonical snapshots through staging
docs(governance): add security disclosure policy
```

Avoid ambiguous messages such as `fix`, `changes`, or `wip`.

## Architecture decisions

Use [the ADR template](docs/development/adr-template.md) for material contract, storage, protocol,
or dependency-boundary decisions. ADRs are immutable once accepted; supersede them with a new ADR.
