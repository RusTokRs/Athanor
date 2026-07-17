//! Adapter runtime composition, plugin discovery, trust, and process execution.

mod builder;
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

pub fn try_install_default_adapter_registry(
    factory: AdapterRegistryFactory,
) -> Result<(), crate::LegacyFactoryInstallError> {
    legacy_registry::try_install_default_adapter_registry(factory)
}

pub fn install_default_adapter_registry(factory: AdapterRegistryFactory) {
    legacy_registry::install_default_adapter_registry(factory);
}

pub fn try_install_builtin_adapter_resolver(
    resolver: BuiltinAdapterResolver,
) -> Result<(), crate::LegacyFactoryInstallError> {
    legacy_registry::try_install_builtin_adapter_resolver(resolver)
}

pub fn install_builtin_adapter_resolver(resolver: BuiltinAdapterResolver) {
    legacy_registry::install_builtin_adapter_resolver(resolver);
}

pub fn try_default_adapter_registry(
) -> Result<AdapterRegistry, crate::LegacyFactoryUnavailableError> {
    legacy_registry::try_default_adapter_registry()
}

pub fn default_adapter_registry() -> AdapterRegistry {
    legacy_registry::default_adapter_registry()
}
