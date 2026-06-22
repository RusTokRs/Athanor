use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use athanor_domain::Severity;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub docs: DocsConfig,
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DocsConfig {
    pub editable_path: String,
    pub completeness: CompletenessPolicy,
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self {
            editable_path: "docs".to_string(),
            completeness: CompletenessPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CompletenessPolicy {
    pub required_fields: Vec<String>,
    pub allowed_statuses: Vec<String>,
    pub minimum_diagnostic_severity: Severity,
    pub require_current_snapshot: bool,
}

impl Default for CompletenessPolicy {
    fn default() -> Self {
        Self {
            required_fields: vec![
                "id".to_string(),
                "kind".to_string(),
                "language".to_string(),
                "source_language".to_string(),
                "last_verified_snapshot".to_string(),
                "status".to_string(),
            ],
            allowed_statuses: vec!["verified".to_string()],
            minimum_diagnostic_severity: Severity::Medium,
            require_current_snapshot: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiSourceOfTruth {
    CodeFirst,
    OpenapiFirst,
    Hybrid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ApiConfig {
    pub enabled: bool,
    pub source_of_truth: ApiSourceOfTruth,
    pub strict: bool,
    pub fail_on_missing_docs: bool,
    pub fail_on_openapi_mismatch: bool,
    pub fail_on_undocumented_status_code: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            source_of_truth: ApiSourceOfTruth::Hybrid,
            strict: true,
            fail_on_missing_docs: true,
            fail_on_openapi_mismatch: true,
            fail_on_undocumented_status_code: true,
        }
    }
}

pub fn load_config(root: &Path) -> Result<ProjectConfig> {
    let path = root.join("athanor.toml");
    if !path.exists() {
        return Ok(ProjectConfig::default());
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}
