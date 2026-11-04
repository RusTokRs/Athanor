//! Browser presentation of an existing bounded explore report.

use athanor_app::ExploreReport;

use super::{describe_source, escape, page, url_encode};

/// Renders the explore report page.
pub(crate) fn explore_page(report: &ExploreReport) -> String {
    let anchor = report
        .anchor
        .as_deref()
        .filter(|anchor| !anchor.is_empty())
        .unwrap_or(report.query.as_str());
    let mut body = String::new();
    body.push_str("<p class=\"crumb\"><a href=\"/\">documentation</a> / explore</p>");
    body.push_str(&format!(
        "<h1>Explore &ldquo;{}&rdquo;</h1>",
        escape(anchor)
    ));
    body.push_str(&format!(
        "<dl class=\"meta\">\
         <dt>Snapshot</dt><dd><code>{snapshot}</code></dd>\
         <dt>Mode</dt><dd>{mode}</dd>\
         <dt>Limits</dt><dd>entries {entries}, links {links}, depth {depth}, paths {paths}</dd>\
         </dl>",
        snapshot = escape(&report.snapshot),
        mode = escape(&report.mode.to_string()),
        entries = report.limits.entries,
        links = report.limits.links,
        depth = report.limits.depth,
        paths = report.limits.paths
    ));
    body.push_str("<p class=\"hint\">Graph claims and citations belong to the snapshot above. Source excerpts read the current working tree and are not an exact-snapshot source checkout.</p>");
    body.push_str(&blast_section(report));

    if report.entries.is_empty() {
        body.push_str(
            "<p>No canonical entity matched this request. Search may find a starting point first.</p>",
        );
    }
    for entry in &report.entries {
        body.push_str(&format!(
            "<article class=\"entry\"><h2><span class=\"score\">{score:.4}</span> {name} <span class=\"kind\">{kind}</span></h2>\
             <p class=\"source\"><code>{stable_key}</code>{source}{diagnostics}</p>",
            score = entry.score,
            name = escape(&entry.name),
            kind = escape(&entry.kind),
            stable_key = escape(&entry.stable_key),
            source = match &entry.source {
                Some(source) => format!(
                    " &middot; {}",
                    escape(&describe_source(&source.path, source.line_start))
                ),
                None => String::new(),
            },
            diagnostics = if entry.open_diagnostics > 0 {
                format!(" &middot; {} open diagnostic(s)", entry.open_diagnostics)
            } else {
                String::new()
            }
        ));
        if !entry.citations.is_empty() {
            body.push_str("<p class=\"citations\">Cites: ");
            body.push_str(
                &entry
                    .citations
                    .iter()
                    .map(|citation| match &citation.source_file {
                        Some(path) => escape(&describe_source(path, citation.line_start)),
                        None => "unlocated evidence".to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            body.push_str("</p>");
        }
        if let Some(excerpt) = &entry.excerpt {
            body.push_str(&excerpt_block(excerpt));
        }
        for (label, links) in [
            ("Reaches", &entry.outgoing),
            ("Reached by", &entry.incoming),
        ] {
            if links.is_empty() {
                continue;
            }
            body.push_str(&format!("<h3>{label}</h3><ul class=\"links\">"));
            for link in links {
                body.push_str(&format!(
                    "<li><span class=\"relation\">{}</span> <a href=\"/explore?target={}\">{}</a> <span class=\"kind\">{}</span> <span class=\"status\">{}</span> <span class=\"confidence\">{:.2}</span></li>",
                    escape(&link.relation),
                    url_encode(&link.counterpart.stable_key),
                    escape(&link.counterpart.name),
                    escape(&link.counterpart.kind),
                    escape(&link.status),
                    link.confidence
                ));
            }
            body.push_str("</ul>");
        }
        body.push_str("</article>");
    }

    if !report.paths.is_empty() {
        body.push_str("<section><h2>Relation chains</h2>");
        for path in &report.paths {
            body.push_str(&format!(
                "<article class=\"chain\"><h3>{} &middot; depth {}</h3><ol>",
                escape(&path.target.name),
                path.depth
            ));
            for step in &path.steps {
                body.push_str(&format!(
                    "<li><code>{from}</code> <span class=\"relation\">{relation}</span> <span class=\"status\">&rarr; {status}</span> <code>{to}</code></li>",
                    from = escape(&step.from.name),
                    relation = escape(&step.relation),
                    status = escape(&step.status),
                    to = escape(&step.to.name)
                ));
            }
            body.push_str("</ol>");
            if let Some(excerpt) = &path.excerpt {
                body.push_str(&excerpt_block(excerpt));
            }
            body.push_str("</article>");
        }
        body.push_str("</section>");
    }

    body.push_str(&omissions_section(report));
    body.push_str(&format!(
        "<p class=\"actions\"><a href=\"/api/explore?q={query}&amp;target={target}&amp;mode={mode}&amp;depth={depth}\">JSON report</a></p>",
        query = url_encode(&report.query),
        target = report.anchor.as_deref().map(url_encode).unwrap_or_default(),
        mode = url_encode(&report.mode.to_string()),
        depth = report.limits.depth
    ));
    page("Explore", &body)
}

fn blast_section(report: &ExploreReport) -> String {
    let mut body = String::from("<section class=\"blast\"><h2>Blast radius</h2>");
    body.push_str(&format!(
        "<p>Within {} hop(s): {} entr(ies), {} reached entities, {} relations, {} file(s).</p>",
        report.blast.depth,
        report.blast.entries,
        report.blast.reached,
        report.blast.relations,
        report.blast.files.len()
    ));
    if !report.blast.by_depth.is_empty() {
        body.push_str("<p class=\"depth\">");
        body.push_str(
            &report
                .blast
                .by_depth
                .iter()
                .map(|depth| format!("hop {}: {} new", depth.depth, depth.entities))
                .collect::<Vec<_>>()
                .join(" &middot; "),
        );
        body.push_str("</p>");
    }
    if !report.blast.files.is_empty() {
        body.push_str("<ul class=\"files\">");
        for file in &report.blast.files {
            body.push_str(&format!("<li><code>{}</code></li>", escape(file)));
        }
        body.push_str("</ul>");
    }
    if report.blast.files_omitted > 0 {
        body.push_str(&format!("<p class=\"notice\">{} additional file(s) are omitted by the blast-radius file-list bound.</p>", report.blast.files_omitted));
    }
    body.push_str(&format!(
        "<p class=\"hint\">Expanded {} relation edges; {} open diagnostics in the reached set.</p>",
        report.blast.edge_visits, report.blast.open_diagnostics
    ));
    body.push_str("</section>");
    body
}

fn excerpt_block(excerpt: &athanor_app::ExploreExcerpt) -> String {
    format!(
        "<pre class=\"excerpt\"><code>{}</code></pre><p class=\"source\">{}:{}-{}{}</p>",
        escape(&excerpt.text),
        escape(&excerpt.source_file),
        excerpt.line_start,
        excerpt.line_end,
        if excerpt.truncated {
            " (truncated)"
        } else {
            ""
        }
    )
}

fn omissions_section(report: &ExploreReport) -> String {
    let omitted = &report.omitted;
    let mut body = String::new();
    if omitted.notes.is_empty()
        && omitted.entries_unresolved == 0
        && omitted.entries_over_budget == 0
        && omitted.links_truncated == 0
        && omitted.paths_truncated == 0
        && omitted.source_unread == 0
    {
        return body;
    }
    body.push_str("<section><h2>Omitted</h2><ul class=\"omitted\">");
    for (label, count) in [
        ("unresolved entries", omitted.entries_unresolved),
        ("entries over budget", omitted.entries_over_budget),
        ("truncated links", omitted.links_truncated),
        ("truncated chains", omitted.paths_truncated),
        ("unreadable source excerpts", omitted.source_unread),
    ] {
        if count > 0 {
            body.push_str(&format!("<li>{count} {label}</li>"));
        }
    }
    for note in &omitted.notes {
        body.push_str(&format!("<li>{}</li>", escape(note)));
    }
    body.push_str("</ul></section>");
    body
}
