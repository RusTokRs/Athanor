use std::path::Path;

use anyhow::Result;
use serde_json::Value;

pub(crate) const WIKI_PROJECTION_SCHEMA: &str = "athanor.wiki_projection.v1";
pub(crate) const HTML_REPORT_PROJECTION_SCHEMA: &str = "athanor.html_report_projection.v1";

pub type ProjectionFactory = fn(&Path, &str, Value, &dyn Fn() -> bool) -> Result<()>;
