use std::error::Error;
use std::fmt;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyFactoryInstallError {
    factory: &'static str,
}

impl LegacyFactoryInstallError {
    pub const fn new(factory: &'static str) -> Self {
        Self { factory }
    }

    pub const fn factory(self) -> &'static str {
        self.factory
    }
}

impl fmt::Display for LegacyFactoryInstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "legacy {} factory is already installed; use explicit RuntimeComposition instead",
            self.factory
        )
    }
}

impl Error for LegacyFactoryInstallError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyFactoryUnavailableError {
    factory: &'static str,
}

impl LegacyFactoryUnavailableError {
    pub const fn new(factory: &'static str) -> Self {
        Self { factory }
    }

    pub const fn factory(self) -> &'static str {
        self.factory
    }
}

impl fmt::Display for LegacyFactoryUnavailableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "legacy {} factory is not installed; use explicit RuntimeComposition instead",
            self.factory
        )
    }
}

impl Error for LegacyFactoryUnavailableError {}

pub(crate) fn install_once<T>(
    slot: &OnceLock<T>,
    value: T,
    factory: &'static str,
) -> Result<(), LegacyFactoryInstallError> {
    slot.set(value)
        .map_err(|_| LegacyFactoryInstallError::new(factory))
}

pub(crate) fn require_installed<'a, T>(
    slot: &'a OnceLock<T>,
    factory: &'static str,
) -> Result<&'a T, LegacyFactoryUnavailableError> {
    slot.get()
        .ok_or_else(|| LegacyFactoryUnavailableError::new(factory))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_legacy_factory_install_is_an_explicit_error() {
        let slot = OnceLock::new();
        install_once(&slot, 1_u8, "test").unwrap();
        let error = install_once(&slot, 2_u8, "test").unwrap_err();
        assert_eq!(error.factory(), "test");
        assert!(error.to_string().contains("already installed"));
        assert!(error.to_string().contains("RuntimeComposition"));
    }

    #[test]
    fn missing_legacy_factory_is_an_explicit_error() {
        let slot = OnceLock::<u8>::new();
        let error = require_installed(&slot, "test").unwrap_err();
        assert_eq!(error.factory(), "test");
        assert!(error.to_string().contains("not installed"));
        assert!(error.to_string().contains("RuntimeComposition"));
    }
}
