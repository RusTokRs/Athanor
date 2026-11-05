# athanor-transport-mcp

Model Context Protocol (MCP) transport adapter for Athanor.

Exposes Athanor's indexing, searching, explaining, repository orientation, bounded code exploration,
context pack generation, impact analysis, change maps, diagnostics, and completeness reporting over the
stdin/stdout stdio protocol.

## Tools Exposed

1. `index`: runs the index pipeline to parse and link code/docs.
2. `explain`: resolves details, facts, relations, and diagnostics of an entity by stable key.
3. `overview`: summarizes the latest canonical snapshot.
4. `search`: performs lexical, semantic, or hybrid retrieval over indexed knowledge.
5. `explore`: returns ranked declarations, bounded source excerpts, citations, relation chains, and blast radius.
6. `context`: builds a task-focused context pack from the latest snapshot.
7. `impact`: calculates direct and transitive blast radius of changes.
8. `change_map`: returns bounded evidence-backed change locations, relation chains, diagnostics, and test coverage.
9. `rustok_architecture_context`: resolves bounded RusTok ownership, contracts, interactions, tests, and evidence.
10. `check`: returns scoped API, documentation, environment, script, deployment, or runbook diagnostics.
11. `capabilities`: reports bounded analysis completeness of the latest canonical snapshot.

See `docs/adapters/transport-mcp.md` for protocol revisions, cancellation, deadlines, concurrency bounds,
and client configuration.

Read tools select the latest committed Store snapshot per call, not once per stdio session. External
index commits become visible without restarting MCP; source edits alone do not trigger indexing.
See [query freshness](../../docs/development/query-freshness.md) for the daemon/MCP boundary and tests.
