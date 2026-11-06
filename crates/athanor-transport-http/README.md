---
id: doc://crates/athanor-transport-http/README.md
kind: adapter
language: en
source_language: en
status: active
---
# athanor-transport-http

Loopback-only HTTP/1.x adapter behind `ath docs serve`. It reads immutable documentation generations
and exposes the existing bounded search/explore application services as script-free HTML and JSON.

The CLI supplies `RuntimeComposition` to `BoundDocumentationServer::bind`, then calls `serve` with a
`CancellationToken`. HTTP, Markdown presentation, path confinement, and connection limits stay outside
`athanor-app`, `athanor-core`, and `athanor-domain`. This entrypoint adapter does not add a new core port
or reuse the daemon's non-HTTP protocol.

No canonical entities, facts, relations, diagnostics, snapshots, or documentation generations are
written. Existing search/explore services may refresh their **derived search cache**, just as the CLI
queries do. There are no provider calls, remote assets, shell commands, or arbitrary raw-source routes.

See [HTTP documentation reader](../../docs/adapters/transport-http.md) for commands, limits, routes,
JSON schemas, security boundaries, cache semantics, verification status, and known limitations.

```bash
cargo test -p athanor-transport-http --locked
cargo test -p ath --test documentation_serve_cli --locked
```

Rust 1.95 workspace tests, Clippy, cross-platform indexing/docs smoke, feature sets, coverage, and
cargo-deny passed on `173355983a348f263b0e4e516f159cfcdcce06d2` in [CI 41926650130](https://github.com/RusTokRs/Athanor/actions/runs/41926650130).
See [PR #183](https://github.com/RusTokRs/Athanor/pull/183) for current-head checks; this is not a release.
