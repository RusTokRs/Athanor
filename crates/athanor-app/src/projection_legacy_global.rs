use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Result, bail};
use serde_json::Value;

use super::ProjectionFactory;
use crate::legacy_factory::{LegacyFactoryInstallError, install_once};

static WIKI_PROJECTOR_FACTORY: OnceLock<ProjectionFactory> = OnceLock::new();
static HTML_PROJECTOR_FACTORY: OnceLock<ProjectionFactory> = OnceLock::new();

pub(super) fn try_install_wiki_projector_factory(
    factory: ProjectionFactory,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(&WIKI_PROJECTOR_FACTORY, factory, "wiki projector")
}

pub(super) fn install_wiki_projector_factory(factory: ProjectionFactory) {
    try_install_wiki_projector_factory(factory)
        .expect("legacy wiki projector factory installation must be unique");
}

pub(super) fn try_install_html_projector_factory(
    factory: ProjectionFactory,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(&HTML_PROJECTOR_FACTORY, factory, "HTML projector")
}

pub(super) fn install_html_projector_factory(factory: ProjectionFactory) {
    try_install_html_projector_factory(factory)
        .expect("legacy HTML projector factory installation must be unique");
}

pub(super) fn project_wiki_payload(
    target: &Path,
    snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let Some(factory) = WIKI_PROJECTOR_FACTORY.get() else {
        bail!("no Athanor wiki projector factory is installed");
    };
    factory(target, snapshot, payload, is_cancelled)
}

pub(super) fn project_html_payload(
    target: &Path,
    snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let Some(factory) = HTML_PROJECTOR_FACTORY.get() else {
        bail!("no Athanor HTML projector factory is installed");
    };
    factory(target, snapshot, payload, is_cancelled)
}
