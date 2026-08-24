//! Deterministic evidence-backed onboarding documentation profile.
//!
//! Slice 5B keeps onboarding pure over one exact canonical snapshot while enriching the bounded
//! newcomer-facing inventory with onboarding-scoped facts, supported canonical relations, and open
//! diagnostics. Publication, Store loading, CLI, daemon, MCP, provider access, and coordinated
//! generation remain later bounded slices.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Diagnostic, DiagnosticStatus, Entity, EntityKind, Fact, Relation, RelationKind, Severity,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::documentation_evidence_location::{entity_evidence_locations, evidence_locations};
use crate::{
    DOCUMENTATION_REFERENCE_LIMIT, DocumentationCitation, DocumentationContext,
    DocumentationContextItem, DocumentationContextItemKind, DocumentationContractError,
    DocumentationDataHandlingPolicy, DocumentationDraft, DocumentationDraftClaim,
    DocumentationDraftDiagramEdge, DocumentationDraftSection, DocumentationGenerationRequest,
    DocumentationInference, DocumentationOmittedCounts, DocumentationOutline,
    DocumentationOutlineSection, DocumentationProfile, DocumentationQualityMetrics,
    DocumentationRelationDirection, DocumentationSectionKind, DocumentationValidationReport,
    DocumentationValidationStatus, validate_documentation_report_chain,
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

#[derive(Debug)]
struct ScopedCandidate {
    sort_key: String,
    summary: String,
    stable_keys: Vec<String>,
    evidence: Vec<crate::DocumentationEvidenceLocation>,
    source_stable_key: Option<String>,
    target_stable_key: Option<String>,
    relation_direction: Option<DocumentationRelationDirection>,
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
            outline_section(
                "overview",
                "Onboarding Overview",
                DocumentationSectionKind::Overview,
                "exact snapshot identity, bounded onboarding totals, and omission disclosure",
            ),
            outline_section(
                "inventory",
                "Getting Started Inventory",
                DocumentationSectionKind::Components,
                "evidence-backed guides, sections, packages, runnable commands, environment variables, and verification entrypoints in deterministic category round-robin order",
            ),
            outline_section(
                "facts",
                "Onboarding Facts",
                DocumentationSectionKind::Components,
                "canonical facts whose subject or object is a selected onboarding entity",
            ),
            outline_section(
                "relationships",
                "Onboarding Relationships",
                DocumentationSectionKind::Relationships,
                "supported containment, documentation, environment, and test relations that touch a selected onboarding entity",
            ),
            outline_section(
                "diagnostics",
                "Onboarding Diagnostics",
                DocumentationSectionKind::Diagnostics,
                "open evidence-backed diagnostics that reference a selected onboarding entity",
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
    let selected_entities = select_entities_round_robin(buckets, selected_limit);
    let selected_onboarding_ids = selected_entities
        .iter()
        .map(|candidate| candidate.entity_id.clone())
        .collect::<BTreeSet<_>>();
    let entity_items = selected_entities
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

    let fact_candidates = snapshot
        .facts
        .iter()
        .filter(|fact| fact_is_onboarding_scoped(fact, &selected_onboarding_ids))
        .filter_map(|fact| fact_candidate(fact, &entities_by_id))
        .collect::<Vec<_>>();
    let eligible_facts = fact_candidates.len();

    let relation_candidates = snapshot
        .relations
        .iter()
        .filter(|relation| relation_kind_is_supported(&relation.kind))
        .filter(|relation| relation_is_onboarding_scoped(relation, &selected_onboarding_ids))
        .filter_map(|relation| relation_candidate(relation, &entities_by_id))
        .collect::<Vec<_>>();
    let eligible_relations = relation_candidates.len();

    let diagnostic_candidates = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .filter(|diagnostic| diagnostic_is_onboarding_scoped(diagnostic, &selected_onboarding_ids))
        .filter_map(|diagnostic| diagnostic_candidate(diagnostic, &entities_by_id))
        .collect::<Vec<_>>();
    let eligible_diagnostics = diagnostic_candidates.len();

    let fact_items = candidates_to_items(
        fact_candidates,
        request.limits.max_facts,
        DocumentationContextItemKind::Fact,
        "onboarding-fact",
    );
    let relation_items = candidates_to_items(
        relation_candidates,
        request.limits.max_relations,
        DocumentationContextItemKind::Relation,
        "onboarding-relation",
    );
    let diagnostic_items = candidates_to_items(
        diagnostic_candidates,
        request.limits.max_diagnostics,
        DocumentationContextItemKind::Diagnostic,
        "onboarding-diagnostic",
    );

    let items = apply_context_item_budget(entity_items, fact_items, relation_items, diagnostic_items);
    let omitted = DocumentationOmittedCounts {
        entities: eligible_entities
            .saturating_sub(count_items(&items, DocumentationContextItemKind::Entity)),
        facts: eligible_facts.saturating_sub(count_items(&items, DocumentationContextItemKind::Fact)),
        relations: eligible_relations
            .saturating_sub(count_items(&items, DocumentationContextItemKind::Relation)),
        diagnostics: eligible_diagnostics
            .saturating_sub(count_items(&items, DocumentationContextItemKind::Diagnostic)),
    };

    Ok(DocumentationContext {
        schema: DocumentationContext::SCHEMA.to_string(),
        request_schema: DocumentationGenerationRequest::SCHEMA.to_string(),
        outline_schema: DocumentationOutline::SCHEMA.to_string(),
        snapshot: request.snapshot.clone(),
        profile: DocumentationProfile::Onboarding,
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
        category.label(),
        entity.title.as_deref().unwrap_or(entity.name.as_str()),
        entity.stable_key.0
    )
}

fn fact_is_onboarding_scoped(fact: &Fact, onboarding_ids: &BTreeSet<String>) -> bool {
    onboarding_ids.contains(&fact.subject.0)
        || fact
            .object
            .as_ref()
            .is_some_and(|object| onboarding_ids.contains(&object.0))
}

fn relation_is_onboarding_scoped(
    relation: &Relation,
    onboarding_ids: &BTreeSet<String>,
) -> bool {
    onboarding_ids.contains(&relation.from.0) || onboarding_ids.contains(&relation.to.0)
}

fn diagnostic_is_onboarding_scoped(
    diagnostic: &Diagnostic,
    onboarding_ids: &BTreeSet<String>,
) -> bool {
    diagnostic
        .entities
        .iter()
        .any(|entity| onboarding_ids.contains(&entity.0))
}

fn relation_kind_is_supported(kind: &RelationKind) -> bool {
    matches!(
        kind,
        RelationKind::Contains
            | RelationKind::Documents
            | RelationKind::UsesEnv
            | RelationKind::TestedBy
    )
}

fn fact_candidate(fact: &Fact, entities: &HashMap<&str, &Entity>) -> Option<ScopedCandidate> {
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
    Some(ScopedCandidate {
        sort_key: format!("{}\0{}", serialized_name(&fact.kind), fact.id.0),
        summary: object.map_or_else(
            || {
                format!(
                    "Onboarding fact {} describes `{}`.",
                    serialized_name(&fact.kind),
                    subject.stable_key.0
                )
            },
            |object| {
                format!(
                    "Onboarding fact {} links `{}` to `{}`.",
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

fn relation_candidate(
    relation: &Relation,
    entities: &HashMap<&str, &Entity>,
) -> Option<ScopedCandidate> {
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
    Some(ScopedCandidate {
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
) -> Option<ScopedCandidate> {
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
    Some(ScopedCandidate {
        sort_key: format!(
            "{}\0{}\0{}",
            severity_rank(diagnostic.severity),
            serialized_name(&diagnostic.kind),
            diagnostic.id.0
        ),
        summary: format!(
            "{} onboarding diagnostic {}: {}",
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

fn candidates_to_items(
    mut candidates: Vec<ScopedCandidate>,
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

fn apply_context_item_budget(
    entities: Vec<DocumentationContextItem>,
    facts: Vec<DocumentationContextItem>,
    relations: Vec<DocumentationContextItem>,
    diagnostics: Vec<DocumentationContextItem>,
) -> Vec<DocumentationContextItem> {
    let mut candidates = [
        entities.into_iter(),
        facts.into_iter(),
        relations.into_iter(),
        diagnostics.into_iter(),
    ];
    let mut items = Vec::with_capacity(DOCUMENTATION_REFERENCE_LIMIT);
    while items.len() < DOCUMENTATION_REFERENCE_LIMIT {
        let mut selected = false;
        for candidates in &mut candidates {
            if items.len() == DOCUMENTATION_REFERENCE_LIMIT {
                break;
            }
            if let Some(item) = candidates.next() {
                items.push(item);
                selected = true;
            }
        }
        if !selected {
            break;
        }
    }
    items
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
            unsupported_relations: context.omitted.relations,
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

fn count_kind(context: &DocumentationContext, kind: DocumentationContextItemKind) -> usize {
    count_items(&context.items, kind)
}

fn count_items(items: &[DocumentationContextItem], kind: DocumentationContextItemKind) -> usize {
    items.iter().filter(|item| item.kind == kind).count()
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
    let Ok(value) = serde_json::to_value(value) else {
        return "unknown".to_string();
    };
    match value {
        serde_json::Value::String(name) => name,
        serde_json::Value::Object(object) if object.len() == 1 => {
            let (name, detail) = object.into_iter().next().expect("single enum field");
            detail
                .as_str()
                .map(|detail| format!("{name}:{detail}"))
                .unwrap_or(name)
        }
        _ => "unknown".to_string(),
    }
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
