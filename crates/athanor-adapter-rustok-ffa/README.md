# Athanor RusTok FFA Adapter

`athanor-adapter-rustok-ffa` is an opt-in Athanor adapter for auditing RusTok Fluid Frontend Architecture (FFA) code.

It provides three built-in adapter ids:

- `builtin.extractor.rustok_ffa`
- `builtin.linker.rustok_ffa`
- `builtin.checker.rustok_ffa`

The adapter reads code for readiness classification. It also compares the central readiness board with module-local status blocks as secondary drift evidence; markdown never overrides the code-derived FFA shape.

## Model

Canonical entities:

- `ffa_surface://<module>/<surface>`
- `ffa_layer://<module>/<surface>/<role>`

Supported roles are `core`, `transport`, `ui_leptos`, `ui_support`, `api`, `host_wiring`, `manifest`, `crate_root`, and `other`. Only `ui/leptos.rs` or `ui/leptos/` satisfies the explicit UI-adapter layer; adjacent UI files remain visible as `ui_support`.

The linker connects surfaces to layers with `contains` and layers to files with `implemented_by`.

## Diagnostics

The checker emits FFA-only diagnostics with ids prefixed by `rustok_ffa_`, including core Leptos imports, raw UI transport calls, missing FFA layers, missing transport profiles, host-owned module UI, and duplicate readiness-board entries.
