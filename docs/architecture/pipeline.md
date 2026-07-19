---
id: doc://docs/architecture/pipeline.md
kind: architecture
language: en
source_language: en
status: active
---
# Indexing Pipeline

This document separates the pipeline that exists on the current `main` branch from target work and
historical layouts. Source presence is implementation evidence; current-commit execution evidence
must come from the verification matrix in `athanor_implementation_plan_ru.md`.

## Current Architecture

### Composition Root

`ath`, `athd`, and the MCP host construct a `RuntimeComposition` and pass it explicitly to application
services. Indexing uses:

- `index_project_with_composition` for ordinary CLI-style execution;
- `index_project_with_composition_and_operation_context` for transport-owned deadlines and
  cancellation;
- cancellable composition-aware variants for daemon work.

`index_runtime.rs` canonicalizes the project root, acquires the project publication lock, loads
configuration, obtains the canonical Store through `RuntimeComposition::init_store`, recovers an
interrupted publication, loads prior index state and the exact prior snapshot, and constructs the
configured pipeline through the composition-aware runtime builder.

There is no public Store initialization fallback. Read and write hosts must supply composition.

### Pipeline Phases

The canonical execution order is:

```text
SourceProvider discovery
  -> affected-file classification
  -> snapshot allocation
  -> Extractor execution
  -> incremental canonical merge
  -> Linker execution
  -> Checker execution
  -> canonicalization and validation
  -> prepared IndexPipelineOutput
  -> transactional publication
```

Phase ownership is split across:

| Owner | Responsibility |
| --- | --- |
| `pipeline_source.rs` | source discovery under operation deadline/cancellation rules |
| `pipeline_extract.rs` | bounded concurrent extraction and adapter metrics |
| `pipeline_link.rs` | linker execution over full and affected contexts |
| `pipeline_check.rs` | checker execution and diagnostics |
| `pipeline_merge.rs` | deterministic merge and canonical ID deduplication |
| `pipeline_ownership.rs` | ownership-based invalidation and carried-object pruning |
| `pipeline_metrics.rs` | phase and adapter metrics |
| `pipeline_support.rs` | shared pre-commit deadline/cancellation guards and helpers |
| `pipeline.rs` | orchestration, limits, validation, and `IndexPipelineOutput` |

### Sources And Adapters

The production runtime can assemble built-in and trusted external adapters. Current built-in families
cover filesystem inventory, Markdown, Rust, OpenAPI, GraphQL, JavaScript/TypeScript, operations
sources, API and Markdown linking/checking, plus opt-in RusTok FFA/FBA/Page Builder adapters.

External process adapters retain their schema-less typed request/response protocol. Their execution
uses bounded stdin/stdout/stderr, explicit working directories, allowlists, trust hashes, timeouts,
and cooperative cancellation. Adapter manifests and trust state use the versioned contract lifecycle
documented in `docs/development/json-contract-inventory.md`.

### Incremental State

`IndexStateStore` records the prior snapshot identity and file state. Source discovery classifies
files as changed, unchanged, or removed. When the previous exact canonical snapshot is available:

1. only changed files are extracted;
2. unchanged canonical objects are carried forward;
3. objects owned by changed or removed paths are pruned;
4. adapter invalidation declarations determine local, dependency-closure, or global downstream work;
5. entities, facts, relations, and diagnostics are canonicalized by ID before publication.

If no source changes exist and the previous snapshot is usable, the pipeline may reuse the current
snapshot rather than create a new durable publication.

### Resource Limits

Pipeline configuration provides bounded defaults and project overrides for:

```toml
[pipeline]
extraction_concurrency = 16
max_extraction_bytes_in_flight = 67108864
max_snapshot_batch_objects = 1000000
max_snapshot_batch_bytes = 536870912

[pipeline.extraction_concurrency_by_adapter]
"builtin.extractor.markdown" = 2
```

All limits must be positive. Per-adapter concurrency prevents one extractor from consuming every
shared slot. The aggregate extraction-byte budget is held while work is in flight. The complete
canonical batch is bounded by object count and serialized byte size before the Store boundary.

### Validation-Only Execution

`ath index --validate-only` builds the normal configured adapter pipeline over a
`TransientKnowledgeStore`. It performs discovery, extraction, linking, checking, merge, and
validation but does not publish a durable canonical snapshot, read model, index state, or current
pointer. A validation result may be written to the explicitly selected output path.

### Operation Cancellation And Deadlines

Every pre-commit adapter or Store boundary executed through `within_operation_deadline` checks
`OperationContext::check_active()` before polling work and after a successful future. Cancellation or
deadline expiry before durable publication therefore rejects late success.

The durable atomic publication boundary is intentionally different. It owns commit reconciliation
and must not apply a generic postflight that could replace an already committed result with a
cancellation error.

### Transactional Publication

`index_publication.rs` and `index_publication_snapshot.rs` coordinate the application publication.
The current sequence is:

1. persist an Index publication journal containing the intended snapshot and artifact identities;
2. prepare a complete JSONL read-model replacement while retaining the previous artifact;
3. prepare the next IndexState replacement while retaining the previous state;
4. publish the complete canonical `SnapshotBatch` through the Store atomic publication boundary;
5. if the exact snapshot did not commit, roll back prepared artifacts and abort the snapshot;
6. if the exact snapshot committed, retain durable success even when cancellation or deadline raced
   with the backend return path;
7. finalize prepared artifacts and clear the publication journal;
8. publish or recover the `IndexCurrent` generation pointer and clear its pointer journal.

Exact snapshot identity is authoritative. Missing, uncommitted, mismatched, or ambiguous probes fail
closed rather than guessing whether commit occurred.

The project-local publication lock prevents concurrent Index processes from interleaving journals,
state, read models, or current pointers.

### Current Artifact Layout

Canonical snapshots are immutable Store objects. Index publication currently writes the compatibility
working paths:

```text
.athanor/generated/current/jsonl
.athanor/state/index-state.json
```

The current-pointer publication owner validates their snapshot/generation identities, copies them to
immutable generation-specific paths, and publishes `IndexCurrent`. The compatibility working paths
are an internal migration layout, not a public service API.

Coordinated `ath generate` output is separate: it publishes immutable JSONL, Wiki, and HTML generation
artifacts and then switches its generated-current pointer.

### Snapshot Query Semantics

Low-level Store queries always use a `SnapshotSelector`:

- `LatestCommitted` resolves only visible committed state;
- `Exact` rejects known but uncommitted state;
- relation and diagnostic filters use canonical `EntityId` values;
- stable-key callers resolve through `EntityResolver` before ID-based queries.

The reusable Store conformance package defines this visibility contract for supported backends.
SurrealDB remains opt-in and environment-backed; default CLI/daemon builds use JSONL.

### Read Services And Projections

Read services load a committed canonical snapshot through composition-aware owners and produce typed,
bounded reports. Important owners include:

- Check execution in `check/execution.rs`, with diagnostics and affected logic in sibling modules;
- API snapshot publication in `application_report_composition/api_direct.rs` plus pure
  `api/snapshot.rs`, `api/diff.rs`, and `api/retention.rs`;
- Graph contracts and pure adapters in `graph/model.rs`, `graph/standard.rs`, and `graph/rustok.rs`;
- Repair execution in `repair_composition/direct.rs`;
- Docs execution in `application_report_composition/docs_direct/`;
- composition-aware Search, Context, Explain, Change Map, Overview, Capabilities, Impact, and
  Coverage owners.

CLI, daemon, and MCP equivalents serialize the same typed application report rather than maintaining
transport-specific payload schemas.

## Target Architecture

The following items are future work and must not be described as current behavior:

### Remove The Compatibility Publication Layout

Move Index publication directly to immutable generation-specific read-model and state paths, then
remove the internal compatibility working-path branch and the textual legacy publication split. The
migration must preserve recovery of already-written journals and current pointers.

### Portable Cross-Backend Transaction Handle

The current atomic canonical batch contract is sufficient for supported Store behavior, while the
application coordinates filesystem read-model and state artifacts. A future portable transaction
handle may unify prepare/commit/abort semantics across every backend without weakening exact-snapshot
reconciliation.

### Composition-Owned Process Runner

External adapters currently use an application cancellation-aware runner path. Injecting that runner
through `RuntimeComposition` requires an application-level port that preserves process-tree
termination and bounded I/O semantics.

### MCP Control-Plane Responsiveness

`MCP-004` must prove that cancellation, disconnect, and other control messages remain responsive when
ordinary request slots are saturated.

### Broader Adapter Completeness

Future GraphQL, framework, database, frontend, and language adapters should add evidence-backed
canonical knowledge behind existing Source/Extractor/Linker/Checker ports. They must not move
adapter-specific semantics into domain or core.

### Current-Commit Verification

Formatting, workspace checks, tests, Clippy, and smoke commands must run on one commit before this
architecture can be described as verified for that commit.

## Historical Notes

Earlier pipeline documentation described a minimal linear flow ending directly in JSONL output. The
repository later added incremental merge, ownership-aware invalidation, prepared artifacts, recovery
journals, atomic canonical publication, immutable generation pointers, explicit composition, and
operation-aware cancellation.

The following historical interfaces or paths are not active architecture owners:

- process-global runtime factory installation;
- public `store::init_store`;
- no-composition Check, Docs, Repair, API snapshot, Search, Context, Change Map, or Graph services;
- the deleted Graph monolith;
- aggregate roadmap claims tied only to an old canonical documentation snapshot.

Git history and focused adapter documents retain feature history. This document describes current
pipeline ownership first, target work second, and compatibility history last.

## Verification Matrix

Run the full matrix from `athanor_implementation_plan_ru.md` on one commit. The pipeline-focused subset
is:

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test -p athanor-app pipeline --locked
cargo test -p athanor-app pipeline_support --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app store_publication_cancellation --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
```
