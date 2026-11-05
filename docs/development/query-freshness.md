---
id: doc://docs/development/query-freshness.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Query Freshness

## Scope And Status

Branch-local slice F1 aligns daemon reads with the existing direct MCP latest-committed contract.
Implementation is not execution evidence: use the exact-head checks and source SHA recorded in
[PR #183](https://github.com/RusTokRs/Athanor/pull/183). This is not a release or completion of the
broader daemon-served MCP/auto-synchronization workstream.

The concrete scenario is a long-lived daemon or MCP session while another `ath index <root>` process
publishes a new canonical generation. A successful subsequent read must select that committed
snapshot, not silently continue serving the first snapshot observed by the process.

## Selection Contract

- Daemon Overview, Explain, Search, and non-diff Context resolve the latest committed snapshot through
  the composition-owned Store **on every request**, before consulting derived report/search caches.
- A report uses one selected canonical snapshot. An external commit racing a read may be observed by
  that read or the next one; fields are not assembled from two canonical snapshots. This is a
  per-request selection boundary, not a promise that the response is still latest when delivered.
- Missing, malformed, uncommitted, or unavailable latest state fails the read. A previously successful
  process-local snapshot is not a fallback and reads do not repair canonical pointers.
- Overview/Context caches remain bounded and keyed by snapshot identity plus query/limits. Search
  rebuilds or selects its derived index for the selected snapshot; unchanged-snapshot derived results
  remain reusable. Older keyed entries can remain until normal bounded eviction.
- Direct stdio MCP already selects through its composition-owned Store per tool call. Regression tests
  keep a single real `ath mcp` process alive across external index commits, pointer failures and recovery.
- Existing Context diff and Change Map paths retain their operation-aware application selection.

No CLI flag, MCP revision, canonical migration, new dependency, or new public report schema is added.
Daemon status additively exposes `cache.last_observed_snapshot` (snapshot id or null); existing status
keys and daemon v1/v2 migration inputs/v3 outputs remain unchanged.

## Status Is Observation, Not Automatic Synchronization

`cache.snapshot_loaded` and `cache.last_observed_snapshot` describe the last successful daemon read,
not a fresh Store probe by Status. They can predate a newer external commit or a failed refresh.
`last_successful_index` still describes daemon-owned index jobs only, not external CLI publications.
Use the read report's snapshot identity for the request, not either status field as a liveness signal.

A source edit without a successful index publication does not make canonical knowledge current.
MCP does not start the watcher, launch an index implicitly, or route through the daemon in F1. Existing
opt-in daemon watching remains separate; MCP-to-daemon bridging, watcher event-loss/liveness signals,
source-vs-index freshness reports and multi-repository routing still need their own bounded slices.
Explore excerpts read the current working tree and are not historical snapshot text.

## Cost, Cancellation And Writes

F1 deliberately uses the existing operation-aware canonical read port rather than introducing a
second latest-pointer parser, TTL, filesystem-mtime heuristic or backend-specific app-layer shortcut.
This reloads the canonical snapshot per daemon read instead of trusting an indefinitely resident copy;
there is no latency improvement claim. Head-only validated loading is a future measured optimization,
not a reason to serve stale state or weaken Store conformance.

Deadline/cancellation checks continue before and after Store reads and before derived cache hits.
The transport's admission limits, job accounting, bounded histories and cancellation behavior remain
unchanged. Reads never publish or repair sources, canonical snapshots or documentation. Retrieval
may rebuild the existing disposable derived search cache.

## Regression And Verification

`daemon_freshness_tests.rs` uses real JSONL publications and warmed daemon report/search caches:

- external commits update Overview, Search, Explain and Context without a daemon restart;
- an unchanged snapshot reuses bounded derived reports;
- prepared snapshots stay hidden until committed;
- missing/malformed/missing-target/uncommitted-target pointers cannot trigger stale success or repair;
- pre-cancelled/expired operations still fail with their typed categories;
- legacy compatible latest pointers remain readable after an external publication.

`mcp_freshness_cli.rs` exercises the real stdio handshake and bounded responses in one live MCP
session: a source-only edit remains on the old snapshot, external indexing becomes visible in
Overview/all Search modes/Explain/Context/Explore, and pointer errors do not return stale success or
prevent later recovery. These are transport regressions, not comparative retrieval-quality benchmarks.

```bash
cargo test -p athanor-app daemon_freshness_tests --locked
cargo test -p athanor-app daemon_read_dispatch_tests --locked
cargo test -p athanor-app --test daemon_composition_inventory --locked
cargo test -p ath --test mcp_freshness_cli --locked
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
```
