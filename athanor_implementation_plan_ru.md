# Athanor: план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-23  
> Статус: `API-001`, `REL-001` verified; `DOCGEN-001 / Slices 0A–1C` execution-confirmed

## 1. Статусы и evidence

- `[x] implemented` — код и regressions присутствуют.
- `[-] in progress` — package Definition of Done закрыт частично.
- `[x] verified` — required matrix успешна на одном exact source commit.
- `[ ] planned` — следующий bounded этап.

Promotion требует успешных `athanor/verification-matrix`, `athanor/appsec` и
`athanor/store-conformance` на одном SHA.

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
  `30025931953`, Store `30025932704`.

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

#### Slice 1A — deterministic profile

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
- [x] Exact matrix on `042d02ac6b4c89d90a5b76c818098eb0c6b41920`:
  - Verification Matrix `30025932615`;
  - AppSec `30025931953`;
  - Store Conformance `30025932704`.

Existing coordinated `ath generate` is unchanged. Provider/LLM, daemon and MCP remain out of scope.
`DOCGEN-001` remains `[-] in progress` only until a bounded Rustok run records usefulness, omissions,
unsupported relations, repeatability, and review findings before profile expansion.

### 4.2 Product backlog

- [ ] module/API/operations/onboarding profiles;
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
| `DOCGEN-001` | P2 | `[-] in progress` | Slices 0A–1C confirmed; Rustok evaluation next |

## 6. Verification matrix

```bash
cargo-deny check
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
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

Выполнить bounded Rustok architecture-generation evaluation на одном exact committed snapshot и записать
полезные/отсутствующие sections, citation/diagram validity, omissions, unsupported relations,
repeatability и review/tuning decisions. Не подключать provider, daemon или MCP и не менять coordinated
`ath generate` без отдельного contract migration.
