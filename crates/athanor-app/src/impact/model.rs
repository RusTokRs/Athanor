use std::path::PathBuf;

use athanor_domain::{Diagnostic, Entity, Relation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct RelationFlow {
    pub relation: Relation,
    pub direction: FlowDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactedEntity {
    pub entity: Entity,
    pub depth: usize,
    pub path: Vec<RelationFlow>,
    pub path_steps: Vec<ImpactPathStep>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactPathStep {
    pub relation_id: String,
    pub relation_kind: String,
    pub direction: FlowDirection,
    pub from: ImpactPathEndpoint,
    pub to: ImpactPathEndpoint,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactPathEndpoint {
    pub entity_id: String,
    pub stable_key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactAnalysis {
    pub schema: String,
    pub snapshot: String,
    pub starting_entities: Vec<Entity>,
    pub impacted_entities: Vec<ImpactedEntity>,
    pub impacted_files: Vec<String>,
    pub impacted_diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct ImpactOptions {
    pub root: PathBuf,
    pub target: Option<String>,
    pub diff: bool,
    pub max_depth: usize,
}
