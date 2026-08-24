//! Deterministic evidence-backed module documentation profile.
//!
//! Slice 2A is intentionally narrow: it consumes one exact canonical snapshot and renders only
//! evidence-backed `EntityKind::Module` inventory. Module-scoped facts, relations, diagnostics,
//! publication, Store loading, and transport commands remain separate bounded slices.

use athanor_core::CanonicalSnapshot;
use athanor_domain::{Entity, EntityKind};
use sha2::{Digest, Sha256};

use crate::{
    DOCUMENTATION_REFERENCE_LIMIT, DocumentationCitation, DocumentationContext,
    DocumentationContextItem, DocumentationContextItemKind, DocumentationContractError,
    DocumentationDataHandlingPolicy, DocumentationDraft, DocumentationDraftClaim,
    DocumentationDraftSection, DocumentationEvidenceLocation, DocumentationGenerationRequest,
    DocumentationInference, DocumentationOmittedCounts, DocumentationOutline,
    DocumentationOutlineSection, DocumentationProfile, DocumentationQualityMetrics,
    DocumentationSectionKind, DocumentationValidationReport, DocumentationValidationStatus,
    validate_documentation_report_chain,
};

pub const MODULE_DOCUMENT_PATH: &str = "modules/index.md";
pub const MODULE_DOCUMENT_MEDIA_TYPE: &str = "text/markdown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationModuleDocument {
    pub path: String,
    pub media_type: String,
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationModuleProfile {
    pub outline: DocumentationOutline,
    pub context: DocumentationContext,
    pub draft: DocumentationDraft,
    pub validation_report: DocumentationValidationReport,
    pub document: DocumentationModuleDocument,
}

/// Builds the bounded Slice 2A module inventory from one exact canonical snapshot.
pub fn build_documentation_module_profile(
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationModuleProfile, DocumentationContractError> {
    request.validate()?;
    if request.profile != DocumentationProfile::Module {
        return Err(error("module documentation profile requires profile `module`"));
    }
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.as_str())
        .ok_or_else(|| error("documentation module profile requires an exact snapshot id"))?;
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
    let document = DocumentationModuleDocument {
        path: MODULE_DOCUMENT_PATH.to_string(),
        media_type: MODULE_DOCUMENT_MEDIA_TYPE.to_string(),
        sha256: sha256_hex(content.as_bytes()),
        content,
    };

    Ok(DocumentationModuleProfile {
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
        profile: DocumentationProfile::Module,
        sections: vec![
            DocumentationOutlineSection {
                id: "overview".to_string(),
                title: "Module Overview".to_string(),
                kind: DocumentationSectionKind::Overview,
                selection_reasons: vec![
                    "exact snapshot identity, bounded module totals, and omission disclosure"
                        .to_string(),
                ],
            },
            DocumentationOutlineSection {
                id: "modules".to_string(),
                title: "Modules".to_string(),
                kind: DocumentationSectionKind::Components,
                selection_reasons: vec![
                    "evidence-backed canonical module entities in stable-key order".to_string(),
                ],
            },
        ],
    }
}

fn build_context(
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationContext, DocumentationContractError> {
    let mut candidates = snapshot
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::Module)
        .filter_map(module_candidate)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.stable_key
            .cmp(&right.stable_key)
            .then_with(|| left.entity_id.cmp(&right.entity_id))
    });
    if candidates.is_empty() {
        return Err(error(
            "documentation module context has no source-backed module entity",
        ));
    }

    let eligible = candidates.len();
    let selected = request
        .limits
        .max_entities
        .min(DOCUMENTATION_REFERENCE_LIMIT)
        .min(eligible);
    let items = candidates
        .into_iter()
        .take(selected)
        .enumerate()
        .map(|(index, candidate)| DocumentationContextItem {
            id: format!("module-{:04}", index + 1),
            kind: DocumentationContextItemKind::Entity,
            summary: format!("Module `{}` is a canonical module entity.", candidate.stable_key),
            stable_keys: vec![candidate.stable_key],
            evidence: vec![candidate.evidence],
            source_stable_key: None,
            target_stable_key: None,
            relation_direction: None,
        })
        .collect();

    Ok(DocumentationContext {
        schema: DocumentationContext::SCHEMA.to_string(),
        request_schema: DocumentationGenerationRequest::SCHEMA.to_string(),
        outline_schema: DocumentationOutline::SCHEMA.to_string(),
        snapshot: request.snapshot.clone(),
        profile: DocumentationProfile::Module,
        effective_limits: request.limits,
        omitted: DocumentationOmittedCounts {
            entities: eligible - selected,
            facts: 0,
            relations: 0,
            diagnostics: 0,
        },
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
struct ModuleCandidate {
    entity_id: String,
    stable_key: String,
    evidence: DocumentationEvidenceLocation,
}

fn module_candidate(entity: &Entity) -> Option<ModuleCandidate> {
    let source = entity.source.as_ref()?;
    let start_line = source.line_start.unwrap_or(1).max(1);
    Some(ModuleCandidate {
        entity_id: entity.id.0.clone(),
        stable_key: entity.stable_key.0.clone(),
        evidence: DocumentationEvidenceLocation {
            path: source.path.clone(),
            start_line,
            end_line: source.line_end.unwrap_or(start_line).max(start_line),
        },
    })
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

    DocumentationDraft {
        schema: DocumentationDraft::SCHEMA.to_string(),
        context_schema: DocumentationContext::SCHEMA.to_string(),
        outline_schema: DocumentationOutline::SCHEMA.to_string(),
        snapshot: context.snapshot.clone(),
        profile: DocumentationProfile::Module,
        citations,
        sections: vec![
            DocumentationDraftSection {
                id: outline.sections[0].id.clone(),
                title: outline.sections[0].title.clone(),
                claims: vec![DocumentationDraftClaim {
                    id: "overview-bounded-modules".to_string(),
                    text: format!(
                        "Snapshot `{}` selected {} source-backed modules; omitted module count is disclosed above.",
                        context.snapshot,
                        context.items.len()
                    ),
                    citation_ids: Vec::new(),
                    inference: Some(DocumentationInference {
                        confidence_basis_points: 10_000,
                        rationale:
                            "deterministic count derived from the bounded module documentation context"
                                .to_string(),
                    }),
                }],
                diagram_edges: Vec::new(),
            },
            DocumentationDraftSection {
                id: outline.sections[1].id.clone(),
                title: outline.sections[1].title.clone(),
                claims: context
                    .items
                    .iter()
                    .map(|item| DocumentationDraftClaim {
                        id: format!("claim-{}", item.id),
                        text: item.summary.clone(),
                        citation_ids: vec![format!("citation-{}", item.id)],
                        inference: None,
                    })
                    .collect(),
                diagram_edges: Vec::new(),
            },
        ],
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
        profile: DocumentationProfile::Module,
        status: DocumentationValidationStatus::Valid,
        policy: context.policy,
        diagnostics: Vec::new(),
        metrics: DocumentationQualityMetrics {
            citation_coverage_basis_points: 10_000,
            citation_validity_basis_points: 10_000,
            diagram_validity_basis_points: 10_000,
            deterministic_repeatability: true,
            unsupported_relations: 0,
            prompt_tokens: None,
            completion_tokens: None,
            provider_cost_microunits: None,
            human_review_score_basis_points: None,
        },
    }
}

fn render_markdown(context: &DocumentationContext, draft: &DocumentationDraft) -> String {
    let mut output = String::from("# Module Overview\n\n");
    output.push_str(&format!("- Snapshot: `{}`\n", context.snapshot));
    output.push_str("- Profile: `module`\n");
    output.push_str(&format!(
        "- Effective module limit: {}\n",
        context.effective_limits.max_entities
    ));
    output.push_str(&format!(
        "- Citation/context budget: {} items\n",
        DOCUMENTATION_REFERENCE_LIMIT
    ));
    output.push_str(&format!(
        "- Omitted modules: {}\n",
        context.omitted.entities
    ));
    output.push_str(
        "- Slice 2A scope: canonical module entities only; module facts, relations, diagnostics, publication, and transport integration are deferred.\n\n",
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
        output.push('\n');
    }

    output.push_str("## Evidence\n\n");
    for citation in &draft.citations {
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

fn sha256_hex(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

fn error(message: impl Into<String>) -> DocumentationContractError {
    DocumentationContractError(message.into())
}
