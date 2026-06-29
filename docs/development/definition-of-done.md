---
id: doc://docs/development/definition-of-done.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000255
source_language: en
status: verified
---
# Definition of Done

A feature is not done until it is implemented, verified, and documented.

## Required

- Core/domain remain free of adapter-specific details.
- New format/backend/transport/UI behavior is isolated in an adapter crate.
- New facts, relations, and diagnostics include evidence.
- Unit tests or contract tests cover the behavior.
- CLI/runtime behavior is checked when the feature is user-facing.
- Relevant English documentation is updated in the same task.
- Features that create or rely on large generated artifacts include a bounded agent-facing query,
  context, or inspection path. Agents must not need to read full JSONL, wiki, HTML, graph, search,
  or API artifact outputs to use the feature.

## Required Commands

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
```

For indexing-related changes:

```bash
cargo run -p ath --quiet -- index .
```

The GitHub Actions matrix runs these checks with `--locked` where Cargo resolves dependencies, then
runs `ath docs check` against the newly indexed snapshot. Local verification remains required before
handoff; CI is the repository-level enforcement layer.

## Documentation Updates

Update the relevant files:

- `docs/architecture/pipeline.md`
- `docs/architecture/adapters.md`
- `docs/development/roadmap-status.md`
- adapter crate `README.md`
- adapter overview under `docs/adapters/`

If a change adds a new adapter or plugin and no documentation is added, the change is incomplete.
