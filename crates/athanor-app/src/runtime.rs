//! Adapter runtime composition, plugin discovery, trust, and process execution.

mod builder;
mod model;
mod plugin_discovery;
mod plugin_hash;
mod plugin_trust_path;
mod plugin_trust_record;
mod plugin_trust_registry;
mod plugin_trust_status;
mod process_adapter;
mod process_adapter_support;
mod process_runner;
mod registry;
mod trust;

#[cfg(test)]
mod tests;

pub use builder::RuntimeBuilder;
pub use model::{
    AdapterPluginEntry, AdapterPluginKind, AdapterPluginManifest, AdapterProcessCommand,
    AdapterTrustListOptions, AdapterTrustOptions, AdapterTrustRegistry, AdapterTrustStatus,
    DiscoveredAdapterPlugin, TrustedAdapterExecutable, TrustedAdapterPlugin,
};
pub use process_runner::TokioProcessRunner;
pub use registry::{AdapterRegistry, AdapterRegistryFactory, BuiltinAdapterResolver};
pub use trust::{
    default_adapter_trust_path, discover_adapter_plugins, list_adapter_plugin_trust,
    trust_adapter_plugin, untrust_adapter_plugin,
};

#[deprecated(note = "use adapter_contract::VersionedAdapterTrustReport")]
pub type AdapterTrustReport = crate::adapter_contract::VersionedAdapterTrustReport;

#[deprecated(
    note = "use ADAPTER_MANIFEST_SCHEMA_V1 for current output or ADAPTER_MANIFEST_SCHEMA_LEGACY for legacy input normalization"
)]
pub const ADAPTER_MANIFEST_SCHEMA: &str = crate::adapter_contract::ADAPTER_MANIFEST_SCHEMA_LEGACY;

#[deprecated(
    note = "use ADAPTER_TRUST_REGISTRY_SCHEMA_V2 for current persistence or ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2 for legacy input normalization"
)]
pub const ADAPTER_TRUST_SCHEMA: &str =
    crate::adapter_contract::ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2;
