//! Application services for Athanor.

pub mod adapter_contract;
pub mod api;
pub mod api_registry;
pub mod application_report_composition;
mod artifact_checksum;
pub mod automation_contract;
pub mod bench;
pub mod boundary_contract;
pub mod cancellation;
pub mod capabilities;
pub mod change_map;
pub mod check;
pub mod composition;
pub mod config;
#[path = "context_composition.rs"]
pub mod context;
mod context_operation;
pub mod context_report;
pub mod coverage;
pub mod daemon;
mod daemon_client;
mod daemon_command_dispatch;
mod daemon_connection;
mod daemon_derived_read_dispatch;
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
mod daemon_read_dispatch;
#[cfg(test)]
mod daemon_read_dispatch_tests;
mod daemon_recovery;
pub mod daemon_runtime;
mod daemon_watcher;
#[cfg(test)]
mod daemon_write_job_contract_tests;
mod daemon_write_jobs;
pub mod derived_read_operation;
pub mod docs;
#[path = "documentation_architecture_profile_v1.rs"]
pub mod documentation_architecture_profile;
pub mod documentation_architecture_publication;
pub mod documentation_generation_alignment;
pub mod documentation_generation_contract;
pub mod explain;
#[cfg(test)]
mod fact_query_tests;
pub mod generation;
#[path = "graph/mod.rs"]
pub mod graph;
pub mod graph_cooperative;
pub mod graph_operation;
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
pub mod json_contract;
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
mod process_execution_scope;
mod project_path;
pub mod project_registry;
mod projection;
pub mod read_model;
pub mod repair;
pub mod repair_composition;
pub mod report;
pub mod runtime;
pub mod rustok_architecture;
mod rustok_architecture_cooperative;
mod rustok_audit_cooperative;
pub mod rustok_composition_operation;
mod rustok_graph_cooperative;
pub mod rustok_json_contract;
#[path = "search_facade.rs"]
pub mod search;
pub mod search_operation;
#[path = "store_facade.rs"]
pub mod store;
#[cfg(test)]
mod store_publication_cancellation_tests;
#[cfg(test)]
mod test_runtime;
mod transient_store;
pub mod validate_changed;
pub mod wiki;

/// Stable indexing-facing application API.
pub mod indexing {
    pub use crate::index::{
        IndexOptions, IndexReport, IndexReportMetrics, index_project_cancellable_with_composition,
        index_project_cancellable_with_composition_and_operation_context,
        index_project_with_composition, index_project_with_composition_and_operation_context,
    };
    pub use crate::invalidation::*;
    pub use crate::pipeline::{IndexPipeline, IndexPipelineMetrics, IndexPipelineOutput};
}

/// Publication artefacts and lifecycle APIs.
pub mod publication {
    pub use crate::documentation_architecture_publication::*;
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
    pub use crate::context_report::*;
    pub use crate::derived_read_operation::*;
    pub use crate::explain::*;
    pub use crate::graph::*;
    pub use crate::graph_cooperative::*;
    pub use crate::graph_operation::*;
    pub use crate::impact::*;
    pub use crate::overview::*;
    pub use crate::rustok_composition_operation::*;
    pub use crate::rustok_json_contract::*;
    pub use crate::search::*;
    pub use crate::search_operation::*;
}

/// Project registration and repository identity APIs.
pub mod projects {
    pub use crate::project_registry::*;
}

pub use adapter_contract::*;
pub use api::*;
pub use api_registry::{
    API_REGISTRY_SCHEMA, ApiRegistryEndpoint, ApiRegistryOptions, ApiRegistryReport,
};
pub use application_report_composition::*;
pub use automation_contract::*;
pub use bench::*;
pub use boundary_contract::*;
pub use cancellation::*;
pub use capabilities::*;
pub use change_map::*;
pub use check::*;
pub use composition::*;
pub use config::*;
pub use context::*;
pub use context_report::*;
pub use coverage::*;
pub use daemon::*;
pub use daemon_runtime::*;
pub use derived_read_operation::*;
pub use docs::*;
pub use documentation_architecture_profile::*;
pub use documentation_architecture_publication::*;
pub use documentation_generation_alignment::*;
pub use documentation_generation_contract::*;
pub use explain::*;
pub use generation::*;
pub use graph::*;
pub use graph_cooperative::*;
pub use graph_operation::*;
pub use impact::*;
pub use index::*;
pub use index_state::*;
pub use init::*;
pub use invalidation::*;
pub use json_contract::*;
pub use overview::*;
pub use pipeline::*;
pub use prepared_publication::*;
pub use process_execution_scope::{
    CancellableProcessRunner, SharedProcessRunner, default_process_runner, with_process_runner,
};
pub(crate) use process_execution_scope::{
    current_process_execution_context, current_process_runner, with_process_execution_context,
};
pub use project_registry::*;
pub use read_model::*;
pub use repair::*;
pub use repair_composition::*;
pub use report::*;
pub use runtime::*;
pub use rustok_architecture::*;
pub use rustok_composition_operation::*;
pub use rustok_json_contract::*;
pub use search::*;
pub use search_operation::*;
pub use store::*;
pub use validate_changed::*;
pub use wiki::*;
