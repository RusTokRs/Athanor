use std::path::PathBuf;

use athanor_domain::Diagnostic;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DOCS_CHECK_SCHEMA: &str = "athanor.docs_check.v1";
pub const DOCS_DRIFT_SCHEMA: &str = "athanor.docs_drift.v1";
pub const DOCS_PATCH_SCHEMA: &str = "athanor.docs_patch.v1";

#[derive(Debug, Clone)]
pub struct DocsCheckOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocsPolicyViolation {
    pub path: String,
    pub stable_key: String,
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocsCheckReport {
    pub schema: String,
    pub snapshot: String,
    pub passed: bool,
    pub editable_documents: usize,
    pub policy_violations: Vec<DocsPolicyViolation>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct DocsDriftOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftedDocument {
    pub path: String,
    pub stable_key: String,
    pub verified_snapshot: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocsDriftReport {
    pub schema: String,
    pub snapshot: String,
    pub editable_documents: usize,
    pub current_documents: usize,
    pub drifted_documents: Vec<DriftedDocument>,
}

#[derive(Debug, Clone)]
pub struct DocsProposeFixOptions {
    pub root: PathBuf,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct DocsApplyPatchOptions {
    pub root: PathBuf,
    pub patch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocsFrontmatterChange {
    pub field: String,
    pub old_value: Option<Value>,
    pub new_value: Value,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocsPatchOperation {
    pub path: String,
    pub stable_key: String,
    #[serde(default)]
    pub create: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub changes: Vec<DocsFrontmatterChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocsPatchProposal {
    pub schema: String,
    pub id: String,
    pub snapshot: String,
    pub operations: Vec<DocsPatchOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocsProposeFixReport {
    pub proposal: DocsPatchProposal,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocsApplyPatchReport {
    pub schema: String,
    pub id: String,
    pub snapshot: String,
    pub files_changed: usize,
    pub changes_applied: usize,
}
