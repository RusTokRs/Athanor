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
mod index_current;
#[cfg(test)]
mod index_current_runtime_tests;
#[path = "index_publication.rs"]
mod index_publication;
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
mod index_publication_journal;
#[cfg(test)]
mod index_publication_recovery_fault_tests;
#[cfg(test)]
mod index_runtime_tests;
#[path = "index_state_pointer.rs"]
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
#[path = "repair_latest.rs"]
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
    pub use crate::invalidation::*;
    pub use crate::pipeline::{IndexPipeline, IndexPipelineMetrics, IndexPipelineOutput};
}

/// Publication artefacts and lifecycle APIs.
pub mod publication {
    pub use crate::index_current::{resolve_index_state_path, resolve_read_model_path};
    pub use crate::index_state::{IndexState, IndexStateStore, PreparedIndexState};
    pub use crate::prepared_publication::{PreparedSnapshot, PreparedSnapshotPublication};
    pub use crate::read_model::{
        JsonlReadModelReport, JsonlReadModelWriter, PreparedJsonlReadModel,
    };
}

/// Read-only canonical knowledge query use cases and backend-neutral request contracts.
pub mod query {
    pub use athanor_core::{FactQuery, FactQueryStore};

    pub use crate::context::*;
    pub use crate::explain::*;
    pub use crate::graph::*;
    pub use crate::impact::*;
    pub use crate::overview::*;
    pub use crate::search::*;
}

/// Project registration and repository identity APIs.
pub mod projects {
    pub use crate::project_registry::*;
}

pub use api::*;
pub use api_registry::*;
pub use bench::*;
pub use cancellation::*;
pub use capabilities::*;
pub use change_map::*;
pub use check::*;
pub use composition::*;
pub use config::*;
pub use context::*;
pub use coverage::*;
pub use daemon::*;
pub use daemon_runtime::*;
pub use docs::*;
pub use explain::*;
pub use generation::*;
pub use graph::*;
pub use impact::*;
pub use index::*;
pub use index_state::*;
pub use init::*;
pub use invalidation::*;
pub use overview::*;
pub use pipeline::*;
pub use prepared_publication::*;
pub use project_registry::*;
pub use projection::{install_html_projector_factory, install_wiki_projector_factory};
pub use read_model::*;
pub use repair::*;
pub use report::*;
pub use runtime::*;
pub use rustok_architecture::*;
pub use search::*;
pub use store::*;
pub use validate_changed::*;
pub use wiki::*;

#[cfg(test)]
pub(crate) fn ensure_test_runtime() {
    test_runtime::install();
}
