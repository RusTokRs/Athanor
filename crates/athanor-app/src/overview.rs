mod aggregation;
mod execution;
mod model;

pub use aggregation::build_repository_overview;
pub use execution::overview_project_with_composition;
pub use model::{
    ApiOverview, DiagnosticOverview, DocsOverview, EntityOverview, IntegrationBoundaryOverview,
    ModuleOverview, NamedCount, OperationsOverview, OverviewOptions, OverviewTotals,
    RepositoryOverview, OVERVIEW_SCHEMA,
};

#[cfg(test)]
mod tests;
