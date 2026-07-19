use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_domain::Severity;
use serde::{Deserialize, Serialize};

pub const CONFIG_VALIDATE_SCHEMA: &str = "athanor.config_validate.v1";
pub const CONFIG_DOCTOR_SCHEMA: &str = "athanor.config_doctor.v1";

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub docs: DocsConfig,
    pub api: ApiConfig,
    pub storage: StorageConfig,
    pub adapters: AdaptersConfig,
    pub pipeline: PipelineConfig,
}

#[derive(Debug, Clone)]
pub struct ConfigReportOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigValidateReport {
    pub schema: &'static str,
    pub root: PathBuf,
    #[serde(flatten)]
    pub config: ProjectConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigDoctorCheck {
    pub name: &'static str,
    pub status: &'static str,
    pub detail: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigDoctorReport {
    pub schema: &'static str,
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub checks: Vec<ConfigDoctorCheck>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AdaptersConfig {
    pub allow_external_process: bool,
    pub external_process_allowlist: Vec<String>,
    pub external_process_sandbox: ExternalProcessSandboxProfile,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalProcessSandboxProfile {
    #[default]
    Disabled,
    CleanEnvironment,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PipelineConfig {
    pub extraction_concurrency: usize,
    pub max_extraction_bytes_in_flight: usize,
    pub max_snapshot_batch_objects: usize,
    pub max_snapshot_batch_bytes: usize,
    pub extraction_concurrency_by_adapter: BTreeMap<String, usize>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            extraction_concurrency: 16,
            max_extraction_bytes_in_flight: 64 * 1024 * 1024,
            max_snapshot_batch_objects: 1_000_000,
            max_snapshot_batch_bytes: 512 * 1024 * 1024,
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
        if self.pipeline.max_snapshot_batch_objects == 0 {
            anyhow::bail!("[pipeline].max_snapshot_batch_objects must be at least 1");
        }
        if self.pipeline.max_snapshot_batch_bytes == 0 {
            anyhow::bail!("[pipeline].max_snapshot_batch_bytes must be at least 1");
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
                "status".to_string(),
            ],
            allowed_statuses: ["active", "implemented", "planned", "draft", "verified"]
                .into_iter()
                .map(str::to_string)
                .collect(),
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

pub fn validate_project_config(options: ConfigReportOptions) -> Result<ConfigValidateReport> {
    let config = load_config(&options.root)?;
    Ok(ConfigValidateReport {
        schema: CONFIG_VALIDATE_SCHEMA,
        root: options.root,
        config,
    })
}

pub fn doctor_project_config(options: ConfigReportOptions) -> Result<ConfigDoctorReport> {
    let config = load_config(&options.root)?;
    let external_process_sandbox = match config.adapters.external_process_sandbox {
        ExternalProcessSandboxProfile::Disabled => "disabled",
        ExternalProcessSandboxProfile::CleanEnvironment => "clean_environment",
    };

    Ok(ConfigDoctorReport {
        schema: CONFIG_DOCTOR_SCHEMA,
        root: options.root,
        config,
        checks: vec![
            ConfigDoctorCheck {
                name: "storage_backend",
                status: "available",
                detail: "the configured storage mode is compiled into this build",
            },
            ConfigDoctorCheck {
                name: "external_process_adapters",
                status: "configured",
                detail: "external process adapters require explicit enablement, trust, and allowlisting",
            },
            ConfigDoctorCheck {
                name: "external_process_sandbox",
                status: external_process_sandbox,
                detail: "clean_environment clears inherited environment variables only; it is not OS-level filesystem, network, or CPU isolation",
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        CONFIG_DOCTOR_SCHEMA, CONFIG_VALIDATE_SCHEMA, ConfigReportOptions,
        ExternalProcessSandboxProfile, ProjectConfig, doctor_project_config,
        validate_project_config,
    };

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
            "[pipeline]\nextraction_concurrency = 4\nmax_extraction_bytes_in_flight = 1048576\nmax_snapshot_batch_objects = 42\nmax_snapshot_batch_bytes = 2097152",
        )
        .expect("pipeline configuration should parse");
        assert_eq!(config.pipeline.extraction_concurrency, 4);
        assert_eq!(config.pipeline.max_extraction_bytes_in_flight, 1_048_576);
        assert_eq!(config.pipeline.max_snapshot_batch_objects, 42);
        assert_eq!(config.pipeline.max_snapshot_batch_bytes, 2_097_152);
    }

    #[test]
    fn parses_opt_in_clean_environment_process_profile() {
        let config = toml::from_str::<ProjectConfig>(
            "[adapters]\nexternal_process_sandbox = \"clean_environment\"",
        )
        .expect("sandbox configuration should parse");
        assert_eq!(
            config.adapters.external_process_sandbox,
            ExternalProcessSandboxProfile::CleanEnvironment
        );
    }

    #[test]
    fn defaults_external_process_sandbox_to_disabled() {
        assert_eq!(
            ProjectConfig::default().adapters.external_process_sandbox,
            ExternalProcessSandboxProfile::Disabled
        );
    }

    #[test]
    fn default_docs_policy_separates_completeness_from_snapshot_drift() {
        let policy = ProjectConfig::default().docs.completeness;
        assert_eq!(
            policy.required_fields,
            ["id", "kind", "language", "source_language", "status"]
        );
        assert_eq!(
            policy.allowed_statuses,
            ["active", "implemented", "planned", "draft", "verified"]
        );
        assert!(!policy.require_current_snapshot);
        assert!(!policy
            .required_fields
            .iter()
            .any(|field| field == "last_verified_snapshot"));
    }

    #[test]
    fn reports_use_versioned_top_level_shapes() {
        let root = PathBuf::from("project");
        let validation = validate_project_config(ConfigReportOptions { root: root.clone() })
            .expect("default validation report");
        let doctor = doctor_project_config(ConfigReportOptions { root })
            .expect("default doctor report");

        let validation = serde_json::to_value(validation).expect("serialize validation report");
        assert_eq!(validation["schema"], CONFIG_VALIDATE_SCHEMA);
        assert_eq!(validation["root"], "project");
        assert!(validation.get("config").is_none());
        assert!(validation.get("storage").is_some());

        let doctor = serde_json::to_value(doctor).expect("serialize doctor report");
        assert_eq!(doctor["schema"], CONFIG_DOCTOR_SCHEMA);
        assert_eq!(doctor["root"], "project");
        assert_eq!(doctor["checks"].as_array().map(Vec::len), Some(3));
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
    fn rejects_zero_snapshot_batch_limits() {
        let objects = toml::from_str::<ProjectConfig>("[pipeline]\nmax_snapshot_batch_objects = 0")
            .expect("configuration syntax should parse");
        assert!(objects.validate().is_err());

        let bytes = toml::from_str::<ProjectConfig>("[pipeline]\nmax_snapshot_batch_bytes = 0")
            .expect("configuration syntax should parse");
        assert!(bytes.validate().is_err());
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
