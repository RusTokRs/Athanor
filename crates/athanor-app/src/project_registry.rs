use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::project_path::normalize_canonical_path;

pub const PROJECT_REGISTRY_SCHEMA: &str = "athanor.project_registry.v1";
pub const PROJECT_RESOLUTION_SCHEMA: &str = "athanor.project_resolution.v1";

#[derive(Debug, Clone)]
pub struct ProjectRegistryOptions {
    pub registry_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectRegisterOptions {
    pub registry_path: PathBuf,
    pub project_id: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectUnregisterOptions {
    pub registry_path: PathBuf,
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRegistration {
    pub project_id: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRegistry {
    pub schema: String,
    pub projects: Vec<ProjectRegistration>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectRegistryReport {
    pub schema: String,
    pub registry_path: PathBuf,
    pub projects: Vec<ProjectRegistration>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectResolutionReport {
    pub schema: String,
    pub registry_path: PathBuf,
    pub project: ProjectRegistration,
}

pub fn default_project_registry_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ATHANOR_PROJECT_REGISTRY") {
        if path.is_empty() {
            bail!("ATHANOR_PROJECT_REGISTRY must not be empty");
        }
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "cannot determine user home directory; set ATHANOR_PROJECT_REGISTRY explicitly"
            )
        })?;
    Ok(PathBuf::from(home).join(".athanor/projects.json"))
}

pub fn list_registered_projects(options: ProjectRegistryOptions) -> Result<ProjectRegistryReport> {
    let registry = load_registry(&options.registry_path)?;
    Ok(report(options.registry_path, registry))
}

pub fn register_project(options: ProjectRegisterOptions) -> Result<ProjectRegistryReport> {
    validate_project_id(&options.project_id)?;
    let root = normalize_canonical_path(options.root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize project root {}",
            options.root.display()
        )
    })?);
    if !root.is_dir() {
        bail!("project root is not a directory: {}", root.display());
    }

    let mut registry = load_registry(&options.registry_path)?;
    if registry
        .projects
        .iter()
        .any(|project| project.project_id == options.project_id)
    {
        bail!("project id `{}` is already registered", options.project_id);
    }
    if let Some(existing) = registry
        .projects
        .iter()
        .find(|project| project.root == root)
    {
        bail!(
            "project root {} is already registered as `{}`",
            root.display(),
            existing.project_id
        );
    }

    registry.projects.push(ProjectRegistration {
        project_id: options.project_id,
        root,
    });
    sort_projects(&mut registry.projects);
    write_registry(&options.registry_path, &registry)?;
    Ok(report(options.registry_path, registry))
}

pub fn unregister_project(options: ProjectUnregisterOptions) -> Result<ProjectRegistryReport> {
    validate_project_id(&options.project_id)?;
    let mut registry = load_registry(&options.registry_path)?;
    let previous_len = registry.projects.len();
    registry
        .projects
        .retain(|project| project.project_id != options.project_id);
    if registry.projects.len() == previous_len {
        bail!("project id `{}` is not registered", options.project_id);
    }
    write_registry(&options.registry_path, &registry)?;
    Ok(report(options.registry_path, registry))
}

pub fn resolve_registered_project(
    options: ProjectRegistryOptions,
    project_id: &str,
) -> Result<ProjectResolutionReport> {
    validate_project_id(project_id)?;
    let registry = load_registry(&options.registry_path)?;
    let project = registry
        .projects
        .into_iter()
        .find(|project| project.project_id == project_id)
        .ok_or_else(|| anyhow::anyhow!("project id `{project_id}` is not registered"))?;
    Ok(ProjectResolutionReport {
        schema: PROJECT_RESOLUTION_SCHEMA.to_string(),
        registry_path: options.registry_path,
        project,
    })
}

fn load_registry(path: &Path) -> Result<ProjectRegistry> {
    if !path.exists() {
        return Ok(ProjectRegistry {
            schema: PROJECT_REGISTRY_SCHEMA.to_string(),
            projects: Vec::new(),
        });
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut registry: ProjectRegistry = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if registry.schema != PROJECT_REGISTRY_SCHEMA {
        bail!(
            "unsupported project registry schema `{}` in {}",
            registry.schema,
            path.display()
        );
    }
    for project in &registry.projects {
        validate_project_id(&project.project_id)?;
        if !project.root.is_absolute() {
            bail!(
                "registered project `{}` has non-absolute root {}",
                project.project_id,
                project.root.display()
            );
        }
    }
    sort_projects(&mut registry.projects);
    Ok(registry)
}

fn write_registry(path: &Path, registry: &ProjectRegistry) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        anyhow::anyhow!("project registry path has no parent: {}", path.display())
    })?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create registry directory {}", parent.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid project registry path: {}", path.display()))?;
    let staging = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let backup = parent.join(format!(".{file_name}.backup-{}", std::process::id()));
    if staging.exists() {
        fs::remove_file(&staging)
            .with_context(|| format!("failed to remove stale staging {}", staging.display()))?;
    }
    if backup.exists() {
        fs::remove_file(&backup)
            .with_context(|| format!("failed to remove stale backup {}", backup.display()))?;
    }
    let content = serde_json::to_string_pretty(registry)?;
    fs::write(&staging, format!("{content}\n"))
        .with_context(|| format!("failed to write staged registry {}", staging.display()))?;
    if path.exists() {
        fs::rename(path, &backup)
            .with_context(|| format!("failed to stage previous registry {}", path.display()))?;
    }
    if let Err(error) = fs::rename(&staging, path) {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&staging);
        return Err(error)
            .with_context(|| format!("failed to publish project registry {}", path.display()));
    }
    if backup.exists() {
        fs::remove_file(&backup)
            .with_context(|| format!("failed to remove registry backup {}", backup.display()))?;
    }
    Ok(())
}

fn validate_project_id(project_id: &str) -> Result<()> {
    let valid = !project_id.is_empty()
        && project_id.len() <= 64
        && project_id
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && project_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        });
    if !valid {
        bail!(
            "invalid project id `{project_id}`; use 1-64 lowercase letters, digits, dots, underscores, or hyphens, starting with a letter or digit"
        );
    }
    Ok(())
}

fn sort_projects(projects: &mut [ProjectRegistration]) {
    projects.sort_by(|left, right| left.project_id.cmp(&right.project_id));
}

fn report(registry_path: PathBuf, registry: ProjectRegistry) -> ProjectRegistryReport {
    ProjectRegistryReport {
        schema: PROJECT_REGISTRY_SCHEMA.to_string(),
        registry_path,
        projects: registry.projects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_lists_resolves_and_unregisters_projects() {
        let root = temp_root("lifecycle");
        let registry_path = root.join("state/projects.json");
        let alpha = root.join("alpha");
        let beta = root.join("beta");
        fs::create_dir_all(&alpha).unwrap();
        fs::create_dir_all(&beta).unwrap();

        register_project(ProjectRegisterOptions {
            registry_path: registry_path.clone(),
            project_id: "beta".to_string(),
            root: beta.clone(),
        })
        .unwrap();
        let report = register_project(ProjectRegisterOptions {
            registry_path: registry_path.clone(),
            project_id: "alpha".to_string(),
            root: alpha.clone(),
        })
        .unwrap();

        assert_eq!(
            report
                .projects
                .iter()
                .map(|project| project.project_id.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "beta"]
        );
        assert_eq!(
            resolve_registered_project(
                ProjectRegistryOptions {
                    registry_path: registry_path.clone(),
                },
                "alpha",
            )
            .unwrap()
            .project
            .root,
            normalize_canonical_path(alpha.canonicalize().unwrap())
        );

        let report = unregister_project(ProjectUnregisterOptions {
            registry_path: registry_path.clone(),
            project_id: "alpha".to_string(),
        })
        .unwrap();
        assert_eq!(report.projects.len(), 1);
        assert_eq!(report.projects[0].project_id, "beta");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_duplicate_ids_roots_and_invalid_ids() {
        let root = temp_root("duplicates");
        let registry_path = root.join("projects.json");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        register_project(ProjectRegisterOptions {
            registry_path: registry_path.clone(),
            project_id: "project-one".to_string(),
            root: project.clone(),
        })
        .unwrap();

        assert!(
            register_project(ProjectRegisterOptions {
                registry_path: registry_path.clone(),
                project_id: "project-one".to_string(),
                root: project.clone(),
            })
            .unwrap_err()
            .to_string()
            .contains("already registered")
        );
        assert!(
            register_project(ProjectRegisterOptions {
                registry_path,
                project_id: "project-two".to_string(),
                root: project,
            })
            .unwrap_err()
            .to_string()
            .contains("already registered as")
        );
        assert!(validate_project_id("Invalid Project").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unknown_registry_schema() {
        let root = temp_root("schema");
        let registry_path = root.join("projects.json");
        fs::write(&registry_path, r#"{"schema":"unknown","projects":[]}"#).unwrap();

        let error = list_registered_projects(ProjectRegistryOptions { registry_path }).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("unsupported project registry schema")
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "athanor-project-registry-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
