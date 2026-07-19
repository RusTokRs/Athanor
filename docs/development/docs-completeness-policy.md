---
id: doc://docs/development/docs-completeness-policy.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Documentation Completeness Policy

`ath docs check` is the CI-oriented structural and diagnostic gate for editable Markdown
 documentation. `ath docs drift` is the separate verification-age report. `ath docs propose-fix` and
`ath docs apply-patch` provide the patch-based remediation workflow. These commands read the latest
canonical snapshot; run `ath index` first when source files changed.

## Configuration

`ath init` writes a current `ProjectConfig`-compatible policy to `athanor.toml`:

```toml
[docs]
editable_path = "docs"

[docs.completeness]
required_fields = ["id", "kind", "language", "source_language", "status"]
allowed_statuses = ["active", "implemented", "planned", "draft", "verified"]
minimum_diagnostic_severity = "medium"
require_current_snapshot = false
```

The Athanor repository keeps the same policy in its root `athanor.toml` so self-indexing and CI use an
explicit contract rather than an accidental default.

- `editable_path` limits the gate to documentation source owned by that project-relative path.
- `required_fields` names frontmatter keys that must be explicitly present.
- `allowed_statuses` is a lifecycle vocabulary, not execution evidence:
  - `draft` — incomplete material under active authoring;
  - `planned` — accepted future work or design;
  - `active` — a current status/architecture owner;
  - `implemented` — source behavior is present but current-commit execution evidence is absent;
  - `verified` — the document's own declared verification semantics are satisfied.
- `minimum_diagnostic_severity` accepts `low`, `medium`, `high`, or `critical`; findings at or above
  the threshold fail the gate.
- `require_current_snapshot` optionally requires `last_verified_snapshot` to equal the latest
  canonical snapshot id.

`last_verified_snapshot` is not required by the default completeness policy. Verification age belongs
 to `ath docs drift`; projects that need a strict snapshot gate can add the field to `required_fields`
and set `require_current_snapshot = true`.

When `athanor.toml` is absent, the same lifecycle-aware defaults apply.

## Commands

```bash
cargo run -p ath --quiet -- docs check
cargo run -p ath --quiet -- docs check --json
cargo run -p ath --quiet -- docs drift
cargo run -p ath --quiet -- docs drift --json
cargo run -p ath --quiet -- docs propose-fix
cargo run -p ath --quiet -- docs propose-fix --output .athanor/patches/docs/manual.json
cargo run -p ath --quiet -- docs apply-patch docs_patch_snap_jsonl_00000030
cargo run -p ath --quiet -- docs apply-patch .athanor/patches/docs/manual.json
cargo run -p ath --quiet -- docs operations check
cargo run -p ath --quiet -- docs operations check --json
```

The check command returns success only when every selected editable page satisfies the configured
policy and no selected open documentation diagnostic reaches the configured threshold. JSON output
uses `athanor.docs_check.v1`. Generated documentation is excluded, and the command never edits source
files or re-indexes the project.

`ath docs drift` lists editable pages whose `last_verified_snapshot` is absent or older than the
latest canonical snapshot's immediate predecessor. The immediate predecessor is accepted because the
normal `docs propose-fix` -> `docs apply-patch` -> `index` loop can only write the snapshot that was
current before the follow-up index minted the next sequential snapshot id. Drift does not apply the
completeness policy or fail merely because lifecycle status is `active`, `implemented`, `planned`, or
`draft`. JSON output uses `athanor.docs_drift.v1` and includes current and drifted document counts.

`ath docs operations check` aggregates environment, script, deployment, and runbook documentation
diagnostics from the latest canonical snapshot. Text output prints one operational summary and the
same per-scope details as `ath check env`, `ath check scripts`, `ath check deployment`, and
`ath check runbooks`. JSON output uses `athanor.operations_docs_check.v1`. The command is read-only
and returns a non-zero process status when any operational documentation diagnostic is open.

`ath docs propose-fix` writes a versioned `athanor.docs_patch.v1` JSON proposal under
`.athanor/patches/docs/` by default. The proposal is reviewable and contains one operation per
editable Markdown file. It currently supports deterministic frontmatter remediation,
evidence-backed API documentation drafts, and operations documentation drafts:

- missing fields required by the configured completeness policy, with safe deterministic defaults;
- disallowed `status` values, replaced with the first configured allowed status;
- missing or stale `last_verified_snapshot` updates produced by drift remediation;
- new Markdown API pages for `api_endpoint_implemented_but_not_documented` diagnostics, written
  under `<editable_path>/api/` with frontmatter `entities` that points at the missing endpoint;
  these drafts include the endpoint contract plus linked handler, schema, example, security,
  response, and source evidence when those canonical graph objects are available;
- updates to existing editable API documentation pages that already declare or link to one or more
  API endpoints; these proposals insert or refresh endpoint-specific Athanor-managed Markdown
  blocks delimited by
  `<!-- athanor:api-doc:start ... -->` and `<!-- athanor:api-doc:end -->` while preserving
  human-authored text outside the blocks;
- frontmatter `entities` stabilization for editable API documentation pages that are already linked
  to endpoints through canonical documentation relations but do not explicitly declare those
  endpoint stable keys;
- coordination blocks for endpoints documented by multiple editable API pages, so overview and
  operation pages can review the same endpoint-to-page map before applying generated edits;
- narrative review blocks when human-authored API page text mentions `METHOD /path` routes that do
  not match the current endpoints linked to that page; when the page has exactly one linked current
  endpoint, the block also includes original-line and draft-line rewrite suggestions for review;
- new Markdown operations pages for `missing_env_var`, scoped runtime configuration
  `missing_documentation`, scoped script `missing_documentation`, scoped deployment
  `missing_documentation`, and scoped runbook `stale_documentation` diagnostics, written under
  `<editable_path>/operations/` with frontmatter `entities` that points at the missing or stale
  operational entity.

`ath docs apply-patch <patch-id-or-path>` applies one proposal explicitly. A bare patch id resolves
to `.athanor/patches/docs/<id>.json`; a JSON path can also be supplied. Apply fails if the proposal
targets an older snapshot than the current canonical snapshot. For create operations, apply refuses
to overwrite an existing file. This keeps proposals reviewable and prevents stale patches from
silently rewriting editable documentation.

The repository CI workflow indexes the checkout first and then runs the completeness gate on every
operating system in its matrix. Snapshot freshness remains separately observable through
`ath docs drift`.
