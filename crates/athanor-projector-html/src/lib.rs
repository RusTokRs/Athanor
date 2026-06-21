use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, ProjectInput, Projector};
use athanor_domain::{Diagnostic, DiagnosticStatus, Entity};
use athanor_projector_support::{
    CanonicalProjectionPayload, publish_staged_directory, write_output_file,
};
use serde::Serialize;
use serde_json::json;

pub const HTML_REPORT_PROJECTION_SCHEMA: &str = "athanor.html_report_projection.v1";
pub const HTML_REPORT_MANIFEST_SCHEMA: &str = "athanor.html_report_manifest.v1";
pub const HTML_REPORT_FORMAT_VERSION: &str = "athanor-html-report-v1";

pub type HtmlReportProjectionPayload = CanonicalProjectionPayload;

#[derive(Debug, Default, Clone)]
pub struct HtmlReportProjector;

#[async_trait]
impl Projector for HtmlReportProjector {
    fn name(&self) -> &str {
        "html-report-projector"
    }

    async fn project(&self, input: ProjectInput) -> CoreResult<()> {
        let payload: HtmlReportProjectionPayload = serde_json::from_value(input.payload)
            .map_err(|error| adapter_error(format!("invalid HTML report payload: {error}")))?;
        if payload.schema != HTML_REPORT_PROJECTION_SCHEMA {
            return Err(adapter_error(format!(
                "unsupported HTML report projection schema: {}",
                payload.schema
            )));
        }

        write_projection(PathBuf::from(input.target), &input.snapshot.0, payload)
    }
}

fn write_projection(
    target: PathBuf,
    snapshot: &str,
    mut payload: HtmlReportProjectionPayload,
) -> CoreResult<()> {
    payload
        .entities
        .sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
    payload
        .diagnostics
        .sort_by(|left, right| left.id.0.cmp(&right.id.0));
    let open_diagnostics = payload
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .count();
    let report = render_report(snapshot, &payload);
    let manifest = json!({
        "schema": HTML_REPORT_MANIFEST_SCHEMA,
        "report_format_version": HTML_REPORT_FORMAT_VERSION,
        "snapshot": snapshot,
        "status": "complete",
        "entities": payload.entities.len(),
        "facts": payload.facts.len(),
        "relations": payload.relations.len(),
        "open_diagnostics": open_diagnostics,
    });
    let manifest = serde_json::to_string_pretty(&manifest)
        .map_err(|error| adapter_error(format!("serialize HTML report manifest: {error}")))?;

    publish_staged_directory(&target, "HTML report", |staging| {
        write_output_file(&staging.join("index.html"), &report)?;
        write_output_file(&staging.join("manifest.json"), &manifest)
    })
}

fn render_report(snapshot: &str, payload: &HtmlReportProjectionPayload) -> String {
    let open_diagnostics = payload
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .collect::<Vec<_>>();
    let diagnostic_counts = diagnostic_counts_by_entity(&open_diagnostics);
    let mut output = String::from(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
    );
    output.push_str(&format!(
        "<title>Athanor report - {}</title>\n",
        escape_html(snapshot)
    ));
    output.push_str(STYLES);
    output.push_str("</head>\n<body>\n<main>\n<header>\n<p class=\"eyebrow\">Athanor canonical report</p>\n<h1>Project knowledge snapshot</h1>\n");
    output.push_str(&format!(
        "<p>Snapshot <code>{}</code></p>\n</header>\n",
        escape_html(snapshot)
    ));
    output.push_str("<section aria-labelledby=\"summary\"><h2 id=\"summary\">Summary</h2><div class=\"metrics\">\n");
    metric(&mut output, "Entities", payload.entities.len());
    metric(&mut output, "Facts", payload.facts.len());
    metric(&mut output, "Relations", payload.relations.len());
    metric(&mut output, "Open diagnostics", open_diagnostics.len());
    output.push_str("</div></section>\n<section aria-labelledby=\"diagnostics\"><h2 id=\"diagnostics\">Open diagnostics</h2>\n");
    if open_diagnostics.is_empty() {
        output.push_str("<p class=\"empty\">No open diagnostics.</p>\n");
    } else {
        for diagnostic in open_diagnostics {
            render_diagnostic(&mut output, diagnostic, &payload.entities);
        }
    }
    output.push_str("</section>\n<section aria-labelledby=\"entities\"><h2 id=\"entities\">Canonical entities</h2>\n<div class=\"table-wrap\"><table><thead><tr><th>Kind</th><th>Name</th><th>Stable key</th><th>Source</th><th>Diagnostics</th></tr></thead><tbody>\n");
    for entity in &payload.entities {
        render_entity_row(&mut output, entity, &diagnostic_counts);
    }
    output.push_str("</tbody></table></div></section>\n</main>\n</body>\n</html>\n");
    output
}

fn metric(output: &mut String, label: &str, value: usize) {
    output.push_str(&format!(
        "<div class=\"metric\"><strong>{value}</strong><span>{}</span></div>\n",
        escape_html(label)
    ));
}

fn render_diagnostic(output: &mut String, diagnostic: &Diagnostic, entities: &[Entity]) {
    let severity = serialized_name(&diagnostic.severity);
    output.push_str(&format!(
        "<article class=\"diagnostic severity-{}\"><div class=\"diagnostic-heading\"><span class=\"badge\">{}</span><h3>{}</h3></div><p>{}</p>\n",
        escape_html(&severity),
        escape_html(&severity),
        escape_html(&diagnostic.title),
        escape_html(&diagnostic.message)
    ));
    if !diagnostic.entities.is_empty() {
        output.push_str("<p><strong>Entities:</strong> ");
        for (index, entity_id) in diagnostic.entities.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            let label = entities
                .iter()
                .find(|entity| entity.id == *entity_id)
                .map_or(entity_id.0.as_str(), |entity| entity.stable_key.0.as_str());
            output.push_str(&format!("<code>{}</code>", escape_html(label)));
        }
        output.push_str("</p>\n");
    }
    let locations = diagnostic
        .evidence
        .iter()
        .filter_map(|evidence| {
            evidence.source_file.as_ref().map(|path| {
                evidence
                    .line_start
                    .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
            })
        })
        .collect::<Vec<_>>();
    if !locations.is_empty() {
        output.push_str(&format!(
            "<p><strong>Evidence:</strong> <code>{}</code></p>\n",
            escape_html(&locations.join(", "))
        ));
    }
    if let Some(fix) = &diagnostic.suggested_fix {
        output.push_str(&format!(
            "<p><strong>Suggested fix:</strong> {}</p>\n",
            escape_html(fix)
        ));
    }
    output.push_str("</article>\n");
}

fn render_entity_row(
    output: &mut String,
    entity: &Entity,
    diagnostic_counts: &HashMap<String, usize>,
) {
    let source = entity.source.as_ref().map_or_else(String::new, |source| {
        source.line_start.map_or_else(
            || source.path.clone(),
            |line| format!("{}:{line}", source.path),
        )
    });
    output.push_str(&format!(
        "<tr><td>{}</td><td>{}</td><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>\n",
        escape_html(&serialized_name(&entity.kind)),
        escape_html(entity.title.as_deref().unwrap_or(&entity.name)),
        escape_html(&entity.stable_key.0),
        escape_html(&source),
        diagnostic_counts.get(&entity.id.0).copied().unwrap_or(0)
    ));
}

fn diagnostic_counts_by_entity(diagnostics: &[&Diagnostic]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for diagnostic in diagnostics {
        for entity in &diagnostic.entities {
            *counts.entry(entity.0.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn serialized_name(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn adapter_error(message: String) -> CoreError {
    CoreError::Adapter(message)
}

const STYLES: &str = r#"<style>
:root { color-scheme: light dark; font-family: system-ui, sans-serif; }
body { margin: 0; background: #111827; color: #e5e7eb; }
main { width: min(1200px, calc(100% - 2rem)); margin: 0 auto; padding: 3rem 0; }
header { border-bottom: 1px solid #374151; margin-bottom: 2rem; }
h1, h2, h3 { color: #f9fafb; }
.eyebrow { color: #60a5fa; font-weight: 700; text-transform: uppercase; letter-spacing: .08em; }
.metrics { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 1rem; }
.metric, .diagnostic { background: #1f2937; border: 1px solid #374151; border-radius: .75rem; padding: 1rem; }
.metric strong { display: block; color: #93c5fd; font-size: 2rem; }
.metric span { color: #9ca3af; }
.diagnostic { margin: 1rem 0; }
.diagnostic-heading { display: flex; gap: .75rem; align-items: center; }
.diagnostic-heading h3 { margin: 0; }
.badge { border-radius: 999px; background: #374151; padding: .2rem .6rem; font-size: .8rem; text-transform: uppercase; }
.severity-critical { border-color: #ef4444; } .severity-high { border-color: #f97316; } .severity-medium { border-color: #eab308; }
.table-wrap { overflow-x: auto; }
table { width: 100%; border-collapse: collapse; background: #1f2937; }
th, td { text-align: left; vertical-align: top; padding: .65rem; border-bottom: 1px solid #374151; }
th { color: #93c5fd; position: sticky; top: 0; background: #1f2937; }
code { overflow-wrap: anywhere; color: #bfdbfe; }
.empty { color: #9ca3af; }
</style>
"#;

#[cfg(test)]
mod tests {
    use std::fs;

    use athanor_domain::{EntityId, EntityKind, SnapshotId, StableKey};

    use super::*;

    #[tokio::test]
    async fn writes_self_contained_escaped_report() {
        let root = std::env::temp_dir().join(format!(
            "athanor-html-projector-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("html");
        let payload = HtmlReportProjectionPayload {
            schema: HTML_REPORT_PROJECTION_SCHEMA.to_string(),
            entities: vec![Entity {
                id: EntityId("ent_login".to_string()),
                stable_key: StableKey("api://POST:/login?a=<unsafe>".to_string()),
                kind: EntityKind::ApiEndpoint,
                name: "<script>alert(1)</script>".to_string(),
                title: None,
                source: None,
                language: None,
                aliases: Vec::new(),
                ownership: Vec::new(),
                payload: json!({}),
            }],
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };

        HtmlReportProjector
            .project(ProjectInput {
                snapshot: SnapshotId("snap_test".to_string()),
                target: target.to_string_lossy().into_owned(),
                payload: serde_json::to_value(payload).unwrap(),
            })
            .await
            .unwrap();

        let html = fs::read_to_string(target.join("index.html")).unwrap();
        assert!(html.contains("snap_test"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(target.join("manifest.json").is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn rejects_unknown_payload_schema() {
        let error = HtmlReportProjector
            .project(ProjectInput {
                snapshot: SnapshotId("snap_test".to_string()),
                target: "unused".to_string(),
                payload: json!({
                    "schema": "unknown",
                    "entities": [],
                    "facts": [],
                    "relations": [],
                    "diagnostics": []
                }),
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("unsupported HTML report"));
    }
}
