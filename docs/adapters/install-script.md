---
id: doc://docs/adapters/install-script.md
kind: adapter_guide
status: active
---
# Install Script Extraction Boundary

This document records the bounded semantics for the root `install.sh` artifact.

## Scope

The installer is an operational entry point. Future extraction should treat it as evidence-backed
repository metadata rather than executing shell logic.

Supported bounded facts:

- script presence and ownership evidence;
- executable entry-point identity;
- referenced binaries (`ath`, `athd`) as installation targets;
- checksum verification mechanism (`SHA256SUMS`, `sha256sum`, or `shasum`) as a security-related
  operation anchor.

## Explicit non-goals

The extractor must not execute the installer or infer shell control-flow semantics. It must not
capture environment values, filesystem contents, user paths, or generated installation state.

## Verification intent

The implementation should remain deterministic, file-local, and compatible with the existing
`ScriptCommand` and operational documentation contracts.
