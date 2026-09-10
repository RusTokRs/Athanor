---
id: doc://docs/development/slice-8f-first-party-runtime-artifacts.md
kind: developer_guide
status: active
---

# Slice 8F — First-party runtime artifacts

## Status

Source implementation is landed on `main` through the bounded operations extractor. Focused format/test/
Clippy verification remains pending; the maintainer runs those checks separately.

## Scope

Continue from the 8E completeness baseline with bounded semantic projections of first-party runtime
artifacts. The implementation prefers product meaning over raw coverage percentage.

## Implemented projections

### Root `install.sh`

- Recognizes only the repository-root `install.sh` path and a supported POSIX-shell shebang.
- Projects one `ScriptCommand` entry point with `ath`/`athd` installation targets.
- Records `SHA256SUMS` plus the bounded `sha256sum`/`shasum` verification anchors.
- Attaches deterministic source-line evidence and ownership.
- Does not expose `ATHANOR_INSTALL_DIR`, `$HOME`, resolved filesystem paths, filesystem state, or shell control flow.

### `scripts/verify_release_version.py`

- Recognizes only the exact first-party script path and its Python 3 shebang.
- Projects one `ScriptCommand` for the release-contract verifier.
- Records the bounded `v<semver>` tag contract, Cargo `package.version` coherence requirement, dated changelog
  section requirement, substantive release-note requirement, output-writing contract, and declared CLI inputs.
- Uses exact local contract anchors and fails closed when a required anchor drifts.
- Does not execute Python, inspect manifests/changelog contents, capture concrete versions/tags/paths, or parse
  generic Python AST/control flow.

## Slice 8G follow-up

The Windows counterpart `install.ps1` is implemented separately from the generic PowerShell environment
reference extractor. It emits one bounded `ScriptCommand` only for the exact root installer path, anchored by
`SHA256SUMS`, `ath.exe`/`athd.exe`, and `Get-FileHash`. It intentionally does not publish `$InstallDir`,
`$env:LOCALAPPDATA`, concrete filesystem paths, or generic PowerShell control flow.

Focused verification and completeness evidence remain pending for 8F–8G.

## Constraints

- Do not add generic shell parsing.
- Do not add generic Python AST extraction.
- Reuse existing evidence, ownership, redaction, and bounded publication contracts.
- Keep unrelated fixtures and coverage-only parsing out of scope.

## Acceptance direction

Focused verification must run against one exact source commit before Slice 8F is promoted to verified.
Completeness should be re-evaluated after verification; its resulting percentage must not be inferred from the
number of newly recognized files alone.
