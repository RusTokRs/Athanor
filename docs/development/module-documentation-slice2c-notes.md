---
id: doc://docs/development/module-documentation-slice2c-notes.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Module Documentation Slice 2C Notes

Slice 2C reuses the existing documentation-generation v1 contracts and immutable generation root. The
only new persisted payload is `modules/index.md`; request, manifest, validation, and current-pointer JSON
schemas are unchanged.

The shared `current.json` pointer is profile-aware. Architecture inspection rejects a module current
pointer and module inspection rejects an architecture current pointer before returning generation data.

Supported module commands:

```bash
ath docs generate-module <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs module current <PATH> [--json]
ath docs module manifest <PATH> [--json]
ath docs module validation <PATH> [--json]
```

No latest-snapshot fallback, provider, daemon, MCP integration, or coordinated `ath generate` change is
introduced by this slice.
