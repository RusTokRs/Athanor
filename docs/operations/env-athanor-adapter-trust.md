---
entities:
- env://ATHANOR_ADAPTER_TRUST
id: doc://docs/operations/env-athanor-adapter-trust.md
kind: operations_documentation
language: en
last_verified_snapshot: snap_jsonl_00000251
source_language: en
status: verified
---
# Environment Variable `ATHANOR_ADAPTER_TRUST`

## Purpose

Overrides the user-level adapter plugin trust store path used by Athanor runtime commands.

## Contract

- Variable: `ATHANOR_ADAPTER_TRUST`
- Canonical entity: `env://ATHANOR_ADAPTER_TRUST`
- Expected value: a non-empty filesystem path to the adapter trust JSON file
- Default: `<home>/.athanor/adapter-trust.json`, where `<home>` is resolved from `USERPROFILE` or `HOME`

## Evidence

- `crates/athanor-app/src/runtime.rs:794`

## Notes

Set this variable when adapter trust decisions need to live outside the default user home location,
such as isolated CI runs, portable workspaces, or tests that must not modify a developer's normal
trust store.
