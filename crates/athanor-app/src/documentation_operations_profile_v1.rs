//! Deterministic evidence-backed operations documentation profile.
//!
//! Slice 4A establishes a pure exact-snapshot inventory over canonical operational entities. Facts,
//! relations, diagnostics, publication, Store loading, CLI, daemon, MCP, and provider integration are
//! deliberately left to later bounded slices.

use std::collections::{BTreeMap, VecDeque};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{Entity, EntityKind};
use sha2::{Digest, Sha256};

use crate::documentation_evidence_location::entity_evidence_locations;
use crate::{
    DOCUMENTATION_REFERENCE_LIMIT, DocumentationCitation, DocumentationContext,
    DocumentationContextItem, DocumentationContextItemKind, DocumentationContractError,
    DocumentationDataHandlingPolicy, DocumentationDraft, DocumentationDraftClaim,
    DocumentationDraftSection, DocumentationGenerationRequest, DocumentationOmittedCounts,
    DocumentationOutline, DocumentationOutlineSection, DocumentationProfile,
    DocumentationQualityMetrics, DocumentationSectionKind, DocumentationValidationReport,
    DocumentationValidationStatus, validate_documentation_report_chain,
};

pub const OPERATIONS_DOCUMENT_PATH: &str = "operations/index.md";
pub const OPERATIONS_DOCUMENT_MEDIA_TYPE: &str = "text/markdown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationOperationsDocument {
    pub path: String,
    pub media_type: String,
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationOperationsProfile {
    pub outline: DocumentationOutline,
    pub context: DocumentationContext,
    pub draft: DocumentationDraft,
    pub validation_report: DocumentationValidationReport,
    pub document: DocumentationOperationsDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OperationsCategory {
    Environment,
    Automation,
    Deployment,
    Data,
    Configuration,
    Runbooks,
}

impl OperationsCategory {
    const ALL: [Self; 6] = [
        Self::Environment,
        Self::Automation,
        Self::Deployment,
        Self::Data,
        Self::Configuration,
        Self::Runbooks,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Automation => "automation",
            Self::Deployment => "deployment",
            Self::Data => "data",
            Self::Configuration => "configuration",
            Self::Runbooks => "runbooks",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Automation => "automation",
            Self::Deployment => "deployment",
            Self::Data => "data",
            Self::Configuration => "configuration",
            Self::Runbooks => "runbook",
        }
    }
}

#[derive(Debug, Clone)]
struct OperationsEntityCandidate {
    entity_id: String,
    stable_key: String,
    category: OperationsCategory,
    summary: String,
    evidence: Vec<crate::DocumentationEvidenceLocation>,
}

/// Builds the bounded operations profile from one exact canonical snapshot.
pub fn build_documentation_operations_profile(
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationOperationsProfile, DocumentationContractError> {
    request.validate()?;
    if request.profile != DocumentationProfile::Operations {
        return Err(error(
            "operations documentation profile requires profile `operations`",
        ));
    }
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.as_str())
        .ok_or_else(|| error("documentation operations profile requires an exact snapshot id"))?;
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
    let document = DocumentationOperationsDocument {
        path: OPERATIONS_DOCUMENT_PATH.to_string(),
        media_type: OPERATIONS_DOCUMENT_MEDIA_TYPE.to_string(),
        sha256: sha256_hex(content.as_bytes()),
        content,
    };

    Ok(DocumentationOperationsProfile {
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
        profile: DocumentationProfile::Operations,
        sections: vec![
            DocumentationOutlineSection {
                id: "overview".to_string(),
                title: "Operations Overview".to_string(),
                kind: DocumentationSectionKind::Overview,
                selection_reasons: vec![
                    "exact snapshot identity, bounded operations totals, and omission disclosure"
                        .to_string(),
                ],
            },
            DocumentationOutlineSection {
                id: "inventory".to_string(),
                title: "Operations Inventory".to_string(),
                kind: DocumentationSectionKind::Components,
                selection_reasons: vec![
                    "evidence-backed environment, automation, deployment, data, configuration, and runbook entities in deterministic category round-robin order"
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
    let mut buckets = OperationsCategory::ALL
        .into_iter()
        .map(|category| (category, Vec::new()))
        .collect::<BTreeMap<_, Vec<OperationsEntityCandidate>>>();

    for entity in &snapshot.entities {
        let Some(category) = operations_category(&entity.kind) else {
            continue;
        };
        let evidence = entity_evidence_locations(entity);
        if evidence.is_empty() {
            continue;
        }
        buckets.entry(category).or_default().push(OperationsEntityCandidate {
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
            "documentation operations context has no evidence-backed operational entity",
        ));
    }

    let mut queues = OperationsCategory::ALL
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

    let selected_limit = request
        .limits
        .max_entities
        .min(DOCUMENTATION_REFERENCE_LIMIT)
        .min(eligible_entities);
    let mut selected = Vec::with_capacity(selected_limit);
    while selected.len() < selected_limit {
        let mut progressed = false;
        for category in OperationsCategory::ALL {
            if selected.len() == selected_limit {
                break;
            }
            if let Some(candidate) = queues.get_mut(&category).and_then(VecDeque::pop_front) {
                selected.push(candidate);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }

    let items = selected
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| DocumentationContextItem {
            id: format!(
                "operations-{}-{:04}",
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
        profile: DocumentationProfile::Operations,
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

fn operations_category(kind: &EntityKind) -> Option<OperationsCategory> {
    match kind {
        EntityKind::EnvVar => Some(OperationsCategory::Environment),
        EntityKind::Script | EntityKind::ScriptCommand | EntityKind::CiJob => {
            Some(OperationsCategory::Automation)
        }
        EntityKind::DockerService => Some(OperationsCategory::Deployment),
        EntityKind::DbMigration | EntityKind::DbTable => Some(OperationsCategory::Data),
        EntityKind::Feature => Some(OperationsCategory::Configuration),
        EntityKind::Runbook | EntityKind::OperationStep => Some(OperationsCategory::Runbooks),
        _ => None,
    }
}

fn entity_summary(entity: &Entity, category: OperationsCategory) -> String {
    format!(
        "{} `{}` — {}",
        category.label(), entity.name, entity.stable_key.0
    )
}

fn build_draft(outline: &DocumentationOutline, context: &DocumentationContext) -> DocumentationDraft {
    let citations = context
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| DocumentationCitation {
            schema: DocumentationCitation::SCHEMA.to_string(),
            id: format!("operations-citation-{:04}", index + 1),
            snapshot: context.snapshot.clone(),
            stable_keys: item.stable_keys.clone(),
            evidence: item.evidence.clone(),
        })
        .collect::<Vec<_>>();
    let citation_ids = citations
        .iter()
        .map(|citation| citation.id.clone())
        .collect::<Vec<_>>();

    let overview = DocumentationDraftSection {
        id: outline.sections[0].id.clone(),
        title: outline.sections[0].title.clone(),
        claims: vec![DocumentationDraftClaim {
            id: "operations-overview".to_string(),
            text: format!(
                "Exact snapshot {} contributes {} selected evidence-backed operations entities; {} additional eligible entities are omitted by the bounded selection.",
                context.snapshot,
                context.items.len(),
                context.omitted.entities
            ),
            citation_ids: citation_ids.clone(),
            inference: None,
        }],
        diagram_edges: Vec::new(),
    };

    let inventory = DocumentationDraftSection {
        id: outline.sections[1].id.clone(),
        title: outline.sections[1].title.clone(),
        claims: context
            .items
            .iter()
            .zip(&citations)
            .enumerate()
            .map(|(index, (item, citation))| DocumentationDraftClaim {
                id: format!("operations-inventory-{:04}", index + 1),
                text: item.summary.clone(),
                citation_ids: vec![citation.id.clone()],
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
        profile: DocumentationProfile::Operations,
        citations,
        sections: vec![overview, inventory],
    }
}

fn build_validation_report(
    _draft: &DocumentationDraft,
    context: &DocumentationContext,
) -> DocumentationValidationReport {
    DocumentationValidationReport {
        schema: DocumentationValidationReport::SCHEMA.to_string(),
        draft_schema: DocumentationDraft::SCHEMA.to_string(),
        snapshot: context.snapshot.clone(),
        profile: DocumentationProfile::Operations,
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
    let mut output = String::new();
    output.push_str("# Operations Documentation\n\n");
    output.push_str(&format!("- Snapshot: `{}`\n", context.snapshot));
    output.push_str("- Profile: `operations`\n");
    output.push_str(&format!("- Selected operations entities: {}\n", context.items.len()));
    output.push_str(&format!("- Omitted operations entities: {}\n\n", context.omitted.entities));

    for section in &draft.sections {
        output.push_str(&format!("## {}\n\n", section.title));
        for claim in &section.claims {
            output.push_str("- ");
            output.push_str(&claim.text);
            for citation in &claim.citation_ids {
                output.push_str(&format!(" [^{citation}]"));
            }
            output.push('\n');
        }
        output.push('\n');
    }

    output.push_str("## Disclosures\n\n");
    output.push_str("- Slice 4A scope is a pure exact-snapshot operational entity inventory.\n");
    output.push_str("- Facts, relations, diagnostics, publication, Store loading, CLI, daemon, MCP, provider/LLM, and coordinated `ath generate` integration are out of scope.\n");
    output.push_str("- Raw file access, network access, and secrets are disabled by policy.\n\n");

    for citation in &draft.citations {
        output.push_str(&format!("[^{}]:", citation.id));
        for stable_key in &citation.stable_keys {
            output.push_str(&format!(" `{stable_key}`"));
        }
        for location in &citation.evidence {
            output.push_str(&format!(
                " — {}:{}-{}",
                location.path, location.start_line, location.end_line
            ));
        }
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
