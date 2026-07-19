use std::collections::BTreeSet;
use std::path::PathBuf;

use athanor_domain::{
    Diagnostic, DiagnosticId, Entity, EntityId, Evidence, Relation,
};
use serde::{Deserialize, Serialize};

use crate::impact::FlowDirection;

#[derive(Debug, Clone)]
pub struct ChangeMapOptions {
    pub root: PathBuf,
    pub task: Option<String>,
    pub target: Option<String>,
    pub diff: bool,
    pub max_entities: usize,
    pub max_files: usize,
    pub max_diagnostics: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeMapReport {
    pub schema: String,
    pub snapshot: String,
    pub query: ChangeMapQuery,
    pub limits: ChangeMapLimits,
    pub returned: ChangeMapCounts,
    pub omitted: ChangeMapCounts,
    pub items: Vec<ChangeMapItem>,
    pub files: Vec<ChangeMapFile>,
    pub diagnostics: Vec<Diagnostic>,
    pub completeness: ChangeMapCompleteness,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapQuery {
    pub task: Option<String>,
    pub target: Option<String>,
    pub diff: bool,
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapLimits {
    pub max_entities: usize,
    pub max_files: usize,
    pub max_diagnostics: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapCounts {
    pub entities: usize,
    pub files: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeMapItem {
    pub rank: usize,
    pub score: i64,
    pub depth: usize,
    pub entity: Entity,
    pub reasons: Vec<String>,
    pub path: Vec<ChangeMapPathStep>,
    pub files: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub diagnostics: Vec<DiagnosticId>,
    pub test_coverage: ChangeMapTestCoverage,
    pub annotations: Vec<ChangeMapAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeMapPathStep {
    pub relation_id: String,
    pub relation_kind: String,
    pub direction: FlowDirection,
    pub from: ChangeMapEndpoint,
    pub to: ChangeMapEndpoint,
    pub confidence: f32,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapEndpoint {
    pub entity_id: String,
    pub stable_key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeMapTestStatus {
    Linked,
    NotLinked,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapTestCoverage {
    pub status: ChangeMapTestStatus,
    pub tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChangeMapAnnotation {
    pub source: String,
    pub schema: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapFile {
    pub rank: usize,
    pub path: String,
    pub score: i64,
    pub entity_kinds: Vec<String>,
    pub stable_keys: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapCompleteness {
    pub candidate_limit_reached: bool,
    pub candidate_limit: usize,
    pub note: String,
    pub suggested_command: String,
}

#[derive(Debug, Clone)]
pub(super) struct Seed {
    pub(super) id: EntityId,
    pub(super) score: i64,
    pub(super) reason: String,
}

#[derive(Debug, Clone)]
pub(super) struct Candidate {
    pub(super) id: EntityId,
    pub(super) seed_score: i64,
    pub(super) depth: usize,
    pub(super) reasons: BTreeSet<String>,
    pub(super) path: Vec<PathLink>,
}

#[derive(Debug, Clone)]
pub(super) struct PathLink {
    pub(super) relation: Relation,
    pub(super) direction: FlowDirection,
    pub(super) from: EntityId,
    pub(super) to: EntityId,
}
