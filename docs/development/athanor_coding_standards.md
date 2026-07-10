# Athanor Coding Standards for Human and AI Contributors

> Status: normative  
> Scope: all Rust code, tests, examples, scripts, configuration, and developer documentation in the Athanor repository  
> Audience: human contributors and coding agents  
> Keywords: **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative

---

## 1. Purpose

This document defines how Athanor code is designed, named, placed, implemented, refactored, tested, and reviewed.

Its purpose is to prevent:

- arbitrary file and symbol names;
- accidental architectural coupling;
- large speculative rewrites;
- behavior-changing refactors without tests;
- duplicate infrastructure and domain concepts;
- hidden global state;
- unbounded async work;
- backend-dependent semantics;
- public APIs that expose internal implementation details;
- code that compiles but does not preserve Athanor invariants.

This document complements:

- `AGENTS.md`;
- `docs/development/agent-workflow.md`;
- `docs/development/roadmap-status.md`;
- `docs/architecture/pipeline.md`;
- `docs/architecture/adapters.md`;
- relevant crate `README.md` files.

When this document conflicts with an accepted ADR, the ADR wins for its explicitly stated scope. The conflict MUST be documented.

---

# 2. Fundamental Engineering Rules

## 2.1. Correctness before elegance

Code MUST preserve observable behavior and documented invariants.

A refactor is not successful merely because:

- the code is shorter;
- the type hierarchy looks cleaner;
- fewer files exist;
- more abstractions were introduced;
- all code compiles.

A change is successful only when its required behavior is verified.

## 2.2. The smallest complete change

Contributors MUST implement the smallest change that fully solves the current problem.

A change MUST NOT include unrelated:

- renaming;
- formatting churn;
- dependency upgrades;
- module moves;
- API redesign;
- cleanup in distant crates.

Adjacent cleanup MAY be included only when it is required to implement or verify the requested behavior safely.

## 2.3. Explicit over implicit

The following MUST be explicit:

- inputs and outputs;
- side effects;
- ownership of state;
- cancellation and deadlines;
- snapshot selection;
- resource limits;
- error categories;
- backend capabilities;
- transaction and publication boundaries.

Hidden process-global state SHOULD NOT be introduced.

## 2.4. Domain language over technical noise

Names MUST describe Athanor concepts or precise technical responsibilities.

Prefer:

```text
SnapshotSelector
EntityResolver
PublicationCoordinator
ExtractionExecutor
DiagnosticQuery
ProcessRunner
```

Avoid:

```text
DataManager
ThingProcessor
Helper
Utils
Common
Misc
Stuff
Handler2
NewService
TempManager
```

A generic word is acceptable only when its scope gives it one unambiguous meaning.

## 2.5. One source of truth

A concept MUST have one canonical implementation.

Do not duplicate:

- stable ID generation;
- path normalization;
- evidence construction;
- canonical ordering;
- JSONL serialization rules;
- snapshot selection logic;
- adapter registry logic;
- trust verification;
- error code mapping.

Before creating a helper, search the workspace for an existing implementation.

---

# 3. Repository and Layer Boundaries

## 3.1. Dependency direction

The intended dependency direction is:

```text
athanor-domain
      ^
      |
athanor-core
      ^
      |
athanor-app
      ^
      |
runtime composition and entrypoints
```

Adapters depend inward on domain/core contracts:

```text
adapters -> athanor-core / athanor-domain
```

Entry points depend on application services and runtime composition:

```text
apps -> athanor-app / athanor-runtime-defaults
```

## 3.2. `athanor-domain`

`athanor-domain` MUST contain only durable domain concepts and invariants.

Allowed examples:

- entities;
- facts;
- relations;
- diagnostics;
- evidence;
- stable keys;
- snapshot identities;
- ownership;
- canonical domain enums and value objects.

It MUST NOT contain:

- file system access;
- database clients;
- CLI types;
- daemon protocol types;
- MCP types;
- Tantivy types;
- SurrealDB types;
- Tokio runtime orchestration;
- adapter discovery;
- process execution;
- framework-specific parser state.

A concept belongs in domain only if it remains meaningful after replacing every adapter.

## 3.3. `athanor-core`

`athanor-core` MUST define stable ports, shared contracts, and cross-adapter errors.

Allowed examples:

- `KnowledgeQuery`;
- `SnapshotReader`;
- `SnapshotWriter`;
- `EntityResolver`;
- adapter input/output contracts;
- typed error codes;
- resource-limit value objects.

It MUST NOT depend on concrete adapters.

Ports MUST describe capabilities, not implementations.

Prefer:

```rust
trait SnapshotReader
trait EntityResolver
trait ProcessRunner
```

Avoid:

```rust
trait JsonlReader
trait SurrealResolver
trait TokioProcessRunner
```

## 3.4. `athanor-app`

`athanor-app` MUST implement use cases and orchestration.

Allowed examples:

- indexing workflow;
- query services;
- invalidation planning;
- publication coordination;
- graph/context services;
- application-level validation.

Application services MUST depend on ports, not concrete adapters.

Concrete adapter construction belongs in runtime composition.

## 3.5. Adapter crates

Format-, framework-, transport-, storage-, search-, rendering-, and platform-specific behavior MUST be implemented as adapters.

Crate naming pattern:

```text
athanor-{adapter-role}-{technology-or-domain}
```

Examples:

```text
athanor-store-jsonl
athanor-store-surrealdb
athanor-source-fs
athanor-search-tantivy
athanor-extractor-rust
athanor-linker-js-ts
athanor-projector-html
athanor-transport-mcp
```

An adapter MUST map external concepts into canonical Athanor contracts at its boundary.

Adapter-specific types MUST NOT leak into domain or generic application APIs.

## 3.6. Application entrypoints

`apps/ath` and `apps/athd` SHOULD contain:

- argument parsing;
- process startup;
- output formatting;
- dependency composition;
- exit code mapping;
- OS service integration.

They SHOULD NOT contain core business rules.

A command implementation that becomes substantial SHOULD delegate to a named application service.

---

# 4. Naming Rules

## 4.1. Rust casing

Use standard Rust casing:

| Item | Convention | Example |
|---|---|---|
| crates | `kebab-case` | `athanor-store-jsonl` |
| files/modules | `snake_case` | `publication_coordinator.rs` |
| functions/methods | `snake_case` | `resolve_stable_key` |
| variables/fields | `snake_case` | `snapshot_id` |
| types/traits/enums | `PascalCase` | `SnapshotSelector` |
| enum variants | `PascalCase` | `LatestCommitted` |
| constants/statics | `SCREAMING_SNAKE_CASE` | `DEFAULT_BATCH_SIZE` |
| lifetimes | short lowercase | `'a`, `'ctx` |

## 4.2. Files and modules

A file name MUST describe the primary responsibility of the module.

Good:

```text
snapshot_reader.rs
entity_resolver.rs
process_runner.rs
plugin_trust.rs
publication.rs
invalidation.rs
```

Bad:

```text
helpers.rs
utils.rs
common.rs
misc.rs
new.rs
temp.rs
stuff.rs
logic.rs
types2.rs
```

Use a directory module when a concept has multiple cohesive parts:

```text
publication/
    mod.rs
    coordinator.rs
    manifest.rs
    recovery.rs
    staging.rs
```

Do not create a directory for a single tiny file unless the structure is required by an imminent, already-scoped change.

## 4.3. Types

Types MUST be nouns or noun phrases.

Good:

```rust
SnapshotSelector
PublicationManifest
AdapterTrustRecord
ResourceLimits
```

Avoid vague suffixes unless they have precise meaning:

```text
Data
Info
Object
Item
Thing
Manager
Processor
Handler
```

`Manager` MUST NOT be used when a more precise role exists:

- `Coordinator` coordinates multiple components;
- `Registry` stores named implementations;
- `Resolver` maps one identity to another;
- `Factory` constructs implementations;
- `Repository` persists and retrieves aggregates;
- `Reader` reads;
- `Writer` writes;
- `Executor` executes planned work;
- `Planner` produces a plan;
- `Validator` validates;
- `Projector` builds a read model.

## 4.4. Traits

Traits MUST name a capability or role.

Good:

```rust
SnapshotReader
SnapshotWriter
EntityResolver
KnowledgeQuery
ProcessRunner
```

Do not prefix traits with `I`.

Bad:

```rust
ISnapshotReader
BaseStore
AbstractResolver
```

A trait SHOULD be introduced only when at least one of these is true:

- multiple implementations exist;
- a boundary requires substitution;
- tests need a controlled fake;
- the capability is part of the architecture;
- dynamic composition is required.

Do not create a trait only to wrap one private function.

## 4.5. Functions and methods

Functions MUST start with a verb or established Rust constructor/conversion convention.

Good:

```rust
resolve_stable_key
load_committed_snapshot
prepare_publication
validate_manifest
write_batch
```

Avoid:

```rust
snapshot
data
thing
do_it
run_logic
handle
process
```

`handle_*` and `process_*` MAY be used only when the exact event or protocol operation follows:

```rust
handle_index_request
process_adapter_response
```

Prefer a more specific verb whenever possible.

### Constructor and conversion vocabulary

| Name | Meaning |
|---|---|
| `new` | primary constructor without fallible validation |
| `try_new` | constructor that may fail |
| `from_*` | infallible conversion from a named source |
| `try_from_*` | fallible conversion |
| `as_*` | cheap borrowed view |
| `to_*` | potentially expensive borrowed conversion or allocation |
| `into_*` | consumes `self` |
| `with_*` | builder-style configuration or copy with one changed property |
| `is_*` | boolean state/predicate |
| `has_*` | boolean ownership/presence predicate |
| `can_*` | boolean capability predicate |
| `should_*` | policy decision |

An async function SHOULD NOT include `_async` in its name merely because it is async.

## 4.6. Variables

Variable names MUST communicate domain meaning.

Good:

```rust
committed_snapshot
candidate_entities
publication_manifest
executable_digest
```

Bad:

```rust
x
tmp
data
obj
res
val
foo
bar
```

Short names are acceptable for narrow conventional scopes:

- `i` for a small loop index;
- `id` when the type and scope are obvious;
- `tx`/`rx` for a local channel pair;
- `err` for a local error;
- `path` for one unambiguous path.

Do not encode types into names:

```text
snapshot_vec
entity_string
resolver_arc
```

Name by meaning, not storage representation.

## 4.7. Boolean names

Boolean fields and predicates MUST read naturally as a question.

Good:

```rust
is_committed
has_evidence
can_retry
should_rebuild
```

Bad:

```rust
committed_flag
evidence_bool
retry
rebuild_status
```

Avoid boolean function parameters because call sites are unclear.

Bad:

```rust
publish(snapshot, true, false)
```

Good:

```rust
publish(
    snapshot,
    PublicationOptions {
        update_latest: true,
        write_read_model: false,
    },
)
```

## 4.8. Identifiers

Different identifier domains MUST use distinct newtypes.

Good:

```rust
struct EntityId(String);
struct StableKey(String);
struct SnapshotId(String);
struct TraceId(String);
```

Do not use raw `String` where mixing identities would create a correctness risk.

Storage foreign keys MUST use canonical IDs, not display names or stable keys, unless the schema explicitly defines otherwise.

---

# 5. API Design

## 5.1. Make invalid states difficult to represent

Prefer enums and newtypes over loosely related optional fields.

Bad:

```rust
struct SnapshotQuery {
    latest: bool,
    snapshot_id: Option<String>,
}
```

Good:

```rust
enum SnapshotSelector {
    Exact(SnapshotId),
    LatestCommitted,
}
```

## 5.2. Inputs and outputs

Public operations SHOULD use named request/response types when they have more than three meaningful fields.

Good:

```rust
async fn index(&self, request: IndexRequest) -> Result<IndexReport>;
```

Avoid long positional parameter lists.

## 5.3. Public surface

Public exports MUST be intentional.

Avoid wildcard re-exports:

```rust
pub use module::*;
```

Prefer explicit exports:

```rust
pub use indexing::{IndexReport, IndexRequest, IndexService};
```

Internal types SHOULD be `pub(crate)` or private.

A type MUST NOT be public only to simplify an internal test.

## 5.4. Collections and bounds

Public APIs MUST document:

- ordering guarantees;
- duplicate behavior;
- truncation behavior;
- maximum result count;
- snapshot semantics;
- whether results are deterministic.

Agent-facing APIs MUST be bounded.

A query response that truncates results MUST report:

- returned count;
- total or known remaining count when available;
- truncation state;
- applied limit.

## 5.5. Compatibility

Public schema, CLI, daemon, MCP, and serialized format changes MUST include:

- compatibility impact;
- migration plan;
- versioning decision;
- tests for old/new behavior where compatibility is promised.

Silent semantic changes are forbidden.

---

# 6. Function and Module Design

## 6.1. Single level of abstraction

A function SHOULD operate at one conceptual level.

Bad:

```text
parse CLI arguments
open files
run extractor
commit database transaction
format terminal output
```

inside one function.

Good:

```text
parse command
build request
invoke application service
render response
map exit code
```

## 6.2. Function size

There is no universal correct line count, but these are review thresholds:

- over 40 logical lines: review for extraction;
- over 60 logical lines: splitting is normally required;
- over 100 logical lines: MUST include a documented reason if retained.

Generated code, declarative tables, and exhaustive match mappings may exceed these thresholds.

Do not split a cohesive function into meaningless one-line helpers merely to satisfy a number.

## 6.3. Parameter count

A function SHOULD have no more than four meaningful parameters.

Use a request/config struct when:

- parameters share one lifecycle;
- multiple booleans appear;
- optional values are present;
- the call is public;
- the operation is expected to evolve.

## 6.4. Module size

Review thresholds:

- over 500 lines: evaluate decomposition;
- over 800 lines: decomposition is normally required;
- mixed unrelated responsibilities: split regardless of size.

A module MUST have one sentence that explains its responsibility without using “and” repeatedly.

## 6.5. Helpers

A helper MUST have a meaningful name and a stable responsibility.

Do not create:

```rust
fn helper(...)
fn helper2(...)
fn do_work(...)
fn process_data(...)
```

Private helpers SHOULD stay close to their caller unless they represent a reusable concept.

Shared helpers MUST be placed in the narrowest common module, not a global `utils` module.

---

# 7. Error Handling

## 7.1. Library versus application errors

Library crates SHOULD use typed errors, normally with `thiserror`.

Entry-point crates MAY use `anyhow` for top-level context and reporting.

Do not expose `anyhow::Error` as a stable library API.

## 7.2. Error categories

Errors MUST preserve machine-readable categories.

Expected categories include:

```text
InvalidInput
NotFound
Conflict
Busy
Unauthorized
Forbidden
Cancelled
DeadlineExceeded
AdapterProtocol
AdapterExecution
StorageUnavailable
StorageCorruption
SnapshotNotCommitted
Unsupported
Internal
```

Do not force callers to parse error strings.

## 7.3. Context

Errors SHOULD include actionable context:

- operation;
- canonical identifier;
- safe path where appropriate;
- backend/adapter name;
- expected and actual state.

Do not leak:

- secrets;
- authentication tokens;
- unfiltered environment variables;
- arbitrary raw subprocess output;
- sensitive paths in public protocol responses.

## 7.4. Panic policy

Production library code MUST NOT use `unwrap()` or `expect()` for recoverable input, I/O, parsing, process, network, storage, or protocol failures.

Allowed cases:

- tests;
- compile-time constants;
- proven invariants that cannot be represented in the type system.

An invariant `expect` MUST explain why failure is impossible:

```rust
.expect("validated non-empty batch must have a first element")
```

Do not use:

```rust
.expect("should work")
```

`panic!`, `todo!`, and `unimplemented!` MUST NOT be committed on reachable production paths.

---

# 8. Async and Concurrency Rules

## 8.1. No blocking work on async workers

Async code MUST NOT directly call:

- `std::process::Command`;
- blocking file APIs for large or unpredictable work;
- `std::thread::sleep`;
- blocking locks across `.await`;
- synchronous network clients.

Use:

- Tokio async APIs;
- a dedicated `ProcessRunner`;
- `spawn_blocking` only as a bounded transitional boundary.

## 8.2. Cancellation

Long-running operations MUST accept or derive an `OperationContext` containing:

- cancellation token;
- deadline;
- trace ID;
- resource limits.

Cancellation MUST propagate into:

- discovery;
- extraction;
- linking;
- checking;
- storage;
- projection;
- external processes.

Cancellation MUST NOT publish a partial generation.

## 8.3. Bounded concurrency

Every concurrent fan-out MUST have an explicit bound.

Bad:

```rust
join_all(files.into_iter().map(extract))
```

when the number of files is unbounded.

Good:

- bounded stream concurrency;
- semaphore;
- per-adapter concurrency limit;
- queue size limit;
- memory budget.

## 8.4. Locks

Do not hold a lock across `.await` unless the lock type and critical section were explicitly designed for it.

Lock scope MUST be minimal.

Cross-process state requires an OS/database-level coordination mechanism; an in-process mutex is insufficient.

## 8.5. Tasks

Spawned tasks MUST have an owner.

The code MUST define:

- who cancels the task;
- who awaits or supervises it;
- how errors are observed;
- when it terminates.

Detached fire-and-forget tasks are forbidden unless their loss is explicitly harmless and documented.

---

# 9. Storage and Snapshot Rules

## 9.1. Snapshot selection

Every user-visible query MUST select:

```rust
SnapshotSelector::Exact(id)
```

or:

```rust
SnapshotSelector::LatestCommitted
```

Queries MUST NOT merge multiple snapshots unless the API explicitly represents a historical query.

## 9.2. Committed visibility

Uncommitted snapshots MUST NOT be visible to normal readers.

`latest` always means latest fully committed and published generation.

## 9.3. Publication

Publication MUST follow:

```text
stage
validate
prepare
commit immutable artifacts
switch pointers last
```

Direct writes to final mutable locations are forbidden for multi-file publication.

## 9.4. Atomicity

After a crash, readers MUST observe either:

- the previous complete generation;
- the new complete generation.

A mixed generation is a correctness defect.

## 9.5. Conformance

All storage backends MUST pass the same contract suite.

Backend-specific tests do not replace shared conformance tests.

---

# 10. Adapter Rules

## 10.1. Adapter identity

Every adapter MUST define:

- stable adapter ID;
- supported inputs;
- produced entity/fact/relation/diagnostic kinds;
- evidence behavior;
- deterministic ordering behavior;
- invalidation scope;
- resource limits;
- version/capability information.

## 10.2. Evidence

Every emitted fact, relation, and diagnostic MUST include evidence where the canonical contract requires it.

Evidence SHOULD be as narrow as the source permits.

Do not emit guessed source locations.

## 10.3. Determinism

Given identical:

- input bytes;
- configuration;
- adapter version;
- dependency versions;

an adapter SHOULD produce canonically equivalent output.

Nondeterminism MUST be documented and isolated.

## 10.4. External processes

External adapters MUST:

- be disabled by default unless explicitly trusted;
- use canonical executable paths;
- verify manifest and executable identity;
- clear the environment and pass an allowlist;
- enforce input/output/time/process limits;
- separate protocol stdout from diagnostic stderr;
- terminate the full process tree;
- never rely on shell string concatenation.

---

# 11. Refactoring Protocol

## 11.1. Define the invariant first

Before refactoring, write down:

- behavior that MUST remain unchanged;
- behavior intentionally changed;
- public APIs affected;
- data/schema compatibility;
- performance constraints;
- tests that prove preservation.

If the current behavior is unclear, add characterization tests before changing structure.

## 11.2. Separate mechanical and semantic changes

Whenever practical:

1. add or strengthen tests;
2. perform mechanical rename/move;
3. verify;
4. change behavior;
5. verify again;
6. remove compatibility scaffolding.

A PR SHOULD NOT combine a workspace-wide rename with unrelated logic changes.

## 11.3. Preserve intermediate buildability

Each commit SHOULD compile and have a coherent purpose.

A large migration MAY use temporary adapters or deprecated APIs, but the compatibility path MUST be explicit and time-bounded.

## 11.4. Rename correctly

Before renaming a symbol:

- identify its actual responsibility;
- choose the domain term;
- search all call sites, docs, examples, schemas, tests, and CLI text;
- update errors and metrics where the old term appears;
- avoid aliases that leave two names indefinitely.

Do not rename solely for personal taste.

## 11.5. Move before rewriting

When decomposing a large module, first move behavior with minimal edits.

After tests pass, simplify each extracted component.

This makes review and regression detection easier.

## 11.6. Remove dead code

A completed refactor MUST remove:

- obsolete types;
- unused compatibility aliases;
- dead branches;
- duplicate helpers;
- stale documentation;
- old feature flags.

Do not leave “temporary” code without an issue reference and removal condition.

## 11.7. No speculative abstractions

Do not introduce an abstraction for hypothetical future needs.

Use the rule of three as guidance:

- one case: keep concrete;
- two cases: observe the shared shape;
- three cases or a clear architectural boundary: extract deliberately.

Ports required by architecture are an exception.

---

# 12. Testing Rules

## 12.1. Test observable behavior

Tests SHOULD assert:

- outputs;
- state transitions;
- error codes;
- publication visibility;
- deterministic canonical form;
- cancellation behavior;
- resource boundaries.

Avoid tests that merely duplicate private implementation details.

## 12.2. Test naming

Test names MUST describe scenario and expected result.

Good:

```rust
latest_selector_returns_only_committed_snapshot
relation_query_filters_by_from_entity
cancelled_publication_keeps_previous_generation_visible
```

Bad:

```rust
test1
works
snapshot_test
```

## 12.3. Test structure

Prefer Arrange–Act–Assert with visually separated phases.

One test SHOULD prove one principal behavior.

## 12.4. Required test classes

Use the appropriate level:

- unit tests for local invariants;
- contract tests for shared ports;
- integration tests for use cases;
- property tests for canonicalization and determinism;
- fault-injection tests for publication/recovery;
- concurrency tests for readers/writers;
- fixture tests for extractors;
- protocol compatibility tests for daemon/MCP.

## 12.5. Regression tests

Every bug fix MUST include a test that fails before the fix and passes after it, unless the failure cannot be reproduced in automation.

When automation is impossible, the PR MUST explain why and provide a deterministic manual verification procedure.

## 12.6. Fixtures

Fixtures MUST be:

- minimal;
- named by scenario;
- free of unrelated syntax;
- checked into the narrowest relevant crate;
- documented when their purpose is not obvious.

Large real-world fixtures SHOULD be separated from fast unit tests.

---

# 13. Documentation and Comments

## 13.1. Comments explain why

Comments SHOULD explain:

- invariant;
- tradeoff;
- non-obvious safety condition;
- protocol requirement;
- compatibility constraint;
- reason for an unusual implementation.

Comments SHOULD NOT narrate obvious code.

Bad:

```rust
// Increment counter
counter += 1;
```

Good:

```rust
// Snapshot IDs are allocated under the writer lock so two CLI processes
// cannot publish the same immutable generation.
let snapshot_id = sequence.next();
```

## 13.2. Rustdoc

Public APIs MUST document:

- purpose;
- invariants;
- error conditions;
- cancellation;
- ordering;
- snapshot semantics;
- examples where useful.

Use intra-doc links for related types.

## 13.3. TODO policy

TODO/FIXME comments MUST include:

- concrete missing work;
- reason it is deferred;
- issue or roadmap reference;
- removal condition where applicable.

Bad:

```rust
// TODO improve this
```

Good:

```rust
// TODO(#184): replace the compatibility resolver after protocol v2 support is removed.
```

## 13.4. Documentation changes

Changes to architecture, behavior, CLI, adapters, schemas, generated artifacts, or Definition of Done MUST update the relevant English documentation in the same task.

---

# 14. Lints, Formatting, and Unsafe Code

## 14.1. Formatting

`rustfmt` is authoritative.

Contributors MUST NOT manually align code in a way that fights `rustfmt`.

Required:

```bash
cargo fmt --all -- --check
```

## 14.2. Clippy

Required:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Full-feature CI SHOULD also run:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Do not enable every Clippy restriction lint blindly.

Lints SHOULD be adopted deliberately, with repository-wide consistency.

## 14.3. Lint suppression

Prefer:

```rust
#[expect(clippy::some_lint, reason = "precise documented reason")]
```

over an unexplained `#[allow(...)]`.

A suppression MUST be scoped as narrowly as possible.

Do not suppress a lint to avoid understanding the warning.

## 14.4. Unsafe code

`unsafe` MUST NOT be introduced unless:

- safe Rust cannot satisfy the requirement;
- the performance or FFI need is demonstrated;
- invariants are documented in a `// SAFETY:` comment;
- tests cover boundary cases;
- review explicitly examines soundness.

Each unsafe block MUST be minimal.

Crates not requiring unsafe SHOULD deny it:

```rust
#![forbid(unsafe_code)]
```

---

# 15. Dependency Rules

A new dependency MUST have a documented reason.

Before adding one, check:

- can the standard library solve it clearly;
- is an existing workspace dependency sufficient;
- maintenance status;
- license compatibility;
- MSRV compatibility;
- feature footprint;
- transitive dependency cost;
- security history;
- whether it is required in default features.

Dependencies MUST be declared in `[workspace.dependencies]` when shared.

Optional backends SHOULD remain optional and MUST NOT inflate minimal builds without a justified reason.

---

# 16. Performance Rules

Do not optimize based only on intuition.

A performance-sensitive change MUST define:

- workload;
- baseline;
- metric;
- target;
- benchmark or trace;
- correctness constraints.

Important Athanor metrics include:

- wall-clock duration by phase;
- peak RSS;
- allocation volume;
- task concurrency;
- queue depth;
- snapshot size;
- incremental/full rebuild ratio;
- external adapter duration;
- query latency and returned/truncated counts.

Avoid cloning large source content or complete canonical collections without measurement and justification.

---

# 17. Security Rules

Code MUST treat repository content, manifests, plugin output, and external process output as untrusted input.

Required practices:

- validate before use;
- bound memory and output sizes;
- avoid shell interpolation;
- canonicalize and verify paths;
- reject directory traversal;
- avoid secret logging;
- use constant-time comparison where secrets are involved;
- preserve least privilege;
- fail closed for trust decisions;
- use explicit allowlists.

Security-sensitive behavior MUST include negative tests.

---

# 18. Agent-Specific Working Rules

A coding agent MUST perform these steps before editing:

1. Read `AGENTS.md`.
2. Read this document.
3. Read `docs/development/agent-workflow.md`.
4. Read the relevant architecture document.
5. Read the relevant crate `README.md`.
6. Search for existing symbols and equivalent implementations.
7. Identify the exact invariant and acceptance criteria.
8. List files expected to change.
9. Identify tests and documentation to update.

The agent MUST NOT:

- invent architecture without reading current contracts;
- create generic `utils`, `helpers`, or `manager` modules;
- introduce a new type when an equivalent canonical type exists;
- bypass a port by importing a concrete adapter into application logic;
- silently change serialized or protocol behavior;
- perform a broad rewrite when a local fix is sufficient;
- delete tests merely because they fail after a refactor;
- mark work complete without reporting verification;
- claim commands passed when they were not run;
- hide deferred work or known limitations.

The agent SHOULD stop and re-plan when:

- more than the expected crates need changes;
- a new cross-layer dependency appears;
- an existing invariant conflicts with the requested change;
- a supposedly mechanical refactor changes behavior;
- test failures reveal unrelated architectural debt.

---

# 19. Required Verification

## 19.1. Every Rust code change

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
```

## 19.2. Indexing or runtime changes

```bash
cargo run -p ath --quiet -- index .
```

Also run the narrowest relevant bounded query or validation command.

## 19.3. Feature-sensitive changes

Run affected feature combinations, including minimal and relevant optional backends.

Examples:

```bash
cargo check --workspace --no-default-features
cargo test -p <affected-crate> --all-features
```

## 19.4. Documentation-only changes

Rust checks are not required unless code snippets, generated behavior, configuration, schemas, or commands changed.

Links and referenced paths MUST still be checked.

---

# 20. Pull Request Quality Gate

A PR is ready only when all applicable answers are “yes”.

## Architecture

- [ ] Is the change in the correct layer/crate?
- [ ] Do dependencies point inward?
- [ ] Are adapter-specific details contained?
- [ ] Is there only one source of truth?

## Naming

- [ ] Do files describe responsibilities?
- [ ] Do types use domain nouns?
- [ ] Do functions use precise verbs?
- [ ] Are vague names avoided?
- [ ] Are IDs represented by distinct types?

## Implementation

- [ ] Are side effects explicit?
- [ ] Is concurrency bounded?
- [ ] Is cancellation propagated?
- [ ] Are errors typed?
- [ ] Are panics avoided?
- [ ] Are public results bounded and deterministic?

## Refactoring

- [ ] Was behavior characterized first?
- [ ] Are mechanical and semantic changes reviewable?
- [ ] Were obsolete paths removed?
- [ ] Was public compatibility handled explicitly?

## Tests

- [ ] Does a regression test cover the defect?
- [ ] Do shared ports have contract tests?
- [ ] Are failure and cancellation paths tested?
- [ ] Are tests named by behavior?

## Documentation

- [ ] Are public APIs documented?
- [ ] Are architecture/adapter docs updated?
- [ ] Is `docs/README.md` updated when document structure changed?
- [ ] Are limitations and deferred items explicit?

## Verification

- [ ] Did formatting pass?
- [ ] Did workspace tests pass?
- [ ] Did Clippy pass with warnings denied?
- [ ] Did relevant runtime/indexing checks pass?
- [ ] Are actual commands and results reported?

---

# 21. Completion Report Format

Every implementation report MUST include:

```text
Summary
- What behavior changed.

Architecture
- Why the change belongs in the selected crates/modules.

Files changed
- Exact files and their responsibilities.

Tests
- Tests added or updated and what they prove.

Verification
- Exact commands run and their outcomes.

Compatibility
- API/schema/protocol/storage impact.

Known limitations
- Explicit deferred work.

Next step
- The smallest logical follow-up.
```

---

# 22. Enforcement Plan

Documentation alone is not sufficient. These rules SHOULD be enforced through:

- `rustfmt`;
- Clippy with warnings denied;
- workspace lint configuration;
- architectural dependency checks;
- store contract suites;
- protocol schema tests;
- required GitHub checks;
- PR template checklists;
- code ownership for critical crates;
- coverage/no-regression gates;
- fault-injection and concurrency CI jobs.

Recommended follow-up files:

```text
rustfmt.toml
clippy.toml
.github/pull_request_template.md
.github/CODEOWNERS
CONTRIBUTING.md
SECURITY.md
```

Recommended root `Cargo.toml` addition after lint review:

```toml
[workspace.lints.rust]
unsafe_code = "warn"
unused_must_use = "deny"

[workspace.lints.clippy]
correctness = { level = "deny", priority = -1 }
suspicious = { level = "deny", priority = -1 }
complexity = { level = "warn", priority = -1 }
perf = { level = "warn", priority = -1 }
style = { level = "warn", priority = -1 }
```

Each crate SHOULD inherit approved workspace lints:

```toml
[lints]
workspace = true
```

Do not add lint groups without first running them across the workspace and reviewing false positives.

---

# 23. External References

These references inform the standard but do not override Athanor-specific rules:

- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
- Rust Clippy lint index: https://rust-lang.github.io/rust-clippy/master/index.html
- Rust documentation guidelines: https://doc.rust-lang.org/rustdoc/
- Rust Reference: https://doc.rust-lang.org/reference/
- Cargo Book: https://doc.rust-lang.org/cargo/
