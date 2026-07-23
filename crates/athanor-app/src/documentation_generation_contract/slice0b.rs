//! Intermediate contracts for evidence-backed documentation planning and validation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{
    DocumentationContractError, DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationOmittedCounts, DocumentationProfile, validate_non_empty,
    validate_relative_output_path, validate_schema,
};

pub const DOCUMENTATION_OUTLINE_SCHEMA_V1: &str = "athanor.documentation_outline.v1";
pub const DOCUMENTATION_CONTEXT_SCHEMA_V1: &str = "athanor.documentation_context.v1";
pub const DOCUMENTATION_CITATION_SCHEMA_V1: &str = "athanor.documentation_citation.v1";
pub const DOCUMENTATION_DRAFT_SCHEMA_V1: &str = "athanor.documentation_draft.v1";
pub const DOCUMENTATION_VALIDATION_REPORT_SCHEMA_V1: &str =
    "athanor.documentation_validation_report.v1";

const SECTION_LIMIT: usize = 64;
const REFERENCE_LIMIT: usize = 256;
const BASIS_POINTS_MAX: u16 = 10_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationSectionKind {
    Overview,
    Components,
    Relationships,
    Diagnostics,
    Risks,
    Glossary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationOutlineSection {
    pub id: String,
    pub title: String,
    pub kind: DocumentationSectionKind,
    pub selection_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationOutline {
    pub schema: String,
    pub request_schema: String,
    pub snapshot: String,
    pub profile: DocumentationProfile,
    pub sections: Vec<DocumentationOutlineSection>,
}

impl DocumentationOutline {
    pub const SCHEMA: &'static str = DOCUMENTATION_OUTLINE_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), DocumentationContractError> {
        validate_schema("documentation outline", &self.schema, Self::SCHEMA)?;
        validate_schema(
            "documentation outline request",
            &self.request_schema,
            DocumentationGenerationRequest::SCHEMA,
        )?;
        validate_non_empty("outline snapshot", &self.snapshot)?;
        validate_len("documentation outline sections", self.sections.len(), SECTION_LIMIT)?;

        let mut ids = BTreeSet::new();
        for section in &self.sections {
            validate_slug("outline section id", &section.id)?;
            validate_non_empty("outline section title", &section.title)?;
            validate_len(
                "outline section selection_reasons",
                section.selection_reasons.len(),
                REFERENCE_LIMIT,
            )?;
            validate_unique_text(
                "outline section selection reason",
                section.selection_reasons.iter().map(String::as_str),
            )?;
            if !ids.insert(section.id.as_str()) {
                return Err(error(format!(
                    "duplicate documentation outline section id {}",
                    section.id
                )));
            }
        }
        Ok(())
    }

    pub fn validate_for_request(
        &self,
        request: &DocumentationGenerationRequest,
    ) -> Result<(), DocumentationContractError> {
        request.validate()?;
        self.validate()?;
        validate_identity(
            "outline",
            &self.snapshot,
            self.profile,
            &request.snapshot,
            request.profile,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct DocumentationEvidenceLocation {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
}

impl DocumentationEvidenceLocation {
    fn validate(&self) -> Result<(), DocumentationContractError> {
        validate_relative_output_path(&self.path)?;
        if self.start_line == 0 || self.end_line < self.start_line {
            return Err(error(format!(
                "documentation evidence range {}:{}-{} is invalid",
                self.path, self.start_line, self.end_line
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationCitation {
    pub schema: String,
    pub id: String,
    pub snapshot: String,
    pub stable_keys: Vec<String>,
    pub evidence: Vec<DocumentationEvidenceLocation>,
}

impl DocumentationCitation {
    pub const SCHEMA: &'static str = DOCUMENTATION_CITATION_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), DocumentationContractError> {
        validate_schema("documentation citation", &self.schema, Self::SCHEMA)?;
        validate_slug("citation id", &self.id)?;
        validate_non_empty("citation snapshot", &self.snapshot)?;
        validate_len(
            "documentation citation stable_keys",
            self.stable_keys.len(),
            REFERENCE_LIMIT,
        )?;
        validate_unique_text(
            "documentation citation stable key",
            self.stable_keys.iter().map(String::as_str),
        )?;
        validate_evidence("citation", &self.evidence)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationDataHandlingPolicy {
    pub provider_enabled: bool,
    pub network_enabled: bool,
    pub raw_file_access: bool,
    pub secrets_included: bool,
}

impl DocumentationDataHandlingPolicy {
    fn validate(self) -> Result<(), DocumentationContractError> {
        if self.raw_file_access || self.secrets_included {
            return Err(error(
                "documentation context must exclude raw file access and secrets",
            ));
        }
        if self.network_enabled && !self.provider_enabled {
            return Err(error(
                "documentation network access requires explicit provider enablement",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationContextItemKind {
    Entity,
    Fact,
    Relation,
    Diagnostic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationRelationDirection {
    Directed,
    Bidirectional,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationContextItem {
    pub id: String,
    pub kind: DocumentationContextItemKind,
    pub summary: String,
    pub stable_keys: Vec<String>,
    pub evidence: Vec<DocumentationEvidenceLocation>,
    pub source_stable_key: Option<String>,
    pub target_stable_key: Option<String>,
    pub relation_direction: Option<DocumentationRelationDirection>,
}

impl DocumentationContextItem {
    fn validate(&self) -> Result<(), DocumentationContractError> {
        validate_slug("context item id", &self.id)?;
        validate_non_empty("context item summary", &self.summary)?;
        validate_len(
            "documentation context item stable_keys",
            self.stable_keys.len(),
            REFERENCE_LIMIT,
        )?;
        validate_unique_text(
            "documentation context item stable key",
            self.stable_keys.iter().map(String::as_str),
        )?;
        validate_evidence("context item", &self.evidence)?;

        let relation_fields = (
            self.source_stable_key.as_deref(),
            self.target_stable_key.as_deref(),
            self.relation_direction,
        );
        match (self.kind, relation_fields) {
            (
                DocumentationContextItemKind::Relation,
                (Some(source), Some(target), Some(_)),
            ) => {
                validate_non_empty("relation source stable key", source)?;
                validate_non_empty("relation target stable key", target)?;
                if !self.stable_keys.iter().any(|key| key == source)
                    || !self.stable_keys.iter().any(|key| key == target)
                {
                    return Err(error(
                        "documentation relation endpoints must be included in stable_keys",
                    ));
                }
            }
            (DocumentationContextItemKind::Relation, _) => {
                return Err(error(
                    "documentation relation requires source, target, and direction",
                ));
            }
            (_, (None, None, None)) => {}
            _ => {
                return Err(error(
                    "non-relation documentation context item carries relation fields",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationContext {
    pub schema: String,
    pub request_schema: String,
    pub outline_schema: String,
    pub snapshot: String,
    pub profile: DocumentationProfile,
    pub effective_limits: DocumentationGenerationLimits,
    pub omitted: DocumentationOmittedCounts,
    pub policy: DocumentationDataHandlingPolicy,
    pub items: Vec<DocumentationContextItem>,
}

impl DocumentationContext {
    pub const SCHEMA: &'static str = DOCUMENTATION_CONTEXT_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), DocumentationContractError> {
        validate_schema("documentation context", &self.schema, Self::SCHEMA)?;
        validate_schema(
            "documentation context request",
            &self.request_schema,
            DocumentationGenerationRequest::SCHEMA,
        )?;
        validate_schema(
            "documentation context outline",
            &self.outline_schema,
            DocumentationOutline::SCHEMA,
        )?;
        validate_non_empty("context snapshot", &self.snapshot)?;
        self.effective_limits.validate()?;
        self.policy.validate()?;
        validate_len(
            "documentation context items",
            self.items.len(),
            self.effective_limits.max_entities
                + self.effective_limits.max_facts
                + self.effective_limits.max_relations
                + self.effective_limits.max_diagnostics,
        )?;

        let mut ids = BTreeSet::new();
        let mut counts = [0_usize; 4];
        for item in &self.items {
            item.validate()?;
            if !ids.insert(item.id.as_str()) {
                return Err(error(format!(
                    "duplicate documentation context item id {}",
                    item.id
                )));
            }
            counts[item_kind_index(item.kind)] += 1;
        }
        for (name, actual, limit) in [
            ("entities", counts[0], self.effective_limits.max_entities),
            ("facts", counts[1], self.effective_limits.max_facts),
            ("relations", counts[2], self.effective_limits.max_relations),
            (
                "diagnostics",
                counts[3],
                self.effective_limits.max_diagnostics,
            ),
        ] {
            if actual > limit {
                return Err(error(format!(
                    "documentation context {name} count {actual} exceeds limit {limit}"
                )));
            }
        }
        if counts[0] == 0 {
            return Err(error(
                "documentation context must contain at least one entity item",
            ));
        }
        Ok(())
    }

    pub fn validate_for_request_and_outline(
        &self,
        request: &DocumentationGenerationRequest,
        outline: &DocumentationOutline,
    ) -> Result<(), DocumentationContractError> {
        request.validate()?;
        outline.validate_for_request(request)?;
        self.validate()?;
        validate_identity(
            "context",
            &self.snapshot,
            self.profile,
            &request.snapshot,
            request.profile,
        )?;
        if self.effective_limits != request.limits || self.outline_schema != outline.schema {
            return Err(error(
                "documentation context does not match request limits or outline schema",
            ));
        }
        Ok(())
    }

    fn stable_keys(&self) -> BTreeSet<&str> {
        self.items
            .iter()
            .flat_map(|item| item.stable_keys.iter().map(String::as_str))
            .collect()
    }

    fn evidence(&self) -> BTreeSet<&DocumentationEvidenceLocation> {
        self.items
            .iter()
            .flat_map(|item| item.evidence.iter())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationInference {
    pub confidence_basis_points: u16,
    pub rationale: String,
}

impl DocumentationInference {
    fn validate(&self) -> Result<(), DocumentationContractError> {
        validate_basis_points("inference confidence", self.confidence_basis_points, false)?;
        validate_non_empty("inference rationale", &self.rationale)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationDraftClaim {
    pub id: String,
    pub text: String,
    pub citation_ids: Vec<String>,
    pub inference: Option<DocumentationInference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationDraftDiagramEdge {
    pub source_stable_key: String,
    pub target_stable_key: String,
    pub relation: String,
    pub citation_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationDraftSection {
    pub id: String,
    pub title: String,
    pub claims: Vec<DocumentationDraftClaim>,
    pub diagram_edges: Vec<DocumentationDraftDiagramEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationDraft {
    pub schema: String,
    pub context_schema: String,
    pub outline_schema: String,
    pub snapshot: String,
    pub profile: DocumentationProfile,
    pub citations: Vec<DocumentationCitation>,
    pub sections: Vec<DocumentationDraftSection>,
}

impl DocumentationDraft {
    pub const SCHEMA: &'static str = DOCUMENTATION_DRAFT_SCHEMA_V1;

    pub fn validate_for_context_and_outline(
        &self,
        context: &DocumentationContext,
        outline: &DocumentationOutline,
    ) -> Result<(), DocumentationContractError> {
        context.validate()?;
        outline.validate()?;
        validate_schema("documentation draft", &self.schema, Self::SCHEMA)?;
        validate_schema(
            "documentation draft context",
            &self.context_schema,
            DocumentationContext::SCHEMA,
        )?;
        validate_schema(
            "documentation draft outline",
            &self.outline_schema,
            DocumentationOutline::SCHEMA,
        )?;
        validate_identity(
            "draft",
            &self.snapshot,
            self.profile,
            &context.snapshot,
            context.profile,
        )?;
        validate_len(
            "documentation draft citations",
            self.citations.len(),
            REFERENCE_LIMIT,
        )?;
        validate_len(
            "documentation draft sections",
            self.sections.len(),
            SECTION_LIMIT,
        )?;

        let known_keys = context.stable_keys();
        let known_evidence = context.evidence();
        let mut citation_ids = BTreeSet::new();
        for citation in &self.citations {
            citation.validate()?;
            if citation.snapshot != self.snapshot {
                return Err(error(format!(
                    "documentation citation {} snapshot does not match draft",
                    citation.id
                )));
            }
            if !citation_ids.insert(citation.id.as_str()) {
                return Err(error(format!(
                    "duplicate documentation citation id {}",
                    citation.id
                )));
            }
            if citation
                .stable_keys
                .iter()
                .any(|key| !known_keys.contains(key.as_str()))
                || citation
                    .evidence
                    .iter()
                    .any(|location| !known_evidence.contains(location))
            {
                return Err(error(format!(
                    "documentation citation {} escapes the bounded context",
                    citation.id
                )));
            }
        }

        let outline_ids = outline
            .sections
            .iter()
            .map(|section| section.id.as_str())
            .collect::<Vec<_>>();
        let draft_ids = self
            .sections
            .iter()
            .map(|section| section.id.as_str())
            .collect::<Vec<_>>();
        if draft_ids != outline_ids {
            return Err(error(
                "documentation draft must preserve outline section order and identity",
            ));
        }

        let mut claim_ids = BTreeSet::new();
        for section in &self.sections {
            validate_non_empty("draft section title", &section.title)?;
            if section.claims.is_empty() && section.diagram_edges.is_empty() {
                return Err(error(format!(
                    "documentation draft section {} is empty",
                    section.id
                )));
            }
            for claim in &section.claims {
                validate_slug("draft claim id", &claim.id)?;
                validate_non_empty("draft claim text", &claim.text)?;
                if !claim_ids.insert(claim.id.as_str()) {
                    return Err(error(format!(
                        "duplicate documentation draft claim id {}",
                        claim.id
                    )));
                }
                validate_references(
                    "draft claim citation",
                    &claim.citation_ids,
                    &citation_ids,
                    claim.inference.is_none(),
                )?;
                if let Some(inference) = &claim.inference {
                    inference.validate()?;
                }
            }
            for edge in &section.diagram_edges {
                validate_non_empty("diagram source stable key", &edge.source_stable_key)?;
                validate_non_empty("diagram target stable key", &edge.target_stable_key)?;
                validate_non_empty("diagram relation", &edge.relation)?;
                if !known_keys.contains(edge.source_stable_key.as_str())
                    || !known_keys.contains(edge.target_stable_key.as_str())
                {
                    return Err(error(
                        "documentation diagram edge references an out-of-context stable key",
                    ));
                }
                validate_references(
                    "diagram citation",
                    &edge.citation_ids,
                    &citation_ids,
                    true,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationValidationStatus {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationValidationDiagnostic {
    pub code: String,
    pub severity: DocumentationDiagnosticSeverity,
    pub message: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationQualityMetrics {
    pub citation_coverage_basis_points: u16,
    pub citation_validity_basis_points: u16,
    pub diagram_validity_basis_points: u16,
    pub deterministic_repeatability: bool,
    pub unsupported_relations: usize,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub provider_cost_microunits: Option<u64>,
    pub human_review_score_basis_points: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationValidationReport {
    pub schema: String,
    pub draft_schema: String,
    pub snapshot: String,
    pub profile: DocumentationProfile,
    pub status: DocumentationValidationStatus,
    pub policy: DocumentationDataHandlingPolicy,
    pub diagnostics: Vec<DocumentationValidationDiagnostic>,
    pub metrics: DocumentationQualityMetrics,
}

impl DocumentationValidationReport {
    pub const SCHEMA: &'static str = DOCUMENTATION_VALIDATION_REPORT_SCHEMA_V1;

    pub fn validate_for_draft_and_context(
        &self,
        draft: &DocumentationDraft,
        context: &DocumentationContext,
    ) -> Result<(), DocumentationContractError> {
        validate_schema("documentation validation report", &self.schema, Self::SCHEMA)?;
        validate_schema(
            "documentation validation report draft",
            &self.draft_schema,
            DocumentationDraft::SCHEMA,
        )?;
        validate_identity(
            "validation report",
            &self.snapshot,
            self.profile,
            &draft.snapshot,
            draft.profile,
        )?;
        if self.policy != context.policy || self.draft_schema != draft.schema {
            return Err(error(
                "documentation validation report does not match draft or context policy",
            ));
        }
        self.policy.validate()?;
        if self.diagnostics.len() > REFERENCE_LIMIT {
            return Err(error("documentation validation report has too many diagnostics"));
        }
        for diagnostic in &self.diagnostics {
            validate_non_empty("validation diagnostic code", &diagnostic.code)?;
            validate_non_empty("validation diagnostic message", &diagnostic.message)?;
            if let Some(location) = &diagnostic.location {
                validate_non_empty("validation diagnostic location", location)?;
            }
        }
        validate_metrics(&self.metrics, self.policy)?;

        let errors = self
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DocumentationDiagnosticSeverity::Error)
            .count();
        match self.status {
            DocumentationValidationStatus::Valid => {
                if errors != 0
                    || self.metrics.citation_validity_basis_points != BASIS_POINTS_MAX
                    || self.metrics.diagram_validity_basis_points != BASIS_POINTS_MAX
                {
                    return Err(error(
                        "valid documentation report requires no errors and fully valid references",
                    ));
                }
            }
            DocumentationValidationStatus::Invalid if errors == 0 => {
                return Err(error(
                    "invalid documentation report requires an error diagnostic",
                ));
            }
            DocumentationValidationStatus::Invalid => {}
        }
        Ok(())
    }
}

fn validate_metrics(
    metrics: &DocumentationQualityMetrics,
    policy: DocumentationDataHandlingPolicy,
) -> Result<(), DocumentationContractError> {
    for (field, value) in [
        (
            "citation coverage",
            metrics.citation_coverage_basis_points,
        ),
        (
            "citation validity",
            metrics.citation_validity_basis_points,
        ),
        ("diagram validity", metrics.diagram_validity_basis_points),
    ] {
        validate_basis_points(field, value, true)?;
    }
    if let Some(score) = metrics.human_review_score_basis_points {
        validate_basis_points("human review score", score, true)?;
    }
    if !policy.provider_enabled
        && (metrics.prompt_tokens.is_some()
            || metrics.completion_tokens.is_some()
            || metrics.provider_cost_microunits.is_some())
    {
        return Err(error(
            "provider metrics require explicit provider enablement",
        ));
    }
    Ok(())
}

fn validate_evidence(
    owner: &str,
    evidence: &[DocumentationEvidenceLocation],
) -> Result<(), DocumentationContractError> {
    validate_len(
        &format!("documentation {owner} evidence"),
        evidence.len(),
        REFERENCE_LIMIT,
    )?;
    let mut unique = BTreeSet::new();
    for location in evidence {
        location.validate()?;
        if !unique.insert(location) {
            return Err(error(format!(
                "duplicate documentation {owner} evidence {}:{}-{}",
                location.path, location.start_line, location.end_line
            )));
        }
    }
    Ok(())
}

fn validate_references(
    owner: &str,
    values: &[String],
    known: &BTreeSet<&str>,
    required: bool,
) -> Result<(), DocumentationContractError> {
    if required {
        validate_len(owner, values.len(), REFERENCE_LIMIT)?;
    } else if values.len() > REFERENCE_LIMIT {
        return Err(error(format!("{owner} exceeds {REFERENCE_LIMIT} entries")));
    }
    validate_unique_text(owner, values.iter().map(String::as_str))?;
    if let Some(unknown) = values
        .iter()
        .find(|value| !known.contains(value.as_str()))
    {
        return Err(error(format!("{owner} references unknown id {unknown}")));
    }
    Ok(())
}

fn validate_unique_text<'a>(
    field: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<(), DocumentationContractError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_non_empty(field, value)?;
        if !unique.insert(value) {
            return Err(error(format!("duplicate {field} {value}")));
        }
    }
    Ok(())
}

fn validate_len(field: &str, len: usize, max: usize) -> Result<(), DocumentationContractError> {
    if len == 0 || len > max {
        Err(error(format!(
            "{field} must contain between 1 and {max} entries"
        )))
    } else {
        Ok(())
    }
}

fn validate_slug(field: &str, value: &str) -> Result<(), DocumentationContractError> {
    validate_non_empty(field, value)?;
    if value.len() > 128
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(error(format!(
            "documentation generation {field} must be a lowercase ASCII slug"
        )));
    }
    Ok(())
}

fn validate_basis_points(
    field: &str,
    value: u16,
    allow_zero: bool,
) -> Result<(), DocumentationContractError> {
    if value > BASIS_POINTS_MAX || (!allow_zero && value == 0) {
        return Err(error(format!(
            "documentation {field} must be within the permitted basis-point range"
        )));
    }
    Ok(())
}

fn validate_identity(
    owner: &str,
    actual_snapshot: &str,
    actual_profile: DocumentationProfile,
    expected_snapshot: &str,
    expected_profile: DocumentationProfile,
) -> Result<(), DocumentationContractError> {
    if actual_snapshot != expected_snapshot || actual_profile != expected_profile {
        return Err(error(format!(
            "documentation {owner} does not match parent snapshot and profile"
        )));
    }
    Ok(())
}

fn item_kind_index(kind: DocumentationContextItemKind) -> usize {
    match kind {
        DocumentationContextItemKind::Entity => 0,
        DocumentationContextItemKind::Fact => 1,
        DocumentationContextItemKind::Relation => 2,
        DocumentationContextItemKind::Diagnostic => 3,
    }
}

fn error(message: impl Into<String>) -> DocumentationContractError {
    DocumentationContractError(message.into())
}
