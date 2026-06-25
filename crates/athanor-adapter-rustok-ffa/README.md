# Athanor RusTok FFA Adapter

`athanor-adapter-rustok-ffa` is an opt-in Athanor adapter for auditing RusTok Fluid Frontend Architecture (FFA) code.

It provides three built-in adapter ids:

- `builtin.extractor.rustok_ffa`
- `builtin.linker.rustok_ffa`
- `builtin.checker.rustok_ffa`

The adapter reads code, not markdown readiness statuses. Documentation may be used later only as drift evidence.

## Model

Canonical entities:

- `ffa_surface://<module>/<surface>`
- `ffa_layer://<module>/<surface>/<role>`

Supported roles are `core`, `transport`, `ui_leptos`, `api`, `host_wiring`, `manifest`, `crate_root`, and `other`.

The linker connects surfaces to layers with `contains` and layers to files with `implemented_by`.

## Diagnostics

The checker emits FFA-only diagnostics with ids prefixed by `rustok_ffa_`, including core Leptos imports, raw UI transport calls, missing FFA layers, missing transport profiles, and host-owned module UI.
