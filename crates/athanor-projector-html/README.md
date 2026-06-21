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
manifest.json
```

The HTML contains snapshot metrics, open diagnostic cards, and a canonical entity table. Dynamic canonical text is HTML-escaped and the report has no external scripts, stylesheets, or network dependencies.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Writes only to the configured target and temporary sibling directories used for staged replacement.

## Limitations

- The first report is a single static page without client-side filtering.
- Facts and relations are summarized by count rather than rendered individually.
- It rebuilds the complete report on every invocation.

## Tests

```bash
cargo test -p athanor-projector-html
```
