mod affected;
mod diagnostics;
mod execution;
mod model;

pub use diagnostics::build_check_report;
pub(crate) use diagnostics::diagnostic_matches_scope;
pub use execution::{
    check_affected_with_composition, check_operations_docs_with_composition,
    check_project_with_composition,
};
pub use model::{
    AffectedArtifactKind, AffectedArtifactStatus, AffectedCheckOptions, AffectedCheckReport,
    AffectedFileCounts, DiagnosticCheckOptions, DiagnosticCheckReport, DiagnosticCounts,
    DiagnosticScope, OperationsDocsCheckOptions, OperationsDocsCheckReport,
};

#[cfg(test)]
mod tests;
