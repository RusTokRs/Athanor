use super::{AdapterRegistry, BuiltinAdapterResolver};
use crate::LegacyFactoryUnavailableError;

pub(super) fn try_default_adapter_registry(
) -> Result<AdapterRegistry, LegacyFactoryUnavailableError> {
    Err(LegacyFactoryUnavailableError::new(
        "default adapter registry (legacy-global-runtime disabled)",
    ))
}

pub(super) fn default_adapter_registry() -> AdapterRegistry {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

pub(super) fn builtin_adapter_resolver() -> Option<BuiltinAdapterResolver> {
    None
}
