---
id: doc://docs/development/evidence-backed-documentation-generation-plan.md
kind: developer_guide
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000001
status: verified
---
# Evidence-Backed Documentation Generation Plan

## Status

Planned. This is a documentation-generation layer above Athanor's canonical snapshot, not a
replacement indexing pipeline and not a new source of canonical truth.

## Product Direction

Athanor should turn verified repository knowledge into useful technical documentation: architecture
overviews, C4-style diagrams, module and API guides, onboarding material, operational guides, and
reviewable updates to editable documentation. The output must be materially more useful than a raw
entity dump while retaining the trust properties of the canonical graph.

The central rule is:

```text
source files -> adapters -> canonical snapshot -> bounded documentation context -> documents
```

An LLM, when enabled, composes prose from a bounded documentation context. It does not establish
facts, crawl arbitrary repository files, or write editable documentation directly.

## Reference Assessment: Litho (`sopaco/deepwiki-rs`)

The project was reviewed locally at commit `6bd83af`. It is a useful reference for document
profiles, composition roles, external-knowledge categorization, and Mermaid validation. It is not
an Athanor dependency or source-analysis backend.

Its twelve language processors (Rust, JavaScript, TypeScript, PHP, React, Vue, Svelte, Kotlin,
Python, Java, C#, and Swift) use project-owned regular-expression heuristics for imports,
interfaces, and component classification. Its dependency manifest does not include a general AST
or language-server parsing foundation such as Tree-sitter, `syn` outside Rust, SWC/OXC, or ANTLR.
Its LLM workflow can directly explore and read source files through ReAct tools. Those choices are
appropriate for a standalone AI wiki generator, but would bypass Athanor's adapters, evidence,
ownership, stable keys, incremental invalidation, and bounded agent-facing query contract.

Decision:

- retain Litho as a product and UX reference only;
- do not vendor or depend on `deepwiki-rs`;
- do not reuse its regex language processors;
- evaluate individual rendering or validation libraries independently under the library-adoption
  workflow before any implementation.

Reference: <https://github.com/sopaco/deepwiki-rs>.

## Non-Negotiable Boundaries

- The canonical snapshot remains the only truth source for generated claims.
- `athanor-domain` and `athanor-core` remain free of LLM-provider, template, Mermaid, and
  presentation-specific types.
- A generation request consumes an explicit snapshot id and bounded input limits; it never reads
  generated JSONL, wiki, or HTML as source material.
- Every generated factual claim, diagram node, diagram edge, and code reference must carry one or
  more canonical entity stable keys and evidence links, or be marked as an explicitly inferred
  explanation with confidence.
- Generation is snapshot-versioned and published atomically alongside other generated read models.
- Editable files are never overwritten by a model. The only writable editable path is an explicit,
  reviewable patch proposal followed by `ath docs apply-patch`.
- Provider access, credentials, network use, and external knowledge ingestion are opt-in adapter
  configuration. Offline deterministic generation must remain useful without them.

## Target Document Profiles

The implementation should ship profiles incrementally. A profile defines the selection rules,
section schema, citation requirements, diagram policy, validation rules, and output paths.

| Profile | Purpose | Initial input |
| --- | --- | --- |
| `architecture` | System overview, boundaries, major components, dependency/data-flow diagrams | repository overview, graph hubs, selected relations, diagnostics |
| `module` | Responsibility, public interfaces, dependencies, ownership, tests, risks | one entity explanation plus bounded neighbourhood |
| `api-guide` | Contract intent, endpoints/schemas/examples, implementation and documentation gaps | canonical OpenAPI/GraphQL graph |
| `operations` | Runtime configuration, deployment, scripts, runbooks, open gaps | operations entities and diagnostics |
| `onboarding` | A navigable starting path for contributors | architecture and selected module contexts |

`architecture` is the first implementation slice. It must be valuable for the real Rustok
repository before additional profiles expand scope.

## Proposed Adapter Architecture

```text
CanonicalSnapshotReader
  -> DocumentationPlanner             (Athanor-owned, deterministic)
  -> DocumentationContextBuilder      (Athanor-owned, bounded and evidence-linked)
  -> DocumentationComposer port
       -> DeterministicComposer        (built in first)
       -> LlmDocumentationComposer     (optional provider adapter)
  -> DocumentationValidator            (Athanor-owned policy + renderer adapters)
  -> DocumentationProjector            (Markdown/HTML output adapter)
  -> immutable generated generation
```

### Planner and Context Builder

The planner turns a profile request and snapshot into a deterministic document outline. The context
builder resolves only the entity, fact, relation, diagnostic, ownership, and evidence slices named
by that outline. It records effective limits, omitted counts, source snapshot id, and selection
reasons, following the same agent-facing principles as `ath context` and `ath change-map`.

The context format must be an Athanor-owned, versioned JSON schema. It should carry canonical ids,
stable keys, source anchors, evidence locations, confidence, and relation direction. It must not
expose internal provider prompts as the public contract.

### Composer Port

The initial deterministic composer produces a cited Markdown skeleton and diagrams from canonical
relations. It establishes stable layouts, paths, manifests, failure behavior, and test fixtures
without a model dependency.

The optional LLM composer receives only the structured context and a profile-specific instruction.
It returns a structured document draft: sections, claims, citations by stable key, diagram source,
and stated uncertainties. Provider and model identifiers, prompt versions, token use, cache keys,
and failures belong in generation metadata, not canonical facts.

### Validation

Validation runs before publication and rejects or annotates output that has:

- citations to missing or out-of-scope canonical objects;
- factual claims without citation or an allowed inference marker;
- diagram nodes or edges that cannot be mapped to canonical objects/relations;
- invalid Mermaid or unsupported diagram syntax;
- broken relative links, duplicate stable anchors, or output outside the selected generation;
- context-limit omissions that are hidden from the reader.

The validator reports evidence-backed generation diagnostics. A failed profile must not switch the
current generated pointer.

## Delivery Slices

### Slice 0: Contracts, Corpus, and Evaluation Policy

- Define versioned request, outline, context, draft, citation, validation-report, and manifest
  schemas in the app/projector boundary.
- Create small fixture repositories and a Rustok evaluation corpus with expected outline sections,
  evidence links, diagram edges, and known documentation gaps.
- Define quality measurements: citation coverage, citation validity, diagram validity, unsupported
  relation disclosure, deterministic repeatability, prompt/token cost, and human-review score.
- Record data-handling rules: no secrets in context, explicit provider enablement, and no raw
  file-explorer tools for model composition.

### Slice 1: Deterministic Architecture Profile

- Add an opt-in architecture-document projector built from the latest canonical snapshot.
- Generate a project overview, component inventory, relation-backed Mermaid diagrams, diagnostic
  summary, and evidence footnotes without LLM use.
- Publish through the existing immutable generation/pointer mechanics.
- Add bounded inspection commands for document manifests and validation results.
- Run against Rustok and tune section selection, diagram density, and omitted-count reporting.

### Slice 2: Optional LLM Composition Adapter

- Introduce a provider-neutral private composer port and one opt-in adapter behind explicit
  configuration.
- Require structured drafts and stable-key citations; cache by snapshot, profile, context schema,
  prompt version, and provider/model identity.
- Preserve the deterministic document when the provider is disabled or fails.
- Add provider contract tests with recorded responses; no live credential is required in CI.

### Slice 3: Citation, Diagram, and Quality Gates

- Validate every composed output and add clear generation diagnostics for unsupported or weak
  claims.
- Add Mermaid renderer/syntax validation behind a replaceable adapter boundary.
- Add coverage reports that distinguish graph-backed sections, LLM inferences, and unavailable
  source knowledge.

### Slice 4: Reviewable Guides and Editable-Documentation Proposals

- Add `module`, `api-guide`, `operations`, and `onboarding` profiles one at a time.
- Convert reviewed drafts into the existing explicit documentation patch workflow; retain
  human-authored text outside Athanor-managed blocks.
- Extend drift checks so a guide identifies the snapshot and evidence it was generated from.

### Slice 5: Incremental and Ecosystem Work

- Recompose only profiles affected by changed canonical objects while retaining a complete,
  immutable generation publication.
- Evaluate external knowledge sources as explicit source/extractor adapters, with provenance and
  licensing captured before they enter a documentation context.
- Consider semantic retrieval only after the Phase 9 benchmark and offline requirements are met.

## Dependency Decisions

No new dependency is approved by this plan. Candidates for Markdown templating, Mermaid validation,
token counting, model-provider SDKs, and C4 rendering must each pass the documented adoption
workflow: license, maintenance, MSRV, adapter boundary, fixture spike, output comparison, and
replacement path. A CLI from another project is never a substitute for one of these contracts.

## Acceptance Criteria

- `architecture` documents can be generated from an existing snapshot with no model or network.
- Every published document declares its source snapshot, profile, schema version, limits, and
  omitted counts.
- A reader can trace every substantive generated claim and diagram relationship to stable keys and
  evidence locations.
- Model output cannot silently introduce canonical facts or overwrite editable documentation.
- Invalid diagrams, citations, and links fail or visibly annotate the profile before publication.
- Bounded CLI, daemon, and future MCP interfaces expose relevant document/context slices without
  requiring agents to read generated artifacts in full.
- The first Rustok run demonstrates useful architecture documentation with measured signal quality
  before further profiles are implemented.

## Verification Expectations

Each implementation slice must add focused fixture/contract tests. Code changes require the
workspace formatting, tests, and Clippy checks; generation/runtime changes also require `ath index`
and a bounded generation probe. Documentation-only plan updates require a documentation check and
must not claim implementation completion.
