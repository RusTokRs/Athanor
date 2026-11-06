//! Deterministic HTML rendering for the read-only documentation server.
//!
//! Every page is a self-contained document: inline CSS, no script, no external asset, and no
//! value that reaches the output without escaping. Markdown bodies are rendered with raw HTML
//! suppressed, so a published document cannot inject markup or script into the reader.

use athanor_app::SearchReport;

use super::catalog::{
    Catalog, CatalogReport, DocumentReport, Generation, GenerationReport, ValidationSummary,
    profile_label,
};

#[path = "render_explore.rs"]
mod explore;

pub(super) use super::markdown::to_html as markdown_to_html;
pub(super) use explore::explore_page;

/// One document of a generation, ready to render.
pub(super) struct DocumentView<'a> {
    pub(super) report: &'a DocumentReport,
    /// Rendered Markdown body, when the document is Markdown and its checksum matched.
    pub(super) html: Option<String>,
    /// Why the body could not be served, when it could not.
    pub(super) error: Option<String>,
}

/// Renders the catalog index page.
pub(super) fn index_page(
    catalog: &CatalogReport,
    search_limit: usize,
    explore_depth: usize,
) -> String {
    let mut body = String::new();
    body.push_str("<h1>Athanor documentation</h1>");
    body.push_str(&format!(
        "<p class=\"lede\">Read-only reader over published documentation generations for <code>{}</code>.</p>",
        escape(&catalog.root.display().to_string())
    ));
    body.push_str(&format!(
        "<dl class=\"meta\"><dt>Documentation root</dt><dd><code>{}</code></dd><dt>Generations</dt><dd>{}</dd><dt>Current</dt><dd>{}</dd></dl>",
        escape(&catalog.documentation_root.display().to_string()),
        catalog.generations.len(),
        match catalog.current.as_deref() {
            Some(current) => format!("<code>{}</code>", escape(current)),
            None => "none".to_string(),
        }
    ));
    if let Some(error) = catalog.current_error.as_deref() {
        body.push_str(&format!(
            "<p class=\"notice\">Current pointer: {}</p>",
            escape(error)
        ));
    }

    if catalog.omitted_generations > 0 {
        body.push_str(&format!(
                "<p class=\"notice\">{} generation candidate(s) are outside this reader's inspection bound and are not listed.</p>",
                catalog.omitted_generations
            ));
    }

    if catalog.generations.is_empty() {
        body.push_str(
            "<section class=\"empty\"><h2>No published generations yet</h2>\
             <p>This server only reads immutable generations. Publish one from the project root:</p>\
             <pre><code>ath index .\nath docs generate-architecture . --snapshot &lt;exact-snapshot&gt;</code></pre>\
             <p>The page refreshes on every request, so a new generation appears without restarting the server.</p></section>",
        );
    } else {
        body.push_str("<section><h2>Generations</h2>");
        body.push_str(
            "<table><thead><tr><th>Generation</th><th>Profile</th><th>Snapshot</th><th>Documents</th><th>Validation</th><th>Status</th></tr></thead><tbody>",
        );
        for generation in &catalog.generations {
            body.push_str(&generation_row(generation));
        }
        body.push_str("</tbody></table></section>");
    }

    if !catalog.invalid_generations.is_empty() {
        body.push_str(
            "<section><h2>Rejected generations</h2>\
             <p>A directory that does not validate is listed here and is never served.</p><ul>",
        );
        for invalid in &catalog.invalid_generations {
            body.push_str(&format!(
                "<li><code>{}</code> &mdash; {}</li>",
                escape(&invalid.generation),
                escape(&invalid.reason)
            ));
        }
        body.push_str("</ul></section>");
    }

    body.push_str(&format!(
        "<section><h2>Search the knowledge base</h2>\
         <form method=\"get\" action=\"/search\" class=\"form\">\
         <label>Query <input type=\"search\" name=\"q\" required autofocus placeholder=\"store load snapshot\"></label>\
         <label>Mode <select name=\"mode\"><option value=\"lexical\">lexical</option><option value=\"semantic\">semantic</option><option value=\"hybrid\">hybrid</option></select></label>\
         <label>Limit <input type=\"number\" name=\"limit\" min=\"1\" max=\"50\" value=\"{search_limit}\"></label>\
         <button type=\"submit\">Search</button></form>\
         <p class=\"hint\">Search reads the latest canonical snapshot, not the generations above.</p></section>"
    ));

    body.push_str(&format!(
        "<section><h2>Explore one question</h2>\
         <form method=\"get\" action=\"/explore\" class=\"form\">\
         <label>Query <input type=\"search\" name=\"q\" placeholder=\"how does the index pipeline publish\"></label>\
         <label>Or exact target <input type=\"text\" name=\"target\" placeholder=\"src/lib.rs or symbol://rust:crate::store\"></label>\
         <label>Depth <input type=\"number\" name=\"depth\" min=\"1\" max=\"4\" value=\"{explore_depth}\"></label>\
         <button type=\"submit\">Explore</button></form>\
         <p class=\"hint\">One bounded answer with citations, source excerpts, relation chains, and blast radius.</p></section>",
    ));

    page("Athanor documentation", &body)
}

fn generation_row(generation: &GenerationReport) -> String {
    let validation = generation
        .validation
        .as_ref()
        .map(|validation| {
            format!(
                "{} ({})",
                escape(&validation.status),
                validation.diagnostics
            )
        })
        .unwrap_or_else(|| "absent".to_string());
    format!(
        "<tr><td><a href=\"/g/{href}/\"><code>{id}</code></a>{current}</td><td>{profile}</td><td><code>{snapshot}</code></td><td>{documents}</td><td>{validation}</td><td>{status}</td></tr>",
        href = url_encode(&generation.generation),
        id = escape(&generation.generation),
        current = if generation.current {
            " <span class=\"badge\">current</span>"
        } else {
            ""
        },
        profile = escape(&generation.profile),
        snapshot = escape(&generation.snapshot),
        documents = generation.documents.len(),
        validation = validation,
        status = escape(&generation.status)
    )
}

/// Renders one generation page with every declared document.
pub(super) fn generation_page(
    catalog: &Catalog,
    generation: &Generation,
    documents: &[DocumentView<'_>],
) -> String {
    let manifest = &generation.manifest;
    let mut body = String::new();
    body.push_str(&format!(
        "<p class=\"crumb\"><a href=\"/\">documentation</a> / <code>{}</code></p>",
        escape(&generation.id)
    ));
    body.push_str(&format!(
        "<h1>{} documentation</h1>",
        escape(profile_label(manifest.profile))
    ));
    body.push_str(&format!(
        "<dl class=\"meta\">\
         <dt>Generation</dt><dd><code>{generation}</code>{current}</dd>\
         <dt>Snapshot</dt><dd><code>{snapshot}</code></dd>\
         <dt>Status</dt><dd>{status}</dd>\
         <dt>Effective limits</dt><dd>entities {entities}, facts {facts}, relations {relations}, diagnostics {diagnostics}</dd>\
         <dt>Omitted</dt><dd>entities {omitted_entities}, facts {omitted_facts}, relations {omitted_relations}, diagnostics {omitted_diagnostics}</dd>\
         </dl>",
        generation = escape(&generation.id),
        current = if generation.current {
            " <span class=\"badge\">current</span>"
        } else {
            ""
        },
        snapshot = escape(&manifest.snapshot),
        status = escape(&generation_status_label(manifest.status)),
        entities = manifest.effective_limits.max_entities,
        facts = manifest.effective_limits.max_facts,
        relations = manifest.effective_limits.max_relations,
        diagnostics = manifest.effective_limits.max_diagnostics,
        omitted_entities = manifest.omitted.entities,
        omitted_facts = manifest.omitted.facts,
        omitted_relations = manifest.omitted.relations,
        omitted_diagnostics = manifest.omitted.diagnostics
    ));
    body.push_str(&validation_note(generation.validation.as_ref()));
    body.push_str(&format!(
        "<p class=\"actions\"><a href=\"/g/{href}/manifest.json\">manifest.json</a> &middot; <a href=\"/api/generations\">catalog JSON</a> &middot; source root <code>{root}</code></p>",
        href = url_encode(&generation.id),
        root = escape(&catalog.root().display().to_string())
    ));
    body.push_str(&format!(
        "<p class=\"hint\">Produced from the canonical snapshot above by the deterministic {} profile. Every claim in these documents cites a canonical fact or evidence range.</p>",
        escape(profile_label(manifest.profile))
    ));

    for document in documents {
        body.push_str(&format!(
            "<article class=\"document\" id=\"{anchor}\"><header><h2><code>{path}</code></h2>\
             <p class=\"doc-meta\">{media_type} &middot; sha256 <code>{sha256}</code> &middot; <a href=\"/g/{generation}/raw/{raw_path}\">raw</a></p></header>",
            anchor = escape(&anchor_for(&document.report.path)),
            path = escape(&document.report.path),
            raw_path = url_path(&document.report.path),
            media_type = escape(&document.report.media_type),
            sha256 = escape(short_sha(&document.report.sha256)),
            generation = url_encode(&generation.id)
        ));
        match (&document.html, &document.error) {
            (Some(html), _) => body.push_str(html),
            (None, Some(error)) => body.push_str(&format!(
                "<p class=\"notice\">This document could not be served: {}</p>",
                escape(error)
            )),
            (None, None) => body.push_str(&format!(
                "<p class=\"notice\">{} is declared but has no renderable body. Read it as <a href=\"/g/{}/raw/{}\">raw</a>.</p>",
                escape(&document.report.media_type),
                url_encode(&generation.id),
                url_path(&document.report.path)
            )),
        }
        body.push_str("</article>");
    }

    page(
        &format!("{} documentation", profile_label(manifest.profile)),
        &body,
    )
}

fn validation_note(validation: Option<&ValidationSummary>) -> String {
    match validation {
        Some(validation) => format!(
            "<p class=\"{class}\">Validation report: <strong>{status}</strong> with {diagnostics} diagnostic(s).</p>",
            class = if validation.status == "valid" { "ok" } else { "notice" },
            status = escape(&validation.status),
            diagnostics = validation.diagnostics
        ),
        None => "<p class=\"notice\">No matching validation report is published with this generation.</p>".to_string(),
    }
}

/// Renders the search results page.
pub(super) fn search_page(report: &SearchReport) -> String {
    let mut body = String::new();
    body.push_str("<p class=\"crumb\"><a href=\"/\">documentation</a> / search</p>");
    body.push_str(&format!(
        "<h1>Search &ldquo;{}&rdquo;</h1>",
        escape(&report.query)
    ));
    body.push_str(&search_form(
        &report.query,
        &report.mode.to_string(),
        report.limit,
    ));
    body.push_str(&format!(
        "<dl class=\"meta\"><dt>Snapshot</dt><dd><code>{}</code></dd><dt>Mode</dt><dd>{}</dd><dt>Returned</dt><dd>{} of limit {}</dd></dl>",
        escape(&report.snapshot),
        escape(&report.mode.to_string()),
        report.returned,
        report.limit
    ));
    if report.truncated {
        body.push_str(&format!(
            "<p class=\"notice\">Results are truncated: at least {} more candidate(s) were not returned.</p>",
            report.omitted.results_lower_bound
        ));
    }
    if let Some(reason) = report.omitted.reason.as_deref() {
        body.push_str(&format!(
            "<p class=\"notice\">Omission note: {}</p>",
            escape(reason)
        ));
    }
    if report.results.is_empty() {
        body.push_str(
            "<p>No canonical entity matched this query. Try another term, or a different mode.</p>",
        );
    } else {
        body.push_str("<ol class=\"results\">");
        for item in &report.results {
            let source = match &item.source {
                Some(source) => describe_source(&source.path, source.line_start),
                None => "no source anchor".to_string(),
            };
            body.push_str(&format!(
                "<li><span class=\"score\">{score:.4}</span> <a href=\"/explore?target={target}\">{name}</a> <span class=\"kind\">{kind}</span><br><code>{stable_key}</code><br><span class=\"source\">{source}</span></li>",
                score = item.score,
                target = url_encode(&item.stable_key),
                name = escape(item.title.as_deref().unwrap_or(&item.name)),
                kind = escape(&item.kind),
                stable_key = escape(&item.stable_key),
                source = escape(&source)
            ));
        }
        body.push_str("</ol>");
    }
    body.push_str(&format!(
        "<p class=\"actions\"><a href=\"/api/search?q={query}&amp;mode={mode}&amp;limit={limit}\">JSON report</a></p>",
        query = url_encode(&report.query),
        mode = url_encode(&report.mode.to_string()),
        limit = report.limit
    ));
    page("Search", &body)
}

fn search_form(query: &str, mode: &str, limit: usize) -> String {
    let option = |value: &str| {
        if value == mode {
            format!("<option value=\"{value}\" selected>{value}</option>")
        } else {
            format!("<option value=\"{value}\">{value}</option>")
        }
    };
    format!(
        "<form method=\"get\" action=\"/search\" class=\"form\">\
         <label>Query <input type=\"search\" name=\"q\" required value=\"{query}\"></label>\
         <label>Mode <select name=\"mode\">{lexical}{semantic}{hybrid}</select></label>\
         <label>Limit <input type=\"number\" name=\"limit\" min=\"1\" max=\"50\" value=\"{limit}\"></label>\
         <button type=\"submit\">Search</button></form>",
        query = escape(query),
        lexical = option("lexical"),
        semantic = option("semantic"),
        hybrid = option("hybrid"),
        limit = limit
    )
}

/// Renders a bounded error page.
pub(super) fn error_page(status: u16, title: &str, message: &str, hint: Option<&str>) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "<h1>{} {}</h1><p>{}</p>",
        status,
        escape(title),
        escape(message)
    ));
    if let Some(hint) = hint {
        body.push_str(&format!("<p class=\"hint\">{}</p>", escape(hint)));
    }
    body.push_str("<p class=\"actions\"><a href=\"/\">Back to the catalog</a></p>");
    page(title, &body)
}

fn page(title: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title} &middot; Athanor</title><style>{STYLES}</style></head>\
         <body><header class=\"site\"><a href=\"/\">Athanor</a><span>read-only documentation reader</span></header>\
         <main>{body}</main>\
         <footer>athanor.documentation_serve.v1 &middot; loopback only &middot; published docs + bounded live queries</footer>\
         </body></html>",
        title = escape(title)
    )
}

fn generation_status_label(status: athanor_app::DocumentationGenerationStatus) -> String {
    // The v1 contract has one complete status; keep the label derived rather than hard-coded.
    match status {
        athanor_app::DocumentationGenerationStatus::Complete => "complete".to_string(),
    }
}

fn describe_source(path: &str, line_start: Option<u32>) -> String {
    match line_start {
        Some(line) => format!("{path}:{line}"),
        None => path.to_string(),
    }
}

fn anchor_for(path: &str) -> String {
    path.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn short_sha(sha256: &str) -> &str {
    sha256.get(..16).unwrap_or(sha256)
}

/// Escapes text for HTML text and attribute positions.
pub(super) fn escape(value: &str) -> String {
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

fn url_path(path: &str) -> String {
    path.split('/')
        .map(url_encode)
        .collect::<Vec<_>>()
        .join("/")
}

/// Percent-encodes a value for a query string, leaving only RFC 3986 unreserved bytes.
pub(super) fn url_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

const STYLES: &str = r#"
:root { color-scheme: dark; font-family: system-ui, -apple-system, "Segoe UI", sans-serif; }
body { margin: 0; background: #0b1220; color: #e5e7eb; line-height: 1.55; }
header.site { display: flex; gap: .75rem; align-items: baseline; padding: 1rem 2rem; border-bottom: 1px solid #1f2937; }
header.site a { color: #93c5fd; font-weight: 700; text-decoration: none; letter-spacing: .02em; }
header.site span { color: #6b7280; font-size: .85rem; }
main { width: min(1100px, calc(100% - 3rem)); margin: 0 auto; padding: 2rem 0 4rem; }
footer { padding: 1.25rem 2rem 2rem; color: #6b7280; font-size: .8rem; border-top: 1px solid #1f2937; }
h1 { font-size: 1.7rem; margin: 0 0 .75rem; color: #f9fafb; }
h2 { font-size: 1.15rem; margin: 1.75rem 0 .6rem; color: #f3f4f6; }
h3 { font-size: 1rem; margin: 1.1rem 0 .4rem; color: #d1d5db; }
a { color: #93c5fd; text-decoration-color: #1d4ed8; }
code, pre { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .88rem; }
code { color: #bfdbfe; }
pre { background: #0f172a; border: 1px solid #1f2937; border-radius: .5rem; padding: .9rem; overflow-x: auto; }
pre code { color: #dbeafe; }
.lede { color: #9ca3af; margin-top: 0; }
.crumb { color: #6b7280; font-size: .85rem; margin: 0 0 .5rem; }
.meta { display: grid; grid-template-columns: max-content 1fr; gap: .3rem 1.25rem; background: #0f172a; border: 1px solid #1f2937; border-radius: .5rem; padding: .9rem 1rem; margin: 1rem 0; }
.meta dt { color: #9ca3af; font-size: .85rem; }
.meta dd { margin: 0; }
table { width: 100%; border-collapse: collapse; margin: .5rem 0 1rem; font-size: .9rem; }
th, td { text-align: left; padding: .5rem .6rem; border-bottom: 1px solid #1f2937; }
th { color: #9ca3af; font-weight: 600; font-size: .8rem; text-transform: uppercase; letter-spacing: .04em; }
.badge { background: #1d4ed8; color: #eff6ff; border-radius: 999px; padding: .1rem .5rem; font-size: .7rem; text-transform: uppercase; }
.notice { border-left: 3px solid #b45309; background: #1c1917; padding: .6rem .9rem; color: #fde68a; }
.ok { border-left: 3px solid #15803d; background: #052e16; padding: .6rem .9rem; color: #bbf7d0; }
.hint { color: #9ca3af; font-size: .88rem; }
.actions { color: #9ca3af; font-size: .9rem; }
.empty { border: 1px dashed #374151; border-radius: .5rem; padding: 1rem 1.25rem; }
.form { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: .75rem; align-items: end; margin: .75rem 0 1rem; }
.form label { display: grid; gap: .3rem; color: #9ca3af; font-size: .8rem; }
input, select { width: 100%; box-sizing: border-box; background: #0f172a; color: #e5e7eb; border: 1px solid #374151; border-radius: .4rem; padding: .5rem .6rem; }
button { background: #1d4ed8; color: #eff6ff; border: 0; border-radius: .4rem; padding: .6rem 1rem; cursor: pointer; }
button:hover { background: #2563eb; }
.results { padding-left: 1.2rem; }
.results li { margin-bottom: .9rem; }
.score { color: #34d399; font-variant-numeric: tabular-nums; font-size: .85rem; }
.kind, .status, .confidence { color: #9ca3af; font-size: .8rem; border: 1px solid #374151; border-radius: 999px; padding: .05rem .45rem; }
.relation { color: #fbbf24; font-size: .85rem; }
.source { color: #6b7280; font-size: .82rem; }
.entry, .document, .chain { border-top: 1px solid #1f2937; padding-top: .5rem; margin-top: 1.75rem; }
.entry h2, .document h2 { margin-top: 0; }
.doc-meta { color: #6b7280; font-size: .8rem; margin: 0 0 1rem; }
.citations { color: #9ca3af; font-size: .85rem; }
.links { padding-left: 1.2rem; }
.links li { margin-bottom: .35rem; }
.excerpt { margin: .5rem 0 .25rem; }
.blast { background: #0f172a; border: 1px solid #1f2937; border-radius: .5rem; padding: .9rem 1.1rem; }
.blast h2 { margin-top: 0; }
.files, .omitted { padding-left: 1.2rem; }
.depth { color: #9ca3af; font-size: .85rem; }
.document h1, .document h2, .document h3 { scroll-margin-top: 1rem; }
.document blockquote { border-left: 3px solid #374151; margin-left: 0; padding-left: 1rem; color: #cbd5e1; }
.document img { max-width: 100%; }
.document table { font-size: .85rem; }
.evidence-panel { border: 1px solid #374151; border-radius: .5rem; padding: .7rem 1rem; margin-top: 1.5rem; }
.evidence-panel h3 { margin-top: 0; }
.footnote-definition { margin: .8rem 0; overflow-wrap: anywhere; }
@media (max-width: 700px) { .meta { display: block; } .meta dd { margin-bottom: .5rem; } table { display: block; overflow-x: auto; } }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppresses_raw_html_in_markdown_bodies() {
        let rendered = markdown_to_html("# Title\n\n<script>alert(1)</script>\n\nfine\n");
        assert!(!rendered.contains("<script>"));
        assert!(rendered.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(rendered.contains("<h1>Title</h1>"));
    }

    #[test]
    fn renders_tables_and_code_fences() {
        let rendered =
            markdown_to_html("| a | b |\n| - | - |\n| 1 | 2 |\n\n```rust\nlet x = 1;\n```\n");
        assert!(rendered.contains("<table>"));
        assert!(rendered.contains("language-rust"));
    }

    #[test]
    fn escapes_every_attribute_breaking_character() {
        assert_eq!(
            escape("<a href=\"x\">'&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;"
        );
    }

    #[test]
    fn percent_encodes_query_values() {
        assert_eq!(
            url_encode("symbol://rust:crate::store"),
            "symbol%3A%2F%2Frust%3Acrate%3A%3Astore"
        );
        assert_eq!(url_encode("a b+c"), "a%20b%2Bc");
        assert_eq!(url_encode("AZaz09-._~"), "AZaz09-._~");
    }

    #[test]
    fn renders_an_empty_catalog_without_a_pointer() {
        let report = CatalogReport {
            schema: "athanor.documentation_serve_catalog.v1".to_string(),
            root: "/repo".into(),
            documentation_root: "/repo/.athanor/generated/documentation".into(),
            current: None,
            current_error: None,
            limits: super::super::catalog::CatalogLimits {
                max_generations: 200,
                max_document_bytes: 4 * 1024 * 1024,
            },
            returned: 0,
            omitted_generations: 0,
            generations: Vec::new(),
            invalid_generations: Vec::new(),
        };
        let page = index_page(&report, 10, 2);
        assert!(page.contains("No published generations yet"));
        assert!(page.contains("/repo/.athanor/generated/documentation"));
        assert!(page.contains("value=\"10\""));
    }
}
