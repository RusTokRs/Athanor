# Athanor RusTok Page Builder Adapter

`athanor-adapter-rustok-page-builder` is an opt-in Athanor adapter for auditing the RusTok Page Builder provider/consumer track.

It provides three built-in adapter ids:

- `builtin.extractor.rustok_page_builder`
- `builtin.linker.rustok_page_builder`
- `builtin.checker.rustok_page_builder`

The adapter is separate from the RusTok FFA and FBA adapters. It reads Page Builder provider registry JSON, adapter seam contracts, provider-local and module-local wave evidence packets, consumer manifests, content-format markers, and FSD/FFA-style surface markers. It does not run RusTok verifier scripts; those remain external smoke gates.

Consumer manifests are read from `[fba.builder_consumer]` first and from `[dependencies.page_builder]` as a fallback. Fallback profiles can be declared directly or derived from `[fba.builder_consumer.toggle_profiles]`.

## Model

Canonical entities:

- `page_builder_provider://<module>`
- `page_builder_consumer://<module>`
- `page_builder_contract://<contract>`
- `page_builder_capability://<capability>`
- `page_builder_fallback_profile://<profile>`
- `page_builder_wave_evidence://<module>/wave<wave>`
- `page_builder_adapter_seam://<seam>`
- `page_builder_content_surface://<module>/<format>`
- `page_builder_fsd_surface://<module>/<surface>`

The linker connects provider, consumer, contract, capability, fallback, evidence, content, and FSD surface entities with bounded canonical relations.

## Diagnostics

The checker emits only diagnostics whose kind starts with `rustok_page_builder_`, including registry drift, contract/capability/fallback drift, missing or stale wave evidence, missing adapter seams, content-format drift, and FSD boundary violations.
