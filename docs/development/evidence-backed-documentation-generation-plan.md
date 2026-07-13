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

## Selected Reference Portfolio

The following material was re-reviewed for direct fit, scope, and licensing. Local clones are
inspection copies only; they are outside the Athanor workspace and must never become vendored
source or a runtime dependency without a separate adoption spike.

| Reference | Decision | Retain from it | Explicitly exclude |
| --- | --- | --- | --- |
| C4 model and Structurizr documentation | Required conceptual reference; no source adoption | One canonical model with independently selectable, hierarchical views; C4 context, container, component, and supporting dynamic/deployment view vocabulary | Structurizr DSL, Java model, layout engine, and workspace format as Athanor public contracts |
| `arc42/arc42-template` (`8dff0d9`) | Required architecture-profile checklist; reference-only | Coverage checklist: goals, constraints, context, strategy, building blocks, runtime, deployment, concepts, decisions, quality, risks, glossary | Copying chapter prose, diagrams, or the template structure verbatim; it is CC BY-SA 4.0, so Athanor profiles must be independently authored |
| `evildmp/diataxis-documentation-framework` (`855e9c1`) | Required content-classification reference; reference-only | Separate generated content into tutorial, how-to, reference, and explanation; in particular, preserve neutral graph-backed reference versus explicitly interpretive explanation | Reusing its text or template directly; it is CC BY-SA 4.0, and Diataxis is not a renderer or an indexing model |
| `adr/madr` (`835fc94`) | Conditional, later ADR/documentation-extractor reference | A small, reviewable Markdown decision record with status, context, options, and outcome; the template is MIT or CC0 | Generating architectural decisions from code or treating an ADR as evidence of current implementation without linking/checking it |
| `mermaid-js/mermaid` | Conditional Slice 3 rendering/validation candidate | Text diagram format compatible with Markdown, version-controlled source, and syntax validation through a replaceable rendering adapter | A JavaScript dependency in domain/core, direct model calls during indexing, or making Mermaid the only output diagram format |
| `sourcegraph/scip` (`e01e97e`) | Retain as a Phase 12 external-index importer reference, not documentation generation work | Language-neutral definitions, references, implementations, and Rust bindings that can enrich canonical code relations through an adapter | Replacing Athanor stable IDs, evidence, ownership, or snapshots with SCIP documents; building an importer before its language-specific fixture contract |
| Backstage TechDocs | Reference-only, do not clone or adopt | Entity-associated docs-as-code publication and CI-built static documentation separation | Backstage portal, catalog, Node/Python runtime, or storage model; its scope is a developer portal rather than Athanor projection |
| OpenTelemetry semantic conventions and current LLM documentation papers | Evaluation and naming references only | Versioned terminology, requirement levels, and quality-evaluation questions | Importing telemetry schemas or experimental research workflows as Athanor architecture |

The local inspection set is deliberately small:

```text
D:\deepwiki-rs                         # product/UX reference only
D:\athanor-references\arc42-template   # architecture coverage checklist
D:\athanor-references\diataxis-framework # document-type taxonomy
D:\athanor-references\madr             # ADR format reference
D:\athanor-references\scip             # future external-index protocol reference
```

No further repositories should be cloned for this plan until a delivery slice names a concrete
replacement boundary and acceptance corpus. In particular, a full Backstage clone, a Structurizr
implementation clone, an OpenTelemetry clone, or another AI wiki generator would add maintenance
surface without improving the first deterministic architecture-document slice.

## Reference Intake Protocol

References are mined for a bounded idea, not adopted as packages by default. Every candidate passes
the following sequence before it can change Athanor's roadmap or code.

1. **Name the Athanor gap.** State the user-facing capability that is missing and identify the
   existing snapshot, context, projector, extractor, checker, or adapter boundary it would touch.
   A reference does not create a new product goal by itself.
2. **Extract one transferable pattern.** Record the exact input contract, output contract,
   validation rule, failure mode, or evaluation method worth retaining. Do not record vague ideas
   such as "use agents" or "support many languages".
3. **Apply the rejection filter.** Reject the candidate when it duplicates an existing Athanor
   capability, bypasses canonical evidence/ownership, needs unbounded raw-file access, makes an
   external format the source of truth, or expands a delivery slice without a Rustok scenario.
4. **Check provenance.** Pin the source URL and revision, license, maintenance signal, dependency
   footprint, data/secret exposure, and whether the material is reference-only or could legally be
   reused. CC BY-SA material is treated as a conceptual reference unless its attribution and
   share-alike consequences are explicitly accepted.
5. **Create the smallest independent spike.** The spike stays behind an adapter or private app
   boundary, uses a minimal fixture corpus, and produces Athanor-owned canonical or projection
   contracts. It must be removable without a domain/core migration.
6. **Measure against a real task.** Compare the baseline and spike on Rustok or another bounded
   repository scenario: evidence/citation validity, signal quality, omitted knowledge, deterministic
   output, runtime, resource use, review burden, and provider cost when applicable.
7. **Make one explicit decision.** Classify the result as `adopt`, `defer`, or `reject`. Adoption
   requires a scoped roadmap slice, acceptance criteria, dependency decision, tests, and—when it
   changes material architecture—an ADR. Deferred and rejected candidates retain a concise reason
   so they are not rediscovered and re-evaluated without new evidence.

The intake record for a useful reference therefore contains only:

```text
source + pinned revision + license
specific Athanor gap
one transferable pattern
adapter/app boundary and non-goals
minimal fixture and measurable acceptance criteria
decision with evidence
```

For documentation generation, the first intake exercise is intentionally limited to this question:
"Can a deterministic architecture profile turn the current canonical snapshot into a cited,
readable C4-style overview without an LLM?" C4 supplies the view vocabulary, arc42 supplies the
coverage checklist, and Diataxis separates graph-backed reference from explanatory prose. The
answer must be established by the Slice 1 fixture and Rustok evaluation before looking for more
generators or model orchestration frameworks.

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
