---
id: doc://docs/adapters/rustok-ffa.md
kind: adapter
language: en
source_language: en
status: verified
---

# RusTok FFA Adapter

`athanor-adapter-rustok-ffa` is an opt-in code-audit adapter for RusTok Fluid Frontend Architecture (FFA).

It is intentionally separate from future FBA support. FBA must use its own adapter ids and graph command namespace.

## Built-In Ids

```text
builtin.extractor.rustok_ffa
builtin.linker.rustok_ffa
builtin.checker.rustok_ffa
```

These ids are available to adapter plugin manifests, but they are not registered in the default Athanor pipeline.

Example RusTok manifest:

```json
{
  "schema": "athanor.adapter_manifest",
  "name": "rustok-ffa",
  "version": "0.1.0",
  "adapters": [
    { "id": "builtin.extractor.rustok_ffa", "kind": "extractor", "enabled": true },
    { "id": "builtin.linker.rustok_ffa", "kind": "linker", "enabled": true },
    { "id": "builtin.checker.rustok_ffa", "kind": "checker", "enabled": true }
  ]
}
```

Place it at `.athanor/adapters/rustok-ffa.json` in the RusTok repository, then run:

```bash
ath index .
ath rustok ffa audit . --json
ath graph ffa surface blog admin --json
ath graph ffa violations --module blog --surface admin --json
ath check rustok-ffa --json
```

## Extraction Model

The extractor reads source code paths and file contents. It does not read markdown status tables as readiness classification.

Module-owned surfaces are detected under:

```text
crates/rustok-*/admin
crates/rustok-*/storefront
```

Host wiring is detected under:

```text
apps/admin
apps/storefront
```

Canonical entities:

```text
ffa_surface://<module>/<surface>
ffa_layer://<module>/<surface>/<core|transport|ui_leptos|api|host_wiring|manifest|crate_root|other>
```

The marker fact kind is `rustok_ffa_source_marker`. Marker payloads include the module, surface, role, normalized path, canonical UI-adapter flag, host-wiring flag, Leptos/component/server markers, raw API calls, transport-facade calls, and transport profile hints.

## Linking

The linker consumes FFA marker facts and emits canonical graph relations:

- `contains`: surface to layer
- `implemented_by`: layer to file

Relations include evidence and ownership inherited from marker facts.

## Diagnostics

The checker emits only diagnostics whose kind starts with `rustok_ffa_`:

- `rustok_ffa_core_depends_on_leptos`
- `rustok_ffa_ui_calls_raw_transport`
- `rustok_ffa_surface_missing_core`
- `rustok_ffa_surface_missing_transport`
- `rustok_ffa_surface_missing_ui_adapter`
- `rustok_ffa_transport_profile_missing`
- `rustok_ffa_host_owns_module_ui`
- `rustok_ffa_forgotten_surface`

Documentation drift is reserved for later, evidence-backed docs facts. Markdown status is never the source of FFA readiness.

## Graph Commands

FFA graph commands consume canonical FFA entities, relations, and diagnostics from the latest snapshot. They do not inspect source files directly.

Default limits:

- nodes: 80
- edges: 160

The violations graph includes only violated boundaries and evidence files, not clean implementation edges.
