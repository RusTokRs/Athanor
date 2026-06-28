use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Result, bail};
use serde_json::Value;

pub(crate) const WIKI_PROJECTION_SCHEMA: &str = "athanor.wiki_projection.v1";
pub(crate) const HTML_REPORT_PROJECTION_SCHEMA: &str = "athanor.html_report_projection.v1";

type ProjectionFactory = fn(&Path, &str, Value, &dyn Fn() -> bool) -> Result<()>;

static WIKI_PROJECTOR_FACTORY: OnceLock<ProjectionFactory> = OnceLock::new();
static HTML_PROJECTOR_FACTORY: OnceLock<ProjectionFactory> = OnceLock::new();

pub fn install_wiki_projector_factory(factory: ProjectionFactory) {
    let _ = WIKI_PROJECTOR_FACTORY.set(factory);
}

pub fn install_html_projector_factory(factory: ProjectionFactory) {
    let _ = HTML_PROJECTOR_FACTORY.set(factory);
}

pub(crate) fn project_wiki_payload(
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

pub(crate) fn project_html_payload(
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
