mod execution;
mod model;
mod traversal;

pub use execution::impact_project_with_composition;
pub use model::{
    FlowDirection, ImpactAnalysis, ImpactOptions, ImpactPathEndpoint, ImpactPathStep,
    ImpactedEntity, RelationFlow,
};
pub use traversal::impact_snapshot;

#[cfg(test)]
mod tests;
