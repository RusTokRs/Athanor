---
id: doc://docs/adapters/explore.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Bounded Code Exploration (ath explore, MCP `explore`)

`explore` answers one orientation question in a single bounded call: which canonical entities the
question is about, the cited source slice of those declarations, what they are connected to, which
relation chains lead away from them, and how large their blast radius is within the requested hop
budget. Graph fields are read out of one committed canonical snapshot. Excerpts re-read only the
line range each declaration already cites, clipped to 32 lines and 4,096 bytes, so the query surface
stays bounded even for large repositories.

The owner is `crates/athanor-app/src/explore.rs`, with the traversal in
`crates/athanor-app/src/explore/paths.rs` and the excerpt reader in
`crates/athanor-app/src/explore/excerpts.rs`. The same application service backs the CLI command and
the MCP tool, so both surfaces report identical budgets and disclosures.

## CLI Command Usage

```bash
# Orient around a retrieval query
ath explore "index publication rollback"

# Walk two hops (default) and report the chains plus the blast radius
ath explore "store load" --depth 2 --json

# Anchor on an exact entity or file instead of searching first
ath explore --target "symbol://rust:crate::store::Store::load"

# Anchor on a whole file and widen the budgets
ath explore --target "src/store.rs" --entries 10 --links 12 --paths 32

# Combine with semantic or hybrid retrieval
ath explore "where does the index get published" --mode hybrid --depth 3
```

## Argument Budgets

| Argument | Default | Maximum | Bounds |
| --- | --- | --- | --- |
| `--entries` | 5 | 20 | Entry entities resolved from the query or target. |
| `--links` | 8 | 40 | Relations per entry and direction. |
| `--depth` | 2 | 4 | Code-flow hops explored per entry. |
| `--paths` | 16 | 64 | Multi-hop chains returned per request. |
| `--mode` | `lexical` | — | `lexical`, `semantic`, or `hybrid` retrieval. |

Every value is clamped into its range, so a request cannot fan out into a whole-graph walk. The
traversal additionally stops after 4,096 relation-edge expansions per request and lists at most 24
blast-radius files; both limits are reported, never hidden.

## Reported Fields

- `entries` — ranked entities with stable key, kind, name, source anchor, a bounded `excerpt` of the
  cited declaration range when the working tree can supply it, retrieval score, up to four
  de-duplicated fact citations, open diagnostic counts, and the one-hop relation neighbourhood in both
  directions with per-link evidence. Only diagnostics whose canonical status is `open` are counted.
- `paths` — the shortest chain from each entry to each entity reached at two hops or deeper. Each step
  carries the relation kind, linker status and confidence, traversal `direction` relative to the stored
  relation, both endpoints, and the first cited file and line. The reached declaration also carries a
  bounded `excerpt` when its cited range can be read. Text output prints both entry and path-target
  excerpts. Chains of one hop are deliberately not repeated: they are already the entry's `outgoing`
  and `incoming` links.
- `blast` — aggregate blast radius for the same hop budget: entries, distinct reached entities beyond
  the entry set, distinct
  relations that carried the traversal, expanded edge count, per-hop reach counts at each entity's
  shortest depth across the entry set, open diagnostics over the entry set plus everything it reaches,
  and the sorted file list. An entity reached from several entries counts once in `reached` and once in
  `by_depth`.
- `omitted` — unresolved seeds, candidates dropped by the entry budget, link and path truncation counts,
  unread source excerpts, and human-readable notes for each disclosure.
- `limits` and `anchor` — the effective budgets and the `--target` value, so a response can be replayed.

## Traversal Rules

Chains follow relation kinds that carry code influence, in both directions, with the shortest chain kept
per reached entity: `Calls`, `Imports`, `Contains`, `Extends`, `Implements`, `ImplementedBy`, `TestedBy`,
`CoveredByTest`, and `Documents`. Any other relation kind — contract support edges, environment or schema
uses, and generated or inferred drift edges — never carries the traversal. `ath impact`
([Impact](impact.md)) stays the authoritative deep blast-radius query with its own propagation table,
and `ath change-map` ([Change Map](change-map.md)) remains the ranked candidate list for an edit task.

Ordering is deterministic end to end: adjacency is sorted by relation kind, then counterpart stable key,
then relation id; chains are sorted by entry stable key, hop, and target stable key; files are sorted
lexically. Two identical requests against the same snapshot and the same working tree produce
byte-identical reports.

## Boundaries

- A `--target` that resolves no canonical entity is an error, not an empty report; retrieval falls back
  to nothing rather than guessing, and an empty `query` without `--target` is rejected.
- Results are bounded by the budgets above. Every dropped candidate, truncated link list, truncated chain
  list, and omitted file is counted in `omitted` and described in a note.
- Confidence and status are copied from the canonical relation, so an `inferred` call edge stays marked as
  such instead of reading like a verified fact.
- The report never dumps a file. An excerpt is only the cited declaration range, clipped to 32 lines
  and 4,096 bytes, and an absolute path, a `..` component, a missing file, and non-UTF-8 contents are
  counted in `omitted.source_unread` instead of being guessed. Blast-radius files stay a path list.

## Verification

```bash
cargo test -p athanor-app explore --locked
cargo test -p ath --test direct_read_cli --locked
```
