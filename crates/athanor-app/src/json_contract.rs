//! Shared contracts for stable, versioned agent-facing JSON documents.
//!
//! JSON payloads remain owned by their application use cases. This module owns
//! the cross-cutting rules that make those payloads safe to consume across CLI,
//! daemon, and MCP boundaries without duplicating schema-version validation.

use std::error::Error;
use std::fmt;

use serde::Serialize;
use serde_json::Value;

/// Stable schema identifier for repository overview reports.
pub const OVERVIEW_SCHEMA_V1: &str = "athanor.overview.v1";
/// Stable schema identifier for lexical search reports.
pub const SEARCH_SCHEMA_V1: &str = "athanor.search.v1";

/// One registered, externally consumable JSON document contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonContractDescriptor {
    pub schema: &'static str,
    pub rust_type: &'static str,
}

/// Initial registry of application JSON contracts migrated to the shared rules.
pub const VERSIONED_JSON_CONTRACTS: &[JsonContractDescriptor] = &[
    JsonContractDescriptor {
        schema: OVERVIEW_SCHEMA_V1,
        rust_type: "RepositoryOverview",
    },
    JsonContractDescriptor {
        schema: SEARCH_SCHEMA_V1,
        rust_type: "SearchReport",
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

impl VersionedJsonContract for crate::overview::RepositoryOverview {
    const SCHEMA: &'static str = OVERVIEW_SCHEMA_V1;

    fn schema(&self) -> &str {
        &self.schema
    }
}

impl VersionedJsonContract for crate::search::SearchReport {
    const SCHEMA: &'static str = SEARCH_SCHEMA_V1;

    fn schema(&self) -> &str {
        &self.schema
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
    fn registry_contains_unique_valid_schema_ids() {
        let mut schemas = VERSIONED_JSON_CONTRACTS
            .iter()
            .map(|contract| contract.schema)
            .collect::<Vec<_>>();
        for schema in &schemas {
            validate_schema_id(schema).expect("registered schema id must be valid");
        }
        schemas.sort_unstable();
        schemas.dedup();
        assert_eq!(schemas.len(), VERSIONED_JSON_CONTRACTS.len());
        assert_eq!(crate::overview::OVERVIEW_SCHEMA, OVERVIEW_SCHEMA_V1);
    }
}
