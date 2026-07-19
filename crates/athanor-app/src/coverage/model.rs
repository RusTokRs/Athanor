use std::path::PathBuf;

use serde::Serialize;

pub const COVERAGE_REPORT_SCHEMA: &str = "athanor.coverage.v1";

#[derive(Debug, Clone)]
pub struct CoverageOptions {
    pub root: PathBuf,
    pub adapter: Option<String>,
    pub file: Option<PathBuf>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageReport {
    pub schema: &'static str,
    pub snapshot: String,
    pub root: PathBuf,
    pub filters: CoverageFilters,
    pub limits: CoverageLimits,
    pub totals: CoverageTotals,
    pub files: Vec<FileCoverage>,
    pub adapters: Vec<AdapterCoverage>,
    pub diagnostics: Vec<DiagnosticCoverage>,
    pub omitted: CoverageOmitted,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageFilters {
    pub adapter: Option<String>,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageLimits {
    pub limit: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CoverageTotals {
    pub tracked_files: usize,
    pub files_with_canonical_objects: usize,
    pub files_with_open_diagnostics: usize,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
    pub open_diagnostics: usize,
    pub adapter_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileCoverage {
    pub path: String,
    pub language_hint: Option<String>,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
    pub open_diagnostics: usize,
    pub adapters: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterCoverage {
    pub adapter: String,
    pub files: usize,
    pub facts: usize,
    pub evidence_items: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCoverage {
    pub kind: String,
    pub total: usize,
    pub open: usize,
    pub files: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CoverageOmitted {
    pub files: usize,
    pub adapters: usize,
    pub diagnostics: usize,
}
