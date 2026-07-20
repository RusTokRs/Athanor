mod aggregation;
mod execution;
mod model;

pub use aggregation::build_repository_overview;
pub use execution::overview_project_with_composition;
pub use model::{
    ApiOverview, DiagnosticOverview, DocsOverview, EntityOverview, IntegrationBoundaryOverview,
    ModuleOverview, NamedCount, OVERVIEW_SCHEMA, OperationsOverview, OverviewOptions,
    OverviewTotals, RepositoryOverview,
};

#[cfg(test)]
mod tests;
