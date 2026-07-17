//! Stable, versioned agent-facing JSON contract registry.
//!
//! The original implementation remains isolated in `json_contract_base.rs`.
//! This facade extends the public registry with additive response wrappers and
//! report and transport families without rewriting the validation machinery.

#[path = "json_contract_base.rs"]
mod base;

pub use base::{
    AFFECTED_CHECK_SCHEMA_V1, API_CLEANUP_SCHEMA_V1, API_CONTRACT_DIFF_SCHEMA_V2,
    CAPABILITIES_SCHEMA_V1, CHANGED_VALIDATION_SCHEMA_V1, CHANGE_MAP_SCHEMA_V1,
    CONFIG_DOCTOR_SCHEMA_V1, CONFIG_VALIDATE_SCHEMA_V1, CONTEXT_PACK_SCHEMA_V1,
    COVERAGE_SCHEMA_V1, DIAGNOSTIC_CHECK_SCHEMA_V1, DOCS_APPLY_PATCH_SCHEMA_V1,
    DOCS_CHECK_SCHEMA_V1, DOCS_DRIFT_SCHEMA_V1, ENTITY_EXPLANATION_SCHEMA_V1,
    GENERATION_SCHEMA_V1, GRAPH_CYCLES_SCHEMA_V1, GRAPH_EXPORT_SCHEMA_V1,
    GRAPH_HUBS_SCHEMA_V1, GRAPH_PAGERANK_SCHEMA_V1, GRAPH_PATH_SCHEMA_V1,
    GRAPH_RELATED_SCHEMA_V1, HTML_REPORT_SCHEMA_V1, IMPACT_ANALYSIS_SCHEMA_V1,
    INDEX_BENCHMARK_SCHEMA_V1, INDEX_REPORT_SCHEMA_V1, JsonContractDescriptor,
    JsonContractError, OPERATIONS_DOCS_CHECK_SCHEMA_V1, OVERVIEW_SCHEMA_V1,
    PROJECT_REGISTRY_REPORT_SCHEMA_V1, PROJECT_RESOLUTION_SCHEMA_V1,
    RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA_V1, RUSTOK_FBA_AUDIT_SCHEMA_V1,
    RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA_V1, RUSTOK_FBA_MODULE_GRAPH_SCHEMA_V1,
    RUSTOK_FBA_PORT_GRAPH_SCHEMA_V1, RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA_V1,
    RUSTOK_FFA_AUDIT_SCHEMA_V1, RUSTOK_FFA_SURFACE_GRAPH_SCHEMA_V1,
    RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA_V1, RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA_V1,
    RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA_V1,
    RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA_V1,
    RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA_V1, SEARCH_SCHEMA_V1,
    VersionedJsonContract, WIKI_REPORT_SCHEMA_V1, validate_contract_registry,
    validate_contract_value, validate_schema_id,
};

pub use crate::adapter_contract::{
    ADAPTER_TRUST_REPORT_SCHEMA_V1, VersionedAdapterTrustReport,
};

#[path = "response_contract.rs"]
mod response_contract;

pub use response_contract::{
    API_SNAPSHOT_SCHEMA_V1, DOCS_PROPOSE_FIX_SCHEMA_V1, VersionedApiSnapshotReport,
    VersionedDocsProposeFixReport,
};

#[path = "repair_contract.rs"]
mod repair_contract;

pub use repair_contract::{
    INDEX_GENERATION_CLEANUP_SCHEMA_V1, REPAIR_APPLY_SCHEMA_V2,
    REPAIR_CANONICAL_LATEST_SCHEMA_V1, REPAIR_CLEANUP_SCHEMA_V2,
    REPAIR_INSPECT_SCHEMA_V2, REPAIR_RECOVER_CANONICAL_SCHEMA_V1,
    REPAIR_RECOVER_INDEX_CLEANUP_SCHEMA_V1, REPAIR_RECOVER_INDEX_SCHEMA_V1,
    REPAIR_REGENERATE_SCHEMA_V1,
};

#[path = "daemon_contract.rs"]
mod daemon_contract;

pub use daemon_contract::{
    DAEMON_JOBS_CONTRACT_SCHEMA_V1, DAEMON_REQUEST_CONTRACT_SCHEMA_V3,
    DAEMON_RESPONSE_CONTRACT_SCHEMA_V3,
};

macro_rules! descriptor {
    ($schema:ident, $rust_type:literal) => {
        JsonContractDescriptor {
            schema: $schema,
            rust_type: $rust_type,
        }
    };
}

pub const VERSIONED_JSON_CONTRACTS: &[JsonContractDescriptor] = &[
    descriptor!(OVERVIEW_SCHEMA_V1, "RepositoryOverview"),
    descriptor!(SEARCH_SCHEMA_V1, "SearchReport"),
    descriptor!(ENTITY_EXPLANATION_SCHEMA_V1, "EntityExplanation"),
    descriptor!(IMPACT_ANALYSIS_SCHEMA_V1, "ImpactAnalysis"),
    descriptor!(DIAGNOSTIC_CHECK_SCHEMA_V1, "DiagnosticCheckReport"),
    descriptor!(AFFECTED_CHECK_SCHEMA_V1, "AffectedCheckReport"),
    descriptor!(OPERATIONS_DOCS_CHECK_SCHEMA_V1, "OperationsDocsCheckReport"),
    descriptor!(COVERAGE_SCHEMA_V1, "CoverageReport"),
    descriptor!(CAPABILITIES_SCHEMA_V1, "CapabilitiesReport"),
    descriptor!(CHANGE_MAP_SCHEMA_V1, "ChangeMapReport"),
    descriptor!(CONTEXT_PACK_SCHEMA_V1, "ContextReport"),
    descriptor!(INDEX_REPORT_SCHEMA_V1, "IndexReport"),
    descriptor!(INDEX_BENCHMARK_SCHEMA_V1, "BenchmarkReport"),
    descriptor!(CHANGED_VALIDATION_SCHEMA_V1, "ChangedValidationReport"),
    descriptor!(GENERATION_SCHEMA_V1, "GenerationReport"),
    descriptor!(CONFIG_VALIDATE_SCHEMA_V1, "ConfigValidateReport"),
    descriptor!(CONFIG_DOCTOR_SCHEMA_V1, "ConfigDoctorReport"),
    descriptor!(DOCS_CHECK_SCHEMA_V1, "DocsCheckReport"),
    descriptor!(DOCS_DRIFT_SCHEMA_V1, "DocsDriftReport"),
    descriptor!(DOCS_APPLY_PATCH_SCHEMA_V1, "DocsApplyPatchReport"),
    descriptor!(DOCS_PROPOSE_FIX_SCHEMA_V1, "VersionedDocsProposeFixReport"),
    descriptor!(API_SNAPSHOT_SCHEMA_V1, "VersionedApiSnapshotReport"),
    descriptor!(API_CONTRACT_DIFF_SCHEMA_V2, "ApiContractDiff"),
    descriptor!(API_CLEANUP_SCHEMA_V1, "ApiCleanupReport"),
    descriptor!(ADAPTER_TRUST_REPORT_SCHEMA_V1, "VersionedAdapterTrustReport"),
    descriptor!(REPAIR_INSPECT_SCHEMA_V2, "RepairInspectReport"),
    descriptor!(REPAIR_CLEANUP_SCHEMA_V2, "RepairCleanupReport"),
    descriptor!(REPAIR_REGENERATE_SCHEMA_V1, "RepairRegenerateReport"),
    descriptor!(
        REPAIR_RECOVER_CANONICAL_SCHEMA_V1,
        "RepairRecoverCanonicalReport"
    ),
    descriptor!(REPAIR_APPLY_SCHEMA_V2, "RepairApplyReport"),
    descriptor!(
        INDEX_GENERATION_CLEANUP_SCHEMA_V1,
        "IndexGenerationCleanupReport"
    ),
    descriptor!(REPAIR_RECOVER_INDEX_SCHEMA_V1, "RepairRecoverIndexReport"),
    descriptor!(
        REPAIR_RECOVER_INDEX_CLEANUP_SCHEMA_V1,
        "RepairRecoverIndexCleanupReport"
    ),
    descriptor!(
        REPAIR_CANONICAL_LATEST_SCHEMA_V1,
        "RepairCanonicalLatestReport"
    ),
    descriptor!(DAEMON_REQUEST_CONTRACT_SCHEMA_V3, "DaemonRequest"),
    descriptor!(DAEMON_RESPONSE_CONTRACT_SCHEMA_V3, "DaemonResponse"),
    descriptor!(DAEMON_JOBS_CONTRACT_SCHEMA_V1, "DaemonJobsReport"),
    descriptor!(WIKI_REPORT_SCHEMA_V1, "WikiReport"),
    descriptor!(HTML_REPORT_SCHEMA_V1, "HtmlReport"),
    descriptor!(GRAPH_EXPORT_SCHEMA_V1, "GraphExport"),
    descriptor!(GRAPH_RELATED_SCHEMA_V1, "GraphRelated"),
    descriptor!(GRAPH_PATH_SCHEMA_V1, "GraphPath"),
    descriptor!(GRAPH_HUBS_SCHEMA_V1, "GraphHubs"),
    descriptor!(GRAPH_PAGERANK_SCHEMA_V1, "GraphPageRank"),
    descriptor!(GRAPH_CYCLES_SCHEMA_V1, "GraphCycles"),
    descriptor!(RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA_V1, "RustokArchitectureContext"),
    descriptor!(RUSTOK_FFA_AUDIT_SCHEMA_V1, "RustokFfaAudit"),
    descriptor!(RUSTOK_FFA_SURFACE_GRAPH_SCHEMA_V1, "RustokFfaSurfaceGraphReport"),
    descriptor!(RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA_V1, "RustokFfaViolationsGraphReport"),
    descriptor!(RUSTOK_FBA_AUDIT_SCHEMA_V1, "RustokFbaAudit"),
    descriptor!(RUSTOK_FBA_MODULE_GRAPH_SCHEMA_V1, "RustokFbaModuleGraphReport"),
    descriptor!(RUSTOK_FBA_PORT_GRAPH_SCHEMA_V1, "RustokFbaPortGraphReport"),
    descriptor!(
        RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA_V1,
        "RustokFbaDependenciesGraphReport"
    ),
    descriptor!(
        RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA_V1,
        "RustokFbaViolationsGraphReport"
    ),
    descriptor!(RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA_V1, "RustokPageBuilderAudit"),
    descriptor!(
        RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA_V1,
        "RustokPageBuilderProviderGraphReport"
    ),
    descriptor!(
        RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA_V1,
        "RustokPageBuilderConsumerGraphReport"
    ),
    descriptor!(
        RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA_V1,
        "RustokPageBuilderViolationsGraphReport"
    ),
    descriptor!(PROJECT_REGISTRY_REPORT_SCHEMA_V1, "ProjectRegistryReport"),
    descriptor!(PROJECT_RESOLUTION_SCHEMA_V1, "ProjectResolutionReport"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_registry_contains_application_daemon_and_adapter_owners() {
        validate_contract_registry(VERSIONED_JSON_CONTRACTS)
            .expect("extended JSON contract registry must remain valid");
        assert_eq!(VERSIONED_JSON_CONTRACTS.len(), 60);
        assert_eq!(API_SNAPSHOT_SCHEMA_V1, VersionedApiSnapshotReport::SCHEMA);
        assert_eq!(
            DOCS_PROPOSE_FIX_SCHEMA_V1,
            VersionedDocsProposeFixReport::SCHEMA
        );
        assert_eq!(
            ADAPTER_TRUST_REPORT_SCHEMA_V1,
            VersionedAdapterTrustReport::SCHEMA
        );
        assert_eq!(
            REPAIR_INSPECT_SCHEMA_V2,
            crate::repair::RepairInspectReport::SCHEMA
        );
        assert_eq!(
            REPAIR_CANONICAL_LATEST_SCHEMA_V1,
            crate::repair::RepairCanonicalLatestReport::SCHEMA
        );
        assert_eq!(
            DAEMON_REQUEST_CONTRACT_SCHEMA_V3,
            crate::daemon::DaemonRequest::SCHEMA
        );
        assert_eq!(
            DAEMON_RESPONSE_CONTRACT_SCHEMA_V3,
            crate::daemon::DaemonResponse::SCHEMA
        );
    }
}
