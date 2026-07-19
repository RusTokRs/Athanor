mod aggregation;
mod execution;
mod model;

pub use execution::capabilities_project_with_composition;
pub use model::{
    AdapterCapability, CAPABILITIES_REPORT_SCHEMA, CapabilitiesLimits, CapabilitiesOmitted,
    CapabilitiesOptions, CapabilitiesReport, CapabilitiesTotals, DEFAULT_CAPABILITIES_LIMIT,
    DEFAULT_CONFIDENCE_THRESHOLD, LanguageCapability, LowConfidenceFact, UnprocessedFile,
};

#[cfg(test)]
mod tests;
