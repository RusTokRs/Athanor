//! Adapter runtime composition, plugin discovery, trust, and process execution.

mod builder;
mod legacy_api;
#[cfg(any(feature = "legacy-global-runtime", test))]
mod legacy_registry;
#[cfg(not(any(feature = "legacy-global-runtime", test)))]
#[path = "runtime/legacy_registry_disabled.rs"]
mod legacy_registry;
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
pub use legacy_api::{
    default_adapter_registry, install_builtin_adapter_resolver,
    install_default_adapter_registry, try_default_adapter_registry,
    try_install_builtin_adapter_resolver, try_install_default_adapter_registry,
};
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

pub type AdapterTrustReport = crate::adapter_contract::VersionedAdapterTrustReport;

pub const ADAPTER_MANIFEST_SCHEMA: &str = crate::adapter_contract::ADAPTER_MANIFEST_SCHEMA_LEGACY;
pub const ADAPTER_TRUST_SCHEMA: &str =
    crate::adapter_contract::ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2;
