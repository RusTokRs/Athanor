use std::sync::OnceLock;

use super::{AdapterRegistry, AdapterRegistryFactory, BuiltinAdapterResolver};
use crate::legacy_factory::{LegacyFactoryInstallError, install_once};

static DEFAULT_ADAPTER_REGISTRY_FACTORY: OnceLock<AdapterRegistryFactory> = OnceLock::new();
static BUILTIN_ADAPTER_RESOLVER: OnceLock<BuiltinAdapterResolver> = OnceLock::new();

pub(super) fn try_install_default_adapter_registry(
    factory: AdapterRegistryFactory,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(
        &DEFAULT_ADAPTER_REGISTRY_FACTORY,
        factory,
        "default adapter registry",
    )
}

pub(super) fn install_default_adapter_registry(factory: AdapterRegistryFactory) {
    try_install_default_adapter_registry(factory)
        .expect("legacy default adapter registry installation must be unique");
}

pub(super) fn try_install_builtin_adapter_resolver(
    resolver: BuiltinAdapterResolver,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(
        &BUILTIN_ADAPTER_RESOLVER,
        resolver,
        "built-in adapter resolver",
    )
}

pub(super) fn install_builtin_adapter_resolver(resolver: BuiltinAdapterResolver) {
    try_install_builtin_adapter_resolver(resolver)
        .expect("legacy built-in adapter resolver installation must be unique");
}

pub(super) fn default_adapter_registry() -> AdapterRegistry {
    #[cfg(test)]
    crate::ensure_test_runtime();

    DEFAULT_ADAPTER_REGISTRY_FACTORY
        .get()
        .map(|factory| factory())
        .unwrap_or_else(AdapterRegistry::empty)
}

pub(super) fn builtin_adapter_resolver() -> Option<BuiltinAdapterResolver> {
    BUILTIN_ADAPTER_RESOLVER.get().copied()
}
