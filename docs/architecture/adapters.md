# Adapter Architecture

Athanor uses adapter-first design.

Core crates define contracts and canonical domain types. Format-specific, backend-specific, transport-specific, and UI-specific behavior belongs in adapter crates.

## Current Core Boundary

```text
athanor-domain
  Entity / Fact / Relation / Evidence / Diagnostic / Snapshot / ContextPack / Concept

athanor-core
  KnowledgeStore / SourceProvider / Extractor / Linker / Checker / Projector
  SearchIndex / VectorIndex / EmbeddingProvider / AgentInterface / Transport
```

Core must not depend on:

```text
Markdown parser
OpenAPI parser
Rust parser
Postgres
SurrealDB
Tantivy
MCP
Rustok UI
HTML reports
```

## Adapter Rule

Before adding a feature, ask:

```text
Is this Athanor's domain meaning,
or is this a way to read, write, store, search, transport, or display knowledge?
```

If it is the second one, create or extend an adapter.

## Adapter Documentation Requirements

Every adapter should document:

- what it does
- which port it implements
- what it reads
- what it emits
- which entity/fact/relation/diagnostic kinds it uses
- whether it runs commands
- whether it uses the network
- limitations
- how to test it

## Canonical Output Requirements

Adapters that emit canonical objects must include enough metadata for verification and incremental merge.

Required:

- Entities must include `ownership`.
- Facts must include `evidence` and `ownership`.
- Relations must include `evidence` and `ownership`.
- Diagnostics must include `evidence` and `ownership`.

Ownership should list every source file that can invalidate the emitted object. For single-file extraction output, use the source file path. For cross-file linker or checker output, use the union of all contributing source file paths.

`IndexPipeline` validates newly emitted adapter output before storing canonical objects. Missing evidence or ownership is an adapter error. Validation errors are reported as an aggregated adapter validation report with the adapter name, object type, object id, and missing metadata field for every issue found in that adapter output.

## Existing Adapters

| Crate | Port | Purpose |
| --- | --- | --- |
| `athanor-source-fs` | `SourceProvider` | Discover local files. |
| `athanor-store-memory` | `KnowledgeStore` | In-memory canonical object store. |
| `athanor-store-jsonl` | `KnowledgeStore`, `CanonicalSnapshotStore` | Durable local JSONL canonical snapshot store. |
| `athanor-extractor-basic` | `Extractor` | Emit file entities and file discovery facts. |
| `athanor-extractor-markdown` | `Extractor` | Emit Markdown documentation page/section knowledge. |
| `athanor-linker-markdown` | `Linker` | Link Markdown file/page/section containment. |
| `athanor-checker-markdown` | `Checker` | Diagnose basic Markdown documentation structure. |

## Built-In Registry

`athanor-app` owns adapter assembly through `AdapterRegistry` and `RuntimeBuilder`.

The registry keeps adapter order and construction in one app-layer place. CLI code should ask the runtime builder for an `IndexPipeline` instead of manually listing source providers, extractors, linkers, or checkers.

When a new adapter is added, update:

- this adapter map
- the adapter crate `README.md`
- the relevant `docs/adapters/*.md` page
- the built-in registry only if the adapter should run by default

## Adapter Plugin Manifests

`RuntimeBuilder` discovers adapter plugin manifests before building the indexing pipeline.

Manifest locations:

```text
.athanor/adapters/*.json
.athanor/plugins/*/athanor-adapter.json
```

Manifest schema:

```json
{
  "schema": "athanor.adapter_plugin.v1",
  "name": "example-plugin",
  "version": "0.1.0",
  "adapters": [
    {
      "id": "builtin.extractor.file",
      "kind": "extractor",
      "enabled": true
    }
  ]
}
```

Supported adapter kinds:

```text
source
extractor
linker
checker
```

Current built-in adapter ids:

```text
builtin.source.local_filesystem
builtin.extractor.file
builtin.extractor.markdown
builtin.linker.markdown_containment
builtin.checker.markdown_structure
```

This is the first discovery layer. It gives the app layer a stable manifest contract and a validation path for adapter/plugin configuration. It does not dynamically load external Rust code yet; unknown adapter ids fail fast with a clear runtime-builder error.
