use std::sync::OnceLock;

use super::{AdapterRegistry, AdapterRegistryFactory, BuiltinAdapterResolver};

static DEFAULT_ADAPTER_REGISTRY_FACTORY: OnceLock<AdapterRegistryFactory> = OnceLock::new();
static BUILTIN_ADAPTER_RESOLVER: OnceLock<BuiltinAdapterResolver> = OnceLock::new();

pub(super) fn install_default_adapter_registry(factory: AdapterRegistryFactory) {
    let _ = DEFAULT_ADAPTER_REGISTRY_FACTORY.set(factory);
}

pub(super) fn install_builtin_adapter_resolver(resolver: BuiltinAdapterResolver) {
    let _ = BUILTIN_ADAPTER_RESOLVER.set(resolver);
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
