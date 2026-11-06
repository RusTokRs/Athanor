---
id: doc://docs/adapters/transport-http.md
kind: adapter
language: en
source_language: en
status: active
---
# Local HTTP Documentation Reader

`athanor-transport-http` is the entrypoint adapter for `ath docs serve`: a local, read-only browser
reader over published documentation, with lexical/semantic/hybrid search and bounded Explore answers.
It does not add an LLM, a generation workflow, an HTTP framework, or a daemon/MCP protocol revision.

**Status:** branch-local implementation, not released. Rust 1.95 [CI run 41926650130](https://github.com/RusTokRs/Athanor/actions/runs/41926650130)
passed on exact source `173355983a348f263b0e4e516f159cfcdcce06d2`: formatting, workspace tests, Clippy, indexing and editable-docs smoke
on Linux/Windows/macOS; default/store-surreal/js-ts-precision/all-features; coverage; and cargo-deny.
The Unix adapter suite has 47 tests (45 on Windows), plus five binary CLI regressions. Current-head
AppSec, Store, production E2E, and documentation checks are linked from
[PR #183](https://github.com/RusTokRs/Athanor/pull/183); release gates still require the actual release SHA.
Local crates.io access remains unavailable; the dependency-enabled CI, not isolated local checks,
is the full-workspace execution evidence.

## Start The Reader

Publish documentation from an exact committed snapshot first, then open the URL printed by `serve`:

```bash
ath index . --json
ath docs generate-architecture . --snapshot <exact-snapshot-from-index>
ath docs generate-module . --snapshot <same-exact-snapshot>
ath docs serve . --bind 127.0.0.1:7878
```

The root defaults to `.` and the bind defaults to `127.0.0.1:7878`. An IPv6 loopback address is also
supported, for example `--bind '[::1]:7878'`. Port `0` selects an available port:

```bash
ath docs serve /path/to/project --bind 127.0.0.1:0 --json
```

`--json` emits **one JSON start-up line after binding** (`athanor.documentation_serve.v1`), containing
the canonical root, actual `addr`, browser `url`, documentation root, initial generation/rejection
counts, validated current marker, and connection bound. It is not an event stream or live catalog.
Ctrl-C stops admission, cancels active query operations, and drains their workers.

A project with no published generations still gets an explanatory index page. Search/Explore without
a canonical snapshot return `503 knowledge_unavailable` and an indexing hint; the reader does not
initialize or index the project for the user.

## Pages And Snapshot Semantics

- **Catalog:** all loadable architecture, module, API, operations, and onboarding generations within
  the configured inspection bound; newest numeric generation first, including earlier profiles.
- **Generation:** rendered Markdown, snapshot/profile identity, generation limits and omissions,
  checksum descriptors, matching validation summary, validated manifest, and declared raw artifacts.
- **Evidence panel:** published footnotes retain their jump anchors and cited stable keys link to
  Explore. A key lookup is explicitly a **latest-snapshot** query, not exact historical source access.
- **Search:** the existing `athanor.search.v1` report, all three local retrieval modes, explicit result
  bounds, stable-key links, source anchors, and truncation disclosure.
- **Explore:** the existing `athanor.explore.v1` report, exact `target` or retrieved entries, citations,
  bounded declaration excerpts, incoming/outgoing relations, relation chains, and aggregate blast
  radius. Omitted entries, links, chains, files, and unreadable excerpts remain visible.

All five publishers share `.athanor/generated/documentation/current.json`; publishing another profile
replaces that one pointer, not the earlier immutable directories. Consequently the reader enumerates
`generations/<number>/` and treats `current.json` only as a validated marker. A broken or out-of-bound
pointer is disclosed without hiding otherwise readable history. Staging/non-generation names are
not publications. The catalog is reloaded per request, so external publications appear on refresh
without restarting the reader.

Documents retain their manifest's exact snapshot identity. Search and Explore use the latest canonical
snapshot selected by the existing application services. Inline excerpts read the current working tree,
not a historical source checkout. These distinctions are stated on the pages; there is no silent
historical/latest fallback.

## Read-Only Boundary And Cache Side Effects

The adapter never invokes indexing, generation, repair, patches, plugin execution, or publication.
It emits no canonical entities/facts/relations/diagnostics. It does not modify source files,
canonical snapshots/pointers, published generations, or documentation `current.json`.

**Read-only does not mean no filesystem cache writes.** Existing search/Explore retrieval can rebuild
`.athanor/generated/current/search` and its index metadata/vector sidecar when absent or stale, as
`ath search` already does. Query requests are serialized within one reader to avoid competing cache
rebuilds. Other processes retain the existing application cache concurrency semantics. The CLI suite
compares canonical and documentation file sets/bytes before and after HTTP queries, separately from
this disposable cache. A read-only filesystem with a missing/stale cache can return a query error;
the reader does not silently switch retrieval semantics.

No outbound provider or asset request is introduced. HTTP network activity is a loopback listener and
responses on its accepted sockets. Application dependencies remain explicitly supplied through
`RuntimeComposition`; production composition retains the configured local Store behavior.

## Routes And Contracts

Only `GET` and `HEAD` are supported, one request per connection, with `Connection: close`.

| Route | Result |
| --- | --- |
| `/` | Generation catalog and query forms |
| `/healthz` | Plain text `ok` plus newline |
| `/api/generations` | `athanor.documentation_serve_catalog.v1` |
| `/g/<generation>/` | Generation Markdown and evidence panels |
| `/g/<generation>/manifest.json` | Validated `athanor.documentation_generation_manifest.v1` directly, not a CLI inspection wrapper |
| `/g/<generation>/raw/<declared-path>` | Checksum-verified declared artifact, never an arbitrary repository file |
| `/search?q=...&mode=lexical&limit=10` | Search HTML |
| `/api/search?q=...&mode=hybrid&limit=10` | Existing `athanor.search.v1` report |
| `/explore?q=...&target=...&mode=lexical&depth=2` | Explore HTML; a non-empty query or target is required |
| `/api/explore?q=...&target=...&mode=semantic&depth=2` | Existing `athanor.explore.v1` report |

The catalog includes applied `limits`, `returned`, `omitted_generations`, validated generations,
`invalid_generations`, `current`, and `current_error`. A valid manifest does not assert that every
body has already been checked: each requested/rendered body is checksum-checked before use. A
validation summary additionally requires a declared, checksum-matching report with matching schema,
draft schema, snapshot, and profile.

Routed `/api/*` errors use `athanor.documentation_serve_error.v1` with `code`, `status`, `message`,
and nullable `hint`; other pages use escaped HTML errors. Malformed heads without a usable target
can receive a generic HTML rejection or a closed connection. Typical status codes are `400` for bad
inputs/traversal, `404` for unknown/undeclared artifacts, `405` with `Allow: GET, HEAD` for unsupported
methods, `413` for oversized artifacts, `414` for oversized targets/queries, `421` for non-loopback
Host, `431` for oversized heads, `500` for integrity/read failures, `503` without knowledge, `504` for
operation timeout, and `505` for unsupported HTTP versions. HEAD preserves response headers and
suppresses the body.

These three serve schemas are transport-owned additive contracts; existing CLI, daemon, MCP,
canonical storage, docgen, search, and Explore wire formats are unchanged.

## Bounds

| Option | Default | Accepted range / semantics |
| --- | --- | --- |
| `--search-limit` | `10` | Default result limit, 1–50; URL `limit` may override within 1–50 |
| `--explore-entries` | `5` | Maximum entries per request, 1–20 |
| `--explore-depth` | `2` | Default hops, 1–4; URL `depth` may override within 1–4 |
| `--max-generations` | `200` | Inspect at most 1–1000 newest generation candidates |
| `--max-document-bytes` | `4194304` | Raw body limit, 1024–67108864 bytes; oversized bodies are refused, not truncated |
| `--request-timeout-secs` | `10` | 1–120 seconds separately for head reading, operation execution, and response writing |

Fixed bounds: eight concurrent connections; 8 KiB request heads; 2 KiB targets; 64 headers; eight path
segments of at most 128 decoded bytes; 16 distinct query parameters with at most 1024 decoded bytes
per key/value; 64 KiB manifests; 1 MiB pointers/validation metadata; 16 declared artifacts per generation;
and an aggregate 8 MiB Markdown source budget per generation page. A page refuses bodies that exceed
its remaining budget and retains their raw links. Explore retains its application link/path/traversal
and excerpt bounds. Cancellation/deadline completion drains operation-aware workers rather than
abandoning them. Core cancellation admission prunes released weak registrations so a long-running
reader does not retain every historical request identity.

The directory walk still enumerates generation names to choose the newest candidates, but checks
request cancellation/deadlines between entries and retains only a bounded candidate set in memory.
Bodies are not cached in the reader.

## Security And Rendering

- Non-loopback binds are refused before socket creation; there is no `0.0.0.0` override.
- HTTP/1.1 requires one Host header naming localhost or a loopback IP. Duplicate/malformed Host,
  unexpected host suffixes, ambiguous body framing, and transfer coding are rejected. No CORS access
  is granted. This mitigates browser DNS rebinding; it is not authentication against local users.
- Percent-decoded traversal/separator/control segments are rejected before routing. Publication roots,
  manifests, and bodies are canonicalized and confined; generation-directory symlinks and escaping
  artifact symlinks are refused. An existing dangling publication root is an error, not an empty catalog. Only manifest-declared paths are served, with SHA-256 checked each time.
- Raw Markdown HTML is escaped. Images become alt text; non-local Markdown links are inert. No script,
  remote stylesheet, font, CDN, or external image is needed. Mermaid fences remain readable source,
  not JavaScript-rendered diagrams.
- Responses use a restrictive CSP (`default-src 'none'`, inline styles only, same-origin forms,
  no framing/base), `nosniff`, `no-referrer`, `no-store`, and `X-Frame-Options: DENY`.
- There is no generic raw-source endpoint; Explore only inlines the cited bounded declaration ranges
  already supported by the application. Existing source/excerpt data-handling policy is unchanged.

Use this for a trusted local project, not as a public web service. There is no authentication, TLS,
reverse-proxy mode, editing, streaming Q&A, or multi-repository session. No diagram/provider/daemon/MCP
expansion is implied by this slice.

## Dependency Decision

HTTP framing uses existing Tokio I/O/net plus the standard library; the daemon's framed transport is
not HTTP and is not reused. Markdown uses the already locked workspace `pulldown-cmark 0.13.6` with
only its `html` feature added, not its CLI/default features. That feature adds the dependency-free
`pulldown-cmark-escape 0.11.0` (MIT, Rust 1.70), compatible with the workspace's Rust 1.95 and
`deny.toml` license/source policy. This is not a workspace-wide advisory remediation; existing RUSTSEC
work remains separately gated. The recorded CI includes locked cargo-deny; release checks must still use the exact release source.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-transport-http --locked
cargo test -p ath --test documentation_serve_cli --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```

Regressions cover HTTP parsing, Host/framing/size rejection, route/query limits, published checksum and
path confinement, current-pointer identity, validation-report integrity, deterministic Markdown
escaping/evidence links, HEAD, cancellation with idle clients, CLI start-up, all five profiles,
publication refresh without restart, retrieval modes/Explore, and canonical/documentation immutability.
The exact-source CI record above is execution evidence; source presence and isolated local checks alone are not.
