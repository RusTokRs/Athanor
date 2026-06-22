---
id: doc://docs/development/docs-completeness-policy.md
kind: developer_guide
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000030
status: verified
---

# Documentation Completeness Policy

`ath docs check` is the CI-oriented gate for editable Markdown documentation. It reads the latest
canonical snapshot; run `ath index` first when source files changed.

## Configuration

`ath init` writes the default policy to `athanor.toml`:

```toml
[docs]
editable_path = "docs"

[docs.completeness]
required_fields = ["id", "kind", "language", "source_language", "last_verified_snapshot", "status"]
allowed_statuses = ["verified"]
minimum_diagnostic_severity = "medium"
require_current_snapshot = false
```

- `editable_path` limits the gate to documentation source owned by that project-relative path.
- `required_fields` names frontmatter keys that must be explicitly present.
- `allowed_statuses` lists accepted values for `status` when that field is present.
- `minimum_diagnostic_severity` accepts `low`, `medium`, `high`, or `critical`; findings at or above
  the threshold fail the gate.
- `require_current_snapshot` requires `last_verified_snapshot` to equal the latest canonical
  snapshot id. It defaults to `false` because normal indexing creates a new snapshot even when a
  document's covered knowledge did not change.

When `athanor.toml` is absent, these defaults still apply.

## Commands

```bash
cargo run -p ath --quiet -- docs check
cargo run -p ath --quiet -- docs check --json
```

The command returns success only when every selected editable page satisfies the policy and no
selected open documentation diagnostic reaches the configured threshold. JSON output uses
`athanor.docs_check.v1`. Generated documentation is always excluded, and the command never edits
source files or re-indexes the project.

The repository CI workflow indexes the checkout first and then runs this gate on every operating
system in its matrix.
