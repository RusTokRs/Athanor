//! Script-free Markdown with a separate, linked evidence panel.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

use super::render::{escape, url_encode};

/// Renders published Markdown without trusting its raw HTML, URL schemes, or image resources.
/// Footnotes keep their original anchors and appear in a distinct evidence panel. Looking up a
/// cited stable key is explicitly a latest-snapshot query, not a claim about the published snapshot.
pub(super) fn to_html(source: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let mut body = Vec::new();
    let mut citations = Vec::new();
    let mut in_citation = false;
    let mut count = 0usize;
    for event in Parser::new_ext(source, options) {
        if matches!(&event, Event::Start(Tag::FootnoteDefinition(_))) {
            in_citation = true;
            count += 1;
        }
        let ends_citation = matches!(&event, Event::End(TagEnd::FootnoteDefinition));
        let event = sanitize_event(event);
        if in_citation {
            citations.push(match event {
                Event::Text(text) if text.contains(" — ") => {
                    Event::InlineHtml(citation_html(&text).into())
                }
                other => other,
            });
        } else {
            body.push(event);
        }
        if ends_citation {
            in_citation = false;
        }
    }
    if count > 0 {
        // Keep one renderer for both sections: it assigns footnote numbers in first-reference
        // order, which can differ from the sorted definition order in published Markdown.
        body.push(Event::Html(format!(
            "<aside class=\"evidence-panel\"><h3>Evidence and citations ({count})</h3>\
             <p class=\"hint\">Citations belong to this published snapshot. Stable-key links open the latest snapshot in Explore; source excerpts reflect the working tree.</p>"
        ).into()));
        body.extend(citations);
        body.push(Event::Html("</aside>".into()));
    }
    let mut rendered = String::with_capacity(source.len());
    html::push_html(&mut rendered, body.into_iter());
    rendered
}

fn sanitize_event(event: Event<'_>) -> Event<'_> {
    match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: if safe_destination(&dest_url) {
                dest_url
            } else {
                "#".into()
            },
            title,
            id,
        }),
        // Images are displayed as their alt text, not as browser subresource requests.
        Event::Start(Tag::Image { .. }) => Event::Text("[image: ".into()),
        Event::End(TagEnd::Image) => Event::Text("]".into()),
        other => other,
    }
}

fn safe_destination(url: &str) -> bool {
    !url.starts_with("//")
        && !url.contains('\\')
        && !url
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        && (url.starts_with('#') || url.starts_with('/') || !url.contains(':'))
}

fn citation_html(text: &str) -> String {
    let Some((keys, evidence)) = text.split_once(" — ") else {
        return escape(text);
    };
    let keys = keys
        .split(", ")
        .map(|key| {
            if key.contains("://") && !key.chars().any(char::is_control) {
                format!(
                    "<a href=\"/explore?target={}\"><code>{}</code></a>",
                    url_encode(key),
                    escape(key)
                )
            } else {
                escape(key)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{keys} — {}", escape(evidence))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footnotes_have_a_separate_panel_and_safe_canonical_lookups() {
        let rendered = to_html("Claim.[^c1]\n\n[^c1]: file://src/lib.rs — src/lib.rs:1-2\n");
        assert!(
            rendered.contains("Evidence and citations (1)"),
            "{rendered}"
        );
        assert!(rendered.contains("/explore?target=file%3A%2F%2Fsrc%2Flib.rs"));
        assert!(rendered.contains("src/lib.rs:1-2"));
        assert!(rendered.contains("footnote-reference"));
        assert!(rendered.contains("latest snapshot"));
    }

    #[test]
    fn evidence_labels_follow_reference_order_even_when_definitions_are_sorted() {
        let rendered = to_html(
            "First[^b], then second[^a].\n\n[^a]: file://a.rs — a.rs:1-2\n[^b]: file://b.rs — b.rs:1-2\n",
        );
        assert!(rendered.contains("href=\"#b\">1</a>"), "{rendered}");
        assert!(rendered.contains("href=\"#a\">2</a>"), "{rendered}");
        assert!(
            rendered.contains("id=\"a\"><sup class=\"footnote-definition-label\">2</sup>"),
            "{rendered}"
        );
        assert!(
            rendered.contains("id=\"b\"><sup class=\"footnote-definition-label\">1</sup>"),
            "{rendered}"
        );
    }

    #[test]
    fn suppresses_active_urls_and_all_image_requests() {
        let rendered = to_html(
            "[bad](javascript:alert%281%29) [external](https://evil.example/) ![alt](//evil.example/img.png)\n",
        );
        assert!(!rendered.contains("href=\"javascript:"));
        assert!(!rendered.contains("href=\"https:"));
        assert!(!rendered.contains("<img"));
        assert!(rendered.contains("[image: alt]"));
        assert!(to_html("[ok](#section)").contains("href=\"#section\""));
        assert!(to_html("[ok](/api/generations)").contains("href=\"/api/generations\""));
    }
}
