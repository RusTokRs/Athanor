---
id: doc://docs/adapters/change-map.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Whole-Code Change Map (ath change-map)

The change-map command answers "where must I inspect or edit for this change?" by traversing the canonical entity/relation graph from seed entities and returning a bounded, evidence-backed ranked list of affected code, contracts, documentation, tests, and operations files.

## CLI Command Usage

```bash
# Task-rooted change map
ath change-map "change authentication"

# Target-rooted change map (exact stable key)
ath change-map --target "api://POST:/login"

# Target-rooted change map (file path)
ath change-map --target "src/auth/login.rs"

# Diff-rooted change map (unindexed working-tree changes)
ath change-map --diff

# Combined modes with limits
ath change-map --diff --target "api://POST:/login" --max-entities 30 --json
```

## Modes

### Task Mode

Runs lexical search (BM25 Tantivy) over the canonical snapshot to find matching entities. Each search result becomes a seed scored at `1000 - (rank * 10)`.

### Target Mode

Resolves an exact canonical stable key or source file path. Matched entities become seeds scored at `1200` (the highest base score).

### Diff Mode

Compares current source discovery with `.athanor/state/index-state.json` to find changed/removed files. Every entity whose ownership or source files include a changed or removed file becomes a seed scored at `1100`.

Multiple modes can be combined; seeds from all active modes are merged, keeping the highest score and concatenating reasons for the same entity.

## Core Algorithm

1. **Adjacency construction**: Builds a bidirectional adjacency map from all snapshot relations, sorted by relation id for determinism.
2. **Seed deduplication**: Merges duplicate seed entity ids, keeping the highest score and appending reasons with semicolon separation.
3. **BFS graph traversal**: Breadth-first traversal from all seed entities, following adjacency relations. Traversal stops at `max_depth` hops and globally when a candidate limit (`max(5000, max_entities * 50)`) is reached.
4. **Scoring**: Each candidate score is computed as:
   ```
   seed_score - (depth * 100)
     + sum(relation_weight(kind) + round(confidence * 10)) for each path link
     + diagnostic_count * 20
   ```
5. **Sorting and diversification**: Candidates are sorted by (descending score, ascending depth, ascending stable key). The result is then diversified to ensure file diversity.
6. **File aggregation**: Groups all entity files from ranked items, taking the max score per file, collecting entity kinds, stable keys, and reasons.
7. **Diagnostic attachment**: Open diagnostics are matched to ranked entities, sorted by severity and diagnostic id, and bounded by `max_diagnostics`.

## Relation Weights

| Weight | Relation Kinds |
|---|---|
| 90 | `Calls`, `ImplementedBy`, `Implements`, `SchemaForRequest`, `SchemaForResponse`, `RequiresAuth`, `RequiresPermission`, `QueriesTable` |
| 80 | `Imports`, `TestedBy`, `CoveredByTest`, `DeclaredInOpenapi`, `ChangedWith` |
| 75 | `Other(_)` (adapter-specific relations) |
| 60 | `Documents`, `DocumentsApi`, `DocumentsOperation`, `UsesEnv`, `ExampleFor` |
| 50 | All other relation kinds |
| 30 | `Contains`, `Defines` |

## Output Schema

The command emits `athanor.change_map.v1` with:

- `query`: task, target, diff flag, and changed files
- `limits`: applied entity, file, diagnostic, and depth limits
- `returned` / `omitted`: counts for entities, files, and diagnostics
- `items`: ranked entities with reasons, relation-chain paths, files, evidence, diagnostics, test coverage, and adapter annotations
- `files`: ranked file summaries with max score, entity kinds, stable keys, and reasons
- `diagnostics`: bounded open diagnostics sorted by severity
- `completeness`: candidate-limit flag and suggested follow-up command

## Test Coverage

| Test | What It Validates |
|---|---|
| `builds_deterministic_relation_chains_and_test_coverage` | Relation chain traversal, test coverage detection (TestedBy), adapter annotations |
| `reports_limits_missing_tests_and_open_diagnostics` | Entity/diagnostic limits, missing-test surfacing, diagnostic inclusion |
| `validates_options_requiring_task_target_or_diff` | Input validation: at least one mode required |
| `validates_options_accepting_empty_task` | Empty/whitespace-only task rejected |
| `validates_options_requiring_positive_limits` | Zero limits rejected for entities, files, diagnostics |
| `deduplicates_seeds_keeping_highest_score` | Multi-source seed merge with score maximization and reason concatenation |
| `truncates_bfs_at_max_depth` | BFS respects configured depth limit |
| `ranks_implemented_by_higher_than_contains` | Relation weight scoring: ImplementedBy (90) > Contains (30) |
| `diversifies_entities_across_files` | File diversity when entity limit truncates results |
| `builds_file_aggregation_from_items` | Per-file summary with max score, kinds, keys, reasons |
| `respects_diagnostics_limit` | Diagnostic count bounded independently of entity count |

## Surfaces

- **CLI**: `ath change-map [task] [--target <key-or-path>] [--diff] [--path .] [--max-entities N] [--max-files N] [--max-diagnostics N] [--max-depth N] [--json]`
- **Daemon**: `change-map` command through the authenticated daemon protocol
- **MCP**: `change_map` tool with `task`, `target`, `diff`, `max_entities`, `max_files`, `max_diagnostics`, `max_depth` parameters

## Limitations

- Diff mode requires a previous index state (`.athanor/state/index-state.json`) and full source discovery.
- Task mode quality depends on lexical search relevance; semantic search is deferred.
- Cross-file fragment-spread and type-condition validation for GraphQL is deferred to linker-level resolution.
- Graph visualization and export are disposable read models; normal agent workflows use bounded change-map commands.
- Ranking is not yet tuned against real Rustok repositories.

## Test

```bash
cargo test -p athanor-app -- change_map
```
