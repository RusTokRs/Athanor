use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, ProjectInput, Projector};
use athanor_domain::{Diagnostic, DiagnosticStatus, Entity, Relation};
use athanor_projector_support::{
    CanonicalProjectionIndex, CanonicalProjectionPayload, publish_staged_directory, safe_filename,
    write_output_file, write_output_file_with_existing_parent,
};
use serde::Serialize;
use serde_json::json;

pub const HTML_REPORT_PROJECTION_SCHEMA: &str = "athanor.html_report_projection.v1";
pub const HTML_REPORT_MANIFEST_SCHEMA: &str = "athanor.html_report_manifest.v1";
pub const HTML_REPORT_FORMAT_VERSION: &str = "athanor-html-report-v1";

pub type HtmlReportProjectionPayload = CanonicalProjectionPayload;

#[derive(Debug, Default, Clone)]
pub struct HtmlReportProjector;

impl HtmlReportProjector {
    pub async fn project_cancellable(
        &self,
        input: ProjectInput,
        is_cancelled: impl Fn() -> bool,
    ) -> CoreResult<()> {
        ensure_not_cancelled(&is_cancelled)?;
        let payload: HtmlReportProjectionPayload = serde_json::from_value(input.payload)
            .map_err(|error| adapter_error(format!("invalid HTML report payload: {error}")))?;
        if payload.schema != HTML_REPORT_PROJECTION_SCHEMA {
            return Err(adapter_error(format!(
                "unsupported HTML report projection schema: {}",
                payload.schema
            )));
        }

        project_html_report_payload_cancellable(
            PathBuf::from(input.target),
            &input.snapshot.0,
            payload,
            &is_cancelled,
        )
    }
}

#[async_trait]
impl Projector for HtmlReportProjector {
    fn name(&self) -> &str {
        "html-report-projector"
    }

    async fn project(&self, input: ProjectInput) -> CoreResult<()> {
        self.project_cancellable(input, || false).await
    }
}

pub fn project_html_report_payload_cancellable(
    target: PathBuf,
    snapshot: &str,
    payload: HtmlReportProjectionPayload,
    is_cancelled: &dyn Fn() -> bool,
) -> CoreResult<()> {
    if payload.schema != HTML_REPORT_PROJECTION_SCHEMA {
        return Err(adapter_error(format!(
            "unsupported HTML report projection schema: {}",
            payload.schema
        )));
    }
    write_projection(target, snapshot, payload, is_cancelled)
}

fn write_projection(
    target: PathBuf,
    snapshot: &str,
    mut payload: HtmlReportProjectionPayload,
    is_cancelled: &dyn Fn() -> bool,
) -> CoreResult<()> {
    ensure_not_cancelled(is_cancelled)?;
    payload
        .entities
        .sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
    payload
        .diagnostics
        .sort_by(|left, right| left.id.0.cmp(&right.id.0));
    payload
        .relations
        .sort_by(|left, right| left.id.0.cmp(&right.id.0));
    let open_diagnostics = payload
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .count();
    let report = render_report(snapshot, &payload, is_cancelled)?;
    let manifest = json!({
        "schema": HTML_REPORT_MANIFEST_SCHEMA,
        "report_format_version": HTML_REPORT_FORMAT_VERSION,
        "snapshot": snapshot,
        "status": "complete",
        "entities": payload.entities.len(),
        "entity_pages": payload.entities.len(),
        "facts": payload.facts.len(),
        "relations": payload.relations.len(),
        "open_diagnostics": open_diagnostics,
    });
    let manifest = serde_json::to_string_pretty(&manifest)
        .map_err(|error| adapter_error(format!("serialize HTML report manifest: {error}")))?;
    let projection_index = CanonicalProjectionIndex::new(&payload);

    publish_staged_directory(&target, "HTML report", |staging| {
        ensure_not_cancelled(is_cancelled)?;
        write_output_file(&staging.join("index.html"), &report)?;
        let entities_dir = staging.join("entities");
        fs::create_dir_all(&entities_dir).map_err(|error| {
            adapter_error(format!(
                "create HTML entity directory {}: {error}",
                entities_dir.display()
            ))
        })?;
        for entity in &payload.entities {
            ensure_not_cancelled(is_cancelled)?;
            write_output_file_with_existing_parent(
                &entities_dir.join(entity_report_filename(entity)),
                &render_entity_page(snapshot, &payload, &projection_index, entity),
            )?;
        }
        ensure_not_cancelled(is_cancelled)?;
        write_output_file(&staging.join("manifest.json"), &manifest)
    })
}

fn render_report(
    snapshot: &str,
    payload: &HtmlReportProjectionPayload,
    is_cancelled: &dyn Fn() -> bool,
) -> CoreResult<String> {
    ensure_not_cancelled(is_cancelled)?;
    let open_diagnostics = payload
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .collect::<Vec<_>>();
    let diagnostic_counts = diagnostic_counts_by_entity(&open_diagnostics);
    let diagnostic_severities_by_entity = diagnostic_severities_by_entity(&open_diagnostics);
    let kinds = entity_kinds(&payload.entities);
    let severities = diagnostic_severities(&open_diagnostics);
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
    output.push_str("</div></section>\n");
    render_graph_summary(&mut output, payload);
    render_interactive_graph(&mut output, payload);
    render_filters(&mut output, &kinds, &severities);
    output.push_str(
        "<section aria-labelledby=\"diagnostics\"><h2 id=\"diagnostics\">Open diagnostics</h2>\n",
    );
    if open_diagnostics.is_empty() {
        output.push_str("<p class=\"empty\">No open diagnostics.</p>\n");
    } else {
        for diagnostic in open_diagnostics {
            ensure_not_cancelled(is_cancelled)?;
            render_diagnostic(&mut output, diagnostic, &payload.entities);
        }
    }
    output.push_str("</section>\n<section aria-labelledby=\"entities\"><h2 id=\"entities\">Canonical entities</h2>\n<div class=\"table-wrap\"><table><thead><tr><th>Kind</th><th>Name</th><th>Stable key</th><th>Source</th><th>Diagnostics</th></tr></thead><tbody>\n");
    for entity in &payload.entities {
        ensure_not_cancelled(is_cancelled)?;
        render_entity_row(
            &mut output,
            entity,
            &diagnostic_counts,
            &diagnostic_severities_by_entity,
        );
    }
    output.push_str("</tbody></table></div></section>\n</main>\n");
    output.push_str(SCRIPT);
    output.push_str("</body>\n</html>\n");
    Ok(output)
}

fn ensure_not_cancelled(is_cancelled: &dyn Fn() -> bool) -> CoreResult<()> {
    if is_cancelled() {
        return Err(adapter_error("operation cancelled".to_string()));
    }
    Ok(())
}

fn metric(output: &mut String, label: &str, value: usize) {
    output.push_str(&format!(
        "<div class=\"metric\"><strong>{value}</strong><span>{}</span></div>\n",
        escape_html(label)
    ));
}

fn render_graph_summary(output: &mut String, payload: &HtmlReportProjectionPayload) {
    output.push_str("<section aria-labelledby=\"graph\"><h2 id=\"graph\">Graph summary</h2><div class=\"graph-grid\">\n");
    output.push_str("<article><h3>Relation kinds</h3>");
    let relation_kinds = relation_kind_counts(&payload.relations);
    if relation_kinds.is_empty() {
        output.push_str("<p class=\"empty\">No canonical relations.</p>");
    } else {
        output.push_str("<ol class=\"compact-list\">");
        for (kind, count) in relation_kinds.into_iter().take(12) {
            output.push_str(&format!(
                "<li><span>{}</span><strong>{count}</strong></li>",
                escape_html(&kind)
            ));
        }
        output.push_str("</ol>");
    }
    output.push_str("</article><article><h3>Connected entities</h3>");
    let hubs = graph_hubs(payload);
    if hubs.is_empty() {
        output.push_str("<p class=\"empty\">No connected entities.</p>");
    } else {
        output.push_str("<ol class=\"compact-list\">");
        for hub in hubs.into_iter().take(12) {
            output.push_str(&format!(
                "<li><a href=\"{}\">{}</a><strong>{}</strong></li>",
                escape_html(&entity_report_path(hub.entity)),
                escape_html(hub.entity.title.as_deref().unwrap_or(&hub.entity.name)),
                hub.degree
            ));
        }
        output.push_str("</ol>");
    }
    output.push_str("</article></div></section>\n");
}

fn render_interactive_graph(output: &mut String, payload: &HtmlReportProjectionPayload) {
    const MAX_NODES: usize = 80;
    const MAX_EDGES: usize = 240;

    let graph = graph_view_data(payload, MAX_NODES, MAX_EDGES);
    output.push_str(
        "<section aria-labelledby=\"interactive-graph\"><h2 id=\"interactive-graph\">Interactive graph</h2>\n",
    );
    if graph.nodes.is_empty() {
        output.push_str("<p class=\"empty\">No connected canonical entities.</p></section>\n");
        return;
    }
    output.push_str(&format!(
        "<p class=\"graph-note\">Showing {} nodes and {} relations; omitted {} nodes and {} relations by report limits.</p>\n",
        graph.nodes.len(),
        graph.edges.len(),
        graph.omitted_nodes,
        graph.omitted_edges
    ));
    output.push_str("<div class=\"graph-controls\"><label>Find node<input id=\"graph-search\" type=\"search\" autocomplete=\"off\" placeholder=\"name or stable key\"></label><label>Relation kind<select id=\"graph-relation-kind\"><option value=\"\">All relation kinds</option></select></label><div class=\"graph-buttons\"><button id=\"graph-zoom-in\" type=\"button\">Zoom in</button><button id=\"graph-zoom-out\" type=\"button\">Zoom out</button><button id=\"graph-reset\" type=\"button\">Reset layout</button></div></div>\n");
    output.push_str("<div class=\"graph-shell\"><svg id=\"canonical-graph\" viewBox=\"0 0 1000 620\" role=\"img\" aria-label=\"Interactive bounded canonical entity graph\"><g id=\"graph-viewport\"><g id=\"graph-edges\"></g><g id=\"graph-nodes\"></g></g></svg><aside id=\"graph-selection\" class=\"graph-selection\"><p class=\"empty\">Select a node to inspect its canonical identity and direct relations.</p></aside></div>\n");
    let graph_json = serde_json::to_string(&graph)
        .unwrap_or_else(|_| "{\"nodes\":[],\"edges\":[]}".to_string())
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e");
    output.push_str(&format!(
        "<script id=\"canonical-graph-data\" type=\"application/json\">{graph_json}</script>\n"
    ));
    output.push_str("</section>\n");
}

fn render_filters(output: &mut String, kinds: &[String], severities: &[String]) {
    output.push_str("<section aria-labelledby=\"filters\"><h2 id=\"filters\">Filters</h2><div class=\"filters\">\n");
    output.push_str("<label>Search<input id=\"entity-search\" type=\"search\" autocomplete=\"off\" placeholder=\"stable key, name, source\"></label>\n");
    output.push_str("<label>Source<input id=\"source-filter\" type=\"search\" autocomplete=\"off\" placeholder=\"path fragment\"></label>\n");
    output.push_str("<label>Kind<select id=\"kind-filter\"><option value=\"\">All kinds</option>");
    for kind in kinds {
        output.push_str(&format!(
            "<option value=\"{}\">{}</option>",
            escape_html(kind),
            escape_html(kind)
        ));
    }
    output.push_str("</select></label>\n");
    output.push_str("<label>Diagnostic severity<select id=\"severity-filter\"><option value=\"\">All severities</option>");
    for severity in severities {
        output.push_str(&format!(
            "<option value=\"{}\">{}</option>",
            escape_html(severity),
            escape_html(severity)
        ));
    }
    output.push_str("</select></label>\n");
    output.push_str("</div></section>\n");
}

fn render_diagnostic(output: &mut String, diagnostic: &Diagnostic, entities: &[Entity]) {
    let severity = serialized_name(&diagnostic.severity);
    output.push_str(&format!(
        "<article class=\"diagnostic severity-{}\" data-severity=\"{}\"><div class=\"diagnostic-heading\"><span class=\"badge\">{}</span><h3>{}</h3></div><p>{}</p>\n",
        escape_html(&severity),
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
    diagnostic_severities: &HashMap<String, Vec<String>>,
) {
    let source = entity.source.as_ref().map_or_else(String::new, |source| {
        source.line_start.map_or_else(
            || source.path.clone(),
            |line| format!("{}:{line}", source.path),
        )
    });
    let kind = serialized_name(&entity.kind);
    let diagnostic_count = diagnostic_counts.get(&entity.id.0).copied().unwrap_or(0);
    let search = format!(
        "{} {} {} {}",
        kind,
        entity.title.as_deref().unwrap_or(&entity.name),
        entity.stable_key.0,
        source
    )
    .to_lowercase();
    output.push_str(&format!(
        "<tr data-kind=\"{}\" data-source=\"{}\" data-search=\"{}\" data-diagnostics=\"{}\" data-severities=\"{}\"><td>{}</td><td><a href=\"{}\">{}</a></td><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>\n",
        escape_html(&kind),
        escape_html(&source.to_lowercase()),
        escape_html(&search),
        diagnostic_count,
        escape_html(
            &diagnostic_severities
                .get(&entity.id.0)
                .map_or_else(String::new, |severities| severities.join(" "))
        ),
        escape_html(&kind),
        escape_html(&entity_report_path(entity)),
        escape_html(entity.title.as_deref().unwrap_or(&entity.name)),
        escape_html(&entity.stable_key.0),
        escape_html(&source),
        diagnostic_count
    ));
}

fn render_entity_page(
    snapshot: &str,
    payload: &HtmlReportProjectionPayload,
    projection_index: &CanonicalProjectionIndex<'_>,
    entity: &Entity,
) -> String {
    let relations = projection_index.relations(&entity.id);
    let facts = projection_index.facts(&entity.id);
    let diagnostics = projection_index.diagnostics(&entity.id);
    let source = entity.source.as_ref().map_or_else(String::new, |source| {
        source.line_start.map_or_else(
            || source.path.clone(),
            |line| format!("{}:{line}", source.path),
        )
    });
    let mut output = String::from(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
    );
    output.push_str(&format!(
        "<title>{} - Athanor entity</title>\n",
        escape_html(entity.title.as_deref().unwrap_or(&entity.name))
    ));
    output.push_str(STYLES);
    output.push_str("</head>\n<body>\n<main>\n<header>\n<p><a href=\"../index.html\">Back to report</a></p>\n<p class=\"eyebrow\">Athanor entity detail</p>\n");
    output.push_str(&format!(
        "<h1>{}</h1><p>Snapshot <code>{}</code></p>\n</header>\n",
        escape_html(entity.title.as_deref().unwrap_or(&entity.name)),
        escape_html(snapshot)
    ));
    output.push_str("<section><h2>Identity</h2><dl class=\"details\">");
    detail(&mut output, "Entity id", &entity.id.0);
    detail(&mut output, "Stable key", &entity.stable_key.0);
    detail(&mut output, "Kind", &serialized_name(&entity.kind));
    detail(&mut output, "Source", &source);
    detail(
        &mut output,
        "Language",
        entity.language.as_ref().map_or("", |language| &language.0),
    );
    if !entity.aliases.is_empty() {
        detail(&mut output, "Aliases", &entity.aliases.join(", "));
    }
    if !entity.ownership.is_empty() {
        let owners = entity
            .ownership
            .iter()
            .map(|ownership| ownership.source_file.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        detail(&mut output, "Ownership", &owners);
    }
    output.push_str("</dl></section>\n");
    render_entity_relations(&mut output, relations, entity, projection_index);
    render_entity_facts(&mut output, facts);
    render_entity_diagnostics(&mut output, diagnostics, &payload.entities);
    output.push_str("</main>\n</body>\n</html>\n");
    output
}

fn detail(output: &mut String, label: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    output.push_str(&format!(
        "<dt>{}</dt><dd><code>{}</code></dd>",
        escape_html(label),
        escape_html(value)
    ));
}

fn render_entity_relations(
    output: &mut String,
    relations: &[&Relation],
    current: &Entity,
    projection_index: &CanonicalProjectionIndex<'_>,
) {
    output.push_str("<section><h2>Relations</h2>");
    if relations.is_empty() {
        output.push_str("<p class=\"empty\">No relations attached to this entity.</p></section>\n");
        return;
    }
    output.push_str("<table><thead><tr><th>Relation id</th><th>Direction</th><th>Kind</th><th>Neighbor</th><th>Status</th><th>Evidence</th></tr></thead><tbody>");
    for relation in relations {
        let outgoing = relation.from == current.id;
        let neighbor_id = if outgoing {
            &relation.to
        } else {
            &relation.from
        };
        let neighbor = projection_index.entity(neighbor_id);
        output.push_str(&format!(
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{} ({:.2})</td><td><code>{}</code></td></tr>",
            escape_html(&relation.id.0),
            if outgoing { "outgoing" } else { "incoming" },
            escape_html(&serialized_name(&relation.kind)),
            linked_entity_label(neighbor, neighbor_id.0.as_str()),
            escape_html(&serialized_name(&relation.status)),
            relation.confidence,
            escape_html(&evidence_locations(&relation.evidence).join(", "))
        ));
    }
    output.push_str("</tbody></table></section>\n");
}

fn render_entity_facts(output: &mut String, facts: &[&athanor_domain::Fact]) {
    output.push_str("<section><h2>Facts</h2>");
    if facts.is_empty() {
        output.push_str("<p class=\"empty\">No facts attached to this entity.</p></section>\n");
        return;
    }
    output.push_str(
        "<table><thead><tr><th>Kind</th><th>Fact id</th><th>Evidence</th></tr></thead><tbody>",
    );
    for fact in facts {
        output.push_str(&format!(
            "<tr><td>{}</td><td><code>{}</code></td><td><code>{}</code></td></tr>",
            escape_html(&serialized_name(&fact.kind)),
            escape_html(&fact.id.0),
            escape_html(&evidence_locations(&fact.evidence).join(", "))
        ));
    }
    output.push_str("</tbody></table></section>\n");
}

fn render_entity_diagnostics(
    output: &mut String,
    diagnostics: &[&Diagnostic],
    entities: &[Entity],
) {
    output.push_str("<section><h2>Diagnostics</h2>");
    if diagnostics.is_empty() {
        output
            .push_str("<p class=\"empty\">No diagnostics attached to this entity.</p></section>\n");
        return;
    }
    for diagnostic in diagnostics {
        render_diagnostic(output, diagnostic, entities);
    }
    output.push_str("</section>\n");
}

fn linked_entity_label(entity: Option<&Entity>, fallback_id: &str) -> String {
    match entity {
        Some(entity) => format!(
            "<a href=\"{}\">{}</a><br><code>{}</code>",
            escape_html(&format!("../{}", entity_report_path(entity))),
            escape_html(entity.title.as_deref().unwrap_or(&entity.name)),
            escape_html(&entity.stable_key.0)
        ),
        None => format!("<code>{}</code>", escape_html(fallback_id)),
    }
}

fn entity_report_path(entity: &Entity) -> String {
    format!("entities/{}", entity_report_filename(entity))
}

fn entity_report_filename(entity: &Entity) -> String {
    format!("{}.html", safe_filename(&entity.id.0))
}

fn evidence_locations(evidence: &[athanor_domain::Evidence]) -> Vec<String> {
    evidence
        .iter()
        .filter_map(|evidence| {
            evidence.source_file.as_ref().map(|path| {
                evidence
                    .line_start
                    .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
            })
        })
        .collect()
}

fn entity_kinds(entities: &[Entity]) -> Vec<String> {
    let mut counts = BTreeMap::new();
    for entity in entities {
        counts.insert(serialized_name(&entity.kind), ());
    }
    counts.into_keys().collect()
}

fn diagnostic_severities(diagnostics: &[&Diagnostic]) -> Vec<String> {
    let mut counts = BTreeMap::new();
    for diagnostic in diagnostics {
        counts.insert(serialized_name(&diagnostic.severity), ());
    }
    counts.into_keys().collect()
}

fn relation_kind_counts(relations: &[Relation]) -> Vec<(String, usize)> {
    let mut counts = BTreeMap::<String, usize>::new();
    for relation in relations {
        *counts.entry(serialized_name(&relation.kind)).or_default() += 1;
    }
    let mut counts = counts.into_iter().collect::<Vec<_>>();
    counts.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    counts
}

#[derive(Debug, Clone, Copy)]
struct GraphHub<'a> {
    entity: &'a Entity,
    degree: usize,
}

#[derive(Debug, Serialize)]
struct GraphViewData {
    nodes: Vec<GraphViewNode>,
    edges: Vec<GraphViewEdge>,
    omitted_nodes: usize,
    omitted_edges: usize,
}

#[derive(Debug, Serialize)]
struct GraphViewNode {
    id: String,
    stable_key: String,
    kind: String,
    name: String,
    href: String,
    degree: usize,
}

#[derive(Debug, Serialize)]
struct GraphViewEdge {
    id: String,
    kind: String,
    from: String,
    to: String,
    evidence: Vec<String>,
}

fn graph_view_data(
    payload: &HtmlReportProjectionPayload,
    max_nodes: usize,
    max_edges: usize,
) -> GraphViewData {
    let mut degrees = HashMap::<&str, usize>::new();
    for relation in &payload.relations {
        *degrees.entry(relation.from.0.as_str()).or_default() += 1;
        *degrees.entry(relation.to.0.as_str()).or_default() += 1;
    }
    let mut selected = payload
        .entities
        .iter()
        .filter(|entity| degrees.contains_key(entity.id.0.as_str()))
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| {
        degrees
            .get(right.id.0.as_str())
            .unwrap_or(&0)
            .cmp(degrees.get(left.id.0.as_str()).unwrap_or(&0))
            .then_with(|| left.stable_key.0.cmp(&right.stable_key.0))
    });
    let connected_entities = selected.len();
    selected.truncate(max_nodes);
    let selected_ids = selected
        .iter()
        .map(|entity| entity.id.0.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let nodes = selected
        .into_iter()
        .map(|entity| GraphViewNode {
            id: entity.id.0.clone(),
            stable_key: entity.stable_key.0.clone(),
            kind: serialized_name(&entity.kind),
            name: entity.title.clone().unwrap_or_else(|| entity.name.clone()),
            href: entity_report_path(entity),
            degree: *degrees.get(entity.id.0.as_str()).unwrap_or(&0),
        })
        .collect::<Vec<_>>();

    let mut relations = payload
        .relations
        .iter()
        .filter(|relation| {
            selected_ids.contains(relation.from.0.as_str())
                && selected_ids.contains(relation.to.0.as_str())
        })
        .collect::<Vec<_>>();
    let selected_relations = relations.len();
    relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    relations.truncate(max_edges);
    let edges = relations
        .into_iter()
        .map(|relation| GraphViewEdge {
            id: relation.id.0.clone(),
            kind: serialized_name(&relation.kind),
            from: relation.from.0.clone(),
            to: relation.to.0.clone(),
            evidence: evidence_locations(&relation.evidence),
        })
        .collect();

    GraphViewData {
        nodes,
        edges,
        omitted_nodes: connected_entities.saturating_sub(max_nodes),
        omitted_edges: payload
            .relations
            .len()
            .saturating_sub(selected_relations.min(max_edges)),
    }
}

fn graph_hubs(payload: &HtmlReportProjectionPayload) -> Vec<GraphHub<'_>> {
    let mut degrees = HashMap::<&str, usize>::new();
    for relation in &payload.relations {
        *degrees.entry(relation.from.0.as_str()).or_default() += 1;
        *degrees.entry(relation.to.0.as_str()).or_default() += 1;
    }
    let mut hubs = payload
        .entities
        .iter()
        .filter_map(|entity| {
            degrees
                .get(entity.id.0.as_str())
                .copied()
                .filter(|degree| *degree > 0)
                .map(|degree| GraphHub { entity, degree })
        })
        .collect::<Vec<_>>();
    hubs.sort_by(|left, right| {
        right
            .degree
            .cmp(&left.degree)
            .then_with(|| left.entity.stable_key.0.cmp(&right.entity.stable_key.0))
    });
    hubs
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

fn diagnostic_severities_by_entity(diagnostics: &[&Diagnostic]) -> HashMap<String, Vec<String>> {
    let mut severities = HashMap::<String, Vec<String>>::new();
    for diagnostic in diagnostics {
        let severity = serialized_name(&diagnostic.severity);
        for entity in &diagnostic.entities {
            let entity_severities = severities.entry(entity.0.clone()).or_default();
            if !entity_severities.contains(&severity) {
                entity_severities.push(severity.clone());
            }
        }
    }
    for entity_severities in severities.values_mut() {
        entity_severities.sort();
    }
    severities
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
a { color: #93c5fd; text-decoration-color: #3b82f6; }
.eyebrow { color: #60a5fa; font-weight: 700; text-transform: uppercase; letter-spacing: .08em; }
.metrics { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 1rem; }
.metric, .diagnostic, .graph-grid article { background: #1f2937; border: 1px solid #374151; border-radius: .5rem; padding: 1rem; }
.metric strong { display: block; color: #93c5fd; font-size: 2rem; }
.metric span { color: #9ca3af; }
.graph-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 1rem; }
.graph-note { color: #9ca3af; }
.graph-controls { display: grid; grid-template-columns: 2fr 1.5fr auto; gap: 1rem; align-items: end; margin-bottom: 1rem; }
.graph-controls label { display: grid; gap: .35rem; color: #9ca3af; font-size: .9rem; }
.graph-buttons { display: flex; gap: .5rem; flex-wrap: wrap; }
button { border: 1px solid #374151; border-radius: .4rem; background: #1f2937; color: #e5e7eb; padding: .6rem .8rem; cursor: pointer; }
button:hover { border-color: #60a5fa; }
.graph-shell { display: grid; grid-template-columns: minmax(0, 3fr) minmax(220px, 1fr); gap: 1rem; }
#canonical-graph { width: 100%; min-height: 520px; border: 1px solid #374151; border-radius: .5rem; background: #0f172a; touch-action: none; }
.graph-edge { stroke: #475569; stroke-width: 1.5; opacity: .72; }
.graph-edge.is-dimmed { opacity: .08; }
.graph-node { cursor: grab; }
.graph-node circle { fill: #1d4ed8; stroke: #93c5fd; stroke-width: 1.5; }
.graph-node text { fill: #e5e7eb; font-size: 11px; pointer-events: none; }
.graph-node.is-dimmed { opacity: .14; }
.graph-node.is-selected circle { fill: #b45309; stroke: #fde68a; stroke-width: 3; }
.graph-selection { background: #1f2937; border: 1px solid #374151; border-radius: .5rem; padding: 1rem; overflow-wrap: anywhere; }
.graph-selection h3 { margin-top: 0; }
.graph-selection ul { padding-left: 1.2rem; }
.compact-list { list-style: none; padding: 0; margin: 0; }
.compact-list li { display: flex; justify-content: space-between; gap: 1rem; padding: .45rem 0; border-bottom: 1px solid #374151; }
.filters { display: grid; grid-template-columns: 2fr 1.5fr 1fr 1fr; gap: 1rem; margin: 1rem 0; }
.filters label { display: grid; gap: .35rem; color: #9ca3af; font-size: .9rem; }
input, select { width: 100%; box-sizing: border-box; border: 1px solid #374151; border-radius: .4rem; background: #0f172a; color: #e5e7eb; padding: .6rem; }
.diagnostic { margin: 1rem 0; }
.diagnostic-heading { display: flex; gap: .75rem; align-items: center; }
.diagnostic-heading h3 { margin: 0; }
.badge { border-radius: 999px; background: #374151; padding: .2rem .6rem; font-size: .8rem; text-transform: uppercase; }
.severity-critical { border-color: #ef4444; } .severity-high { border-color: #f97316; } .severity-medium { border-color: #eab308; }
.table-wrap { overflow-x: auto; }
table { width: 100%; border-collapse: collapse; background: #1f2937; }
th, td { text-align: left; vertical-align: top; padding: .65rem; border-bottom: 1px solid #374151; }
th { color: #93c5fd; position: sticky; top: 0; background: #1f2937; }
.details { display: grid; grid-template-columns: minmax(120px, 220px) 1fr; gap: .5rem 1rem; background: #1f2937; border: 1px solid #374151; border-radius: .5rem; padding: 1rem; }
.details dt { color: #9ca3af; }
.details dd { margin: 0; }
code { overflow-wrap: anywhere; color: #bfdbfe; }
.empty { color: #9ca3af; }
.is-hidden { display: none; }
@media (max-width: 760px) { .filters, .details, .graph-controls, .graph-shell { grid-template-columns: 1fr; } #canonical-graph { min-height: 420px; } }
</style>
"#;

const SCRIPT: &str = r#"<script>
(() => {
  const search = document.getElementById('entity-search');
  const source = document.getElementById('source-filter');
  const kind = document.getElementById('kind-filter');
  const severity = document.getElementById('severity-filter');
  const rows = Array.from(document.querySelectorAll('tbody tr[data-kind]'));
  const diagnostics = Array.from(document.querySelectorAll('.diagnostic[data-severity]'));
  function applyFilters() {
    const query = (search?.value || '').trim().toLowerCase();
    const sourceQuery = (source?.value || '').trim().toLowerCase();
    const selectedKind = kind?.value || '';
    const selectedSeverity = severity?.value || '';
    for (const row of rows) {
      const matchesQuery = !query || row.dataset.search.includes(query);
      const matchesSource = !sourceQuery || row.dataset.source.includes(sourceQuery);
      const matchesKind = !selectedKind || row.dataset.kind === selectedKind;
      const matchesSeverity = !selectedSeverity || row.dataset.severities.split(' ').includes(selectedSeverity);
      row.classList.toggle('is-hidden', !(matchesQuery && matchesSource && matchesKind && matchesSeverity));
    }
    for (const diagnostic of diagnostics) {
      const matchesSeverity = !selectedSeverity || diagnostic.dataset.severity === selectedSeverity;
      diagnostic.classList.toggle('is-hidden', !matchesSeverity);
    }
  }
  search?.addEventListener('input', applyFilters);
  source?.addEventListener('input', applyFilters);
  kind?.addEventListener('change', applyFilters);
  severity?.addEventListener('change', applyFilters);

  const graphDataElement = document.getElementById('canonical-graph-data');
  const graphSvg = document.getElementById('canonical-graph');
  if (graphDataElement && graphSvg) {
    const graph = JSON.parse(graphDataElement.textContent || '{"nodes":[],"edges":[]}');
    const viewport = document.getElementById('graph-viewport');
    const edgeLayer = document.getElementById('graph-edges');
    const nodeLayer = document.getElementById('graph-nodes');
    const selection = document.getElementById('graph-selection');
    const graphSearch = document.getElementById('graph-search');
    const relationKind = document.getElementById('graph-relation-kind');
    const nodeById = new Map(graph.nodes.map(node => [node.id, node]));
    const positions = new Map();
    const nodeElements = new Map();
    const edgeElements = [];
    let scale = 1;
    let translateX = 0;
    let translateY = 0;
    let dragged = null;

    const relationKinds = Array.from(new Set(graph.edges.map(edge => edge.kind))).sort();
    for (const kindName of relationKinds) {
      const option = document.createElement('option');
      option.value = kindName;
      option.textContent = kindName;
      relationKind?.appendChild(option);
    }

    function resetPositions() {
      const centerX = 500;
      const centerY = 310;
      const radius = Math.min(260, 120 + graph.nodes.length * 2);
      graph.nodes.forEach((node, index) => {
        const angle = (Math.PI * 2 * index / Math.max(graph.nodes.length, 1)) - Math.PI / 2;
        positions.set(node.id, {
          x: centerX + Math.cos(angle) * radius,
          y: centerY + Math.sin(angle) * radius
        });
      });
    }

    function svgElement(name, attributes = {}) {
      const element = document.createElementNS('http://www.w3.org/2000/svg', name);
      for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, value);
      return element;
    }

    function renderGraph() {
      edgeLayer.replaceChildren();
      nodeLayer.replaceChildren();
      edgeElements.length = 0;
      nodeElements.clear();
      for (const edge of graph.edges) {
        if (!nodeById.has(edge.from) || !nodeById.has(edge.to)) continue;
        const line = svgElement('line', { class: 'graph-edge', 'data-kind': edge.kind });
        edgeLayer.appendChild(line);
        edgeElements.push({ edge, line });
      }
      for (const node of graph.nodes) {
        const group = svgElement('g', { class: 'graph-node', tabindex: '0', role: 'link' });
        const circle = svgElement('circle', { r: String(Math.min(16, 6 + Math.sqrt(node.degree) * 1.8)) });
        const label = svgElement('text', { x: '10', y: '-10' });
        label.textContent = node.name.length > 26 ? `${node.name.slice(0, 24)}…` : node.name;
        group.append(circle, label);
        group.addEventListener('click', () => selectNode(node.id));
        group.addEventListener('dblclick', () => { window.location.href = node.href; });
        group.addEventListener('keydown', event => {
          if (event.key === 'Enter') window.location.href = node.href;
        });
        group.addEventListener('pointerdown', event => {
          dragged = node.id;
          graphSvg.setPointerCapture(event.pointerId);
          event.preventDefault();
        });
        nodeLayer.appendChild(group);
        nodeElements.set(node.id, group);
      }
      updateGraph();
    }

    function updateGraph() {
      viewport.setAttribute('transform', `translate(${translateX} ${translateY}) scale(${scale})`);
      for (const { edge, line } of edgeElements) {
        const from = positions.get(edge.from);
        const to = positions.get(edge.to);
        line.setAttribute('x1', from.x);
        line.setAttribute('y1', from.y);
        line.setAttribute('x2', to.x);
        line.setAttribute('y2', to.y);
      }
      for (const [id, element] of nodeElements) {
        const position = positions.get(id);
        element.setAttribute('transform', `translate(${position.x} ${position.y})`);
      }
      applyGraphFilters();
    }

    function applyGraphFilters() {
      const query = (graphSearch?.value || '').trim().toLowerCase();
      const selectedKind = relationKind?.value || '';
      const visibleNodeIds = new Set();
      for (const { edge, line } of edgeElements) {
        const visible = !selectedKind || edge.kind === selectedKind;
        line.classList.toggle('is-dimmed', !visible);
        if (visible) {
          visibleNodeIds.add(edge.from);
          visibleNodeIds.add(edge.to);
        }
      }
      for (const node of graph.nodes) {
        const matchesQuery = !query || `${node.name} ${node.stable_key} ${node.kind}`.toLowerCase().includes(query);
        const matchesRelation = !selectedKind || visibleNodeIds.has(node.id);
        nodeElements.get(node.id)?.classList.toggle('is-dimmed', !(matchesQuery && matchesRelation));
      }
    }

    function selectNode(id) {
      const node = nodeById.get(id);
      if (!node) return;
      for (const [nodeId, element] of nodeElements) element.classList.toggle('is-selected', nodeId === id);
      const attached = graph.edges.filter(edge => edge.from === id || edge.to === id);
      const relationItems = attached.slice(0, 20).map(edge => {
        const otherId = edge.from === id ? edge.to : edge.from;
        const other = nodeById.get(otherId);
        const direction = edge.from === id ? 'outgoing' : 'incoming';
        const evidence = edge.evidence.length ? ` — ${edge.evidence.join(', ')}` : '';
        return `<li><code>${escapeGraphHtml(edge.id)}</code> ${escapeGraphHtml(edge.kind)} (${direction}) → ${escapeGraphHtml(other?.name || otherId)}${escapeGraphHtml(evidence)}</li>`;
      }).join('');
      selection.innerHTML = `<h3>${escapeGraphHtml(node.name)}</h3><p><code>${escapeGraphHtml(node.stable_key)}</code></p><p>Kind: ${escapeGraphHtml(node.kind)} · Degree: ${node.degree}</p><p><a href="${escapeGraphHtml(node.href)}">Open entity detail</a></p>${relationItems ? `<h4>Direct relations</h4><ul>${relationItems}</ul>` : '<p class="empty">No displayed direct relations.</p>'}`;
    }

    function escapeGraphHtml(value) {
      return String(value).replace(/[&<>"']/g, character => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[character]));
    }

    graphSvg.addEventListener('pointermove', event => {
      if (!dragged) return;
      const rect = graphSvg.getBoundingClientRect();
      const x = ((event.clientX - rect.left) / rect.width * 1000 - translateX) / scale;
      const y = ((event.clientY - rect.top) / rect.height * 620 - translateY) / scale;
      positions.set(dragged, { x, y });
      updateGraph();
    });
    graphSvg.addEventListener('pointerup', () => { dragged = null; });
    graphSvg.addEventListener('pointercancel', () => { dragged = null; });
    graphSearch?.addEventListener('input', applyGraphFilters);
    relationKind?.addEventListener('change', applyGraphFilters);
    document.getElementById('graph-zoom-in')?.addEventListener('click', () => { scale = Math.min(2.5, scale * 1.2); updateGraph(); });
    document.getElementById('graph-zoom-out')?.addEventListener('click', () => { scale = Math.max(.45, scale / 1.2); updateGraph(); });
    document.getElementById('graph-reset')?.addEventListener('click', () => {
      scale = 1;
      translateX = 0;
      translateY = 0;
      resetPositions();
      updateGraph();
    });
    resetPositions();
    renderGraph();
  }
})();
</script>
"#;

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;

    use athanor_domain::{
        DiagnosticId, DiagnosticKind, EntityId, EntityKind, Relation, RelationId, RelationKind,
        RelationStatus, Severity, SnapshotId, StableKey,
    };

    use super::*;

    #[tokio::test]
    async fn cancellation_keeps_previous_report_published() {
        let root = std::env::temp_dir().join(format!(
            "athanor-html-projector-cancel-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("html");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("previous.html"), "previous").unwrap();
        let checks = Cell::new(0);

        let error = HtmlReportProjector
            .project_cancellable(
                ProjectInput {
                    snapshot: SnapshotId("snap_test".to_string()),
                    target: target.to_string_lossy().into_owned(),
                    payload: serde_json::to_value(HtmlReportProjectionPayload {
                        schema: HTML_REPORT_PROJECTION_SCHEMA.to_string(),
                        entities: Vec::new(),
                        facts: Vec::new(),
                        relations: Vec::new(),
                        diagnostics: Vec::new(),
                    })
                    .unwrap(),
                },
                || {
                    checks.set(checks.get() + 1);
                    checks.get() >= 4
                },
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("operation cancelled"));
        assert_eq!(
            fs::read_to_string(target.join("previous.html")).unwrap(),
            "previous"
        );
        assert!(!target.join("index.html").exists());
        fs::remove_dir_all(root).unwrap();
    }

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
            relations: vec![Relation {
                id: RelationId("rel_login".to_string()),
                kind: RelationKind::Documents,
                from: EntityId("ent_login".to_string()),
                to: EntityId("ent_login".to_string()),
                status: RelationStatus::Verified,
                confidence: 1.0,
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_test".to_string()),
                payload: json!({}),
            }],
            diagnostics: vec![Diagnostic {
                id: DiagnosticId("diag_login".to_string()),
                kind: DiagnosticKind::MissingDocumentation,
                severity: Severity::High,
                status: DiagnosticStatus::Open,
                title: "Missing docs".to_string(),
                message: "Document this endpoint".to_string(),
                entities: vec![EntityId("ent_login".to_string())],
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_test".to_string()),
                suggested_fix: None,
                payload: json!({}),
            }],
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
        assert!(html.contains("Graph summary"));
        assert!(html.contains("Interactive graph"));
        assert!(html.contains("id=\"canonical-graph\""));
        assert!(html.contains("id=\"canonical-graph-data\""));
        assert!(html.contains("id=\"graph-relation-kind\""));
        assert!(html.contains("\"id\":\"ent_login\""));
        assert!(html.contains("\"id\":\"rel_login\""));
        assert!(html.contains("id=\"source-filter\""));
        assert!(html.contains("data-severities=\"high\""));
        assert!(html.contains("<script>"));
        assert!(target.join("entities/ent_login.html").is_file());
        let entity_html = fs::read_to_string(target.join("entities/ent_login.html")).unwrap();
        assert!(entity_html.contains("Athanor entity detail"));
        assert!(entity_html.contains("rel_login"));
        assert!(target.join("manifest.json").is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bounds_interactive_graph_by_degree_and_relation_limit() {
        let center = Entity {
            id: EntityId("ent_center".to_string()),
            stable_key: StableKey("symbol://center".to_string()),
            kind: EntityKind::Function,
            name: "center".to_string(),
            title: None,
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        };
        let mut entities = vec![center.clone()];
        let mut relations = Vec::new();
        for index in 0..99 {
            let leaf = Entity {
                id: EntityId(format!("ent_leaf_{index:03}")),
                stable_key: StableKey(format!("symbol://leaf/{index:03}")),
                kind: EntityKind::Function,
                name: format!("leaf {index:03}"),
                title: None,
                source: None,
                language: None,
                aliases: Vec::new(),
                ownership: Vec::new(),
                payload: json!({}),
            };
            relations.push(Relation {
                id: RelationId(format!("rel_{index:03}")),
                kind: RelationKind::Calls,
                from: leaf.id.clone(),
                to: center.id.clone(),
                status: RelationStatus::Verified,
                confidence: 1.0,
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_test".to_string()),
                payload: json!({}),
            });
            entities.push(leaf);
        }
        let payload = HtmlReportProjectionPayload {
            schema: HTML_REPORT_PROJECTION_SCHEMA.to_string(),
            entities,
            facts: Vec::new(),
            relations,
            diagnostics: Vec::new(),
        };

        let graph = graph_view_data(&payload, 80, 40);

        assert_eq!(graph.nodes.len(), 80);
        assert_eq!(graph.nodes[0].id, "ent_center");
        assert_eq!(graph.edges.len(), 40);
        assert_eq!(graph.omitted_nodes, 20);
        assert_eq!(graph.omitted_edges, 59);
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
