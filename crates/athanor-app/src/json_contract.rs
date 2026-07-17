//! Shared contracts for stable, versioned agent-facing JSON documents.
//!
//! JSON payloads remain owned by their application use cases. This module owns
//! the cross-cutting rules that make those payloads safe to consume across CLI,
//! daemon, and MCP boundaries without duplicating schema-version validation.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde::Serialize;
use serde_json::Value;

pub const OVERVIEW_SCHEMA_V1: &str = "athanor.overview.v1";
pub const SEARCH_SCHEMA_V1: &str = "athanor.search.v1";
pub const ENTITY_EXPLANATION_SCHEMA_V1: &str = "athanor.entity_explanation.v1";
pub const IMPACT_ANALYSIS_SCHEMA_V1: &str = "athanor.impact_analysis.v1";
pub const DIAGNOSTIC_CHECK_SCHEMA_V1: &str = "athanor.diagnostic_check.v1";
pub const AFFECTED_CHECK_SCHEMA_V1: &str = "athanor.affected_check.v1";
pub const OPERATIONS_DOCS_CHECK_SCHEMA_V1: &str = "athanor.operations_docs_check.v1";
pub const COVERAGE_SCHEMA_V1: &str = "athanor.coverage.v1";
pub const CAPABILITIES_SCHEMA_V1: &str = "athanor.capabilities.v1";
pub const CHANGE_MAP_SCHEMA_V1: &str = "athanor.change_map.v1";
pub const CONTEXT_PACK_SCHEMA_V1: &str = "athanor.context_pack.v1";
pub const INDEX_REPORT_SCHEMA_V1: &str = crate::index::INDEX_REPORT_SCHEMA;
pub const INDEX_BENCHMARK_SCHEMA_V1: &str = crate::bench::INDEX_BENCHMARK_SCHEMA;
pub const CHANGED_VALIDATION_SCHEMA_V1: &str = crate::validate_changed::CHANGED_VALIDATION_SCHEMA;
pub const GENERATION_SCHEMA_V1: &str = "athanor.generation.v1";
pub const CONFIG_VALIDATE_SCHEMA_V1: &str = crate::config::CONFIG_VALIDATE_SCHEMA;
pub const CONFIG_DOCTOR_SCHEMA_V1: &str = crate::config::CONFIG_DOCTOR_SCHEMA;
pub const DOCS_CHECK_SCHEMA_V1: &str = crate::docs::DOCS_CHECK_SCHEMA;
pub const DOCS_DRIFT_SCHEMA_V1: &str = crate::docs::DOCS_DRIFT_SCHEMA;
pub const DOCS_APPLY_PATCH_SCHEMA_V1: &str = "athanor.docs_apply_patch.v1";
pub const API_CONTRACT_DIFF_SCHEMA_V2: &str = crate::api::API_CONTRACT_DIFF_SCHEMA;
pub const API_CLEANUP_SCHEMA_V1: &str = "athanor.api_cleanup.v1";
pub const WIKI_REPORT_SCHEMA_V1: &str = crate::wiki::WIKI_REPORT_SCHEMA;
pub const HTML_REPORT_SCHEMA_V1: &str = crate::report::HTML_REPORT_SCHEMA;
pub const GRAPH_EXPORT_SCHEMA_V1: &str = crate::graph::GRAPH_EXPORT_SCHEMA;
pub const GRAPH_RELATED_SCHEMA_V1: &str = crate::graph::GRAPH_RELATED_SCHEMA;
pub const GRAPH_PATH_SCHEMA_V1: &str = crate::graph::GRAPH_PATH_SCHEMA;
pub const GRAPH_HUBS_SCHEMA_V1: &str = crate::graph::GRAPH_HUBS_SCHEMA;
pub const GRAPH_PAGERANK_SCHEMA_V1: &str = crate::graph::GRAPH_PAGERANK_SCHEMA;
pub const GRAPH_CYCLES_SCHEMA_V1: &str = crate::graph::GRAPH_CYCLES_SCHEMA;
pub const RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA_V1: &str =
    crate::rustok_architecture::RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA;
pub const RUSTOK_FFA_AUDIT_SCHEMA_V1: &str = crate::graph::RUSTOK_FFA_AUDIT_SCHEMA;
pub const RUSTOK_FFA_SURFACE_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_FFA_SURFACE_GRAPH_SCHEMA;
pub const RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA;
pub const RUSTOK_FBA_AUDIT_SCHEMA_V1: &str = crate::graph::RUSTOK_FBA_AUDIT_SCHEMA;
pub const RUSTOK_FBA_MODULE_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_FBA_MODULE_GRAPH_SCHEMA;
pub const RUSTOK_FBA_PORT_GRAPH_SCHEMA_V1: &str = crate::graph::RUSTOK_FBA_PORT_GRAPH_SCHEMA;
pub const RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA;
pub const RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA;
pub const RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA_V1: &str =
    crate::graph::RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA;
pub const RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA;
pub const RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA;
pub const RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA;
pub const PROJECT_REGISTRY_REPORT_SCHEMA_V1: &str =
    crate::project_registry::PROJECT_REGISTRY_REPORT_SCHEMA;
pub const PROJECT_RESOLUTION_SCHEMA_V1: &str =
    crate::project_registry::PROJECT_RESOLUTION_SCHEMA;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonContractDescriptor {
    pub schema: &'static str,
    pub rust_type: &'static str,
}

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
    descriptor!(API_CONTRACT_DIFF_SCHEMA_V2, "ApiContractDiff"),
    descriptor!(API_CLEANUP_SCHEMA_V1, "ApiCleanupReport"),
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
    descriptor!(RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA_V1, "RustokFbaViolationsGraphReport"),
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

pub trait VersionedJsonContract: Serialize {
    const SCHEMA: &'static str;

    fn schema(&self) -> &str;

    fn validate_contract(&self) -> Result<(), JsonContractError> {
        validate_schema_id(Self::SCHEMA)?;
        if self.schema() != Self::SCHEMA {
            return Err(JsonContractError::SchemaMismatch {
                expected: Self::SCHEMA.to_string(),
                actual: self.schema().to_string(),
            });
        }
        let value = serde_json::to_value(self)
            .map_err(|error| JsonContractError::Serialization(error.to_string()))?;
        validate_contract_value(Self::SCHEMA, &value)
    }
}

macro_rules! impl_owned_schema_contract {
    ($type:path, $schema:ident) => {
        impl VersionedJsonContract for $type {
            const SCHEMA: &'static str = $schema;

            fn schema(&self) -> &str {
                &self.schema
            }
        }
    };
}

macro_rules! impl_static_schema_contract {
    ($type:path, $schema:ident) => {
        impl VersionedJsonContract for $type {
            const SCHEMA: &'static str = $schema;

            fn schema(&self) -> &str {
                self.schema
            }
        }
    };
}

macro_rules! impl_transparent_schema_contract {
    ($type:path, $schema:ident) => {
        impl VersionedJsonContract for $type {
            const SCHEMA: &'static str = $schema;

            fn schema(&self) -> &str {
                &self.as_ref().schema
            }
        }
    };
}

impl_owned_schema_contract!(crate::overview::RepositoryOverview, OVERVIEW_SCHEMA_V1);
impl_owned_schema_contract!(crate::search::SearchReport, SEARCH_SCHEMA_V1);
impl_owned_schema_contract!(crate::explain::EntityExplanation, ENTITY_EXPLANATION_SCHEMA_V1);
impl_owned_schema_contract!(crate::impact::ImpactAnalysis, IMPACT_ANALYSIS_SCHEMA_V1);
impl_owned_schema_contract!(crate::check::DiagnosticCheckReport, DIAGNOSTIC_CHECK_SCHEMA_V1);
impl_owned_schema_contract!(crate::check::AffectedCheckReport, AFFECTED_CHECK_SCHEMA_V1);
impl_owned_schema_contract!(
    crate::check::OperationsDocsCheckReport,
    OPERATIONS_DOCS_CHECK_SCHEMA_V1
);
impl_owned_schema_contract!(crate::change_map::ChangeMapReport, CHANGE_MAP_SCHEMA_V1);
impl_owned_schema_contract!(crate::context_report::ContextReport, CONTEXT_PACK_SCHEMA_V1);
impl VersionedJsonContract for crate::index::IndexReport {
    const SCHEMA: &'static str = INDEX_REPORT_SCHEMA_V1;

    fn schema(&self) -> &str {
        Self::SCHEMA
    }
}
impl_static_schema_contract!(crate::bench::BenchmarkReport, INDEX_BENCHMARK_SCHEMA_V1);
impl_static_schema_contract!(
    crate::validate_changed::ChangedValidationReport,
    CHANGED_VALIDATION_SCHEMA_V1
);
impl_static_schema_contract!(crate::generation::GenerationReport, GENERATION_SCHEMA_V1);
impl_static_schema_contract!(crate::config::ConfigValidateReport, CONFIG_VALIDATE_SCHEMA_V1);
impl_static_schema_contract!(crate::config::ConfigDoctorReport, CONFIG_DOCTOR_SCHEMA_V1);
impl_owned_schema_contract!(crate::docs::DocsCheckReport, DOCS_CHECK_SCHEMA_V1);
impl_owned_schema_contract!(crate::docs::DocsDriftReport, DOCS_DRIFT_SCHEMA_V1);
impl_owned_schema_contract!(crate::docs::DocsApplyPatchReport, DOCS_APPLY_PATCH_SCHEMA_V1);
impl_owned_schema_contract!(crate::api::ApiContractDiff, API_CONTRACT_DIFF_SCHEMA_V2);
impl_owned_schema_contract!(crate::api::ApiCleanupReport, API_CLEANUP_SCHEMA_V1);
impl_static_schema_contract!(crate::wiki::WikiReport, WIKI_REPORT_SCHEMA_V1);
impl_static_schema_contract!(crate::report::HtmlReport, HTML_REPORT_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphExport, GRAPH_EXPORT_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphRelated, GRAPH_RELATED_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphPath, GRAPH_PATH_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphHubs, GRAPH_HUBS_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphPageRank, GRAPH_PAGERANK_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphCycles, GRAPH_CYCLES_SCHEMA_V1);
impl_owned_schema_contract!(
    crate::rustok_architecture::RustokArchitectureContext,
    RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA_V1
);
impl_owned_schema_contract!(crate::graph::RustokFfaAudit, RUSTOK_FFA_AUDIT_SCHEMA_V1);
impl_transparent_schema_contract!(
    crate::rustok_json_contract::RustokFfaSurfaceGraphReport,
    RUSTOK_FFA_SURFACE_GRAPH_SCHEMA_V1
);
impl_transparent_schema_contract!(
    crate::rustok_json_contract::RustokFfaViolationsGraphReport,
    RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA_V1
);
impl_owned_schema_contract!(crate::graph::RustokFbaAudit, RUSTOK_FBA_AUDIT_SCHEMA_V1);
impl_transparent_schema_contract!(
    crate::rustok_json_contract::RustokFbaModuleGraphReport,
    RUSTOK_FBA_MODULE_GRAPH_SCHEMA_V1
);
impl_transparent_schema_contract!(
    crate::rustok_json_contract::RustokFbaPortGraphReport,
    RUSTOK_FBA_PORT_GRAPH_SCHEMA_V1
);
impl_transparent_schema_contract!(
    crate::rustok_json_contract::RustokFbaDependenciesGraphReport,
    RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA_V1
);
impl_transparent_schema_contract!(
    crate::rustok_json_contract::RustokFbaViolationsGraphReport,
    RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA_V1
);
impl_owned_schema_contract!(
    crate::graph::RustokPageBuilderAudit,
    RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA_V1
);
impl_transparent_schema_contract!(
    crate::rustok_json_contract::RustokPageBuilderProviderGraphReport,
    RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA_V1
);
impl_transparent_schema_contract!(
    crate::rustok_json_contract::RustokPageBuilderConsumerGraphReport,
    RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA_V1
);
impl_transparent_schema_contract!(
    crate::rustok_json_contract::RustokPageBuilderViolationsGraphReport,
    RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA_V1
);
impl_owned_schema_contract!(
    crate::project_registry::ProjectRegistryReport,
    PROJECT_REGISTRY_REPORT_SCHEMA_V1
);
impl_owned_schema_contract!(
    crate::project_registry::ProjectResolutionReport,
    PROJECT_RESOLUTION_SCHEMA_V1
);
impl_static_schema_contract!(crate::coverage::CoverageReport, COVERAGE_SCHEMA_V1);
impl_static_schema_contract!(crate::capabilities::CapabilitiesReport, CAPABILITIES_SCHEMA_V1);

pub fn validate_schema_id(schema: &str) -> Result<u32, JsonContractError> {
    let segments = schema.split('.').collect::<Vec<_>>();
    if segments.len() < 3 || segments.first() != Some(&"athanor") {
        return Err(JsonContractError::InvalidSchemaId {
            schema: schema.to_string(),
            reason: "expected athanor.<name>.v<major>",
        });
    }

    let (version, names) = segments
        .split_last()
        .expect("schema segment count was checked above");
    if names[1..].iter().any(|segment| {
        segment.is_empty()
            || !segment.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '_' | '-')
            })
    }) {
        return Err(JsonContractError::InvalidSchemaId {
            schema: schema.to_string(),
            reason: "name segments must use lowercase ASCII, digits, '_' or '-'",
        });
    }

    let major = version
        .strip_prefix('v')
        .filter(|digits| !digits.is_empty() && digits.chars().all(|digit| digit.is_ascii_digit()))
        .and_then(|digits| digits.parse::<u32>().ok())
        .filter(|major| *major > 0)
        .ok_or_else(|| JsonContractError::InvalidSchemaId {
            schema: schema.to_string(),
            reason: "version must be a positive decimal major prefixed with 'v'",
        })?;

    Ok(major)
}

pub fn validate_contract_registry(
    contracts: &[JsonContractDescriptor],
) -> Result<(), JsonContractError> {
    let mut schemas = BTreeSet::new();
    let mut rust_types = BTreeSet::new();

    for contract in contracts {
        validate_schema_id(contract.schema)?;
        if !schemas.insert(contract.schema) {
            return Err(JsonContractError::DuplicateSchemaId {
                schema: contract.schema.to_string(),
            });
        }
        if !rust_types.insert(contract.rust_type) {
            return Err(JsonContractError::DuplicateRustTypeOwner {
                rust_type: contract.rust_type.to_string(),
            });
        }
    }

    Ok(())
}

pub fn validate_contract_value(
    expected_schema: &str,
    value: &Value,
) -> Result<(), JsonContractError> {
    validate_schema_id(expected_schema)?;
    let object = value
        .as_object()
        .ok_or(JsonContractError::TopLevelDocumentRequired)?;
    let actual = object
        .get("schema")
        .ok_or(JsonContractError::MissingSchemaField)?
        .as_str()
        .ok_or(JsonContractError::NonStringSchemaField)?;

    if actual != expected_schema {
        return Err(JsonContractError::SchemaMismatch {
            expected: expected_schema.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonContractError {
    InvalidSchemaId {
        schema: String,
        reason: &'static str,
    },
    DuplicateSchemaId {
        schema: String,
    },
    DuplicateRustTypeOwner {
        rust_type: String,
    },
    TopLevelDocumentRequired,
    MissingSchemaField,
    NonStringSchemaField,
    SchemaMismatch {
        expected: String,
        actual: String,
    },
    Serialization(String),
}

impl fmt::Display for JsonContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchemaId { schema, reason } => {
                write!(formatter, "invalid JSON contract schema `{schema}`: {reason}")
            }
            Self::DuplicateSchemaId { schema } => {
                write!(formatter, "duplicate JSON contract schema owner for `{schema}`")
            }
            Self::DuplicateRustTypeOwner { rust_type } => {
                write!(formatter, "duplicate JSON contract Rust type owner `{rust_type}`")
            }
            Self::TopLevelDocumentRequired => {
                formatter.write_str("versioned JSON contract must serialize as an object")
            }
            Self::MissingSchemaField => {
                formatter.write_str("versioned JSON contract is missing top-level `schema`")
            }
            Self::NonStringSchemaField => {
                formatter.write_str("versioned JSON contract `schema` must be a string")
            }
            Self::SchemaMismatch { expected, actual } => write!(
                formatter,
                "versioned JSON contract schema mismatch: expected `{expected}`, got `{actual}`"
            ),
            Self::Serialization(message) => {
                write!(formatter, "failed to serialize versioned JSON contract: {message}")
            }
        }
    }
}

impl Error for JsonContractError {}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::json;

    use super::*;

    #[derive(Serialize)]
    struct ExampleContract {
        schema: String,
        value: u32,
    }

    impl VersionedJsonContract for ExampleContract {
        const SCHEMA: &'static str = "athanor.example_contract.v2";

        fn schema(&self) -> &str {
            &self.schema
        }
    }

    #[test]
    fn accepts_stable_positive_major_schema_ids() {
        assert_eq!(validate_schema_id("athanor.search.v1"), Ok(1));
        assert_eq!(validate_schema_id("athanor.rustok_architecture.v12"), Ok(12));
        assert_eq!(validate_schema_id("athanor.api-contract.diff.v2"), Ok(2));
    }

    #[test]
    fn rejects_unversioned_or_noncanonical_schema_ids() {
        for schema in [
            "search.v1",
            "athanor.search",
            "athanor.Search.v1",
            "athanor.search.v0",
            "athanor.search.vx",
            "athanor..v1",
        ] {
            assert!(validate_schema_id(schema).is_err(), "accepted `{schema}`");
        }
    }

    #[test]
    fn validates_serialized_schema_field() {
        let contract = ExampleContract {
            schema: ExampleContract::SCHEMA.to_string(),
            value: 7,
        };
        assert_eq!(contract.validate_contract(), Ok(()));
    }

    #[test]
    fn rejects_instance_and_serialized_schema_mismatches() {
        let contract = ExampleContract {
            schema: "athanor.example_contract.v1".to_string(),
            value: 7,
        };
        assert_eq!(
            contract.validate_contract(),
            Err(JsonContractError::SchemaMismatch {
                expected: "athanor.example_contract.v2".to_string(),
                actual: "athanor.example_contract.v1".to_string(),
            })
        );
        assert_eq!(
            validate_contract_value(ExampleContract::SCHEMA, &json!({ "value": 7 })),
            Err(JsonContractError::MissingSchemaField)
        );
    }

    #[test]
    fn registry_contains_unique_valid_schema_and_type_owners() {
        assert_eq!(validate_contract_registry(VERSIONED_JSON_CONTRACTS), Ok(()));
        assert_eq!(VERSIONED_JSON_CONTRACTS.len(), 45);
        assert_eq!(crate::overview::OVERVIEW_SCHEMA, OVERVIEW_SCHEMA_V1);
        assert_eq!(crate::coverage::COVERAGE_REPORT_SCHEMA, COVERAGE_SCHEMA_V1);
        assert_eq!(
            crate::capabilities::CAPABILITIES_REPORT_SCHEMA,
            CAPABILITIES_SCHEMA_V1
        );
        assert_eq!(crate::index::INDEX_REPORT_SCHEMA, INDEX_REPORT_SCHEMA_V1);
        assert_eq!(crate::bench::INDEX_BENCHMARK_SCHEMA, INDEX_BENCHMARK_SCHEMA_V1);
        assert_eq!(
            crate::validate_changed::CHANGED_VALIDATION_SCHEMA,
            CHANGED_VALIDATION_SCHEMA_V1
        );
        assert_eq!(crate::config::CONFIG_VALIDATE_SCHEMA, CONFIG_VALIDATE_SCHEMA_V1);
        assert_eq!(crate::config::CONFIG_DOCTOR_SCHEMA, CONFIG_DOCTOR_SCHEMA_V1);
        assert_eq!(crate::docs::DOCS_CHECK_SCHEMA, DOCS_CHECK_SCHEMA_V1);
        assert_eq!(crate::docs::DOCS_DRIFT_SCHEMA, DOCS_DRIFT_SCHEMA_V1);
        assert_eq!(crate::api::API_CONTRACT_DIFF_SCHEMA, API_CONTRACT_DIFF_SCHEMA_V2);
        assert_eq!(crate::wiki::WIKI_REPORT_SCHEMA, WIKI_REPORT_SCHEMA_V1);
        assert_eq!(crate::report::HTML_REPORT_SCHEMA, HTML_REPORT_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_EXPORT_SCHEMA, GRAPH_EXPORT_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_RELATED_SCHEMA, GRAPH_RELATED_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_PATH_SCHEMA, GRAPH_PATH_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_HUBS_SCHEMA, GRAPH_HUBS_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_PAGERANK_SCHEMA, GRAPH_PAGERANK_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_CYCLES_SCHEMA, GRAPH_CYCLES_SCHEMA_V1);
        assert_eq!(
            crate::rustok_architecture::RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA,
            RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA_V1
        );
        assert_eq!(crate::graph::RUSTOK_FFA_AUDIT_SCHEMA, RUSTOK_FFA_AUDIT_SCHEMA_V1);
        assert_eq!(
            crate::graph::RUSTOK_FFA_SURFACE_GRAPH_SCHEMA,
            RUSTOK_FFA_SURFACE_GRAPH_SCHEMA_V1
        );
        assert_eq!(
            crate::graph::RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA,
            RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA_V1
        );
        assert_eq!(crate::graph::RUSTOK_FBA_AUDIT_SCHEMA, RUSTOK_FBA_AUDIT_SCHEMA_V1);
        assert_eq!(
            crate::graph::RUSTOK_FBA_MODULE_GRAPH_SCHEMA,
            RUSTOK_FBA_MODULE_GRAPH_SCHEMA_V1
        );
        assert_eq!(
            crate::graph::RUSTOK_FBA_PORT_GRAPH_SCHEMA,
            RUSTOK_FBA_PORT_GRAPH_SCHEMA_V1
        );
        assert_eq!(
            crate::graph::RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA,
            RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA_V1
        );
        assert_eq!(
            crate::graph::RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA,
            RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA_V1
        );
        assert_eq!(
            crate::graph::RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA,
            RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA_V1
        );
        assert_eq!(
            crate::graph::RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA,
            RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA_V1
        );
        assert_eq!(
            crate::graph::RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA,
            RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA_V1
        );
        assert_eq!(
            crate::graph::RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA,
            RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA_V1
        );
        assert_eq!(
            crate::project_registry::PROJECT_REGISTRY_REPORT_SCHEMA,
            PROJECT_REGISTRY_REPORT_SCHEMA_V1
        );
        assert_eq!(
            crate::project_registry::PROJECT_RESOLUTION_SCHEMA,
            PROJECT_RESOLUTION_SCHEMA_V1
        );
    }

    #[test]
    fn registry_rejects_duplicate_schema_and_type_owners() {
        let duplicate_schema = [
            JsonContractDescriptor {
                schema: "athanor.example.v1",
                rust_type: "ExampleV1",
            },
            JsonContractDescriptor {
                schema: "athanor.example.v1",
                rust_type: "ExampleAlias",
            },
        ];
        assert_eq!(
            validate_contract_registry(&duplicate_schema),
            Err(JsonContractError::DuplicateSchemaId {
                schema: "athanor.example.v1".to_string(),
            })
        );

        let duplicate_type = [
            JsonContractDescriptor {
                schema: "athanor.example.v1",
                rust_type: "Example",
            },
            JsonContractDescriptor {
                schema: "athanor.example.v2",
                rust_type: "Example",
            },
        ];
        assert_eq!(
            validate_contract_registry(&duplicate_type),
            Err(JsonContractError::DuplicateRustTypeOwner {
                rust_type: "Example".to_string(),
            })
        );
    }
}
