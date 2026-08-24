# Athanor: план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-08-24  
> Статус: `API-001`, `REL-001` verified; `DOCGEN-001 / Slices 0A–1C` execution-confirmed;
> relation-disclosure exact-evaluation-confirmed; module profile Slices 2A–2B implemented in source,
> focused execution evidence pending

## 1. Статусы и evidence

- `[x] implemented` — код, документация и source regressions присутствуют.
- `[-] in progress` — package Definition of Done закрыт частично.
- `[x] verified` — required matrix успешна на одном exact source commit.
- `[ ] planned` — следующий bounded этап.

Promotion требует успешных `athanor/verification-matrix`, `athanor/appsec` и
`athanor/store-conformance` на одном SHA, когда такой matrix входит в Definition of Done конкретного
пакета. Наличие кода или metadata не является execution evidence.

## 2. Baselines

Release baseline `609027eb02caa05346ebfea8538552c42b588c31`:
CI `29995959544`, AppSec `29995960063`, Store `29995959512`, Release `29996579628`, clean-install
smoke `29998347890`. Annotated `v0.2.1` указывает на этот SHA.

Documentation generation:

- Slices 0A–0B: `2a049303e797f00ac53f1e91fc010f284993926d`, CI `30005828864`, AppSec
  `30005828850`, Store `30005828956`;
- Slices 1A–1B: `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`, CI `30013208011`, AppSec
  `30013208197`, Store `30013208312`;
- Slice 1C1: `4f567271ed6d38d30b3c15dc6999aa33152a9312`, CI `30015689753`, AppSec
  `30015691399`, Store `30015689363`;
- Slice 1C2: `042d02ac6b4c89d90a5b76c818098eb0c6b41920`, CI `30025932615`, AppSec
  `30025931953`, Store `30025932704`;
- repaired bounded Rustok evaluation: `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e`, evaluation
  `31625608720`, probe `31625608721`, CI `31625608729`, AppSec `31625608723`, Store `31625608739`;
- relation-disclosure tuning: `6862aee81dd0f53fa8372d1ce3fcb6e2ed198cca`, evaluation
  `32712992516`, probe `32712992421`; artifact confirms `unsupported_relation_disclosed = true`;
- Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; post-merge contexts green:
  Rustok evaluation `32714912841`, Rustok probe `32714912826`, AppSec `32714912809`, Store
  Conformance `32714912932`. Focused module-profile format/test/Clippy evidence не заявляется.

## 3. Завершённые пакеты

- `COMP-003` / `COMP-003C2B2C2B`: explicit runtime composition и bounded owners.
- `MCP-007`: cancellation-safe transactional Index publication.
- `JSON-003`: recursive, disjoint, fixture-backed contract lifecycle.
- `DOC-001` / `DOC-002`: status hygiene, current owners и line budgets.
- `MCP-004`: control-plane responsiveness under saturation.
- `VERIFY-001`: exact cross-platform CI/AppSec/Store evidence.
- `API-001`: verified GraphQL/OpenAPI request/response/status/security consistency.
- `REL-001`: verified immutable `v0.2.1` release and clean Linux/Windows installs.

## 4. Активная разработка

### 4.1 `DOCGEN-001` — evidence-backed documentation generation

#### Slices 0A–0B — contracts

- [x] Strict request/manifest, hard limits, omissions, paths и checksums.
- [x] Versioned outline/context/citation/draft/validation contracts.
- [x] Data policy, quality metrics, fixture repository и Rustok evaluation corpus.
- [x] Full chain alignment и fail-closed regressions.

#### Slice 1A — deterministic architecture profile

- [x] Exact `CanonicalSnapshot`, deterministic sorting и hard limits.
- [x] Overview/Components/Relationships/Diagnostics.
- [x] Stable-key/evidence citations, Mermaid edges, footnotes и SHA-256.

#### Slice 1B — immutable publication

- [x] `.athanor/generated/documentation/generations/<8-digit-id>`.
- [x] Manifest, Markdown, validation report и atomic `current.json`.
- [x] Exact `UpToDate`, force, immutable history, tamper recovery и cancellation.
- [x] Current/report generated boundaries registered and fixture protected.

#### Slice 1C1 — exact committed-snapshot operation

- [x] `RuntimeComposition::init_store` and exact `CanonicalSnapshotStore::load_snapshot`.
- [x] Missing/uncommitted and identity mismatch fail closed.
- [x] Cancellation checks around Store and publication boundaries.
- [x] Real committed JSONL lifecycle tests.
- [x] Exact matrix on `4f567271ed6d38d30b3c15dc6999aa33152a9312`.

#### Slice 1C2 — CLI generation and inspection

- [x] `ath docs generate-architecture <PATH> --snapshot <EXACT-ID>`.
- [x] `--force`, hard-limit flags, text and JSON output.
- [x] Ctrl-C cancellation drains the operation before exit.
- [x] `ath docs architecture current|manifest|validation`.
- [x] Inspection validates path confinement, identity, exact artifact layout and checksums.
- [x] Executable test covers index → generate → inspect → `up_to_date` and missing snapshot.
- [x] Exact matrix on `042d02ac6b4c89d90a5b76c818098eb0c6b41920`.

#### Bounded Rustok evaluation — repaired gate и artifact review closed

- [x] Первый Rustok gate выявил aggregate citation overflow; общий reference budget исправлен на 256.
- [x] Exact source `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e` полностью green по evaluation,
  probe, Verification Matrix, AppSec и Store Conformance.
- [x] Artifact review выявил undisclosed `unsupported_relations = 6962`.
- [x] `6862aee81dd0f53fa8372d1ce3fcb6e2ed198cca` добавил explicit disclosure и fail-closed guard.
- [x] Exact evaluation `32712992516` и probe `32712992421` успешны;
  `unsupported_relation_disclosed = true`, deterministic repeatability unchanged.

#### Slice 2A — deterministic module inventory profile

- [x] `DocumentationProfile::Module` добавлен backward-compatible в существующий v1 contract chain.
- [x] Pure `build_documentation_module_profile` требует exact snapshot и profile `module`.
- [x] Source-backed `EntityKind::Module` сортируются по stable key + entity id.
- [x] `max_entities` и общий `DOCUMENTATION_REFERENCE_LIMIT = 256` ограничивают context/citations.
- [x] `Module Overview` + `Modules`, evidence footnotes, `modules/index.md`, SHA-256.
- [x] Focused regressions: identity, ordering, input-order invariance, omissions, checksum, fail-closed.
- [x] Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; Rustok/AppSec/Store post-merge
  contexts green, но focused format/test/Clippy execution evidence остаётся отдельно.

#### Slice 2B — module-scoped evidence enrichment

- [x] Module facts включаются только когда subject или object — выбранный canonical module.
- [x] Module relations включаются только когда source или target — выбранный canonical module.
- [x] Diagnostics включаются только в status `open` и только если ссылаются на выбранный module.
- [x] Для facts/relations/diagnostics обязательны resolved canonical stable keys и evidence/ownership
  locations с portable slash paths, line defaults, deduplication и deterministic ordering semantics,
  совместимыми с architecture profile.
- [x] Per-kind limits применяются до общего deterministic round-robin budget в 256 context items.
- [x] `Modules` содержит module entities + facts; `Module Relationships` содержит cited claims и Mermaid
  edges; `Module Diagnostics` содержит cited open diagnostics.
- [x] Markdown раскрывает omitted module/fact/relation/diagnostic counts и unsupported module relations;
  validation metric `unsupported_relations` связан с bounded module relation omissions.
- [x] Unrelated canonical relation из общей fixture не попадает в module profile.
- [x] Regression фиксирует exact scope, relation endpoints, ordering invariance и semantic parity с
  architecture profile для stable keys/evidence соответствующих entity/fact/relation/diagnostic items.
- [x] Publication, Store loading, CLI, daemon, MCP, provider и coordinated `ath generate` не затронуты.
- [ ] Focused execution evidence для Slice 2B pending; source implementation не считается verified.

Existing coordinated `ath generate` is unchanged. Provider/LLM, daemon and MCP remain out of scope.
`DOCGEN-001` остаётся `[-] in progress`: enriched pure module profile source-implemented, production
publication/operation surface ещё закрыта.

### 4.2 Product backlog

- [ ] Slice 2C: immutable module publication + exact Store operation + CLI/validated inspection;
- [ ] API/operations/onboarding profiles;
- [ ] broader framework adapters and completeness reports;
- [ ] optional provider, daemon, MCP, i18n and semantic retrieval after deterministic quality gates.

## 5. Программа работ

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[x] verified` | Audit run `29836572040` |
| `COMP-003` | P2 | `[x] implemented` | Explicit composition |
| `MCP-007` | P1 | `[x] implemented` | Transactional cancellation |
| `JSON-003` | P1 | `[x] implemented` | Checked schema lifecycle |
| `DOC-001` / `DOC-002` | P3 | `[x] implemented` | Status hygiene |
| `MCP-004` | P1 | `[x] implemented` | Responsive control plane |
| `VERIFY-001` | P1 | `[x] verified` | Full release baseline matrix |
| `API-001` | P1 | `[x] verified` | Cross-protocol consistency |
| `REL-001` | P1 | `[x] verified` | `v0.2.1` published and installed |
| `DOCGEN-001` | P2 | `[-] in progress` | Module Slices 2A–2B source-implemented; Slice 2C pending |

## 6. Verification matrix

```bash
cargo-deny check
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_module_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked
cargo test -p athanor-app --test documentation_architecture_inspection_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test release_readiness_inventory --locked
cargo test -p athanor-app --test verification_evidence_inventory --locked
cargo test -p ath --test documentation_architecture_cli --locked
cargo test -p athanor-transport-mcp --test control_plane_saturation_inventory --locked
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 7. Следующий шаг

Получить focused format/test/Clippy execution evidence для enriched Slice 2B. После этого отдельным
Slice 2C добавить immutable module publication, exact committed-snapshot Store loading и module
CLI/validated inspection. Не подключать provider, daemon или MCP и не менять coordinated `ath generate`
без отдельного contract migration.
