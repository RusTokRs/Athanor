use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct InitReport {
    pub root: PathBuf,
    pub created: Vec<PathBuf>,
}

pub fn init_project(options: InitOptions) -> Result<InitReport> {
    let root = options.root;
    let athanor_dir = root.join(".athanor");
    let knowledge_dir = athanor_dir.join("knowledge");
    let generated_dir = athanor_dir.join("generated");
    let config_path = root.join("athanor.toml");

    let mut created = Vec::new();

    create_dir(&athanor_dir, &mut created)?;
    create_dir(&knowledge_dir, &mut created)?;
    create_dir(&knowledge_dir.join("docs"), &mut created)?;
    create_dir(&generated_dir, &mut created)?;

    if !config_path.exists() {
        fs::write(&config_path, default_config())
            .with_context(|| format!("failed to write {}", config_path.display()))?;
        created.push(config_path);
    }

    Ok(InitReport { root, created })
}

fn create_dir(path: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
        created.push(path.to_path_buf());
    }

    Ok(())
}

fn default_config() -> &'static str {
    r#"# Athanor project configuration

[docs]
editable_path = "docs"

[docs.completeness]
required_fields = ["id", "kind", "language", "source_language", "status"]
allowed_statuses = ["active", "implemented", "planned", "draft", "verified"]
minimum_diagnostic_severity = "medium"
require_current_snapshot = false

[api]
enabled = true
source_of_truth = "hybrid"
strict = true
fail_on_missing_docs = true
fail_on_openapi_mismatch = true
fail_on_undocumented_status_code = true

[api.retention]
auto_cleanup = false
keep_snapshots = 2
keep_diffs = 2

[storage]
mode = "jsonl"
path = ".athanor/store/canonical/jsonl"

[adapters]
allow_external_process = false
external_process_allowlist = []
external_process_sandbox = "disabled"

[pipeline]
extraction_concurrency = 16
max_extraction_bytes_in_flight = 67108864
max_snapshot_batch_objects = 1000000
max_snapshot_batch_bytes = 536870912
"#
}

#[cfg(test)]
mod tests {
    use crate::config::ProjectConfig;

    use super::*;

    #[test]
    fn init_project_creates_expected_layout() {
        let root = std::env::temp_dir().join(format!(
            "athanor-init-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let report = init_project(InitOptions { root: root.clone() }).unwrap();

        assert!(root.join(".athanor").is_dir());
        assert!(root.join(".athanor/knowledge/docs").is_dir());
        assert!(root.join(".athanor/generated").is_dir());
        assert!(root.join("athanor.toml").is_file());
        assert!(!report.created.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_config_parses_as_the_current_contract() {
        let config = toml::from_str::<ProjectConfig>(default_config())
            .expect("ath init must emit a valid ProjectConfig");
        assert_eq!(config.docs.editable_path, "docs");
        assert_eq!(
            config.docs.completeness.required_fields,
            ["id", "kind", "language", "source_language", "status"]
        );
        assert_eq!(
            config.docs.completeness.allowed_statuses,
            ["active", "implemented", "planned", "draft", "verified"]
        );
        assert!(!config.docs.completeness.require_current_snapshot);
    }
}
