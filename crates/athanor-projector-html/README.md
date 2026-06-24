# HTML Report Projector Adapter

Crate: `athanor-projector-html`

Port: `Projector`

## Purpose

Builds a disposable, self-contained HTML report from canonical snapshot objects. The report is intended for browser inspection and does not become canonical input.

## Input

Receives `ProjectInput` with an `athanor.html_report_projection.v1` payload containing canonical entities, facts, relations, and diagnostics from one snapshot.

## Output

Writes:

```text
index.html
entities/<entity-id>.html
manifest.json
```

The HTML contains snapshot metrics, a compact graph summary, open diagnostic cards, client-side filters, and a canonical entity table. Each canonical entity also receives a detail page with identity metadata, ownership, attached facts, relations, diagnostics, and evidence locations. Dynamic canonical text is HTML-escaped and the report has no external scripts, stylesheets, or network dependencies.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Writes only to the configured target and temporary sibling directories used for staged replacement.

## Limitations

- The graph view is a compact inspection summary rather than an interactive layout algorithm.
- Entity detail pages render direct attached facts, relations, diagnostics, and evidence, but deeper graph traversal remains a CLI/query workflow.
- It rebuilds the complete report on every invocation.

## Tests

```bash
cargo test -p athanor-projector-html
```
