//! Deterministic evidence-backed API documentation profile.
//!
//! Slice 3A is intentionally pure and bounded: it consumes one exact canonical snapshot and renders
//! only evidence-backed API endpoint, schema, and example entities. API-scoped facts, relations,
//! diagnostics, publication, Store loading, and transport commands remain separate slices.

use athanor_core::CanonicalSnapshot;
use athanor_domain::{Entity, EntityKind};
use sha2::{Digest, Sha256};

use crate::documentation_evidence_location::entity_evidence_locations;
use crate::{
    DOCUMENTATION_REFERENCE_LIMIT, DocumentationCitation, DocumentationContext,
    DocumentationContextItem, DocumentationContextItemKind, DocumentationContractError,
    DocumentationDataHandlingPolicy, DocumentationDraft, DocumentationDraftClaim,
    DocumentationDraftSection, DocumentationGenerationRequest, DocumentationInference,
    DocumentationOmittedCounts, DocumentationOutline, DocumentationOutlineSection,
    DocumentationProfile, DocumentationQualityMetrics, DocumentationSectionKind,
    DocumentationValidationReport, DocumentationValidationStatus, validate_documentation_report_chain,
};

pub const API_DOCUMENT_PATH: &str = "api/index.md";
pub const API_DOCUMENT_MEDIA_TYPE: &str = "text/markdown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationApiDocument {
    pub path: String,
    pub media_type: String,
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationApiProfile {
    pub outline: DocumentationOutline,
    pub context: DocumentationContext,
    pub draft: DocumentationDraft,
    pub validation_report: DocumentationValidationReport,
    pub document: DocumentationApiDocument,
}

/// Builds the bounded Slice 3A API inventory from one exact canonical snapshot.
pub fn build_documentation_api_profile(
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationApiProfile, DocumentationContractError> {
    request.validate()?;
    if request.profile != DocumentationProfile::Api {
        return Err(error("API documentation profile requires profile `api`"));
    }
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.as_str())
        .ok_or_else(|| error("documentation API profile requires an exact snapshot id"))?;
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
    let document = DocumentationApiDocument {
        path: API_DOCUMENT_PATH.to_string(),
        media_type: API_DOCUMENT_MEDIA_TYPE.to_string(),
        sha256: sha256_hex(content.as_bytes()),
        content,
    };

    Ok(DocumentationApiProfile {
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
        profile: DocumentationProfile::Api,
        sections: vec![
            DocumentationOutlineSection {
                id: "overview".to_string(),
                title: "API Overview".to_string(),
                kind: DocumentationSectionKind::Overview,
                selection_reasons: vec![
                    "exact snapshot identity, bounded API totals, and omission disclosure".to_string(),
                ],
            },
            DocumentationOutlineSection {
                id: "endpoints".to_string(),
                title: "API Endpoints".to_string(),
                kind: DocumentationSectionKind::Components,
                selection_reasons: vec![
                    "evidence-backed canonical API endpoints in deterministic order".to_string(),
                ],
            },
            DocumentationOutlineSection {
                id: "schemas".to_string(),
                title: "API Schemas".to_string(),
                kind: DocumentationSectionKind::Components,
                selection_reasons: vec![
                    "evidence-backed canonical API schemas in deterministic order".to_string(),
                ],
            },
            DocumentationOutlineSection {
                id: "examples".to_string(),
                title: "API Examples".to_string(),
                kind: DocumentationSectionKind::Components,
                selection_reasons: vec![
                    "evidence-backed canonical API examples in deterministic order".to_string(),
                ],
            },
        ],
    }
}

fn build_context(
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationContext, DocumentationContractError> {
    let mut endpoints = candidates(snapshot, EntityKind::ApiEndpoint, ApiKind::Endpoint);
    let mut schemas = candidates(snapshot, EntityKind::ApiSchema, ApiKind::Schema);
    let mut examples = candidates(snapshot, EntityKind::ApiExample, ApiKind::Example);
    for candidates in [&mut endpoints, &mut schemas, &mut examples] {
        candidates.sort_by(|left, right| {
            left.stable_key
                .cmp(&right.stable_key)
                .then_with(|| left.entity_id.cmp(&right.entity_id))
        });
    }

    let eligible = endpoints.len() + schemas.len() + examples.len();
    if eligible == 0 {
        return Err(error(
            "documentation API context has no evidence-backed API endpoint, schema, or example entity",
        ));
    }

    let selected_limit = request
        .limits
        .max_entities
        .min(DOCUMENTATION_REFERENCE_LIMIT)
        .min(eligible);
    let selected = round_robin_select(endpoints, schemas, examples, selected_limit);
    let items = selected
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| DocumentationContextItem {
            id: format!("api-{}-{:04}", candidate.kind.id(), index + 1),
            kind: DocumentationContextItemKind::Entity,
            summary: candidate.summary,
            stable_keys: vec![candidate.stable_key],
            evidence: candidate.evidence,
            source_stable_key: None,
            target_stable_key: None,
            relation_direction: None,
        })
        .collect::<Vec<_>>();

    Ok(DocumentationContext {
        schema: DocumentationContext::SCHEMA.to_string(),
        request_schema: DocumentationGenerationRequest::SCHEMA.to_string(),
        outline_schema: DocumentationOutline::SCHEMA.to_string(),
        snapshot: request.snapshot.clone(),
        profile: DocumentationProfile::Api,
        effective_limits: request.limits,
        omitted: DocumentationOmittedCounts {
            entities: eligible - items.len(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiKind {
    Endpoint,
    Schema,
    Example,
}

impl ApiKind {
    fn id(self) -> &'static str {
        match self {
            Self::Endpoint => "endpoint",
            Self::Schema => "schema",
            Self::Example => "example",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Endpoint => "API endpoint",
            Self::Schema => "API schema",
            Self::Example => "API example",
        }
    }
}

#[derive(Debug)]
struct ApiCandidate {
    entity_id: String,
    stable_key: String,
    kind: ApiKind,
    summary: String,
    evidence: Vec<crate::DocumentationEvidenceLocation>,
}

fn candidates(
    snapshot: &CanonicalSnapshot,
    entity_kind: EntityKind,
    api_kind: ApiKind,
) -> Vec<ApiCandidate> {
    snapshot
        .entities
        .iter()
        .filter(|entity| entity.kind == entity_kind)
        .filter_map(|entity| api_candidate(entity, api_kind))
        .collect()
}

fn api_candidate(entity: &Entity, kind: ApiKind) -> Option<ApiCandidate> {
    let evidence = entity_evidence_locations(entity);
    if evidence.is_empty() {
        return None;
    }
    Some(ApiCandidate {
        entity_id: entity.id.0.clone(),
        stable_key: entity.stable_key.0.clone(),
        kind,
        summary: entity_summary(entity, kind),
        evidence,
    })
}

fn entity_summary(entity: &Entity, kind: ApiKind) -> String {
    if kind == ApiKind::Endpoint {
        let method = entity.payload["method"].as_str().unwrap_or("UNKNOWN");
        let path = entity.payload["path"].as_str().unwrap_or("");
        if !path.is_empty() {
            return format!(
                "API endpoint `{}` declares `{method} {path}`.",
                entity.stable_key.0
            );
        }
    }
    format!("{} `{}` is a canonical API entity.", kind.label(), entity.stable_key.0)
}

fn round_robin_select(
    endpoints: Vec<ApiCandidate>,
    schemas: Vec<ApiCandidate>,
    examples: Vec<ApiCandidate>,
    limit: usize,
) -> Vec<ApiCandidate> {
    let mut candidates = [
        endpoints.into_iter(),
        schemas.into_iter(),
        examples.into_iter(),
    ];
    let mut selected = Vec::with_capacity(limit);
    while selected.len() < limit {
        let mut advanced = false;
        for candidates in &mut candidates {
            if selected.len() == limit {
                break;
            }
            if let Some(candidate) = candidates.next() {
                selected.push(candidate);
                advanced = true;
            }
        }
        if !advanced {
            break;
        }
    }
    selected
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

    let overview = DocumentationDraftSection {
        id: outline.sections[0].id.clone(),
        title: outline.sections[0].title.clone(),
        claims: vec![DocumentationDraftClaim {
            id: "overview-bounded-api".to_string(),
            text: format!(
                "Snapshot `{}` selected {} API endpoints, {} API schemas, and {} API examples; omitted API entity count is disclosed above.",
                context.snapshot,
                count_prefix(context, "api-endpoint-"),
                count_prefix(context, "api-schema-"),
                count_prefix(context, "api-example-"),
            ),
            citation_ids: Vec::new(),
            inference: Some(DocumentationInference {
                confidence_basis_points: 10_000,
                rationale: "deterministic counts derived from the bounded API documentation context"
                    .to_string(),
            }),
        }],
        diagram_edges: Vec::new(),
    };

    DocumentationDraft {
        schema: DocumentationDraft::SCHEMA.to_string(),
        context_schema: DocumentationContext::SCHEMA.to_string(),
        outline_schema: DocumentationOutline::SCHEMA.to_string(),
        snapshot: context.snapshot.clone(),
        profile: DocumentationProfile::Api,
        citations,
        sections: vec![
            overview,
            entity_section(&outline.sections[1], context, "api-endpoint-", "endpoint"),
            entity_section(&outline.sections[2], context, "api-schema-", "schema"),
            entity_section(&outline.sections[3], context, "api-example-", "example"),
        ],
    }
}

fn entity_section(
    outline: &DocumentationOutlineSection,
    context: &DocumentationContext,
    prefix: &str,
    label: &str,
) -> DocumentationDraftSection {
    let items = context
        .items
        .iter()
        .filter(|item| item.id.starts_with(prefix))
        .collect::<Vec<_>>();
    DocumentationDraftSection {
        id: outline.id.clone(),
        title: outline.title.clone(),
        claims: if items.is_empty() {
            vec![DocumentationDraftClaim {
                id: format!("{label}-none-selected"),
                text: format!("No evidence-backed API {label} entities were selected within the effective limit."),
                citation_ids: Vec::new(),
                inference: Some(DocumentationInference {
                    confidence_basis_points: 10_000,
                    rationale: format!(
                        "deterministic absence in the bounded API {label} documentation context"
                    ),
                }),
            }]
        } else {
            items
                .into_iter()
                .map(|item| DocumentationDraftClaim {
                    id: format!("claim-{}", item.id),
                    text: item.summary.clone(),
                    citation_ids: vec![format!("citation-{}", item.id)],
                    inference: None,
                })
                .collect()
        },
        diagram_edges: Vec::new(),
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
        profile: DocumentationProfile::Api,
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
    let mut output = String::from("# API Overview\n\n");
    output.push_str(&format!("- Snapshot: `{}`\n", context.snapshot));
    output.push_str("- Profile: `api`\n");
    output.push_str(&format!(
        "- Effective API entity limit: {}\n",
        context.effective_limits.max_entities
    ));
    output.push_str(&format!(
        "- Citation/context budget: {} items\n",
        DOCUMENTATION_REFERENCE_LIMIT
    ));
    output.push_str(&format!(
        "- Omitted API entities: {}\n",
        context.omitted.entities
    ));
    output.push_str(
        "- Slice 3A scope: canonical API endpoint/schema/example entities only; API facts, relations, diagnostics, publication, Store loading, and transport integration are deferred.\n\n",
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

fn count_prefix(context: &DocumentationContext, prefix: &str) -> usize {
    context
        .items
        .iter()
        .filter(|item| item.id.starts_with(prefix))
        .count()
}

fn sha256_hex(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

fn error(message: impl Into<String>) -> DocumentationContractError {
    DocumentationContractError(message.into())
}
