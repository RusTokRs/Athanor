//! Deterministic evidence-backed architecture documentation profile.
//!
//! This pure application owner consumes one explicit canonical snapshot and the documentation
//! contracts. It does not load a store, publish files, expose a transport command, use a provider,
//! access the network, or mutate editable documentation.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Diagnostic, DiagnosticStatus, Entity, Evidence, Fact, Relation, Severity, SourceLocation,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    DocumentationCitation, DocumentationContext, DocumentationContextItem,
    DocumentationContextItemKind, DocumentationContractError, DocumentationDataHandlingPolicy,
    DocumentationDraft, DocumentationDraftClaim, DocumentationDraftDiagramEdge,
    DocumentationDraftSection, DocumentationEvidenceLocation, DocumentationGenerationRequest,
    DocumentationInference, DocumentationOmittedCounts, DocumentationOutline,
    DocumentationOutlineSection, DocumentationProfile, DocumentationQualityMetrics,
    DocumentationRelationDirection, DocumentationSectionKind, DocumentationValidationReport,
    DocumentationValidationStatus, validate_documentation_report_chain,
};

pub const ARCHITECTURE_DOCUMENT_PATH: &str = "architecture/index.md";
pub const ARCHITECTURE_DOCUMENT_MEDIA_TYPE: &str = "text/markdown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationArchitectureDocument {
    pub path: String,
    pub media_type: String,
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationArchitectureProfile {
    pub outline: DocumentationOutline,
    pub context: DocumentationContext,
    pub draft: DocumentationDraft,
    pub validation_report: DocumentationValidationReport,
    pub document: DocumentationArchitectureDocument,
}

/// Builds a deterministic, cited architecture document from one exact canonical snapshot.
pub fn build_documentation_architecture_profile(
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationArchitectureProfile, DocumentationContractError> {
    request.validate()?;
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.as_str())
        .ok_or_else(|| error("documentation architecture profile requires an exact snapshot id"))?;
    if snapshot_id != request.snapshot {
        return Err(error(format!(
            "documentation request snapshot {} does not match canonical snapshot {snapshot_id}",
            request.snapshot
        )));
    }

    let outline = build_outline(request);
    let context = build_context(request, snapshot)?;
    let draft = build_draft(&outline, &context);
    let validation_report = build_validation_report(&draft, &context);
    validate_documentation_report_chain(request, &outline, &context, &draft, &validation_report)?;

    let content = render_markdown(&context, &draft);
    let document = DocumentationArchitectureDocument {
        path: ARCHITECTURE_DOCUMENT_PATH.to_string(),
        media_type: ARCHITECTURE_DOCUMENT_MEDIA_TYPE.to_string(),
        sha256: sha256_hex(content.as_bytes()),
        content,
    };

    Ok(DocumentationArchitectureProfile {
        outline,
        context,
        draft,
        validation_report,
        document,
    })
}

fn build_outline(request: &DocumentationGenerationRequest) -> DocumentationOutline {
    DocumentationOutline {
        schema: DocumentationOutline::SCHEMA.to_string(),
        request_schema: DocumentationGenerationRequest::SCHEMA.to_string(),
        snapshot: request.snapshot.clone(),
        profile: request.profile,
        sections: vec![
            outline_section(
                "overview",
                "System Overview",
                DocumentationSectionKind::Overview,
                "snapshot identity, bounded totals, and omission disclosure",
            ),
            outline_section(
                "components",
                "Components",
                DocumentationSectionKind::Components,
                "evidence-backed canonical entities and facts",
            ),
            outline_section(
                "relationships",
                "Relationships",
                DocumentationSectionKind::Relationships,
                "canonical relations with in-context endpoints",
            ),
            outline_section(
                "diagnostics",
                "Diagnostics",
                DocumentationSectionKind::Diagnostics,
                "open evidence-backed canonical diagnostics",
            ),
        ],
    }
}

fn outline_section(
    id: &str,
    title: &str,
    kind: DocumentationSectionKind,
    reason: &str,
) -> DocumentationOutlineSection {
    DocumentationOutlineSection {
        id: id.to_string(),
        title: title.to_string(),
        kind,
        selection_reasons: vec![reason.to_string()],
    }
}

fn build_context(
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationContext, DocumentationContractError> {
    let entities_by_id = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.0.as_str(), entity))
        .collect::<HashMap<_, _>>();

    let entities = select_candidates(
        snapshot
            .entities
            .iter()
            .filter_map(entity_candidate)
            .collect(),
        request.limits.max_entities,
        DocumentationContextItemKind::Entity,
        "entity",
    );
    if entities.is_empty() {
        return Err(error(
            "documentation architecture context has no evidence-backed entity",
        ));
    }
    let facts = select_candidates(
        snapshot
            .facts
            .iter()
            .filter_map(|fact| fact_candidate(fact, &entities_by_id))
            .collect(),
        request.limits.max_facts,
        DocumentationContextItemKind::Fact,
        "fact",
    );
    let relations = select_candidates(
        snapshot
            .relations
            .iter()
            .filter_map(|relation| relation_candidate(relation, &entities_by_id))
            .collect(),
        request.limits.max_relations,
        DocumentationContextItemKind::Relation,
        "relation",
    );
    let open_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .count();
    let diagnostics = select_candidates(
        snapshot
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
            .filter_map(|diagnostic| diagnostic_candidate(diagnostic, &entities_by_id))
            .collect(),
        request.limits.max_diagnostics,
        DocumentationContextItemKind::Diagnostic,
        "diagnostic",
    );

    let omitted = DocumentationOmittedCounts {
        entities: snapshot.entities.len().saturating_sub(entities.len()),
        facts: snapshot.facts.len().saturating_sub(facts.len()),
        relations: snapshot.relations.len().saturating_sub(relations.len()),
        diagnostics: open_diagnostics.saturating_sub(diagnostics.len()),
    };
    let mut items =
        Vec::with_capacity(entities.len() + facts.len() + relations.len() + diagnostics.len());
    items.extend(entities);
    items.extend(facts);
    items.extend(relations);
    items.extend(diagnostics);

    Ok(DocumentationContext {
        schema: DocumentationContext::SCHEMA.to_string(),
        request_schema: DocumentationGenerationRequest::SCHEMA.to_string(),
        outline_schema: DocumentationOutline::SCHEMA.to_string(),
        snapshot: request.snapshot.clone(),
        profile: DocumentationProfile::Architecture,
        effective_limits: request.limits,
        omitted,
        policy: DocumentationDataHandlingPolicy {
            provider_enabled: false,
            network_enabled: false,
            raw_file_access: false,
            secrets_included: false,
        },
        items,
    })
}

#[derive(Debug)]
struct Candidate {
    sort_key: String,
    summary: String,
    stable_keys: Vec<String>,
    evidence: Vec<DocumentationEvidenceLocation>,
    source_stable_key: Option<String>,
    target_stable_key: Option<String>,
    relation_direction: Option<DocumentationRelationDirection>,
}

fn entity_candidate(entity: &Entity) -> Option<Candidate> {
    let evidence = entity_locations(entity);
    if evidence.is_empty() {
        return None;
    }
    Some(Candidate {
        sort_key: format!("{}\0{}", entity.stable_key.0, entity.id.0),
        summary: format!(
            "{} `{}` is a canonical {} entity.",
            entity_title(entity),
            entity.stable_key.0,
            serialized_name(&entity.kind)
        ),
        stable_keys: vec![entity.stable_key.0.clone()],
        evidence,
        source_stable_key: None,
        target_stable_key: None,
        relation_direction: None,
    })
}

fn fact_candidate(fact: &Fact, entities: &HashMap<&str, &Entity>) -> Option<Candidate> {
    let subject = entities.get(fact.subject.0.as_str())?;
    let object = fact
        .object
        .as_ref()
        .and_then(|object| entities.get(object.0.as_str()))
        .copied();
    let mut stable_keys = vec![subject.stable_key.0.clone()];
    if let Some(object) = object {
        stable_keys.push(object.stable_key.0.clone());
    }
    stable_keys.sort();
    stable_keys.dedup();
    let evidence = evidence_locations(
        &fact.evidence,
        fact.ownership
            .iter()
            .map(|ownership| &ownership.source_file),
    );
    if evidence.is_empty() {
        return None;
    }
    Some(Candidate {
        sort_key: format!("{}\0{}", serialized_name(&fact.kind), fact.id.0),
        summary: object.map_or_else(
            || {
                format!(
                    "Fact {} describes `{}`.",
                    serialized_name(&fact.kind),
                    subject.stable_key.0
                )
            },
            |object| {
                format!(
                    "Fact {} links `{}` to `{}`.",
                    serialized_name(&fact.kind),
                    subject.stable_key.0,
                    object.stable_key.0
                )
            },
        ),
        stable_keys,
        evidence,
        source_stable_key: None,
        target_stable_key: None,
        relation_direction: None,
    })
}

fn relation_candidate(relation: &Relation, entities: &HashMap<&str, &Entity>) -> Option<Candidate> {
    let source = entities.get(relation.from.0.as_str())?;
    let target = entities.get(relation.to.0.as_str())?;
    let evidence = evidence_locations(
        &relation.evidence,
        relation
            .ownership
            .iter()
            .map(|ownership| &ownership.source_file),
    );
    if evidence.is_empty() {
        return None;
    }
    let relation_name = serialized_name(&relation.kind);
    let mut stable_keys = vec![source.stable_key.0.clone(), target.stable_key.0.clone()];
    stable_keys.sort();
    stable_keys.dedup();
    Some(Candidate {
        sort_key: format!(
            "{}\0{}\0{}\0{}",
            source.stable_key.0, relation_name, target.stable_key.0, relation.id.0
        ),
        summary: format!(
            "`{}` {} `{}`.",
            source.stable_key.0, relation_name, target.stable_key.0
        ),
        stable_keys,
        evidence,
        source_stable_key: Some(source.stable_key.0.clone()),
        target_stable_key: Some(target.stable_key.0.clone()),
        relation_direction: Some(DocumentationRelationDirection::Directed),
    })
}

fn diagnostic_candidate(
    diagnostic: &Diagnostic,
    entities: &HashMap<&str, &Entity>,
) -> Option<Candidate> {
    let mut stable_keys = diagnostic
        .entities
        .iter()
        .filter_map(|entity| entities.get(entity.0.as_str()))
        .map(|entity| entity.stable_key.0.clone())
        .collect::<Vec<_>>();
    stable_keys.sort();
    stable_keys.dedup();
    if stable_keys.is_empty() {
        return None;
    }
    let evidence = evidence_locations(
        &diagnostic.evidence,
        diagnostic
            .ownership
            .iter()
            .map(|ownership| &ownership.source_file),
    );
    if evidence.is_empty() {
        return None;
    }
    Some(Candidate {
        sort_key: format!(
            "{}\0{}\0{}",
            severity_rank(diagnostic.severity),
            serialized_name(&diagnostic.kind),
            diagnostic.id.0
        ),
        summary: format!(
            "{} diagnostic {}: {}",
            serialized_name(&diagnostic.severity),
            serialized_name(&diagnostic.kind),
            diagnostic.title
        ),
        stable_keys,
        evidence,
        source_stable_key: None,
        target_stable_key: None,
        relation_direction: None,
    })
}

fn select_candidates(
    mut candidates: Vec<Candidate>,
    limit: usize,
    kind: DocumentationContextItemKind,
    prefix: &str,
) -> Vec<DocumentationContextItem> {
    candidates.sort_by(|left, right| left.sort_key.cmp(&right.sort_key));
    candidates
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, candidate)| DocumentationContextItem {
            id: format!("{prefix}-{:04}", index + 1),
            kind,
            summary: candidate.summary,
            stable_keys: candidate.stable_keys,
            evidence: candidate.evidence,
            source_stable_key: candidate.source_stable_key,
            target_stable_key: candidate.target_stable_key,
            relation_direction: candidate.relation_direction,
        })
        .collect()
}

fn build_draft(
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
            "overview-bounded-snapshot",
            format!(
                "Snapshot `{}` selected {} entities, {} facts, {} relations, and {} open diagnostics; omitted counts are disclosed above.",
                context.snapshot,
                count_kind(context, DocumentationContextItemKind::Entity),
                count_kind(context, DocumentationContextItemKind::Fact),
                count_kind(context, DocumentationContextItemKind::Relation),
                count_kind(context, DocumentationContextItemKind::Diagnostic),
            ),
            "deterministic counts derived from the bounded documentation context",
        )],
        diagram_edges: Vec::new(),
    };

    let components = DocumentationDraftSection {
        id: outline.sections[1].id.clone(),
        title: outline.sections[1].title.clone(),
        claims: context
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.kind,
                    DocumentationContextItemKind::Entity | DocumentationContextItemKind::Fact
                )
            })
            .map(|item| cited_claim(item, &citation_by_item))
            .collect(),
        diagram_edges: Vec::new(),
    };

    let relation_items = context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Relation)
        .collect::<Vec<_>>();
    let relationships = DocumentationDraftSection {
        id: outline.sections[2].id.clone(),
        title: outline.sections[2].title.clone(),
        claims: if relation_items.is_empty() {
            vec![inferred_claim(
                "relationships-none-selected",
                "No evidence-backed canonical relations were selected within the effective limits.",
                "deterministic absence in the bounded documentation context",
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

    let diagnostic_items = context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Diagnostic)
        .collect::<Vec<_>>();
    let diagnostics = DocumentationDraftSection {
        id: outline.sections[3].id.clone(),
        title: outline.sections[3].title.clone(),
        claims: if diagnostic_items.is_empty() {
            vec![inferred_claim(
                "diagnostics-none-selected",
                "No evidence-backed open diagnostics were selected within the effective limits.",
                "deterministic absence in the bounded documentation context",
            )]
        } else {
            diagnostic_items
                .iter()
                .map(|item| cited_claim(item, &citation_by_item))
                .collect()
        },
        diagram_edges: Vec::new(),
    };

    DocumentationDraft {
        schema: DocumentationDraft::SCHEMA.to_string(),
        context_schema: DocumentationContext::SCHEMA.to_string(),
        outline_schema: DocumentationOutline::SCHEMA.to_string(),
        snapshot: context.snapshot.clone(),
        profile: context.profile,
        citations,
        sections: vec![overview, components, relationships, diagnostics],
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

fn build_validation_report(
    draft: &DocumentationDraft,
    context: &DocumentationContext,
) -> DocumentationValidationReport {
    DocumentationValidationReport {
        schema: DocumentationValidationReport::SCHEMA.to_string(),
        draft_schema: DocumentationDraft::SCHEMA.to_string(),
        snapshot: draft.snapshot.clone(),
        profile: draft.profile,
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

fn render_markdown(context: &DocumentationContext, draft: &DocumentationDraft) -> String {
    let mut output = String::from("# Architecture Overview\n\n");
    output.push_str(&format!("- Snapshot: `{}`\n", context.snapshot));
    output.push_str("- Profile: `architecture`\n");
    output.push_str(&format!(
        "- Effective limits: entities {}, facts {}, relations {}, diagnostics {}\n",
        context.effective_limits.max_entities,
        context.effective_limits.max_facts,
        context.effective_limits.max_relations,
        context.effective_limits.max_diagnostics
    ));
    output.push_str(&format!(
        "- Omitted: entities {}, facts {}, relations {}, diagnostics {}\n\n",
        context.omitted.entities,
        context.omitted.facts,
        context.omitted.relations,
        context.omitted.diagnostics
    ));

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

fn entity_locations(entity: &Entity) -> Vec<DocumentationEvidenceLocation> {
    let mut locations = entity
        .source
        .as_ref()
        .and_then(source_location)
        .into_iter()
        .collect::<Vec<_>>();
    locations.extend(
        entity
            .ownership
            .iter()
            .filter_map(|ownership| location(&ownership.source_file, None, None)),
    );
    deduplicate_locations(locations)
}

fn evidence_locations<'a>(
    evidence: &[Evidence],
    ownership_paths: impl IntoIterator<Item = &'a String>,
) -> Vec<DocumentationEvidenceLocation> {
    let mut locations = evidence
        .iter()
        .filter_map(|evidence| {
            evidence
                .source_file
                .as_ref()
                .and_then(|path| location(path, evidence.line_start, evidence.line_end))
        })
        .collect::<Vec<_>>();
    locations.extend(
        ownership_paths
            .into_iter()
            .filter_map(|path| location(path, None, None)),
    );
    deduplicate_locations(locations)
}

fn source_location(source: &SourceLocation) -> Option<DocumentationEvidenceLocation> {
    location(&source.path, source.line_start, source.line_end)
}

fn location(
    path: &str,
    start: Option<u32>,
    end: Option<u32>,
) -> Option<DocumentationEvidenceLocation> {
    let path = path.replace('\\', "/");
    if !is_portable_relative_path(&path) {
        return None;
    }
    let start_line = start.unwrap_or(1).max(1);
    Some(DocumentationEvidenceLocation {
        path,
        start_line,
        end_line: end.unwrap_or(start_line).max(start_line),
    })
}

fn is_portable_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\\')
        && path.split('/').all(|component| {
            !component.is_empty()
                && !matches!(component, "." | "..")
                && !component.ends_with('.')
                && !component.ends_with(' ')
                && !component
                    .chars()
                    .any(|character| character.is_control() || character == ':')
                && !is_windows_reserved(component)
        })
}

fn is_windows_reserved(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or_default();
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "CLOCK$"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn deduplicate_locations(
    locations: Vec<DocumentationEvidenceLocation>,
) -> Vec<DocumentationEvidenceLocation> {
    locations
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn count_kind(context: &DocumentationContext, kind: DocumentationContextItemKind) -> usize {
    context
        .items
        .iter()
        .filter(|item| item.kind == kind)
        .count()
}

fn entity_title(entity: &Entity) -> String {
    entity
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(&entity.name)
        .to_string()
}

fn relation_name(summary: &str) -> String {
    summary
        .split('`')
        .nth(2)
        .unwrap_or("relates_to")
        .trim()
        .to_string()
}

fn serialized_name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
    }
}

fn escape_mermaid(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace(['\n', '\r'], " ")
}

fn sha256_hex(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

fn error(message: impl Into<String>) -> DocumentationContractError {
    DocumentationContractError(message.into())
}
