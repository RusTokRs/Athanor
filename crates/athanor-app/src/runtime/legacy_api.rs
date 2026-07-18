use super::{AdapterRegistry, AdapterRegistryFactory, BuiltinAdapterResolver, legacy_registry};

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn try_install_default_adapter_registry(
    factory: AdapterRegistryFactory,
) -> Result<(), crate::LegacyFactoryInstallError> {
    legacy_registry::try_install_default_adapter_registry(factory)
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn try_install_default_adapter_registry(
    _factory: AdapterRegistryFactory,
) -> Result<(), crate::LegacyFactoryInstallError> {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn install_default_adapter_registry(factory: AdapterRegistryFactory) {
    legacy_registry::install_default_adapter_registry(factory);
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn install_default_adapter_registry(_factory: AdapterRegistryFactory) {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn try_install_builtin_adapter_resolver(
    resolver: BuiltinAdapterResolver,
) -> Result<(), crate::LegacyFactoryInstallError> {
    legacy_registry::try_install_builtin_adapter_resolver(resolver)
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn try_install_builtin_adapter_resolver(
    _resolver: BuiltinAdapterResolver,
) -> Result<(), crate::LegacyFactoryInstallError> {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn install_builtin_adapter_resolver(resolver: BuiltinAdapterResolver) {
    legacy_registry::install_builtin_adapter_resolver(resolver);
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn install_builtin_adapter_resolver(_resolver: BuiltinAdapterResolver) {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

pub fn try_default_adapter_registry(
) -> Result<AdapterRegistry, crate::LegacyFactoryUnavailableError> {
    legacy_registry::try_default_adapter_registry()
}

pub fn default_adapter_registry() -> AdapterRegistry {
    legacy_registry::default_adapter_registry()
}
