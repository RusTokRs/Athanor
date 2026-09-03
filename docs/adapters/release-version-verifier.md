---
id: doc://docs/adapters/release-version-verifier.md
kind: adapter_guide
status: active
---
# Release Version Verifier Extraction Boundary

This document records the bounded semantics for `scripts/verify_release_version.py`.

## Scope

The script is a first-party release-contract entry point. Extraction should describe the repository
contract it enforces without executing Python or inspecting runtime files.

Supported bounded facts:

- script presence, ownership evidence, and executable entry-point identity;
- release-tag validation as `v<semver>`;
- release manifest version coherence through Cargo `package.version` values supplied to the script;
- changelog contract validation for one dated `## [<version>] - <date>` section with substantive
  release notes;
- release-notes output generation as the successful verifier result;
- declared CLI contract inputs (`--tag`, `--changelog`, `--notes-output`, and release manifests).

A bounded projection should remain compatible with the existing `ScriptCommand` and operational
documentation contracts. The verifier responsibilities belong on one canonical entry-point entity;
individual branches, helper functions, arguments, and validation failures are not separate commands.

## Explicit non-goals

The extractor must not execute the script, import Python modules, evaluate regular expressions, infer
Python control flow, read manifest or changelog contents, capture concrete release tags or versions,
or expose filesystem paths supplied at runtime. It must not become a generic Python AST or arbitrary
CLI parser.

## Verification intent

Implementation should use deterministic file-local evidence from the first-party script itself. It
may recognize only the exact repository path and bounded declarations needed to prove the semantics
above. If those declarations drift beyond the bounded recognizer, extraction should stop rather than
invent equivalent behavior.