---
id: doc://docs/architecture/adapters.md
kind: architecture
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000030
status: verified
---

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
| `athanor-store-surrealdb` | `KnowledgeStore`, `CanonicalSnapshotStore` | Standalone database store adapter backed by SurrealDB. |
| `athanor-search-tantivy` | `SearchIndex` | Index and search canonical entities using Tantivy. |
| `athanor-extractor-basic` | `Extractor` | Emit file entities and file discovery facts. |
| `athanor-extractor-markdown` | `Extractor` | Parse YAML documentation frontmatter and CommonMark/GFM headings, then emit documentation page/section, runbook, and operation-step knowledge. |
| `athanor-extractor-openapi` | `Extractor` | Parse OpenAPI 3.1 through `oas3`, retain a 3.0 fallback, and emit operation/schema/example knowledge. |
| `athanor-extractor-operations` | `Extractor` | Parse operations files such as dotenv, Cargo manifest, Makefile, Dockerfile, shell script, docker-compose, GitHub Actions, Kubernetes YAML, SQL migration, and runtime config sources into environment, package/dependency, command, deployment/service, database migration, and runtime configuration knowledge without storing raw secret values. |
| `athanor-extractor-rust` | `Extractor` | Emit Rust module, function, and symbol definitions. |
| `athanor-linker-api` | `Linker` | Link OpenAPI operations to Rust functions, Markdown documentation, component schemas, and examples. |
| `athanor-linker-markdown` | `Linker` | Link Markdown containment and exact frontmatter entity/concept references. |
| `athanor-linker-rust` | `Linker` | Link Rust module containment, imports, static function call graph, and test cases. |
| `athanor-checker-markdown` | `Checker` | Diagnose Markdown structure, unresolved frontmatter references, and duplicate document identities. |
| `athanor-checker-api` | `Checker` | Diagnose missing API implementation/documentation links, unresolved local schema references, invalid examples, undocumented environment variables, undocumented script commands, undocumented deployment resources, runbooks not tied to operational knowledge, and runbooks without operation steps. |
| `athanor-projector-wiki` | `Projector` | Project the latest canonical snapshot into a neutral Markdown wiki. |
| `athanor-projector-html` | `Projector` | Project the latest canonical snapshot into a self-contained HTML report. |

`athanor-projector-support` is a shared implementation library rather than an adapter. It owns the canonical projection payload shape, generated filename escaping, staged directory replacement, immutable directory publication, and portable pointer-file replacement used by filesystem projectors and coordinated generations.

## Built-In Registry

`athanor-app` owns adapter assembly through `AdapterRegistry` and `RuntimeBuilder`.

The registry keeps adapter order and construction in one app-layer place. CLI code should ask the runtime builder for an `IndexPipeline` instead of manually listing source providers, extractors, linkers, or checkers.

When a new adapter is added, update:

- this adapter map
- the adapter crate `README.md`
- the relevant `docs/adapters/*.md` page
- the built-in registry only if the adapter should run by default

The wiki and HTML projectors are invoked by app-layer services rather than the indexing `AdapterRegistry`. Projector registration and plugin manifests remain a later runtime extension.

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
  "schema": "athanor.adapter_manifest",
  "name": "example-plugin",
  "version": "0.1.0",
  "adapters": [
    {
      "id": "external.extractor.example",
      "kind": "extractor",
      "enabled": true,
      "command": {
        "program": "adapters/example-extractor",
        "args": ["--mode", "json"]
      },
      "supports_extensions": ["md", "txt"]
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
builtin.extractor.openapi
builtin.extractor.operations
builtin.extractor.rust
builtin.linker.api_knowledge
builtin.linker.markdown_containment
builtin.linker.rust
builtin.checker.markdown_structure
builtin.checker.api_consistency
builtin.checker.env_docs
builtin.checker.script_docs
builtin.checker.deployment_docs
builtin.checker.runbook_consistency
```

This is the first discovery layer. It gives the app layer a single current manifest contract and a validation path for adapter/plugin configuration. It does not dynamically load external Rust code yet; unknown adapter ids fail fast with a clear runtime-builder error. The optional `version` field describes the plugin package, not a separate generation of the adapter contract.

## External Process Adapters

Source, extractor, linker, and checker entries can be loaded from external commands when they provide a `command` field.

The current process adapter protocol is intentionally narrow:

- Athanor starts external extractors once per supported source file.
- Athanor writes `ExtractInput` JSON to extractor stdin.
- Extractor commands write `ExtractOutput` JSON to stdout.
- Athanor starts external linkers once per indexing run.
- Athanor writes `LinkInput` JSON to linker stdin.
- Linker commands write a JSON array of `Relation` objects to stdout.
- Athanor starts external checkers once per indexing run.
- Athanor writes `CheckInput` JSON to checker stdin.
- Checker commands write a JSON array of `Diagnostic` objects to stdout.
- Athanor starts external sources once per indexing run.
- Athanor writes a discovery request containing the absolute project `root` to source stdin.
- Source commands write a JSON array of `SourceFile` objects to stdout.
- stderr is used only for failure details.
- Athanor records external process stdout and stderr through tracing. stdout remains the process adapter protocol stream and is logged only at debug level; stderr is logged when present and is still included in process failure errors.
- `supports_extensions` scopes which source file extensions should be sent to extractor commands; it does not apply to source, linker, or checker commands.

Source discovery request:

```json
{
  "root": "/absolute/project/root"
}
```

Source discovery response:

```json
[
  {
    "path": "virtual/example.md",
    "language_hint": "markdown",
    "content_hash": "provider:stable-content-id",
    "content": "# Example"
  }
]
```

Source adapters should return stable, project-relative paths where possible. `content_hash` must change whenever content or extraction-relevant source metadata changes so incremental indexing can classify the file correctly. `content` may be `null` for binary or remotely referenced sources, but extractors that require text will then have no text to process.

External process adapters must emit normal canonical objects. The same pipeline validation applies: entities need ownership, and facts, relations, and diagnostics need evidence and ownership. Invalid output fails indexing through the existing adapter validation report path.

Relative command paths that include a path separator are resolved relative to the manifest file directory. Bare command names are resolved by the operating system `PATH`.
