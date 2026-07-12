use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use athanor_domain::Severity;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub docs: DocsConfig,
    pub api: ApiConfig,
    pub storage: StorageConfig,
    pub adapters: AdaptersConfig,
    pub pipeline: PipelineConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AdaptersConfig {
    pub allow_external_process: bool,
    pub external_process_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PipelineConfig {
    pub extraction_concurrency: usize,
    pub max_extraction_bytes_in_flight: usize,
    pub extraction_concurrency_by_adapter: BTreeMap<String, usize>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            extraction_concurrency: 16,
            max_extraction_bytes_in_flight: 64 * 1024 * 1024,
            extraction_concurrency_by_adapter: BTreeMap::new(),
        }
    }
}

impl ProjectConfig {
    fn validate(&self) -> Result<()> {
        if self.pipeline.extraction_concurrency == 0 {
            anyhow::bail!("[pipeline].extraction_concurrency must be at least 1");
        }
        if self.pipeline.max_extraction_bytes_in_flight == 0 {
            anyhow::bail!("[pipeline].max_extraction_bytes_in_flight must be at least 1");
        }
        if let Some((adapter, _)) = self
            .pipeline
            .extraction_concurrency_by_adapter
            .iter()
            .find(|(_, limit)| **limit == 0)
        {
            anyhow::bail!(
                "[pipeline].extraction_concurrency_by_adapter.{adapter} must be at least 1"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageConfig {
    pub mode: StorageMode,
    pub path: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            mode: StorageMode::Jsonl,
            path: ".athanor/store/canonical/jsonl".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StorageMode {
    Jsonl,
    SurrealEmbedded,
    SurrealMemory,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
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
#[serde(default, deny_unknown_fields)]
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
#[serde(default, deny_unknown_fields)]
pub struct ApiConfig {
    pub enabled: bool,
    pub source_of_truth: ApiSourceOfTruth,
    pub strict: bool,
    pub fail_on_missing_docs: bool,
    pub fail_on_openapi_mismatch: bool,
    pub fail_on_undocumented_status_code: bool,
    pub retention: ApiRetentionConfig,
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
            retention: ApiRetentionConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ApiRetentionConfig {
    pub auto_cleanup: bool,
    pub keep_snapshots: usize,
    pub keep_diffs: usize,
}

impl Default for ApiRetentionConfig {
    fn default() -> Self {
        Self {
            auto_cleanup: false,
            keep_snapshots: 2,
            keep_diffs: 2,
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
    let config: ProjectConfig =
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?;
    config.validate()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::ProjectConfig;

    #[test]
    fn rejects_unknown_top_level_field() {
        let error = toml::from_str::<ProjectConfig>("unknown = true")
            .expect_err("unknown configuration must fail");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_unknown_nested_field() {
        let error = toml::from_str::<ProjectConfig>("[storage]\nunknown = true")
            .expect_err("unknown nested configuration must fail");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn parses_pipeline_extraction_concurrency() {
        let config = toml::from_str::<ProjectConfig>(
            "[pipeline]\nextraction_concurrency = 4\nmax_extraction_bytes_in_flight = 1048576",
        )
        .expect("pipeline configuration should parse");
        assert_eq!(config.pipeline.extraction_concurrency, 4);
        assert_eq!(config.pipeline.max_extraction_bytes_in_flight, 1_048_576);
    }

    #[test]
    fn rejects_zero_pipeline_extraction_concurrency() {
        let config = toml::from_str::<ProjectConfig>("[pipeline]\nextraction_concurrency = 0")
            .expect("configuration syntax should parse");
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_zero_pipeline_byte_budget() {
        let config =
            toml::from_str::<ProjectConfig>("[pipeline]\nmax_extraction_bytes_in_flight = 0")
                .expect("configuration syntax should parse");
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_zero_per_adapter_extraction_concurrency() {
        let config = toml::from_str::<ProjectConfig>(
            "[pipeline.extraction_concurrency_by_adapter]\n\"builtin.extractor.markdown\" = 0",
        )
        .expect("configuration syntax should parse");
        assert!(config.validate().is_err());
    }
}
