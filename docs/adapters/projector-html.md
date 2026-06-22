---
id: doc://docs/adapters/projector-html.md
kind: module_documentation
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000030
status: verified
---

# HTML Report Projector

Crate: `athanor-projector-html`

Port: `Projector`

## Purpose

Projects the latest canonical snapshot into a disposable, self-contained HTML report for browser inspection. The generated report is a read model and never becomes canonical input.

## Input

`athanor-app::project_html_report` loads the latest snapshot from `CanonicalSnapshotStore` and passes a versioned `athanor.html_report_projection.v1` payload to `HtmlReportProjector`.

The payload contains entities, facts, relations, diagnostics, and the canonical snapshot id in `ProjectInput`.

## Output

The default output is:

```text
.athanor/generated/current/html/
  index.html
  manifest.json
```

The static report includes:

- snapshot and canonical object counts
- complete open diagnostic cards with evidence and suggested fixes
- a deterministic table of canonical entities, source locations, stable keys, and attached diagnostic counts

All dynamic canonical text is HTML-escaped. CSS is embedded and the page does not load scripts, fonts, stylesheets, or other network resources.

The projector uses `athanor-projector-support` for staged directory replacement shared with the Markdown wiki projector. Readers never observe partially written output; platforms that cannot replace a non-empty directory in one operation can briefly observe the target as absent during the final swap.

## CLI

```bash
ath report html
ath report html <project-root>
ath report html <project-root> --output <directory>
```

Relative output paths are resolved against the project root. The command does not index; it fails with a prompt to run `ath index` when no canonical snapshot exists.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Writes only below the selected output path and temporary sibling paths used for replacement.

## Limitations

- Produces one static page without client-side search or filtering.
- Summarizes facts and relations by count rather than rendering each object.
- Rebuilds the complete report on every invocation.
- Projector plugin discovery and runtime registration remain deferred.

## Verification

```bash
cargo test -p athanor-projector-html
cargo run -p ath --quiet -- index .
cargo run -p ath --quiet -- report html .
```
