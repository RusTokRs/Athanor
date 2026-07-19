use std::path::PathBuf;

use athanor_core::CanonicalLatestIdentity;
use serde::Serialize;

mod current {
    include!("repair_cleanup_recovery.rs");
}

pub use current::*;

#[derive(Debug, Clone)]
pub struct RepairCanonicalLatestOptions {
    pub root: PathBuf,
    pub dry_run: bool,
    /// Optional exact target. Without it, index-current is preferred and backend discovery is fallback.
    pub snapshot: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairCanonicalLatestReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub needed: bool,
    pub repaired: bool,
    pub target: CanonicalLatestIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<CanonicalLatestIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_error: Option<String>,
    pub remaining_issues: Vec<RepairIssue>,
}
