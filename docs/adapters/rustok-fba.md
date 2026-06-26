---
id: doc://docs/adapters/rustok-fba.md
kind: adapter
language: en
last_verified_snapshot: snap_jsonl_00000191
source_language: en
status: verified
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
  "schema": "athanor.adapter_manifest",
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

Registry JSON is the source of declared module role, contract version, ports, operations, consumers, provider dependencies, fallback profiles, evidence paths, and contract-test cases. Rust code confirms port traits, operations, shared `PortContext`/`PortError`, and `PortCallPolicy` usage.

Canonical entities:

```text
fba_module://<module>
fba_contract://<module>/<contract_version>
fba_port://<module>/<port_name>
fba_operation://<module>/<port_name>/<operation>
fba_profile://<module>/<profile>
fba_dependency://<consumer>/<provider>/<profile>
```

The registry fact kind is `rustok_fba_registry`. The code marker fact kind is `rustok_fba_port_code`.

## Linking

The linker consumes FBA facts and emits canonical graph relations:

- `contains`: module to contract, contract to port, port to operation, contract to profile
- `implemented_by`: FBA module or port to evidence file
- `rustok_fba_consumer_requires_provider`: consumer dependency to provider module

Relations include evidence and ownership inherited from marker facts.

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

Documentation status is secondary drift evidence only and does not define FBA readiness.

## Graph Commands

FBA graph commands consume canonical FBA entities, relations, and diagnostics from the latest snapshot. They do not inspect source files directly.

Default limits:

- nodes: 80
- edges: 160

The violations graph includes only violated backend boundaries and evidence files, not clean implementation edges.
