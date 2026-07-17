//! Versioned contracts for adapter plugin manifests and trust state.
//!
//! Adapter manifests and persisted trust registries are non-public boundaries.
//! The trust status report is a public application document. Legacy schema ids
//! remain accepted only at read boundaries and are normalized before use.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

use crate::json_contract::VersionedJsonContract;
use crate::runtime::{
    AdapterPluginManifest, AdapterTrustListOptions, AdapterTrustOptions, AdapterTrustRegistry,
    AdapterTrustReport, AdapterTrustStatus, list_adapter_plugin_trust, trust_adapter_plugin,
    untrust_adapter_plugin,
};

pub const ADAPTER_MANIFEST_SCHEMA_V1: &str = "athanor.adapter_manifest.v1";
pub const ADAPTER_MANIFEST_SCHEMA_LEGACY: &str = "athanor.adapter_manifest";
pub const ADAPTER_TRUST_REGISTRY_SCHEMA_V2: &str = "athanor.adapter_trust_registry.v2";
pub const ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2: &str = "athanor.adapter_trust.v2";
pub const ADAPTER_TRUST_REPORT_SCHEMA_V1: &str = "athanor.adapter_trust_report.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterContractError {
    UnsupportedManifestSchema(String),
    UnsupportedTrustRegistrySchema(String),
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionedAdapterTrustReport {
    pub schema: String,
    pub trust_path: PathBuf,
    pub plugins: Vec<AdapterTrustStatus>,
}

impl From<AdapterTrustReport> for VersionedAdapterTrustReport {
    fn from(report: AdapterTrustReport) -> Self {
        Self {
            schema: ADAPTER_TRUST_REPORT_SCHEMA_V1.to_string(),
            trust_path: report.trust_path,
            plugins: report.plugins,
        }
    }
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
    list_adapter_plugin_trust(options).map(Into::into)
}

pub fn trust_adapter_plugin_versioned(
    options: AdapterTrustOptions,
) -> anyhow::Result<VersionedAdapterTrustReport> {
    trust_adapter_plugin(options).map(Into::into)
}

pub fn untrust_adapter_plugin_versioned(
    options: AdapterTrustOptions,
) -> anyhow::Result<VersionedAdapterTrustReport> {
    untrust_adapter_plugin(options).map(Into::into)
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
    fn trust_report_conversion_replaces_legacy_shared_schema() {
        let report = VersionedAdapterTrustReport::from(AdapterTrustReport {
            schema: ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2.to_string(),
            trust_path: PathBuf::from("adapter-trust.json"),
            plugins: Vec::new(),
        });
        assert_eq!(report.schema, ADAPTER_TRUST_REPORT_SCHEMA_V1);
        report.validate_contract().unwrap();
    }
}
