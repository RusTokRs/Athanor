//! Versioned contracts for adapter plugin manifests and trust state.
//!
//! Adapter manifests and persisted trust registries are non-public boundaries.
//! The trust status report is a public application document. Legacy schema ids
//! remain accepted only at read boundaries and are normalized before use.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

use crate::boundary_contract::{
    BoundaryLifecycle, JsonBoundaryClass, NonPublicJsonContractDescriptor,
};
use crate::json_contract::{
    JsonContractDescriptor, VersionedJsonContract, validate_schema_id,
};
use crate::runtime::{
    AdapterPluginManifest, AdapterTrustListOptions, AdapterTrustOptions, AdapterTrustRegistry,
    AdapterTrustStatus, list_adapter_plugin_trust, trust_adapter_plugin, untrust_adapter_plugin,
};

pub const ADAPTER_MANIFEST_SCHEMA_V1: &str = "athanor.adapter_manifest.v1";
pub const ADAPTER_MANIFEST_SCHEMA_LEGACY: &str = "athanor.adapter_manifest";
pub const ADAPTER_TRUST_REGISTRY_SCHEMA_V2: &str = "athanor.adapter_trust_registry.v2";
pub const ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2: &str = "athanor.adapter_trust.v2";
pub const ADAPTER_TRUST_REPORT_SCHEMA_V1: &str = "athanor.adapter_trust_report.v1";

pub const ADAPTER_NON_PUBLIC_JSON_CONTRACTS: &[NonPublicJsonContractDescriptor] = &[
    NonPublicJsonContractDescriptor {
        schema: ADAPTER_MANIFEST_SCHEMA_V1,
        rust_owner: "AdapterPluginManifest",
        class: JsonBoundaryClass::Interchange,
        lifecycle: BoundaryLifecycle::Current,
        required_fields: &["schema", "name", "adapters"],
    },
    NonPublicJsonContractDescriptor {
        schema: ADAPTER_MANIFEST_SCHEMA_LEGACY,
        rust_owner: "AdapterPluginManifest",
        class: JsonBoundaryClass::Interchange,
        lifecycle: BoundaryLifecycle::LegacyInput,
        required_fields: &["schema"],
    },
    NonPublicJsonContractDescriptor {
        schema: ADAPTER_TRUST_REGISTRY_SCHEMA_V2,
        rust_owner: "AdapterTrustRegistry",
        class: JsonBoundaryClass::Persisted,
        lifecycle: BoundaryLifecycle::Current,
        required_fields: &["schema", "trusted_plugins"],
    },
    NonPublicJsonContractDescriptor {
        schema: ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2,
        rust_owner: "AdapterTrustRegistry",
        class: JsonBoundaryClass::Persisted,
        lifecycle: BoundaryLifecycle::LegacyInput,
        required_fields: &["schema"],
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterContractError {
    UnsupportedManifestSchema(String),
    UnsupportedTrustRegistrySchema(String),
    Inventory(String),
}

impl fmt::Display for AdapterContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedManifestSchema(schema) => write!(
                formatter,
                "unsupported adapter plugin manifest schema `{schema}`; expected `{ADAPTER_MANIFEST_SCHEMA_V1}`",
            ),
            Self::UnsupportedTrustRegistrySchema(schema) => write!(
                formatter,
                "unsupported adapter trust registry schema `{schema}`; expected `{ADAPTER_TRUST_REGISTRY_SCHEMA_V2}`",
            ),
            Self::Inventory(message) => formatter.write_str(message),
        }
    }
}

impl Error for AdapterContractError {}

pub fn normalize_adapter_manifest_schema(
    manifest: &mut AdapterPluginManifest,
) -> Result<bool, AdapterContractError> {
    match manifest.schema.as_str() {
        ADAPTER_MANIFEST_SCHEMA_V1 => Ok(false),
        ADAPTER_MANIFEST_SCHEMA_LEGACY => {
            manifest.schema = ADAPTER_MANIFEST_SCHEMA_V1.to_string();
            Ok(true)
        }
        schema => Err(AdapterContractError::UnsupportedManifestSchema(
            schema.to_string(),
        )),
    }
}

pub fn validate_adapter_manifest_schema(schema: &str) -> Result<(), AdapterContractError> {
    match schema {
        ADAPTER_MANIFEST_SCHEMA_V1 | ADAPTER_MANIFEST_SCHEMA_LEGACY => Ok(()),
        schema => Err(AdapterContractError::UnsupportedManifestSchema(
            schema.to_string(),
        )),
    }
}

pub fn normalize_adapter_trust_registry_schema(
    registry: &mut AdapterTrustRegistry,
) -> Result<bool, AdapterContractError> {
    match registry.schema.as_str() {
        ADAPTER_TRUST_REGISTRY_SCHEMA_V2 => Ok(false),
        ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2 => {
            registry.schema = ADAPTER_TRUST_REGISTRY_SCHEMA_V2.to_string();
            Ok(true)
        }
        schema => Err(AdapterContractError::UnsupportedTrustRegistrySchema(
            schema.to_string(),
        )),
    }
}

pub fn validate_adapter_contract_inventory(
    public_contracts: &[JsonContractDescriptor],
) -> Result<(), AdapterContractError> {
    let public_schemas = public_contracts
        .iter()
        .map(|contract| contract.schema)
        .collect::<BTreeSet<_>>();
    let mut schemas = BTreeSet::new();

    for descriptor in ADAPTER_NON_PUBLIC_JSON_CONTRACTS {
        if descriptor.lifecycle == BoundaryLifecycle::Current {
            validate_schema_id(descriptor.schema)
                .map_err(|error| AdapterContractError::Inventory(error.to_string()))?;
        } else if !descriptor.schema.starts_with("athanor.") {
            return Err(AdapterContractError::Inventory(format!(
                "legacy adapter schema {} is not an Athanor schema id",
                descriptor.schema
            )));
        }
        if public_schemas.contains(descriptor.schema) {
            return Err(AdapterContractError::Inventory(format!(
                "adapter non-public schema {} is also a public report",
                descriptor.schema
            )));
        }
        if !schemas.insert(descriptor.schema) {
            return Err(AdapterContractError::Inventory(format!(
                "duplicate adapter boundary schema {}",
                descriptor.schema
            )));
        }
        if descriptor.rust_owner.is_empty() || descriptor.required_fields.is_empty() {
            return Err(AdapterContractError::Inventory(format!(
                "adapter boundary schema {} has incomplete ownership metadata",
                descriptor.schema
            )));
        }
    }
    Ok(())
}

pub fn validate_adapter_non_public_contract_value(
    descriptor: &NonPublicJsonContractDescriptor,
    value: &Value,
) -> Result<(), AdapterContractError> {
    let object = value.as_object().ok_or_else(|| {
        AdapterContractError::Inventory(format!(
            "{} document must be an object",
            descriptor.schema
        ))
    })?;
    let actual = object
        .get("schema")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    if actual != descriptor.schema {
        return Err(AdapterContractError::Inventory(format!(
            "document schema {actual} does not match {}",
            descriptor.schema
        )));
    }
    for field in descriptor.required_fields {
        if !object.contains_key(*field) {
            return Err(AdapterContractError::Inventory(format!(
                "{} document is missing required field {field}",
                descriptor.schema
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionedAdapterTrustReport {
    pub schema: String,
    pub trust_path: PathBuf,
    pub plugins: Vec<AdapterTrustStatus>,
}

impl VersionedJsonContract for VersionedAdapterTrustReport {
    const SCHEMA: &'static str = ADAPTER_TRUST_REPORT_SCHEMA_V1;

    fn schema(&self) -> &str {
        &self.schema
    }
}

pub fn list_adapter_plugin_trust_versioned(
    options: AdapterTrustListOptions,
) -> anyhow::Result<VersionedAdapterTrustReport> {
    list_adapter_plugin_trust(options)
}

pub fn trust_adapter_plugin_versioned(
    options: AdapterTrustOptions,
) -> anyhow::Result<VersionedAdapterTrustReport> {
    trust_adapter_plugin(options)
}

pub fn untrust_adapter_plugin_versioned(
    options: AdapterTrustOptions,
) -> anyhow::Result<VersionedAdapterTrustReport> {
    untrust_adapter_plugin(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_adapter_schemas_normalize_to_distinct_current_owners() {
        let mut manifest = AdapterPluginManifest {
            schema: ADAPTER_MANIFEST_SCHEMA_LEGACY.to_string(),
            name: "legacy".to_string(),
            version: None,
            adapters: Vec::new(),
        };
        assert!(normalize_adapter_manifest_schema(&mut manifest).unwrap());
        assert_eq!(manifest.schema, ADAPTER_MANIFEST_SCHEMA_V1);

        let mut registry = AdapterTrustRegistry {
            schema: ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2.to_string(),
            trusted_plugins: Vec::new(),
        };
        assert!(normalize_adapter_trust_registry_schema(&mut registry).unwrap());
        assert_eq!(registry.schema, ADAPTER_TRUST_REGISTRY_SCHEMA_V2);
        assert_ne!(
            ADAPTER_TRUST_REGISTRY_SCHEMA_V2,
            ADAPTER_TRUST_REPORT_SCHEMA_V1
        );
    }

    #[test]
    fn public_trust_report_has_only_the_current_schema() {
        let report = VersionedAdapterTrustReport {
            schema: ADAPTER_TRUST_REPORT_SCHEMA_V1.to_string(),
            trust_path: PathBuf::from("adapter-trust.json"),
            plugins: Vec::new(),
        };
        report.validate_contract().unwrap();
    }
}
