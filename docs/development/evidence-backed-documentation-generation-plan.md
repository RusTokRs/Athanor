---
id: doc://docs/development/evidence-backed-documentation-generation-plan.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Evidence-Backed Documentation Generation Plan

## Status

In progress. Slices 0A and 0B are implemented as application-owned contracts, fixtures, evaluation
corpus, and fail-closed regressions. No runtime documentation generator, projector wiring, CLI
surface, provider access, or new dependency exists yet. Slice 1 is the next bounded implementation.

This layer sits above the canonical snapshot and never replaces indexing or becomes a second source
of truth:

```text
source files -> adapters -> canonical snapshot -> bounded documentation context -> documents
```

## Product Goal

Athanor should generate useful, reviewable architecture documentation from verified repository
knowledge. The first profile is `architecture`: overview, components, canonical relationships,
diagnostics, diagrams, omissions, and evidence references. Module, API, operations, and onboarding
profiles remain later work.

Offline deterministic generation must remain useful. An optional future model may compose prose only
from the bounded context; it may not establish facts, explore arbitrary source files, or directly
rewrite editable documentation.

## Non-Negotiable Boundaries

- The canonical snapshot is the only source for generated factual claims.
- `athanor-domain` and `athanor-core` remain free of template, renderer, Mermaid, and provider types.
- Every request names an exact snapshot, supported profile, and hard input limits.
- Every factual claim and diagram edge is traceable to in-scope stable keys and evidence locations,
  unless the claim is explicitly marked as an inference with confidence and rationale.
- Context records effective limits and omitted counts instead of hiding truncation.
- Secrets and raw-file explorer access are forbidden; network use requires explicit provider opt-in.
- Publication is immutable and atomic. Failed validation must not advance the current pointer.
- Editable documentation changes remain explicit reviewable patch proposals.

## Reference Decisions

References are used for bounded patterns, not adopted as source or dependencies by default.

| Reference | Decision | Retained idea | Excluded |
| --- | --- | --- | --- |
| C4 / Structurizr docs | conceptual reference | hierarchical system, container, component, dynamic, and deployment views | Structurizr DSL, Java model, workspace format, layout engine |
| `arc42/arc42-template` | checklist reference | goals, constraints, context, strategy, building blocks, runtime, deployment, risks, glossary | copied template prose or structure; CC BY-SA content |
| Diataxis | classification reference | separate reference, explanation, how-to, and tutorial intent | copied text or treating it as a renderer |
| MADR | conditional later reference | small reviewable decision records | inferring decisions from code without evidence |
| Mermaid | conditional Slice 3 adapter candidate | version-controlled diagram source and syntax validation | dependency in domain/core or only supported diagram format |
| SCIP | future external-index adapter reference | definitions, references, and implementations as optional imported evidence | replacing Athanor stable IDs, evidence, or snapshots |
| Backstage TechDocs | product reference only | docs-as-code publication separation | portal/catalog/runtime adoption |
| Litho / `deepwiki-rs` | UX reference only | document profiles and composition roles | dependency, regex processors, or arbitrary ReAct file exploration |

No reference becomes a dependency without a separate adoption record covering license, maintenance,
MSRV, security, adapter boundary, fixture comparison, and replacement path.

## Implemented Contracts

### Slice 0A — Request And Manifest

- `athanor.documentation_generation_request.v1` binds an exact snapshot, the `architecture` profile,
  and hard entity/fact/relation/diagnostic limits.
- `athanor.documentation_generation_manifest.v1` records request schema, generation and snapshot,
  effective limits, omitted counts, status, and checksum-bound output documents.
- Unknown fields, schema drift, empty or untrimmed identities, zero/unbounded limits, unsafe portable
  paths, duplicate IDs/paths, invalid lowercase SHA-256, and request/manifest mismatch fail closed.
- Both contracts are stable public application contracts and are fixture protected.

### Slice 0B — Intermediate Evidence Flow

The following versioned Rust-owned contracts are implemented without a runtime or persisted boundary:

- `athanor.documentation_outline.v1` — ordered section identities, kinds, and selection reasons;
- `athanor.documentation_context.v1` — bounded entity, fact, relation, and diagnostic items with
  stable keys, evidence locations, direction, effective limits, omissions, and data policy;
- `athanor.documentation_citation.v1` — snapshot-bound stable keys and evidence ranges;
- `athanor.documentation_draft.v1` — outline-preserving sections, cited claims, explicit inferences,
  and relation-backed diagram edges;
- `athanor.documentation_validation_report.v1` — diagnostics, status, policy identity, citation and
  diagram validity, repeatability, unsupported relations, provider token/cost fields, and optional
  human-review score.

Fail-closed rules include:

- unique canonical section, item, claim, citation, and output identities;
- portable normalized evidence paths and valid positive line ranges;
- relation endpoints included in the bounded context;
- citations unable to escape context stable keys or evidence;
- factual claims requiring citations unless explicitly inferred;
- diagram edges requiring in-context endpoints and citations;
- raw-file and secret policy rejection;
- provider metrics rejected when provider access is disabled;
- `valid` reports requiring no error diagnostics and fully valid citations and diagrams.

The fixture corpus contains a minimal Rust service and a pinned Rustok architecture evaluation case.
It records expected sections, citation paths, diagram edges, known gaps, citation coverage/validity,
diagram validity, unsupported-relation disclosure, deterministic repeatability, provider cost, and
human-review score.

Intermediate 0B contracts are classified by explicit source-level Rust owners. They are deliberately
not registered as persisted/generated/public transport documents until Slice 1 creates a real runtime
boundary.

## Target Architecture

```text
CanonicalSnapshotReader
  -> DocumentationPlanner
  -> DocumentationContextBuilder
  -> DeterministicComposer
  -> DocumentationValidator
  -> DocumentationProjector
  -> immutable generated generation
```

A later optional provider-neutral composer may consume only `DocumentationContext` and return a
structured `DocumentationDraft`. Provider identity, prompt version, token use, cache keys, cost, and
failures belong in generation metadata, not canonical facts.

## Delivery Slices

### Slice 0A — Implemented

Request and publication manifest contracts, fixture, portable path/checksum policy, and public JSON
contract registration.

### Slice 0B — Implemented

Outline, context, citation, draft, validation-report, data-handling and quality policy contracts;
minimal fixture repository; Rustok evaluation corpus; negative regressions and source ownership.

### Slice 1 — Planned Next

- Build a deterministic `architecture` planner and bounded context builder from one canonical
  snapshot.
- Compose cited Markdown and relation-backed diagram source without a model or network.
- Validate claims, citations, diagram edges, paths, omissions, and manifest checksums before publish.
- Integrate the output with the existing immutable generation staging and current-pointer mechanics.
- Preserve the previous current generation on cancellation or validation failure.
- Add bounded inspection of document manifests and validation reports.
- Evaluate the result against the minimal fixture and pinned Rustok corpus before adding profiles.

### Later Slices

- Slice 2: optional provider-neutral structured composer with recorded-response contract tests.
- Slice 3: replaceable diagram syntax/render validation and quality gates.
- Slice 4: module, API, operations, onboarding, and reviewable editable-document proposals.
- Slice 5: incremental recomposition and explicit external-knowledge adapters.

## Acceptance Criteria

- Architecture documents are generated from an existing snapshot with no model or network.
- Every document declares snapshot, profile, contract versions, effective limits, omissions, and
  checksum identity.
- Every substantive factual claim and diagram relation is traceable to stable keys and evidence.
- Invalid citations, diagrams, paths, links, or hidden omissions block publication or appear as an
  explicit invalid report.
- Model output cannot introduce canonical facts or overwrite editable documentation.
- A Rustok evaluation demonstrates useful signal and reviewable density before profile expansion.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
```

Runtime changes in Slice 1 additionally require the workspace matrix, `ath index`, deterministic
generation probe, cancellation/publication regressions, and one exact-commit CI/AppSec/Store evidence
set before the package may be called verified.
