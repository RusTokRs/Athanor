# Athanor RusTok FBA Adapter

`athanor-adapter-rustok-fba` is an opt-in Athanor adapter for auditing RusTok Fluid Backend Architecture (FBA) contracts.

It provides three built-in adapter ids:

- `builtin.extractor.rustok_fba`
- `builtin.linker.rustok_fba`
- `builtin.checker.rustok_fba`

The adapter is separate from the FFA adapter. FBA reads registry JSON and code markers from Rust port implementations; documentation is secondary evidence only and does not define readiness.

## Model

Canonical entities:

- `fba_module://<module>`
- `fba_contract://<module>/<contract_version>`
- `fba_port://<module>/<port_name>`
- `fba_operation://<module>/<port_name>/<operation>`
- `fba_profile://<module>/<profile>`
- `fba_dependency://<consumer>/<provider>/<profile>`

The linker connects modules, contracts, ports, operations, profiles, dependency declarations, and evidence files with bounded canonical relations.

## Diagnostics

The checker emits FBA-only diagnostics with ids prefixed by `rustok_fba_`, including missing registries, missing port traits or operations, missing shared context/error contracts, missing call policy enforcement, missing write idempotency contract assertions, missing contract tests, missing registry evidence, and unresolved consumer/provider declarations.
