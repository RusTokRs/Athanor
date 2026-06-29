# Athanor RusTok FBA Adapter

`athanor-adapter-rustok-fba` is an opt-in Athanor adapter for auditing RusTok Fluid Backend Architecture (FBA) contracts.

It provides three built-in adapter ids:

- `builtin.extractor.rustok_fba`
- `builtin.linker.rustok_fba`
- `builtin.checker.rustok_fba`

The adapter is separate from the FFA adapter. FBA reads registry JSON and code markers from Rust
port implementations. It also compares registry metadata with module implementation plans and the
central FFA/FBA readiness board. Documentation is secondary drift evidence only and does not define
readiness or increase completion percentages. Port code may live in `src/ports.rs`,
`src/ports/mod.rs`, or nested `src/ports/**/*.rs` files; all port-code files for one module are
merged before diagnostics run.

## Model

Canonical entities:

- `fba_module://<module>`
- `fba_contract://<module>/<contract_version>`
- `fba_port://<module>/<port_name>`
- `fba_operation://<module>/<port_name>/<operation>`
- `fba_profile://<module>/<profile>`
- `fba_dependency://<consumer>/<provider>/<profile>`

The linker connects modules, contracts, ports, operations, profiles, dependency declarations, and evidence files with bounded canonical relations. Consumer registries also link the consumer module and contract to each `fba_dependency://...` node before the dependency points at the provider module, so generic graph path queries can traverse consumer-to-provider dependencies. Provider-side `consumers` declarations preserve hyphenated module slugs such as `ai-media` when creating dependency keys. Registry facts use the first registry identity line. Registry-derived module, contract, port, operation, profile, and consumer dependency entities use the line of their own declaration, while external placeholder entities keep no source location. Registry-derived relations use the target entity source line as evidence where possible, so `ath explain` and graph output navigate to concrete declarations rather than the registry header.

## Diagnostics

The checker emits FBA-only diagnostics with ids prefixed by `rustok_fba_`, including missing registries, missing port traits or operations, missing shared context/error contracts, missing call policy enforcement, missing write idempotency contract assertions, missing contract tests, missing registry evidence, and unresolved consumer/provider declarations. Consumer/provider cross-checks also compare declared contract versions, required ports, required provider profiles, provider-side consumer profiles, fallback profiles, and degraded modes across the two registries.

`rustok_fba_docs_drift` reports status, contract-version, verifier, and evidence-reference drift
between an FBA registry, its local implementation plan, and `docs/modules/registry.md`. These
diagnostics carry registry and documentation evidence paths so bounded violation graphs point at the
files that need synchronization.

The audit summary reports registry-backed and dependency-only modules separately, together with
in-progress and unknown-status counts. Contract consistency is not presented as migration
completion.

Registry-backed modules expose an evidence-derived contract numerator, denominator, and integer
percentage across applicable port, operation, context/error, policy, evidence, contract-test, and
dependency requirements. Dependency-only nodes are not scored, and migration status remains a
separate signal.
