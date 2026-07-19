use std::path::PathBuf;

use serde::Serialize;

pub const CAPABILITIES_REPORT_SCHEMA: &str = "athanor.capabilities.v1";
pub const DEFAULT_CAPABILITIES_LIMIT: usize = 50;
pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 1.0;

pub(super) const UNKNOWN_LANGUAGE: &str = "unknown";

/// Adapter that only records a baseline file inventory entry for every discovered
/// file. A file covered by this adapter alone has not received content extraction,
/// so completeness is measured against adapters other than this one.
pub(super) const BASELINE_ADAPTER: &str = "file";

#[derive(Debug, Clone)]
pub struct CapabilitiesOptions {
    pub root: PathBuf,
    pub limit: usize,
    pub confidence_threshold: f32,
}

/// Bounded analysis-completeness report.
///
/// It answers where the knowledge graph is incomplete before agents rely on
/// query, graph, or daemon answers: which discovered files produced no canonical
/// objects, how completeness breaks down per language and per extractor adapter,
/// and which extracted facts carry below-threshold confidence.
#[derive(Debug, Clone, Serialize)]
pub struct CapabilitiesReport {
    pub schema: &'static str,
    pub snapshot: String,
    pub root: PathBuf,
    pub baseline_adapter: &'static str,
    pub limits: CapabilitiesLimits,
    pub totals: CapabilitiesTotals,
    pub languages: Vec<LanguageCapability>,
    pub adapters: Vec<AdapterCapability>,
    pub low_confidence_facts: Vec<LowConfidenceFact>,
    pub unprocessed_files: Vec<UnprocessedFile>,
    pub omitted: CapabilitiesOmitted,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilitiesLimits {
    pub limit: usize,
    pub confidence_threshold: f32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CapabilitiesTotals {
    pub tracked_files: usize,
    pub processed_files: usize,
    pub unprocessed_files: usize,
    pub processed_ratio_percent: u8,
    pub languages: usize,
    pub adapters: usize,
    pub facts: usize,
    pub low_confidence_facts: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageCapability {
    pub language_hint: String,
    pub tracked_files: usize,
    pub processed_files: usize,
    pub unprocessed_files: usize,
    pub processed_ratio_percent: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterCapability {
    pub adapter: String,
    pub processed_files: usize,
    pub facts: usize,
    pub low_confidence_facts: usize,
    pub min_confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LowConfidenceFact {
    pub fact_id: String,
    pub adapter: String,
    pub kind: String,
    pub confidence: f32,
    pub path: Option<String>,
    pub line_start: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnprocessedFile {
    pub path: String,
    pub language_hint: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CapabilitiesOmitted {
    pub languages: usize,
    pub adapters: usize,
    pub low_confidence_facts: usize,
    pub unprocessed_files: usize,
}
