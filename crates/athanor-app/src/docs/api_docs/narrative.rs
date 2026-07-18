use std::collections::BTreeSet;

use athanor_domain::Entity;

use super::super::frontmatter::split_frontmatter;
use super::content::api_route_signature;
use super::{
    COORDINATION_END, COORDINATION_START_PREFIX, MANAGED_END, MANAGED_START_PREFIX,
    NARRATIVE_REVIEW_END, NARRATIVE_REVIEW_START,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApiNarrativeRewriteDraft {
    stale_mention: String,
    suggested_mention: String,
    current_line: String,
    draft_line: String,
}

pub(crate) fn stale_api_route_mentions(content: &str, endpoints: &[&Entity]) -> Vec<String> {
    let expected = endpoints
        .iter()
        .filter_map(|endpoint| api_route_signature(endpoint))
        .collect::<BTreeSet<_>>();
    if expected.is_empty() {
        return Vec::new();
    }
    let narrative = strip_generated_sections(strip_frontmatter_text(content));
    let mut stale = api_route_mentions(&narrative)
        .into_iter()
        .filter(|mention| !expected.contains(mention))
        .collect::<Vec<_>>();
    stale.sort();
    stale.dedup();
    stale
}

pub(crate) fn upsert_api_narrative_review_section(
    content: &str,
    endpoints: &[&Entity],
    stale_mentions: &[String],
) -> String {
    let drafts = rewrite_drafts(content, endpoints, stale_mentions);
    let section = review_section(endpoints, stale_mentions, &drafts);
    let Some(start) = content.find(NARRATIVE_REVIEW_START) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(&section);
        return updated;
    };
    let Some(relative_end) = content[start..].find(NARRATIVE_REVIEW_END) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(&section);
        return updated;
    };
    let end = start + relative_end + NARRATIVE_REVIEW_END.len();
    let mut updated = String::new();
    updated.push_str(content[..start].trim_end());
    updated.push_str("\n\n");
    updated.push_str(&section);
    updated.push_str(content[end..].trim_start_matches(['\r', '\n']));
    updated
}

fn review_section(
    endpoints: &[&Entity],
    stale_mentions: &[String],
    drafts: &[ApiNarrativeRewriteDraft],
) -> String {
    let mut expected_routes = endpoints
        .iter()
        .filter_map(|endpoint| api_route_signature(endpoint))
        .collect::<Vec<_>>();
    expected_routes.sort();
    expected_routes.dedup();
    let mut stale_mentions = stale_mentions.to_vec();
    stale_mentions.sort();
    stale_mentions.dedup();

    let mut content = String::new();
    content.push_str(NARRATIVE_REVIEW_START);
    content.push_str("\n\n## Athanor API Narrative Review\n\n");
    content
        .push_str("Potentially stale API route mentions found outside Athanor-managed blocks:\n");
    for mention in stale_mentions {
        content.push_str(&format!("- `{mention}`\n"));
    }
    content.push_str("\nCurrent linked endpoint routes for this page:\n");
    for route in expected_routes {
        content.push_str(&format!("- `{route}`\n"));
    }
    if !drafts.is_empty() {
        content.push_str("\nSuggested narrative rewrite drafts:\n");
        for draft in drafts {
            content.push_str(&format!(
                "- Replace `{}` with `{}`\n  - Current: {}\n  - Draft: {}\n",
                draft.stale_mention, draft.suggested_mention, draft.current_line, draft.draft_line
            ));
        }
    }
    content.push('\n');
    content.push_str(NARRATIVE_REVIEW_END);
    content.push('\n');
    content
}

fn rewrite_drafts(
    content: &str,
    endpoints: &[&Entity],
    stale_mentions: &[String],
) -> Vec<ApiNarrativeRewriteDraft> {
    let expected_routes = endpoints
        .iter()
        .filter_map(|endpoint| api_route_signature(endpoint))
        .collect::<Vec<_>>();
    let suggested_mention = match expected_routes.as_slice() {
        [route] => route,
        _ => return Vec::new(),
    };
    let stale_mentions = stale_mentions.iter().collect::<BTreeSet<_>>();
    let narrative = strip_generated_sections(strip_frontmatter_text(content));
    let mut drafts = Vec::new();
    let mut seen = BTreeSet::new();
    for line in narrative.lines() {
        for stale_mention in api_route_mentions(line)
            .iter()
            .filter(|mention| stale_mentions.contains(mention))
        {
            let current_line = line.trim().to_string();
            if current_line.is_empty() {
                continue;
            }
            let draft_line = current_line.replace(stale_mention, suggested_mention);
            let key = (stale_mention.clone(), current_line.clone());
            if draft_line != current_line && seen.insert(key) {
                drafts.push(ApiNarrativeRewriteDraft {
                    stale_mention: stale_mention.clone(),
                    suggested_mention: suggested_mention.clone(),
                    current_line,
                    draft_line,
                });
            }
        }
    }
    drafts.sort_by(|left, right| {
        (&left.stale_mention, &left.current_line).cmp(&(&right.stale_mention, &right.current_line))
    });
    drafts
}

fn api_route_mentions(content: &str) -> Vec<String> {
    const METHODS: &[&str] = &[
        "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE",
    ];
    let mut mentions = Vec::new();
    for line in content.lines() {
        let words = line
            .split_whitespace()
            .map(|word| word.trim_matches(route_token_trim_chars))
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        for pair in words.windows(2) {
            let method = pair[0].to_ascii_uppercase();
            let route = pair[1].trim_end_matches(['.', ',', ';', ':']);
            if METHODS.contains(&method.as_str()) && route.starts_with('/') {
                mentions.push(format!("{method} {route}"));
            }
        }
    }
    mentions
}

fn route_token_trim_chars(character: char) -> bool {
    matches!(
        character,
        '`' | '*' | '_' | '[' | ']' | '(' | ')' | '"' | '\'' | ':' | ','
    )
}

fn strip_frontmatter_text(content: &str) -> &str {
    split_frontmatter(content).map_or(content, |(_, body)| body)
}

fn strip_generated_sections(content: &str) -> String {
    let mut stripped = content.to_string();
    for (start, end) in [
        (MANAGED_START_PREFIX, MANAGED_END),
        (COORDINATION_START_PREFIX, COORDINATION_END),
        (NARRATIVE_REVIEW_START, NARRATIVE_REVIEW_END),
    ] {
        stripped = strip_sections_with_markers(&stripped, start, end);
    }
    stripped
}

fn strip_sections_with_markers(content: &str, start_marker: &str, end_marker: &str) -> String {
    let mut remaining = content;
    let mut stripped = String::new();
    while let Some(start) = remaining.find(start_marker) {
        stripped.push_str(&remaining[..start]);
        let after_start = &remaining[start..];
        let Some(relative_end) = after_start.find(end_marker) else {
            break;
        };
        let end = start + relative_end + end_marker.len();
        remaining = &remaining[end..];
    }
    stripped.push_str(remaining);
    stripped
}
