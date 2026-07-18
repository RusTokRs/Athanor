use std::path::Path;

use anyhow::Result;
#[cfg(not(test))]
use anyhow::bail;
use serde_json::Value;

pub(crate) const WIKI_PROJECTION_SCHEMA: &str = "athanor.wiki_projection.v1";
pub(crate) const HTML_REPORT_PROJECTION_SCHEMA: &str = "athanor.html_report_projection.v1";

pub type ProjectionFactory = fn(&Path, &str, Value, &dyn Fn() -> bool) -> Result<()>;

pub fn install_wiki_projector_factory(_factory: ProjectionFactory) {
    panic!("process-global wiki projector installation was removed; use RuntimeComposition")
}

pub fn install_html_projector_factory(_factory: ProjectionFactory) {
    panic!("process-global HTML projector installation was removed; use RuntimeComposition")
}

pub(crate) fn project_wiki_payload(
    target: &Path,
    snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    #[cfg(test)]
    {
        return crate::test_runtime::composition().project_wiki(
            target,
            snapshot,
            payload,
            is_cancelled,
        );
    }

    #[cfg(not(test))]
    {
        let _ = (target, snapshot, payload, is_cancelled);
        bail!("explicit RuntimeComposition is required for wiki projection")
    }
}

pub(crate) fn project_html_payload(
    target: &Path,
    snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    #[cfg(test)]
    {
        return crate::test_runtime::composition().project_html(
            target,
            snapshot,
            payload,
            is_cancelled,
        );
    }

    #[cfg(not(test))]
    {
        let _ = (target, snapshot, payload, is_cancelled);
        bail!("explicit RuntimeComposition is required for HTML projection")
    }
}
