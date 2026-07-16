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

/// Stable schema identifier for repository overview reports.
pub const OVERVIEW_SCHEMA_V1: &str = "athanor.overview.v1";
/// Stable schema identifier for lexical search reports.
pub const SEARCH_SCHEMA_V1: &str = "athanor.search.v1";
/// Stable schema identifier for canonical entity explanation reports.
pub const ENTITY_EXPLANATION_SCHEMA_V1: &str = "athanor.entity_explanation.v1";
/// Stable schema identifier for code impact analysis reports.
pub const IMPACT_ANALYSIS_SCHEMA_V1: &str = "athanor.impact_analysis.v1";
/// Stable schema identifier for scoped diagnostic check reports.
pub const DIAGNOSTIC_CHECK_SCHEMA_V1: &str = "athanor.diagnostic_check.v1";
/// Stable schema identifier for affected-file check reports.
pub const AFFECTED_CHECK_SCHEMA_V1: &str = "athanor.affected_check.v1";
/// Stable schema identifier for aggregated operations documentation check reports.
pub const OPERATIONS_DOCS_CHECK_SCHEMA_V1: &str = "athanor.operations_docs_check.v1";
/// Stable schema identifier for analysis coverage reports.
pub const COVERAGE_SCHEMA_V1: &str = "athanor.coverage.v1";
/// Stable schema identifier for analysis capability reports.
pub const CAPABILITIES_SCHEMA_V1: &str = "athanor.capabilities.v1";
/// Stable schema identifier for bounded change-map reports.
pub const CHANGE_MAP_SCHEMA_V1: &str = "athanor.change_map.v1";
/// Stable schema identifier for task-focused context reports.
pub const CONTEXT_PACK_SCHEMA_V1: &str = "athanor.context_pack.v1";
/// Stable schema identifier for graph export reports.
pub const GRAPH_EXPORT_SCHEMA_V1: &str = crate::graph::GRAPH_EXPORT_SCHEMA;
/// Stable schema identifier for graph related-entity reports.
pub const GRAPH_RELATED_SCHEMA_V1: &str = crate::graph::GRAPH_RELATED_SCHEMA;
/// Stable schema identifier for graph path reports.
pub const GRAPH_PATH_SCHEMA_V1: &str = crate::graph::GRAPH_PATH_SCHEMA;
/// Stable schema identifier for graph hub reports.
pub const GRAPH_HUBS_SCHEMA_V1: &str = crate::graph::GRAPH_HUBS_SCHEMA;
/// Stable schema identifier for graph PageRank reports.
pub const GRAPH_PAGERANK_SCHEMA_V1: &str = crate::graph::GRAPH_PAGERANK_SCHEMA;
/// Stable schema identifier for graph cycle reports.
pub const GRAPH_CYCLES_SCHEMA_V1: &str = crate::graph::GRAPH_CYCLES_SCHEMA;
/// Stable schema identifier for RusTok FFA audit reports.
pub const RUSTOK_FFA_AUDIT_SCHEMA_V1: &str = crate::graph::RUSTOK_FFA_AUDIT_SCHEMA;
/// Stable schema identifier for RusTok FFA surface graph reports.
pub const RUSTOK_FFA_SURFACE_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_FFA_SURFACE_GRAPH_SCHEMA;
/// Stable schema identifier for RusTok FFA violations graph reports.
pub const RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA_V1: &str =
    crate::graph::RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA;
/// Stable schema identifier for public project-registry reports.
pub const PROJECT_REGISTRY_REPORT_SCHEMA_V1: &str =
    crate::project_registry::PROJECT_REGISTRY_REPORT_SCHEMA;
/// Stable schema identifier for project resolution reports.
pub const PROJECT_RESOLUTION_SCHEMA_V1: &str =
    crate::project_registry::PROJECT_RESOLUTION_SCHEMA;

/// One registered, externally consumable JSON document contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonContractDescriptor {
    pub schema: &'static str,
    pub rust_type: &'static str,
}

/// Application JSON contracts migrated to the shared ownership and validation rules.
pub const VERSIONED_JSON_CONTRACTS: &[JsonContractDescriptor] = &[
    JsonContractDescriptor {
        schema: OVERVIEW_SCHEMA_V1,
        rust_type: "RepositoryOverview",
    },
    JsonContractDescriptor {
        schema: SEARCH_SCHEMA_V1,
        rust_type: "SearchReport",
    },
    JsonContractDescriptor {
        schema: ENTITY_EXPLANATION_SCHEMA_V1,
        rust_type: "EntityExplanation",
    },
    JsonContractDescriptor {
        schema: IMPACT_ANALYSIS_SCHEMA_V1,
        rust_type: "ImpactAnalysis",
    },
    JsonContractDescriptor {
        schema: DIAGNOSTIC_CHECK_SCHEMA_V1,
        rust_type: "DiagnosticCheckReport",
    },
    JsonContractDescriptor {
        schema: AFFECTED_CHECK_SCHEMA_V1,
        rust_type: "AffectedCheckReport",
    },
    JsonContractDescriptor {
        schema: OPERATIONS_DOCS_CHECK_SCHEMA_V1,
        rust_type: "OperationsDocsCheckReport",
    },
    JsonContractDescriptor {
        schema: COVERAGE_SCHEMA_V1,
        rust_type: "CoverageReport",
    },
    JsonContractDescriptor {
        schema: CAPABILITIES_SCHEMA_V1,
        rust_type: "CapabilitiesReport",
    },
    JsonContractDescriptor {
        schema: CHANGE_MAP_SCHEMA_V1,
        rust_type: "ChangeMapReport",
    },
    JsonContractDescriptor {
        schema: CONTEXT_PACK_SCHEMA_V1,
        rust_type: "ContextReport",
    },
    JsonContractDescriptor {
        schema: GRAPH_EXPORT_SCHEMA_V1,
        rust_type: "GraphExport",
    },
    JsonContractDescriptor {
        schema: GRAPH_RELATED_SCHEMA_V1,
        rust_type: "GraphRelated",
    },
    JsonContractDescriptor {
        schema: GRAPH_PATH_SCHEMA_V1,
        rust_type: "GraphPath",
    },
    JsonContractDescriptor {
        schema: GRAPH_HUBS_SCHEMA_V1,
        rust_type: "GraphHubs",
    },
    JsonContractDescriptor {
        schema: GRAPH_PAGERANK_SCHEMA_V1,
        rust_type: "GraphPageRank",
    },
    JsonContractDescriptor {
        schema: GRAPH_CYCLES_SCHEMA_V1,
        rust_type: "GraphCycles",
    },
    JsonContractDescriptor {
        schema: RUSTOK_FFA_AUDIT_SCHEMA_V1,
        rust_type: "RustokFfaAudit",
    },
    JsonContractDescriptor {
        schema: RUSTOK_FFA_SURFACE_GRAPH_SCHEMA_V1,
        rust_type: "RustokFfaSurfaceGraphReport",
    },
    JsonContractDescriptor {
        schema: RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA_V1,
        rust_type: "RustokFfaViolationsGraphReport",
    },
    JsonContractDescriptor {
        schema: PROJECT_REGISTRY_REPORT_SCHEMA_V1,
        rust_type: "ProjectRegistryReport",
    },
    JsonContractDescriptor {
        schema: PROJECT_RESOLUTION_SCHEMA_V1,
        rust_type: "ProjectResolutionReport",
    },
];

/// A serializable document whose top-level `schema` field is a stable contract.
pub trait VersionedJsonContract: Serialize {
    /// Expected schema identifier for this Rust document type.
    const SCHEMA: &'static str;

    /// Schema identifier carried by this document instance.
    fn schema(&self) -> &str;

    /// Validates both the identifier format and the serialized top-level field.
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

impl_owned_schema_contract!(crate::overview::RepositoryOverview, OVERVIEW_SCHEMA_V1);
impl_owned_schema_contract!(crate::search::SearchReport, SEARCH_SCHEMA_V1);
impl_owned_schema_contract!(
    crate::explain::EntityExplanation,
    ENTITY_EXPLANATION_SCHEMA_V1
);
impl_owned_schema_contract!(crate::impact::ImpactAnalysis, IMPACT_ANALYSIS_SCHEMA_V1);
impl_owned_schema_contract!(
    crate::check::DiagnosticCheckReport,
    DIAGNOSTIC_CHECK_SCHEMA_V1
);
impl_owned_schema_contract!(crate::check::AffectedCheckReport, AFFECTED_CHECK_SCHEMA_V1);
impl_owned_schema_contract!(
    crate::check::OperationsDocsCheckReport,
    OPERATIONS_DOCS_CHECK_SCHEMA_V1
);
impl_owned_schema_contract!(crate::change_map::ChangeMapReport, CHANGE_MAP_SCHEMA_V1);
impl_owned_schema_contract!(crate::context_report::ContextReport, CONTEXT_PACK_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphExport, GRAPH_EXPORT_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphRelated, GRAPH_RELATED_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphPath, GRAPH_PATH_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphHubs, GRAPH_HUBS_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphPageRank, GRAPH_PAGERANK_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::GraphCycles, GRAPH_CYCLES_SCHEMA_V1);
impl_owned_schema_contract!(crate::graph::RustokFfaAudit, RUSTOK_FFA_AUDIT_SCHEMA_V1);
impl_owned_schema_contract!(
    crate::rustok_json_contract::RustokFfaSurfaceGraphReport,
    RUSTOK_FFA_SURFACE_GRAPH_SCHEMA_V1
);
impl_owned_schema_contract!(
    crate::rustok_json_contract::RustokFfaViolationsGraphReport,
    RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA_V1
);
impl_owned_schema_contract!(
    crate::project_registry::ProjectRegistryReport,
    PROJECT_REGISTRY_REPORT_SCHEMA_V1
);
impl_owned_schema_contract!(
    crate::project_registry::ProjectResolutionReport,
    PROJECT_RESOLUTION_SCHEMA_V1
);

impl VersionedJsonContract for crate::coverage::CoverageReport {
    const SCHEMA: &'static str = COVERAGE_SCHEMA_V1;

    fn schema(&self) -> &str {
        self.schema
    }
}

impl VersionedJsonContract for crate::capabilities::CapabilitiesReport {
    const SCHEMA: &'static str = CAPABILITIES_SCHEMA_V1;

    fn schema(&self) -> &str {
        self.schema
    }
}

/// Validates an Athanor schema id and returns its positive major version.
///
/// Accepted identifiers follow `athanor.<name>[.<name>...].v<major>`, where
/// name segments contain lowercase ASCII letters, digits, `_`, or `-`.
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

/// Validates that every registered schema and Rust owner is unique.
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

/// Validates the top-level `schema` field of a serialized JSON document.
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
        assert_eq!(VERSIONED_JSON_CONTRACTS.len(), 22);
        assert_eq!(crate::overview::OVERVIEW_SCHEMA, OVERVIEW_SCHEMA_V1);
        assert_eq!(crate::coverage::COVERAGE_REPORT_SCHEMA, COVERAGE_SCHEMA_V1);
        assert_eq!(
            crate::capabilities::CAPABILITIES_REPORT_SCHEMA,
            CAPABILITIES_SCHEMA_V1
        );
        assert_eq!(crate::graph::GRAPH_EXPORT_SCHEMA, GRAPH_EXPORT_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_RELATED_SCHEMA, GRAPH_RELATED_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_PATH_SCHEMA, GRAPH_PATH_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_HUBS_SCHEMA, GRAPH_HUBS_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_PAGERANK_SCHEMA, GRAPH_PAGERANK_SCHEMA_V1);
        assert_eq!(crate::graph::GRAPH_CYCLES_SCHEMA, GRAPH_CYCLES_SCHEMA_V1);
        assert_eq!(crate::graph::RUSTOK_FFA_AUDIT_SCHEMA, RUSTOK_FFA_AUDIT_SCHEMA_V1);
        assert_eq!(
            crate::graph::RUSTOK_FFA_SURFACE_GRAPH_SCHEMA,
            RUSTOK_FFA_SURFACE_GRAPH_SCHEMA_V1
        );
        assert_eq!(
            crate::graph::RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA,
            RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA_V1
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
