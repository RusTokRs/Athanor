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

[project]
name = "athanor-project"

[docs]
editable_path = "docs"
generated_path = ".athanor/generated/current/wiki"
languages = ["ru", "en"]
source_language = "ru"
mode = "patch-based"

[docs.completeness]
required_fields = ["id", "kind", "language", "source_language", "last_verified_snapshot", "status"]
allowed_statuses = ["verified"]
minimum_diagnostic_severity = "medium"
require_current_snapshot = false

[docs.api]
enabled = true
source_of_truth = "hybrid"
strict = true

[docs.operations]
enabled = true
include_scripts = true
include_env = true
include_docker = true
include_ci = true

[commands]
allow_external = false
allow_network = false
allowed = []

[network]
enabled = false
allow = []
"#
}

#[cfg(test)]
mod tests {
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
}
