use std::path::PathBuf;

use serde::Serialize;

pub const OVERVIEW_SCHEMA: &str = crate::json_contract::OVERVIEW_SCHEMA_V1;

#[derive(Debug, Clone)]
pub struct OverviewOptions {
    pub root: PathBuf,
    pub top: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepositoryOverview {
    pub schema: String,
    pub snapshot: String,
    pub totals: OverviewTotals,
    pub entity_kinds: Vec<NamedCount>,
    pub relation_kinds: Vec<NamedCount>,
    pub source_roots: Vec<NamedCount>,
    pub api: ApiOverview,
    pub docs: DocsOverview,
    pub operations: OperationsOverview,
    pub module_structure: Vec<ModuleOverview>,
    pub integration_boundaries: Vec<IntegrationBoundaryOverview>,
    pub graph_hubs: Vec<EntityOverview>,
    pub open_diagnostics: Vec<DiagnosticOverview>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModuleOverview {
    pub stable_key: String,
    pub name: String,
    pub source: Option<String>,
    pub direct_members: usize,
    pub relation_ids: Vec<String>,
    pub omitted_relation_ids: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IntegrationBoundaryOverview {
    pub from_root: String,
    pub to_root: String,
    pub relations: usize,
    pub relation_kinds: Vec<NamedCount>,
    pub relation_ids: Vec<String>,
    pub omitted_relation_ids: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct OverviewTotals {
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
    pub open_diagnostics: usize,
    pub source_files: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NamedCount {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct ApiOverview {
    pub endpoints: usize,
    pub schemas: usize,
    pub examples: usize,
    pub documented_endpoints: usize,
    pub implemented_endpoints: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct DocsOverview {
    pub pages: usize,
    pub sections: usize,
    pub runbooks: usize,
    pub operation_steps: usize,
    pub operations_pages: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct OperationsOverview {
    pub environment_variables: usize,
    pub script_commands: usize,
    pub deployment_resources: usize,
    pub database_migrations: usize,
    pub packages: usize,
    pub dependencies: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EntityOverview {
    pub stable_key: String,
    pub kind: String,
    pub name: String,
    pub source: Option<String>,
    pub degree: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DiagnosticOverview {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub title: String,
    pub source: Option<String>,
}
