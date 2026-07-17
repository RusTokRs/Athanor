---
id: doc://docs/adapters/rustok-fba.md
kind: adapter
language: en
source_language: en
status: draft
---
# RusTok FBA Adapter

`athanor-adapter-rustok-fba` is an opt-in code-audit adapter for RusTok Fluid Backend Architecture (FBA).

It is intentionally separate from FFA support. FFA and FBA use different adapter ids, canonical entity namespaces, diagnostics, and graph command namespaces.

## Built-In Ids

```text
builtin.extractor.rustok_fba
builtin.linker.rustok_fba
builtin.checker.rustok_fba
```

These ids are available to adapter plugin manifests, but they are not registered in the default Athanor pipeline.

Example RusTok manifest:

```json
{
  "schema": "athanor.adapter_manifest.v1",
  "name": "rustok-fba",
  "version": "0.1.0",
  "adapters": [
    { "id": "builtin.extractor.rustok_fba", "kind": "extractor", "enabled": true },
    { "id": "builtin.linker.rustok_fba", "kind": "linker", "enabled": true },
    { "id": "builtin.checker.rustok_fba", "kind": "checker", "enabled": true }
  ]
}
```

Place it at `.athanor/adapters/rustok-fba.json` in the RusTok repository, then run:

```bash
ath index .
ath rustok fba audit . --json
ath graph fba module commerce --json
ath graph fba port inventory InventoryReservationPort --json
ath graph fba dependencies --module commerce --json
ath graph fba violations --module inventory --json
ath check rustok-fba --json
```

## Extraction Model

The extractor reads machine contracts and code markers:

- `crates/rustok-*/contracts/*-fba-registry.json`
- `crates/rustok-*/src/ports.rs`
- `crates/rustok-*/src/ports/mod.rs`
- `crates/rustok-*/src/ports/**/*.rs`
- `crates/rustok-*/docs/implementation-plan.md`
- `docs/modules/registry.md`

Registry JSON is the source of declared module role, contract version, ports, operations, consumers, provider dependencies, fallback profiles, evidence paths, and contract-test cases. Rust code confirms port traits, operations, shared `PortContext`/`PortError`, and `PortCallPolicy` usage.

When a module splits port traits, implementations, and policy calls across multiple
`src/ports` files, the checker merges those code markers into one module-level code
view before emitting diagnostics. This keeps `ports.rs -> ports/` refactors from
creating false missing-port or missing-policy findings.

Canonical entities:

```text
fba_module://<module>
fba_contract://<module>/<contract_version>
fba_port://<module>/<port_name>
fba_operation://<module>/<port_name>/<operation>
fba_profile://<module>/<profile>
fba_dependency://<consumer>/<provider>/<profile>
```

The registry fact kind is `rustok_fba_registry`. The code marker fact kind is
`rustok_fba_port_code`; bounded documentation references use `rustok_fba_docs_status`.

## Linking

The linker consumes FBA facts and emits canonical graph relations:

- `contains`: module to contract, contract to port, port to operation, contract to profile
- `implemented_by`: FBA module or port to evidence file
- `rustok_fba_module_requires_dependency`: consumer module to dependency declaration
- `rustok_fba_contract_requires_dependency`: consumer contract to dependency declaration
- `rustok_fba_consumer_requires_provider`: consumer dependency to provider module

Relations include evidence and ownership inherited from marker facts. Registry facts use the first
registry identity anchor line, such as `module`, `module_slug`, `provider`, `contract_version`, or
`role`. Registry-derived module, contract, port, operation, profile, and consumer dependency
entities use the line of their own JSON declaration instead of sharing one registry-wide line.
Provider and consumer placeholder entities emitted from dependency declarations do not claim source
ownership; registry and code facts provide the primary source for the real module, contract, port,
operation, and dependency entities.
Registry-derived relations use the target entity source line as evidence where possible, so
`ath explain` and graph output navigate to concrete declarations rather than the registry header.

## Diagnostics

The checker emits only diagnostics whose kind starts with `rustok_fba_`:

- `rustok_fba_registry_missing`
- `rustok_fba_port_trait_missing`
- `rustok_fba_port_operation_missing`
- `rustok_fba_context_missing`
- `rustok_fba_error_missing`
- `rustok_fba_policy_missing`
- `rustok_fba_write_idempotency_missing`
- `rustok_fba_contract_tests_missing`
- `rustok_fba_evidence_missing`
- `rustok_fba_consumer_provider_unresolved`
- `rustok_fba_provider_contract_mismatch`
- `rustok_fba_provider_port_unknown`
- `rustok_fba_provider_profile_unknown`
- `rustok_fba_provider_consumer_missing`
- `rustok_fba_consumer_profile_mismatch`
- `rustok_fba_fallback_profile_mismatch`
- `rustok_fba_degraded_mode_mismatch`
- `rustok_fba_docs_drift`

Documentation status is secondary drift evidence only and does not define FBA readiness. The FBA
drift check compares registry status, contract versions, verifier/evidence references, the module
implementation plan, and the central FFA/FBA readiness board. Its diagnostics include both registry
and documentation evidence so violation graphs lead to the file that needs synchronization.
Consumer/provider cross-check diagnostics compare dependency declarations with the provider
registry before declaring an edge clean: required contract versions, ports, provider profiles,
provider-side consumer profile entries, fallback profiles, and degraded modes must agree.

## Graph Commands

FBA graph commands consume canonical FBA entities, relations, and diagnostics from the latest snapshot. They do not inspect source files directly.

Default limits:

- nodes: 80
- edges: 160

The violations graph includes only violated backend boundaries and evidence files, not clean implementation edges.

Audit summaries distinguish registry-backed modules from dependency-only module nodes and report
`in_progress_modules` and `status_unknown_modules` explicitly. A zero diagnostic count therefore
means that checked contracts are consistent; it does not claim that every migration is complete.

Registry-backed rows also expose evidence-derived contract requirements met/total and an integer
`completion_percent`. Applicable requirements cover port code, declared traits and operations,
shared context/error, required policy and idempotency semantics, evidence, contract tests, and
dependency resolution. Requirements that do not apply are excluded from the denominator;
dependency-only rows remain unscored with `completion_percent: null`. Migration `status` and open
diagnostics remain separate fields and are never hidden by the contract percentage.
Operation, context, error, and policy requirements depend on the matching port trait being present;
when the trait itself is missing, those dependent requirements remain unmet instead of being inferred
from the absence of more specific downstream diagnostics.

The summary `dependency_edges_resolved` count is per dependency edge. A consumer with multiple
providers can therefore report partial resolution instead of collapsing all dependencies for that
module to all-resolved or all-unresolved.
Generic graph traversal can follow `consumer module -> dependency -> provider module` and
`consumer contract -> dependency -> provider module` paths for registry-declared provider
dependencies. Provider-side `consumers` declarations still point dependency nodes at the provider
module, preserve hyphenated consumer module slugs, and act as cross-check evidence rather than as
the consumer's primary declaration source.
