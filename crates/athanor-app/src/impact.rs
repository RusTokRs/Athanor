mod execution;
mod model;
mod traversal;

pub use execution::impact_project_with_composition;
pub(crate) use execution::resolve_target_entities;
pub use model::{
    FlowDirection, ImpactAnalysis, ImpactOptions, ImpactPathEndpoint, ImpactPathStep,
    ImpactedEntity, RelationFlow,
};
pub use traversal::impact_snapshot;

#[cfg(test)]
mod heritage_tests;
#[cfg(test)]
mod tests;
