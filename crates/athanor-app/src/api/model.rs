use std::path::PathBuf;

use athanor_domain::{Diagnostic, EntityId, Ownership, SourceLocation};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const API_CONTRACT_SNAPSHOT_SCHEMA: &str = "athanor.api_contract_snapshot.v2";
pub const API_CONTRACT_LATEST_SCHEMA: &str = "athanor.api_contract_latest.v1";
pub const API_CONTRACT_DIFF_SCHEMA: &str = "athanor.api_contract_diff.v2";

#[derive(Debug, Clone)]
pub struct ApiSnapshotOptions {
    pub root: PathBuf,
    pub retention: ApiRetentionOverrides,
}

#[derive(Debug, Clone)]
pub struct ApiDiffOptions {
    pub root: PathBuf,
    pub from: Option<String>,
    pub to: Option<String>,
    pub retention: ApiRetentionOverrides,
}

#[derive(Debug, Clone)]
pub struct ApiCleanupOptions {
    pub root: PathBuf,
    pub dry_run: bool,
    pub keep_snapshots: usize,
    pub keep_diffs: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ApiRetentionOverrides {
    pub auto_cleanup: Option<bool>,
    pub keep_snapshots: Option<usize>,
    pub keep_diffs: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiContractItem {
    #[serde(default)]
    pub entity_id: Option<EntityId>,
    pub stable_key: String,
    pub name: String,
    #[serde(default)]
    pub source: Option<SourceLocation>,
    #[serde(default)]
    pub ownership: Vec<Ownership>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiContractSnapshot {
    pub schema: String,
    pub snapshot: String,
    pub endpoints: Vec<ApiContractItem>,
    pub schemas: Vec<ApiContractItem>,
    pub examples: Vec<ApiContractItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiContractLatest {
    pub schema: String,
    pub snapshot: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiSnapshotReport {
    pub snapshot: String,
    pub path: PathBuf,
    pub created: bool,
    pub endpoints: usize,
    pub schemas: usize,
    pub examples: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<ApiCleanupReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiCleanupReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub keep_snapshots: usize,
    pub keep_diffs: usize,
    pub removed: Vec<ApiCleanupArtifact>,
    pub retained: Vec<ApiCleanupArtifact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiCleanupArtifact {
    pub kind: ApiCleanupArtifactKind,
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiCleanupArtifactKind {
    Snapshot,
    Diff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiContractChangeKind {
    EndpointAdded,
    EndpointRemoved,
    EndpointChanged,
    SchemaAdded,
    SchemaRemoved,
    SchemaChanged,
    ExampleAdded,
    ExampleRemoved,
    ExampleChanged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiContractChange {
    pub kind: ApiContractChangeKind,
    pub stable_key: String,
    pub breaking: bool,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub entity_id: Option<EntityId>,
    #[serde(default)]
    pub source: Option<SourceLocation>,
    #[serde(default)]
    pub ownership: Vec<Ownership>,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiContractDiff {
    pub schema: String,
    pub from: String,
    pub to: String,
    pub breaking_changes: usize,
    pub changes: Vec<ApiContractChange>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub artifact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<ApiCleanupReport>,
}
