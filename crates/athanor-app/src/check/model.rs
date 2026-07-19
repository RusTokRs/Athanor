use std::path::PathBuf;

use athanor_domain::Diagnostic;
use serde::{Deserialize, Serialize};

use crate::docs::DriftedDocument;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticScope {
    Api,
    Docs,
    Env,
    Scripts,
    Deployment,
    Runbooks,
    RustokFfa,
    RustokFba,
    RustokPageBuilder,
}

#[derive(Debug, Clone)]
pub struct DiagnosticCheckOptions {
    pub root: PathBuf,
    pub scope: DiagnosticScope,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DiagnosticCounts {
    pub total: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCheckReport {
    pub schema: String,
    pub snapshot: String,
    pub scope: DiagnosticScope,
    pub counts: DiagnosticCounts,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct AffectedCheckOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedCheckReport {
    pub schema: String,
    pub snapshot: String,
    pub affected_files: AffectedFileCounts,
    pub stale_artifacts: Vec<AffectedArtifactStatus>,
    pub documentation_drift: Vec<DriftedDocument>,
    pub counts: DiagnosticCounts,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AffectedArtifactStatus {
    pub kind: AffectedArtifactKind,
    pub path: PathBuf,
    pub message: String,
    pub suggested_command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AffectedArtifactKind {
    GeneratedCurrent,
    GeneratedGeneration,
    Wiki,
    HtmlReport,
    ApiContract,
    ApiDiff,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AffectedFileCounts {
    pub changed: usize,
    pub unchanged: usize,
    pub removed: usize,
}

#[derive(Debug, Clone)]
pub struct OperationsDocsCheckOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationsDocsCheckReport {
    pub schema: String,
    pub snapshot: String,
    pub counts: DiagnosticCounts,
    pub env: DiagnosticCheckReport,
    pub scripts: DiagnosticCheckReport,
    pub deployment: DiagnosticCheckReport,
    pub runbooks: DiagnosticCheckReport,
}
