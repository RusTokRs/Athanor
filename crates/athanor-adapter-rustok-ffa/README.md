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

Supported roles are `core`, `transport`, `ui_leptos`, `ui_support`, `api`, `host_wiring`, `manifest`, `crate_root`, and `other`. Flat files, `mod.rs`, and nested files under `src/core/`, `src/transport/`, and `src/ui/leptos/` map to the same canonical layer ids as `src/core.rs`, `src/transport.rs`, and `src/ui/leptos.rs`. Only `ui/leptos.rs` or `ui/leptos/` satisfies the explicit UI-adapter layer; adjacent UI files remain visible as `ui_support`.

The linker connects surfaces to layers with `contains` and layers to files with `implemented_by`.

## Diagnostics

The checker emits FFA-only diagnostics with ids prefixed by `rustok_ffa_`, including core Leptos imports, raw UI transport calls, missing FFA layers, missing transport profiles, host-owned module UI, and duplicate readiness-board entries.

The audit summary reports observed, actionable, scaffold, and host-wiring surface counts
separately. Scaffold and host-wiring rows remain visible without inflating the actionable
completion count.

Actionable rows expose core, transport, and explicit UI-adapter presence as a three-requirement
structural numerator/denominator with an integer percentage. Non-actionable rows report no
percentage, and open diagnostics remain visible independently.
