//! Deterministic evidence-backed onboarding documentation profile.
//!
//! Slice 5A projects one exact canonical snapshot into a bounded newcomer-facing inventory. It is
//! intentionally pure: facts, relations, diagnostics, publication, Store loading, CLI, daemon, MCP,
//! provider access, and coordinated generation remain later bounded slices.

use std::collections::{BTreeMap, VecDeque};

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
    DocumentationValidationReport, DocumentationValidationStatus,
    validate_documentation_report_chain,
};

pub const ONBOARDING_DOCUMENT_PATH: &str = "onboarding/index.md";
pub const ONBOARDING_DOCUMENT_MEDIA_TYPE: &str = "text/markdown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationOnboardingDocument {
    pub path: String,
    pub media_type: String,
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationOnboardingProfile {
    pub outline: DocumentationOutline,
    pub context: DocumentationContext,
    pub draft: DocumentationDraft,
    pub validation_report: DocumentationValidationReport,
    pub document: DocumentationOnboardingDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OnboardingCategory {
    Guides,
    Sections,
    Packages,
    Commands,
    Environment,
    Verification,
}

impl OnboardingCategory {
    const ALL: [Self; 6] = [
        Self::Guides,
        Self::Sections,
        Self::Packages,
        Self::Commands,
        Self::Environment,
        Self::Verification,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Guides => "guides",
            Self::Sections => "sections",
            Self::Packages => "packages",
            Self::Commands => "commands",
            Self::Environment => "environment",
            Self::Verification => "verification",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Guides => "guide",
            Self::Sections => "documentation section",
            Self::Packages => "package",
            Self::Commands => "runnable command",
            Self::Environment => "environment variable",
            Self::Verification => "verification entrypoint",
        }
    }
}

#[derive(Debug, Clone)]
struct OnboardingEntityCandidate {
    entity_id: String,
    stable_key: String,
    category: OnboardingCategory,
    summary: String,
    evidence: Vec<crate::DocumentationEvidenceLocation>,
}

/// Builds the bounded onboarding profile from one exact canonical snapshot.
pub fn build_documentation_onboarding_profile(
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationOnboardingProfile, DocumentationContractError> {
    request.validate()?;
    if request.profile != DocumentationProfile::Onboarding {
        return Err(error(
            "onboarding documentation profile requires profile `onboarding`",
        ));
    }
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.as_str())
        .ok_or_else(|| error("documentation onboarding profile requires an exact snapshot id"))?;
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
    let document = DocumentationOnboardingDocument {
        path: ONBOARDING_DOCUMENT_PATH.to_string(),
        media_type: ONBOARDING_DOCUMENT_MEDIA_TYPE.to_string(),
        sha256: sha256_hex(content.as_bytes()),
        content,
    };

    Ok(DocumentationOnboardingProfile {
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
        profile: DocumentationProfile::Onboarding,
        sections: vec![
            DocumentationOutlineSection {
                id: "overview".to_string(),
                title: "Onboarding Overview".to_string(),
                kind: DocumentationSectionKind::Overview,
                selection_reasons: vec![
                    "exact snapshot identity, bounded newcomer-facing totals, and omission disclosure"
                        .to_string(),
                ],
            },
            DocumentationOutlineSection {
                id: "inventory".to_string(),
                title: "Getting Started Inventory".to_string(),
                kind: DocumentationSectionKind::Components,
                selection_reasons: vec![
                    "evidence-backed guides, sections, packages, runnable commands, environment variables, and verification entrypoints in deterministic category round-robin order"
                        .to_string(),
                ],
            },
        ],
    }
}

fn build_context(
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationContext, DocumentationContractError> {
    let mut buckets = OnboardingCategory::ALL
        .into_iter()
        .map(|category| (category, Vec::new()))
        .collect::<BTreeMap<_, Vec<OnboardingEntityCandidate>>>();

    for entity in &snapshot.entities {
        let Some(category) = onboarding_category(&entity.kind) else {
            continue;
        };
        let evidence = entity_evidence_locations(entity);
        if evidence.is_empty() {
            continue;
        }
        buckets
            .entry(category)
            .or_default()
            .push(OnboardingEntityCandidate {
                entity_id: entity.id.0.clone(),
                stable_key: entity.stable_key.0.clone(),
                category,
                summary: entity_summary(entity, category),
                evidence,
            });
    }

    let eligible_entities = buckets.values().map(Vec::len).sum::<usize>();
    if eligible_entities == 0 {
        return Err(error(
            "documentation onboarding context has no evidence-backed onboarding entity",
        ));
    }

    let selected_limit = request
        .limits
        .max_entities
        .min(DOCUMENTATION_REFERENCE_LIMIT)
        .min(eligible_entities);
    let selected = select_entities_round_robin(buckets, selected_limit);
    let items = selected
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| DocumentationContextItem {
            id: format!(
                "onboarding-{}-{:04}",
                candidate.category.id(),
                index + 1
            ),
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
        profile: DocumentationProfile::Onboarding,
        effective_limits: request.limits,
        omitted: DocumentationOmittedCounts {
            entities: eligible_entities.saturating_sub(items.len()),
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

fn select_entities_round_robin(
    mut buckets: BTreeMap<OnboardingCategory, Vec<OnboardingEntityCandidate>>,
    limit: usize,
) -> Vec<OnboardingEntityCandidate> {
    let mut queues = OnboardingCategory::ALL
        .into_iter()
        .map(|category| {
            let mut candidates = buckets.remove(&category).unwrap_or_default();
            candidates.sort_by(|left, right| {
                left.stable_key
                    .cmp(&right.stable_key)
                    .then_with(|| left.entity_id.cmp(&right.entity_id))
            });
            (category, VecDeque::from(candidates))
        })
        .collect::<BTreeMap<_, _>>();
    let mut selected = Vec::with_capacity(limit);
    while selected.len() < limit {
        let mut progressed = false;
        for category in OnboardingCategory::ALL {
            if selected.len() == limit {
                break;
            }
            if let Some(candidate) = queues
                .get_mut(&category)
                .and_then(|queue| queue.pop_front())
            {
                selected.push(candidate);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    selected
}

fn onboarding_category(kind: &EntityKind) -> Option<OnboardingCategory> {
    match kind {
        EntityKind::DocumentationPage => Some(OnboardingCategory::Guides),
        EntityKind::DocumentationSection => Some(OnboardingCategory::Sections),
        EntityKind::Package => Some(OnboardingCategory::Packages),
        EntityKind::ScriptCommand => Some(OnboardingCategory::Commands),
        EntityKind::EnvVar => Some(OnboardingCategory::Environment),
        EntityKind::TestCase | EntityKind::CiJob => Some(OnboardingCategory::Verification),
        _ => None,
    }
}

fn entity_summary(entity: &Entity, category: OnboardingCategory) -> String {
    format!(
        "{} `{}` — {}",
        category.label(), entity.title.as_deref().unwrap_or(&entity.name), entity.stable_key.0
    )
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
            id: "overview-bounded-onboarding".to_string(),
            text: format!(
                "Snapshot `{}` selected {} evidence-backed onboarding entrypoints; {} eligible onboarding entities were omitted by the bounded context.",
                context.snapshot,
                context.items.len(),
                context.omitted.entities,
            ),
            citation_ids: Vec::new(),
            inference: Some(DocumentationInference {
                confidence_basis_points: 10_000,
                rationale: "deterministic counts derived from the bounded onboarding documentation context"
                    .to_string(),
            }),
        }],
        diagram_edges: Vec::new(),
    };

    let inventory = DocumentationDraftSection {
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
    };

    DocumentationDraft {
        schema: DocumentationDraft::SCHEMA.to_string(),
        context_schema: DocumentationContext::SCHEMA.to_string(),
        outline_schema: DocumentationOutline::SCHEMA.to_string(),
        snapshot: context.snapshot.clone(),
        profile: DocumentationProfile::Onboarding,
        citations,
        sections: vec![overview, inventory],
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
        profile: DocumentationProfile::Onboarding,
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
    let mut output = String::from("# Onboarding Documentation\n\n");
    output.push_str(&format!("- Snapshot: `{}`\n", context.snapshot));
    output.push_str("- Profile: `onboarding`\n");
    output.push_str(&format!(
        "- Effective entity limit: {}\n",
        context.effective_limits.max_entities
    ));
    output.push_str(&format!(
        "- Citation/context budget: {} items\n",
        DOCUMENTATION_REFERENCE_LIMIT
    ));
    output.push_str(&format!(
        "- Omitted onboarding entities: {}\n",
        context.omitted.entities
    ));
    output.push_str(
        "- Slice 5A scope: evidence-backed documentation pages/sections, packages, runnable script commands, environment variables, tests, and CI jobs only; facts, relations, diagnostics, publication, Store loading, CLI, daemon, MCP, provider/LLM, and coordinated `ath generate` integration are deferred.\n\n",
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

fn sha256_hex(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

fn error(message: impl Into<String>) -> DocumentationContractError {
    DocumentationContractError(message.into())
}
