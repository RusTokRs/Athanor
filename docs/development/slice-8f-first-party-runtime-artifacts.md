---
id: doc://docs/development/slice-8f-first-party-runtime-artifacts.md
kind: developer_guide
status: active
---

# Slice 8F — First-party runtime artifacts

## Scope

Continue from the 8E completeness baseline with a bounded semantic projection of first-party runtime
artifacts. The next implementation should prefer product meaning over raw coverage percentage.

## Initial candidates

- `install.sh`: inspect whether installer lifecycle facts can be represented using existing contracts.
- `scripts/verify_release_version.py`: inspect whether release verification semantics expose useful
  repository knowledge facts.

## Constraints

- Do not add generic shell parsing.
- Do not add generic Python AST extraction.
- Reuse existing evidence, ownership, redaction, and bounded publication contracts.
- Keep unrelated fixtures and coverage-only parsing out of scope.

## Acceptance direction

A future implementation slice should add only deterministic, evidence-backed facts with exact source
commit verification.
