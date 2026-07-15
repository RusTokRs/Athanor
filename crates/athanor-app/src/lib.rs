//! Application services for Athanor.

pub mod api;
pub mod api_registry;
pub mod bench;
pub mod cancellation;
pub mod capabilities;
pub mod change_map;
pub mod check;
pub mod composition;
pub mod config;
pub mod context;
pub mod coverage;
pub mod daemon;
mod daemon_client;
mod daemon_connection;
mod daemon_endpoint;
mod daemon_job_cancellation;
mod daemon_job_registry;
mod daemon_job_scheduler;
mod daemon_job_state;
mod daemon_jobs_support;
mod daemon_lifecycle;
mod daemon_operation;
mod daemon_protocol;
mod daemon_queries;
mod daemon_recovery;
pub mod daemon_runtime;
mod daemon_watcher;
mod daemon_write_jobs;
pub mod docs;
pub mod explain;
#[cfg(test)]
mod fact_query_tests;
pub mod generation;
pub mod graph;
mod hash;
pub mod impact;
#[path = "index_runtime.rs"]
pub mod index;
#[path = "index_publication_atomic.rs"]
mod index_publication;
#[cfg(test)]
#[allow(dead_code)]
#[path = "index_publication_guard.rs"]
mod index_publication_legacy;
#[allow(dead_code)]
#[path = "index_publication.rs"]
mod index_publication_inner;
#[allow(dead_code)]
mod index_publication_journal;
#[cfg(test)]
mod index_publication_atomic_tests;
#[cfg(test)]
mod index_publication_combined_error_tests;
#[cfg(test)]
mod index_publication_content_tests;
#[cfg(test)]
mod index_publication_fault_tests;
#[cfg(test)]
mod index_publication_finalize_tests;
#[cfg(test)]
mod index_publication_recovery_fault_tests;
#[cfg(test)]
mod index_runtime_tests;
pub mod index_state;
pub mod init;
pub mod invalidation;
mod local_source;
pub mod overview;
pub mod pipeline;
mod pipeline_check;
mod pipeline_extract;
mod pipeline_link;
mod pipeline_merge;
mod pipeline_metrics;
mod pipeline_ownership;
mod pipeline_source;
mod pipeline_support;
mod prepared_publication;
mod project_path;
pub mod project_registry;
mod projection;
pub mod read_model;
pub mod repair;
pub mod report;
pub mod runtime;
pub mod rustok_architecture;
pub mod search;
pub mod store;
#[cfg(test)]
mod test_runtime;
mod transient_store;
pub mod validate_changed;
pub mod wiki;

/// Stable indexing-facing application API.
pub mod indexing {
    pub use crate::index::{
        IndexOptions, IndexReport, IndexReportMetrics, index_project, index_project_cancellable,
        index_project_cancellable_with_composition,
        index_project_cancellable_with_composition_and_operation_context,
        index_project_cancellable_with_operation_context, index_project_with_composition,
        index_project_with_operation_context,
    };
    pub use crate::pipeline::{
        AffectedFiles, IncrementalIndexContext, IndexPipeline, IndexPipelineMetrics,
        IndexPipelineOutput,
    };
}

/// Stable publication-facing application API.
pub mod publication {
    pub use crate::index_state::{IndexFileState, IndexState, IndexStateStore};
    pub use crate::prepared_publication::{PreparedSnapshot, PreparedSnapshotPublication};
    pub use crate::read_model::{
        JSONL_MANIFEST_SCHEMA, JsonlReadModelReport, JsonlReadModelWriter,
    };
}

/// Stable read-only query-facing application API.
pub mod query {
    pub use athanor_core::{FactQuery, FactQueryStore};

    pub use crate::capabilities::{CapabilitiesOptions, CapabilitiesReport, capabilities_project};
    pub use crate::change_map::{ChangeMapOptions, ChangeMapReport, change_map_project};
    pub use crate::context::{ContextOptions, ContextPack, context_project};
    pub use crate::coverage::{CoverageOptions, CoverageReport, coverage_project};
    pub use crate::explain::{ExplainOptions, ExplainReport, explain_project};
    pub use crate::impact::{ImpactOptions, ImpactReport, impact_project};
    pub use crate::overview::{OverviewOptions, OverviewReport, overview_project};
    pub use crate::search::{SearchOptions, SearchReport, search_project};
}

/// Stable project-registry-facing application API.
pub mod projects {
    pub use crate::project_registry::{
        ProjectRegistryEntry, ProjectRegistryReport, ProjectRegistrationOptions,
        ProjectResolutionOptions, list_registered_projects, register_project,
        resolve_registered_project, unregister_project,
    };
}
