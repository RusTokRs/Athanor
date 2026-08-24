//! Stable JSON transport for the exact-snapshot documentation completeness report.

use serde::Serialize;

use crate::{DocumentationCompletenessReport, VersionedJsonContract};

pub const DOCUMENTATION_COMPLETENESS_SCHEMA_V1: &str = "athanor.documentation_completeness.v1";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionedDocumentationCompletenessReport {
    pub schema: &'static str,
    pub snapshot: String,
    pub baseline_adapter: &'static str,
    pub limit: usize,
    pub totals: VersionedDocumentationCompletenessTotals,
    pub languages: Vec<VersionedDocumentationLanguageCompleteness>,
    pub adapters: Vec<VersionedDocumentationAdapterCompleteness>,
    pub unprocessed_files: Vec<VersionedDocumentationUnprocessedFile>,
    pub omitted: VersionedDocumentationCompletenessOmitted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionedDocumentationCompletenessTotals {
    pub tracked_files: usize,
    pub processed_files: usize,
    pub unprocessed_files: usize,
    pub processed_ratio_basis_points: u16,
    pub languages: usize,
    pub adapters: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionedDocumentationLanguageCompleteness {
    pub language: String,
    pub tracked_files: usize,
    pub processed_files: usize,
    pub unprocessed_files: usize,
    pub processed_ratio_basis_points: u16,
    pub adapters: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionedDocumentationAdapterCompleteness {
    pub adapter: String,
    pub files: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionedDocumentationUnprocessedFile {
    pub path: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionedDocumentationCompletenessOmitted {
    pub languages: usize,
    pub adapters: usize,
    pub unprocessed_files: usize,
}

impl From<&DocumentationCompletenessReport> for VersionedDocumentationCompletenessReport {
    fn from(report: &DocumentationCompletenessReport) -> Self {
        Self {
            schema: DOCUMENTATION_COMPLETENESS_SCHEMA_V1,
            snapshot: report.snapshot.clone(),
            baseline_adapter: report.baseline_adapter,
            limit: report.limit,
            totals: VersionedDocumentationCompletenessTotals {
                tracked_files: report.totals.tracked_files,
                processed_files: report.totals.processed_files,
                unprocessed_files: report.totals.unprocessed_files,
                processed_ratio_basis_points: report.totals.processed_ratio_basis_points,
                languages: report.totals.languages,
                adapters: report.totals.adapters,
                facts: report.totals.facts,
                relations: report.totals.relations,
                diagnostics: report.totals.diagnostics,
            },
            languages: report
                .languages
                .iter()
                .map(|row| VersionedDocumentationLanguageCompleteness {
                    language: row.language.clone(),
                    tracked_files: row.tracked_files,
                    processed_files: row.processed_files,
                    unprocessed_files: row.unprocessed_files,
                    processed_ratio_basis_points: row.processed_ratio_basis_points,
                    adapters: row.adapters,
                })
                .collect(),
            adapters: report
                .adapters
                .iter()
                .map(|row| VersionedDocumentationAdapterCompleteness {
                    adapter: row.adapter.clone(),
                    files: row.files,
                    facts: row.facts,
                    relations: row.relations,
                    diagnostics: row.diagnostics,
                })
                .collect(),
            unprocessed_files: report
                .unprocessed_files
                .iter()
                .map(|row| VersionedDocumentationUnprocessedFile {
                    path: row.path.clone(),
                    language: row.language.clone(),
                })
                .collect(),
            omitted: VersionedDocumentationCompletenessOmitted {
                languages: report.omitted.languages,
                adapters: report.omitted.adapters,
                unprocessed_files: report.omitted.unprocessed_files,
            },
        }
    }
}

impl VersionedJsonContract for VersionedDocumentationCompletenessReport {
    const SCHEMA: &'static str = DOCUMENTATION_COMPLETENESS_SCHEMA_V1;

    fn schema(&self) -> &str {
        self.schema
    }
}
