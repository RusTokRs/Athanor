# athanor-transport-mcp

Model Context Protocol (MCP) transport adapter for Athanor.

Exposes Athanor's indexing, searching, explaining, context pack generation, code impact analysis, bounded change maps, and diagnostics check tools over stdin/stdout stdio protocol.

## Tools Exposed:
1. `index`: Runs index pipeline to parse and link code/docs.
2. `explain`: Resolves details, facts, relations, and diagnostics of an entity by stable key.
3. `search`: Performs Tantivy BM25 lexical search over indexed knowledge.
4. `context`: Builds a task-focused context pack from the latest snapshot.
5. `impact`: Calculates direct and transitive blast radius of changes.
6. `change_map`: Returns bounded evidence-backed change locations, relation chains, diagnostics, and test coverage from a task, target, or working-tree diff.
7. `rustok_architecture_context`: Resolves bounded RusTok module ownership, public contracts, interactions, tests, diagnostics, and evidence for one implementation intent.
8. `check`: Returns API or Docs diagnostic reports.
