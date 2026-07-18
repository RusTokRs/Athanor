use std::path::Path;

use anyhow::Result;
#[cfg(not(any(feature = "legacy-global-runtime", test)))]
use anyhow::bail;
use serde_json::Value;

use crate::legacy_factory::LegacyFactoryInstallError;

#[cfg(any(feature = "legacy-global-runtime", test))]
#[path = "projection_legacy_global.rs"]
mod legacy_global;

pub(crate) const WIKI_PROJECTION_SCHEMA: &str = "athanor.wiki_projection.v1";
pub(crate) const HTML_REPORT_PROJECTION_SCHEMA: &str = "athanor.html_report_projection.v1";

pub type ProjectionFactory = fn(&Path, &str, Value, &dyn Fn() -> bool) -> Result<()>;

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn try_install_wiki_projector_factory(
    factory: ProjectionFactory,
) -> Result<(), LegacyFactoryInstallError> {
    legacy_global::try_install_wiki_projector_factory(factory)
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn try_install_wiki_projector_factory(
    _factory: ProjectionFactory,
) -> Result<(), LegacyFactoryInstallError> {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn install_wiki_projector_factory(factory: ProjectionFactory) {
    legacy_global::install_wiki_projector_factory(factory);
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn install_wiki_projector_factory(_factory: ProjectionFactory) {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn try_install_html_projector_factory(
    factory: ProjectionFactory,
) -> Result<(), LegacyFactoryInstallError> {
    legacy_global::try_install_html_projector_factory(factory)
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn try_install_html_projector_factory(
    _factory: ProjectionFactory,
) -> Result<(), LegacyFactoryInstallError> {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn install_html_projector_factory(factory: ProjectionFactory) {
    legacy_global::install_html_projector_factory(factory);
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn install_html_projector_factory(_factory: ProjectionFactory) {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

pub(crate) fn project_wiki_payload(
    target: &Path,
    snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    #[cfg(test)]
    crate::ensure_test_runtime();

    #[cfg(any(feature = "legacy-global-runtime", test))]
    {
        legacy_global::project_wiki_payload(target, snapshot, payload, is_cancelled)
    }

    #[cfg(not(any(feature = "legacy-global-runtime", test)))]
    {
        let _ = (target, snapshot, payload, is_cancelled);
        bail!("legacy wiki projection is disabled; use RuntimeComposition::project_wiki")
    }
}

pub(crate) fn project_html_payload(
    target: &Path,
    snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    #[cfg(test)]
    crate::ensure_test_runtime();

    #[cfg(any(feature = "legacy-global-runtime", test))]
    {
        legacy_global::project_html_payload(target, snapshot, payload, is_cancelled)
    }

    #[cfg(not(any(feature = "legacy-global-runtime", test)))]
    {
        let _ = (target, snapshot, payload, is_cancelled);
        bail!("legacy HTML projection is disabled; use RuntimeComposition::project_html")
    }
}
