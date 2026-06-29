---
id: doc://docs/adapters/impact.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000261
source_language: en
status: verified
---
# Code Impact Analysis (ath impact)

The Code Impact Analysis module calculates the direct and transitive blast radius of code modifications. It traverses dependency relations, ownership, and diagnostics inside the latest canonical snapshot.

## Goal
Implement `ath impact <stable-key | path>` query command to calculate the direct and transitive blast radius of code modifications.

## CLI Command Usage
```bash
# Analyze impact of a specific symbol stable key
ath impact "symbol://rust:crate::auth::login"

# Analyze impact of a specific file path
ath impact "src/auth/login.rs"

# Analyze impact of unstaged/unindexed changes in the working tree
ath impact --diff

# Limit traversal depth and output JSON
ath impact "src/auth/login.rs" --max-depth 5 --json
```

## Propagation Traversal Rules
The command runs a Breadth-First Search (BFS) starting from the resolved initial entities, traversing the relations in the canonical snapshot up to a configured `--max-depth` (defaults to 10). 

Since changes propagate upstream to dependants, relation traversal direction is defined as follows:

| Relation Kind | Direction | Impact Description |
| --- | --- | --- |
| `Calls` | `to` -> `from` | Callee changes impact its Caller. |
| `Imports` | `to` -> `from` | Imported module changes impact the importer. |
| `ImplementedBy` | `to` -> `from` | Function changes impact the API endpoint implementing it. |
| `Documents` / `DocumentsApi` / `DocumentsOperation` | `to` -> `from` | Referenced entity changes impact the documenting page. |
| `SchemaForRequest` / `SchemaForResponse` | `to` -> `from` | Schema changes impact the API endpoint referencing them. |
| `Contains` | `to` -> `from` | Child entity changes impact the parent container/file. |
| `TestedBy` / `CoveredByTest` | `from` -> `to` | Symbol changes impact the test case validating it. |
| `Defines` | `from` -> `to` | Definer entity changes impact the defined symbol. |

All other relations do not propagate changes.

## Diagnostics Gathering
Any open diagnostics attached to any entity in the resolved starting set or within the traversed blast radius are collected and included in the final report. This helps developers identify outstanding issues within the affected scope.

## Diff Mode
When running `ath impact --diff`, Athanor discovers the current files in the workspace, calculates their FNV-1a hashes, and compares them to the hashes stored in the previous `.athanor/state/index-state.json`. All files that were modified, added, or removed are mapped back to their corresponding entities in the latest snapshot.
