use std::path::PathBuf;

use serde::Serialize;

mod retention_guard;

pub use retention_guard::*;

#[derive(Debug, Clone)]
pub struct RepairRecoverIndexOptions {
    pub root: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairRecoverIndexReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub needed: bool,
    pub recovered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    pub remaining_issues: Vec<RepairIssue>,
    pub inspection: RepairInspectReport,
}
