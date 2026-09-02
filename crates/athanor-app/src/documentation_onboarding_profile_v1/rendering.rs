//! Draft, validation, and Markdown rendering for the deterministic onboarding profile.

use std::collections::{BTreeMap, HashMap};

use crate::{
    DOCUMENTATION_REFERENCE_LIMIT, DocumentationCitation, DocumentationContext,
    DocumentationContextItem, DocumentationContextItemKind, DocumentationDraft,
    DocumentationDraftClaim, DocumentationDraftDiagramEdge, DocumentationDraftSection,
    DocumentationInference, DocumentationOutline, DocumentationOutlineSection,
    DocumentationProfile, DocumentationQualityMetrics, DocumentationValidationReport,
    DocumentationValidationStatus,
};

pub(super) fn build_draft(
    outline: &DocumentationOutline,
    context: &DocumentationContext,
) -> DocumentationDraft {
    let citations = context
        .items
        .iter()
        .map(|item| DocumentationCitation {
            schema: DocumentationCitation::SCHEMA.to_string(),
            id: format!("citation-{}", item.id),
            snapshot: context.snapshot.clone(),
            stable_keys: item.stable_keys.clone(),
            evidence: item.evidence.clone(),
        })
        .collect::<Vec<_>>();
    let citation_by_item = context
        .items
        .iter()
        .map(|item| (item.id.as_str(), format!("citation-{}", item.id)))
        .collect::<HashMap<_, _>>();

    let overview = DocumentationDraftSection {
        id: outline.sections[0].id.clone(),
        title: outline.sections[0].title.clone(),
        claims: vec![inferred_claim(
            "overview-bounded-onboarding",
            format!(
                "Snapshot `{}` selected {} onboarding entities, {} onboarding-scoped facts, {} supported onboarding relations, and {} open onboarding diagnostics; omitted counts are disclosed above.",
                context.snapshot,
                count_kind(context, DocumentationContextItemKind::Entity),
                count_kind(context, DocumentationContextItemKind::Fact),
                count_kind(context, DocumentationContextItemKind::Relation),
                count_kind(context, DocumentationContextItemKind::Diagnostic),
            ),
            "deterministic counts derived from the bounded onboarding documentation context",
        )],
        diagram_edges: Vec::new(),
    };

    let inventory = section_for_kind(
        &outline.sections[1],
        context,
        DocumentationContextItemKind::Entity,
        &citation_by_item,
        "inventory-none-selected",
        "No evidence-backed onboarding entities were selected within the effective limit.",
        "deterministic absence in the bounded onboarding entity context",
    );
    let facts = section_for_kind(
        &outline.sections[2],
        context,
        DocumentationContextItemKind::Fact,
        &citation_by_item,
        "facts-none-selected",
        "No evidence-backed onboarding-scoped facts were selected within the effective limits.",
        "deterministic absence in the bounded onboarding fact context",
    );

    let relation_items = context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Relation)
        .collect::<Vec<_>>();
    let relationships = DocumentationDraftSection {
        id: outline.sections[3].id.clone(),
        title: outline.sections[3].title.clone(),
        claims: if relation_items.is_empty() {
            vec![inferred_claim(
                "relationships-none-selected",
                "No evidence-backed supported onboarding relations were selected within the effective limits.",
                "deterministic absence in the bounded onboarding relationship context",
            )]
        } else {
            relation_items
                .iter()
                .map(|item| cited_claim(item, &citation_by_item))
                .collect()
        },
        diagram_edges: relation_items
            .iter()
            .map(|item| DocumentationDraftDiagramEdge {
                source_stable_key: item.source_stable_key.clone().unwrap_or_default(),
                target_stable_key: item.target_stable_key.clone().unwrap_or_default(),
                relation: relation_name(&item.summary),
                citation_ids: vec![citation_id(item, &citation_by_item)],
            })
            .collect(),
    };

    let diagnostics = section_for_kind(
        &outline.sections[4],
        context,
        DocumentationContextItemKind::Diagnostic,
        &citation_by_item,
        "diagnostics-none-selected",
        "No evidence-backed open onboarding diagnostics were selected within the effective limits.",
        "deterministic absence in the bounded onboarding diagnostic context",
    );

    DocumentationDraft {
        schema: DocumentationDraft::SCHEMA.to_string(),
        context_schema: DocumentationContext::SCHEMA.to_string(),
        outline_schema: DocumentationOutline::SCHEMA.to_string(),
        snapshot: context.snapshot.clone(),
        profile: DocumentationProfile::Onboarding,
        citations,
        sections: vec![overview, inventory, facts, relationships, diagnostics],
    }
}

pub(super) fn build_validation_report(
    draft: &DocumentationDraft,
    context: &DocumentationContext,
) -> DocumentationValidationReport {
    DocumentationValidationReport {
        schema: DocumentationValidationReport::SCHEMA.to_string(),
        draft_schema: DocumentationDraft::SCHEMA.to_string(),
        snapshot: draft.snapshot.clone(),
        profile: DocumentationProfile::Onboarding,
        status: DocumentationValidationStatus::Valid,
        policy: context.policy,
        diagnostics: Vec::new(),
        metrics: DocumentationQualityMetrics {
            citation_coverage_basis_points: 10_000,
            citation_validity_basis_points: 10_000,
            diagram_validity_basis_points: 10_000,
            deterministic_repeatability: true,
            unsupported_relations: context.omitted.relations,
            prompt_tokens: None,
            completion_tokens: None,
            provider_cost_microunits: None,
            human_review_score_basis_points: None,
        },
    }
}

pub(super) fn render_markdown(
    context: &DocumentationContext,
    draft: &DocumentationDraft,
) -> String {
    let mut output = String::from("# Onboarding Documentation\n\n");
    output.push_str(&format!("- Snapshot: `{}`\n", context.snapshot));
    output.push_str("- Profile: `onboarding`\n");
    output.push_str(&format!(
        "- Effective limits: entities {}, facts {}, relations {}, diagnostics {}\n",
        context.effective_limits.max_entities,
        context.effective_limits.max_facts,
        context.effective_limits.max_relations,
        context.effective_limits.max_diagnostics
    ));
    output.push_str(&format!(
        "- Citation/context budget: {} items\n",
        DOCUMENTATION_REFERENCE_LIMIT
    ));
    output.push_str(&format!(
        "- Omitted onboarding scope: entities {}, facts {}, relations {}, diagnostics {}\n",
        context.omitted.entities,
        context.omitted.facts,
        context.omitted.relations,
        context.omitted.diagnostics
    ));
    output.push_str(&format!(
        "- Unrepresented supported onboarding relations: {} eligible relations are outside the bounded context and are not represented by relationship claims or Mermaid edges.\n",
        context.omitted.relations
    ));
    output.push_str(
        "- Slice 5B scope: evidence-backed onboarding entities plus onboarding-scoped facts, supported containment/documentation/environment/test relations, and open diagnostics; publication, Store loading, CLI, daemon, MCP, provider/LLM, and coordinated `ath generate` integration are deferred.\n\n",
    );

    for section in &draft.sections {
        output.push_str(&format!("## {}\n\n", section.title));
        for claim in &section.claims {
            output.push_str("- ");
            output.push_str(&claim.text);
            for citation in &claim.citation_ids {
                output.push_str(&format!(" [^{citation}]"));
            }
            if let Some(inference) = &claim.inference {
                output.push_str(&format!(
                    " _(inference {} bp: {})_",
                    inference.confidence_basis_points, inference.rationale
                ));
            }
            output.push('\n');
        }
        if !section.diagram_edges.is_empty() {
            output.push_str("\n```mermaid\n");
            output.push_str(&render_mermaid(&section.diagram_edges));
            output.push_str("```\n");
        }
        output.push('\n');
    }

    output.push_str("## Evidence\n\n");
    let mut citations = draft.citations.iter().collect::<Vec<_>>();
    citations.sort_by(|left, right| left.id.cmp(&right.id));
    for citation in citations {
        output.push_str(&format!("[^{id}]: ", id = citation.id));
        output.push_str(&citation.stable_keys.join(", "));
        output.push_str(" — ");
        output.push_str(
            &citation
                .evidence
                .iter()
                .map(|location| {
                    format!(
                        "{}:{}-{}",
                        location.path, location.start_line, location.end_line
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        output.push('\n');
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn section_for_kind(
    outline: &DocumentationOutlineSection,
    context: &DocumentationContext,
    kind: DocumentationContextItemKind,
    citation_by_item: &HashMap<&str, String>,
    empty_claim_id: &str,
    empty_text: &str,
    rationale: &str,
) -> DocumentationDraftSection {
    let items = context
        .items
        .iter()
        .filter(|item| item.kind == kind)
        .collect::<Vec<_>>();
    DocumentationDraftSection {
        id: outline.id.clone(),
        title: outline.title.clone(),
        claims: if items.is_empty() {
            vec![inferred_claim(empty_claim_id, empty_text, rationale)]
        } else {
            items
                .iter()
                .map(|item| cited_claim(item, citation_by_item))
                .collect()
        },
        diagram_edges: Vec::new(),
    }
}

fn cited_claim(
    item: &DocumentationContextItem,
    citation_by_item: &HashMap<&str, String>,
) -> DocumentationDraftClaim {
    DocumentationDraftClaim {
        id: format!("claim-{}", item.id),
        text: item.summary.clone(),
        citation_ids: vec![citation_id(item, citation_by_item)],
        inference: None,
    }
}

fn citation_id(
    item: &DocumentationContextItem,
    citation_by_item: &HashMap<&str, String>,
) -> String {
    citation_by_item
        .get(item.id.as_str())
        .expect("every context item owns a citation")
        .clone()
}

fn inferred_claim(id: &str, text: impl Into<String>, rationale: &str) -> DocumentationDraftClaim {
    DocumentationDraftClaim {
        id: id.to_string(),
        text: text.into(),
        citation_ids: Vec::new(),
        inference: Some(DocumentationInference {
            confidence_basis_points: 10_000,
            rationale: rationale.to_string(),
        }),
    }
}

fn render_mermaid(edges: &[DocumentationDraftDiagramEdge]) -> String {
    let mut keys = edges
        .iter()
        .flat_map(|edge| [&edge.source_stable_key, &edge.target_stable_key])
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    let nodes = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.as_str(), format!("n{index}")))
        .collect::<BTreeMap<_, _>>();

    let mut output = String::from("flowchart LR\n");
    for key in &keys {
        output.push_str(&format!(
            "  {}[\"{}\"]\n",
            nodes[key.as_str()],
            escape_mermaid(key)
        ));
    }
    let mut sorted_edges = edges.iter().collect::<Vec<_>>();
    sorted_edges.sort_by(|left, right| {
        left.source_stable_key
            .cmp(&right.source_stable_key)
            .then_with(|| left.relation.cmp(&right.relation))
            .then_with(|| left.target_stable_key.cmp(&right.target_stable_key))
    });
    for edge in sorted_edges {
        output.push_str(&format!(
            "  {} -->|{}| {}\n",
            nodes[edge.source_stable_key.as_str()],
            escape_mermaid(&edge.relation),
            nodes[edge.target_stable_key.as_str()]
        ));
    }
    output
}

fn relation_name(summary: &str) -> String {
    summary
        .split('`')
        .nth(2)
        .unwrap_or("relates_to")
        .trim()
        .to_string()
}

fn count_kind(context: &DocumentationContext, kind: DocumentationContextItemKind) -> usize {
    context
        .items
        .iter()
        .filter(|item| item.kind == kind)
        .count()
}

fn escape_mermaid(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace(['\n', '\r'], " ")
}
