# Post-8H Verification Gate

This gate is the required handoff after Slices 8F–8H and before selecting another bounded semantic slice.

## Source

The verification target starts from the exact `main` commit that contains Slice 8H and its hardening. Do not combine evidence from unrelated commits.

Current source at gate definition time:

```text
main = 33b3b119f5d626473161e7c4c59a717671ed8bad
```

## Scope

The gate covers:

- Slice 8F: repository-root `install.sh` and `scripts/verify_release_version.py` first-party projections;
- Slice 8G: repository-root `install.ps1` first-party projection;
- Slice 8H: single-level `.github/ISSUE_TEMPLATE/*.yml|yaml` first-party issue-form projection;
- Slice 8H hardening: fail-closed rejection of unsupported issue-form item types and atomic rejection of a mixed unsupported form.

## Required evidence

1. Run the focused verification matrix for Slices 8F–8H on one exact source commit.
2. Record the exact formatting, workspace test, Clippy, smoke, security, feature, and completeness/self-evaluation evidence produced by that commit.
3. Rerun the exact completeness/self-evaluation report and record the observed processed/unprocessed counts and per-language deltas.
4. Do not infer a new completeness percentage from source inspection alone.
5. Do not promote Slices 8F–8H to `Verified` until the required exact-commit evidence exists.

## Selection rule after the gate

Only after the gate has produced a concrete completeness artifact should the next bounded semantic gap be selected. Prefer a product-semantic gap over a coverage-only target. Generic JSON/test fixtures and unrelated repository files remain out of scope unless a separate product contract justifies them.

## Important boundaries

The gate must not treat any of the following as evidence of successful execution:

- documentation metadata;
- source implementation alone;
- historical verification from a different commit;
- expected coverage deltas;
- skipped workflow runs.

The exact artifact produced by the gate is the authoritative input for the next slice selection.