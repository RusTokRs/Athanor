---
id: doc://docs/adapters/rustok-page-builder.md
kind: adapter
language: en
source_language: en
status: draft
---
# RusTok Page Builder Adapter

`athanor-adapter-rustok-page-builder` is an opt-in code-audit adapter for the RusTok Page Builder provider/consumer track.

It is intentionally separate from FFA and FBA support. Page Builder uses its own adapter ids, canonical entity namespace, diagnostics, and graph command namespace.

## Built-In Ids

```text
builtin.extractor.rustok_page_builder
builtin.linker.rustok_page_builder
builtin.checker.rustok_page_builder
```

These ids are available to adapter plugin manifests, but they are not registered in the default Athanor pipeline.

Example RusTok manifest:

```json
{
  "schema": "athanor.adapter_manifest.v1",
  "name": "rustok-page-builder",
  "version": "0.1.0",
  "adapters": [
    { "id": "builtin.extractor.rustok_page_builder", "kind": "extractor", "enabled": true },
    { "id": "builtin.linker.rustok_page_builder", "kind": "linker", "enabled": true },
    { "id": "builtin.checker.rustok_page_builder", "kind": "checker", "enabled": true }
  ]
}
```

Place it at `.athanor/adapters/rustok-page-builder.json` in the RusTok repository, then run:

```bash
ath index .
ath rustok page-builder audit . --json
ath graph page-builder provider --json
ath graph page-builder consumer pages --json
ath graph page-builder consumer forum --json
ath graph page-builder violations --module pages --json
ath check rustok-page-builder --json
```

## Extraction Model

The extractor reads machine contracts, evidence packets, consumer manifests, content-format markers, and FSD surface markers:

- `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`
- `crates/rustok-page-builder/contracts/page-builder-adapter-seams.json`
- `crates/rustok-page-builder/contracts/evidence/*wave*.json`
- `crates/rustok-*/contracts/evidence/*wave*.json`
- `crates/rustok-*/rustok-module.toml`
- `crates/rustok-{pages,forum,blog,content}/**/*.rs`
- module-local `admin/` and `storefront/` FSD paths

For consumer manifests, the adapter prefers `[fba.builder_consumer]` and falls back to
`[dependencies.page_builder]`. Fallback profiles may be declared explicitly or derived from
`[fba.builder_consumer.toggle_profiles]`.

Canonical entities:

```text
page_builder_provider://<module>
page_builder_consumer://<module>
page_builder_contract://<contract>
page_builder_capability://<capability>
page_builder_fallback_profile://<profile>
page_builder_wave_evidence://<module>/wave<wave>
page_builder_adapter_seam://<seam>
page_builder_content_surface://<module>/<format>
page_builder_fsd_surface://<module>/<surface>
```

The adapter does not run RusTok npm verifier scripts. Those scripts remain external smoke gates; Athanor indexes their contracts and evidence as source facts.

## Diagnostics

The checker emits only diagnostics whose kind starts with `rustok_page_builder_`:

- `rustok_page_builder_registry_missing`
- `rustok_page_builder_consumer_registry_drift`
- `rustok_page_builder_contract_version_drift`
- `rustok_page_builder_capability_drift`
- `rustok_page_builder_fallback_profile_drift`
- `rustok_page_builder_wave_evidence_missing`
- `rustok_page_builder_wave_evidence_stale`
- `rustok_page_builder_adapter_seam_missing`
- `rustok_page_builder_content_format_drift`
- `rustok_page_builder_fsd_core_leaks_ui`
- `rustok_page_builder_fsd_transport_missing`
- `rustok_page_builder_fsd_ui_adapter_missing`
- `rustok_page_builder_host_owns_module_ui`

Markdown status is secondary evidence only and does not define Page Builder readiness.

## Graph Commands

Page Builder graph commands consume canonical Page Builder entities, relations, and diagnostics from the latest snapshot. They do not inspect source files directly.

Default limits:

- nodes: 80
- edges: 160

The violations graph includes only violated Page Builder boundaries and evidence files, not clean implementation edges.
