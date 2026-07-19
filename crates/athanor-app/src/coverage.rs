mod aggregation;
mod execution;
mod model;

pub use execution::coverage_project_with_composition;
pub use model::{
    AdapterCoverage, COVERAGE_REPORT_SCHEMA, CoverageFilters, CoverageLimits, CoverageOmitted,
    CoverageOptions, CoverageReport, CoverageTotals, DiagnosticCoverage, FileCoverage,
};

#[cfg(test)]
mod tests;
