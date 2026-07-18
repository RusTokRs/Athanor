use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_projector_support::replace_output_file;
use serde::{Deserialize, Serialize};

use crate::generation::{
    CurrentGeneration, GENERATED_CURRENT_SCHEMA, GENERATED_GENERATION_SCHEMA, GenerationOptions,
    GenerationReport, generate_project_with_composition,
};
use crate::project_path::normalize_canonical_path;
use crate::RuntimeComposition;

#[derive(Debug, Clone)]
pub struct RepairInspectOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RepairCleanupOptions {
    pub root: PathBuf,
    pub dry_run: bool,
    pub keep_canonical: usize,
    pub keep_generated: usize,
    pub generated_only: bool,
}

#[derive(Debug, Clone)]
pub struct RepairRegenerateOptions {
    pub root: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct RepairRecoverCanonicalOptions {
    pub root: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct RepairApplyOptions {
    pub root: PathBuf,
    pub dry_run: bool,
    pub keep_canonical: usize,
    pub keep_generated: usize,
    pub generated_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairInspectReport {
    pub schema: String,
    pub root: PathBuf,
    pub status: RepairStatus,
    pub issues: Vec<RepairIssue>,
    pub canonical: CanonicalRepairState,
    pub generated: GeneratedRepairState,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairCleanupReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub removed: Vec<RepairCleanupRemoval>,
    pub retained: Vec<RepairCleanupRetained>,
    pub remaining_issues: Vec<RepairIssue>,
    pub inspection: RepairInspectReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairRegenerateReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub needed: bool,
    pub generated: Option<GenerationReport>,
    pub remaining_issues: Vec<RepairIssue>,
    pub inspection: RepairInspectReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairRecoverCanonicalReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub needed: bool,
    pub selected_snapshot: Option<String>,
    pub recovered_snapshot: Option<String>,
    pub remaining_issues: Vec<RepairIssue>,
    pub inspection: RepairInspectReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairApplyReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub canonical: RepairRecoverCanonicalReport,
    pub generated: RepairRegenerateReport,
    pub cleanup: RepairCleanupReport,
    pub remaining_issues: Vec<RepairIssue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairCleanupRemoval {
    pub kind: RepairCleanupRemovalKind,
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairCleanupRetained {
    pub kind: RepairCleanupRemovalKind,
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairCleanupRemovalKind {
    CanonicalSnapshot,
    GeneratedGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairStatus {
    Clean,
    NeedsRepair,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairIssue {
    pub code: String,
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CanonicalRepairState {
    pub latest_snapshot: Option<String>,
    pub snapshot_count: usize,
    pub orphan_snapshots: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedRepairState {
    pub current_generation: Option<String>,
    pub generation_count: usize,
    pub orphan_generations: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LatestSnapshot {
    snapshot: String,
}

#[derive(Debug, Deserialize)]
struct CanonicalManifest {
    schema: String,
    snapshot: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GenerationManifest {
    schema: String,
    generation: String,
    snapshot: String,
}

pub fn inspect_repair(options: RepairInspectOptions) -> Result<RepairInspectReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let canonical_root = root.join(".athanor/store/canonical/jsonl");
    let generated_root = root.join(".athanor/generated");
    let mut issues = Vec::new();

    let canonical = inspect_canonical(&canonical_root, &mut issues)?;
    let generated = inspect_generated(&generated_root, &canonical, &mut issues)?;
    let status = if issues.is_empty() {
        RepairStatus::Clean
    } else {
        RepairStatus::NeedsRepair
    };

    Ok(RepairInspectReport {
        schema: "athanor.repair_inspect.v1".to_string(),
        root,
        status,
        issues,
        canonical,
        generated,
    })
}

pub fn cleanup_repair(options: RepairCleanupOptions) -> Result<RepairCleanupReport> {
    let inspection = inspect_repair(RepairInspectOptions {
        root: options.root.clone(),
    })?;
    let root = inspection.root.clone();
    let canonical_snapshots_root = root.join(".athanor/store/canonical/jsonl/snapshots");
    let generated_generations_root = root.join(".athanor/generated/generations");
    let mut removed = Vec::new();
    let mut retained = Vec::new();

    if !options.generated_only {
        let (canonical_remove, canonical_retain) = split_retained(
            &inspection.canonical.orphan_snapshots,
            options.keep_canonical,
        );
        for snapshot in canonical_retain {
            retained.push(RepairCleanupRetained {
                kind: RepairCleanupRemovalKind::CanonicalSnapshot,
                id: snapshot.clone(),
                path: canonical_snapshots_root.join(snapshot),
            });
        }
        for snapshot in canonical_remove {
            let path = canonical_snapshots_root.join(snapshot);
            removed.push(RepairCleanupRemoval {
                kind: RepairCleanupRemovalKind::CanonicalSnapshot,
                id: snapshot.clone(),
                path: path.clone(),
            });
            if !options.dry_run {
                remove_directory_inside(&canonical_snapshots_root, &path)?;
            }
        }
    }

    let (generated_remove, generated_retain) = split_retained(
        &inspection.generated.orphan_generations,
        options.keep_generated,
    );
    for generation in generated_retain {
        retained.push(RepairCleanupRetained {
            kind: RepairCleanupRemovalKind::GeneratedGeneration,
            id: generation.clone(),
            path: generated_generations_root.join(generation),
        });
    }
    for generation in generated_remove {
        let path = generated_generations_root.join(generation);
        removed.push(RepairCleanupRemoval {
            kind: RepairCleanupRemovalKind::GeneratedGeneration,
            id: generation.clone(),
            path: path.clone(),
        });
        if !options.dry_run {
            remove_directory_inside(&generated_generations_root, &path)?;
        }
    }

    let remaining_issues = if options.dry_run {
        inspection.issues.clone()
    } else {
        inspect_repair(RepairInspectOptions { root: root.clone() })?.issues
    };

    Ok(RepairCleanupReport {
        schema: "athanor.repair_cleanup.v1".to_string(),
        root,
        dry_run: options.dry_run,
        removed,
        retained,
        remaining_issues,
        inspection,
    })
}

pub async fn regenerate_repair(
    options: RepairRegenerateOptions,
    composition: &RuntimeComposition,
) -> Result<RepairRegenerateReport> {
    let inspection = inspect_repair(RepairInspectOptions {
        root: options.root.clone(),
    })?;
    let root = inspection.root.clone();
    let needed = inspection.issues.iter().any(is_generated_pointer_issue);

    let generated = if needed && !options.dry_run {
        Some(
            generate_project_with_composition(
                GenerationOptions {
                    root: root.clone(),
                    force: false,
                },
                composition,
            )
            .await
            .context("failed to regenerate generated outputs")?,
        )
    } else {
        None
    };

    let remaining_issues = if needed && !options.dry_run {
        inspect_repair(RepairInspectOptions { root: root.clone() })?.issues
    } else {
        inspection.issues.clone()
    };

    Ok(RepairRegenerateReport {
        schema: "athanor.repair_regenerate.v1".to_string(),
        root,
        dry_run: options.dry_run,
        needed,
        generated,
        remaining_issues,
        inspection,
    })
}

pub fn recover_canonical_repair(
    options: RepairRecoverCanonicalOptions,
) -> Result<RepairRecoverCanonicalReport> {
    let inspection = inspect_repair(RepairInspectOptions {
        root: options.root.clone(),
    })?;
    let root = inspection.root.clone();
    let needed = inspection.issues.iter().any(is_canonical_pointer_issue);
    let canonical_root = root.join(".athanor/store/canonical/jsonl");
    let selected_snapshot = if needed {
        newest_valid_canonical_snapshot(&canonical_root)?
    } else {
        inspection.canonical.latest_snapshot.clone()
    };
    let recovered_snapshot = if needed && !options.dry_run {
        let selected = selected_snapshot
            .clone()
            .context("cannot recover canonical latest pointer: no valid snapshot found")?;
        write_canonical_latest(&canonical_root, &selected)?;
        Some(selected)
    } else {
        None
    };
    let remaining_issues = if needed && !options.dry_run {
        inspect_repair(RepairInspectOptions { root: root.clone() })?.issues
    } else {
        inspection.issues.clone()
    };

    Ok(RepairRecoverCanonicalReport {
        schema: "athanor.repair_recover_canonical.v1".to_string(),
        root,
        dry_run: options.dry_run,
        needed,
        selected_snapshot,
        recovered_snapshot,
        remaining_issues,
        inspection,
    })
}

pub async fn apply_repair(
    options: RepairApplyOptions,
    composition: &RuntimeComposition,
) -> Result<RepairApplyReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let canonical = recover_canonical_repair(RepairRecoverCanonicalOptions {
        root: root.clone(),
        dry_run: options.dry_run,
    })?;
    let generated = regenerate_repair(
        RepairRegenerateOptions {
            root: root.clone(),
            dry_run: options.dry_run,
        },
        composition,
    )
    .await?;
    let cleanup = cleanup_repair(RepairCleanupOptions {
        root: root.clone(),
        dry_run: options.dry_run,
        keep_canonical: options.keep_canonical,
        keep_generated: options.keep_generated,
        generated_only: options.generated_only,
    })?;
    let remaining_issues = if options.dry_run {
        cleanup.remaining_issues.clone()
    } else {
        inspect_repair(RepairInspectOptions { root: root.clone() })?.issues
    };

    Ok(RepairApplyReport {
        schema: "athanor.repair_apply.v1".to_string(),
        root,
        dry_run: options.dry_run,
        canonical,
        generated,
        cleanup,
        remaining_issues,
    })
}

fn inspect_canonical(
    canonical_root: &Path,
    issues: &mut Vec<RepairIssue>,
) -> Result<CanonicalRepairState> {
    let snapshots_root = canonical_root.join("snapshots");
    let snapshots = list_directory_names(&snapshots_root)?;
    let latest_path = canonical_root.join("latest.json");
    let latest_snapshot =
        read_json::<LatestSnapshot>(&latest_path, issues)?.map(|latest| latest.snapshot);

    if let Some(snapshot) = &latest_snapshot {
        if !snapshots.contains(snapshot) {
            issues.push(issue(
                "missing_latest_snapshot",
                canonical_root.join("snapshots").join(snapshot),
                format!("latest snapshot {snapshot} has no snapshot directory"),
            ));
        }
    } else if !snapshots.is_empty() {
        issues.push(issue(
            "missing_latest_pointer",
            latest_path.clone(),
            "canonical snapshots exist but latest.json is missing or invalid".to_string(),
        ));
    }

    for snapshot in &snapshots {
        let manifest_path = snapshots_root.join(snapshot).join("manifest.json");
        if let Some(manifest) = read_json::<CanonicalManifest>(&manifest_path, issues)? {
            if manifest.schema != "athanor.canonical_snapshot.v1" {
                issues.push(issue(
                    "invalid_canonical_manifest_schema",
                    manifest_path.clone(),
                    format!(
                        "canonical manifest has unsupported schema {}",
                        manifest.schema
                    ),
                ));
            }
            if manifest.snapshot != *snapshot {
                issues.push(issue(
                    "canonical_manifest_snapshot_mismatch",
                    manifest_path,
                    format!(
                        "canonical manifest snapshot {} does not match directory {snapshot}",
                        manifest.snapshot
                    ),
                ));
            }
        }
    }

    let orphan_snapshots = snapshots
        .iter()
        .filter(|snapshot| Some(snapshot.as_str()) != latest_snapshot.as_deref())
        .cloned()
        .collect();

    Ok(CanonicalRepairState {
        latest_snapshot,
        snapshot_count: snapshots.len(),
        orphan_snapshots,
    })
}

fn inspect_generated(
    generated_root: &Path,
    canonical: &CanonicalRepairState,
    issues: &mut Vec<RepairIssue>,
) -> Result<GeneratedRepairState> {
    let generations_root = generated_root.join("generations");
    let generations = list_directory_names(&generations_root)?;
    let current_path = generated_root.join("current.json");
    let current = read_json::<CurrentGeneration>(&current_path, issues)?;
    let current_generation = current.as_ref().map(|current| current.generation.clone());

    if let Some(current) = &current {
        if current.schema != GENERATED_CURRENT_SCHEMA {
            issues.push(issue(
                "invalid_generated_current_schema",
                current_path.clone(),
                format!(
                    "generated current pointer has unsupported schema {}",
                    current.schema
                ),
            ));
        }
        let expected_path = Path::new("generations").join(&current.generation);
        if Path::new(&current.path) != expected_path {
            issues.push(issue(
                "invalid_generated_current_path",
                current_path.clone(),
                format!(
                    "current generation path {} does not match expected {}",
                    current.path,
                    expected_path.display()
                ),
            ));
        }
        let expected_manifest = expected_path.join("manifest.json");
        if Path::new(&current.manifest) != expected_manifest {
            issues.push(issue(
                "invalid_generated_current_manifest_path",
                current_path.clone(),
                format!(
                    "current generation manifest path {} does not match expected {}",
                    current.manifest,
                    expected_manifest.display()
                ),
            ));
        }
        if !generations.contains(&current.generation) {
            issues.push(issue(
                "missing_current_generation",
                generations_root.join(&current.generation),
                format!("current generation {} has no directory", current.generation),
            ));
        }
        if canonical
            .latest_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot != &current.snapshot)
        {
            issues.push(issue(
                "stale_current_generation_snapshot",
                current_path.clone(),
                format!(
                    "current generation points to snapshot {}, latest canonical snapshot is {}",
                    current.snapshot,
                    canonical.latest_snapshot.as_deref().unwrap_or("unknown")
                ),
            ));
        }
    } else if !generations.is_empty() {
        issues.push(issue(
            "missing_generated_current",
            current_path.clone(),
            "generated generations exist but current.json is missing or invalid".to_string(),
        ));
    }

    for generation in &generations {
        let manifest_path = generations_root.join(generation).join("manifest.json");
        let Some(manifest) = inspect_generation_manifest(
            generation,
            &manifest_path,
            canonical.latest_snapshot.as_deref(),
            issues,
        )?
        else {
            if current_generation.as_deref() == Some(generation.as_str()) && !manifest_path.exists()
            {
                issues.push(issue(
                    "missing_current_generation_manifest",
                    manifest_path,
                    format!("current generation {generation} has no manifest.json"),
                ));
            }
            continue;
        };
        if current_generation.as_deref() == Some(generation.as_str()) {
            inspect_current_generation_manifest(
                &manifest,
                current.as_ref(),
                &manifest_path,
                issues,
            );
        }
    }

    let orphan_generations = generations
        .iter()
        .filter(|generation| Some(generation.as_str()) != current_generation.as_deref())
        .cloned()
        .collect();

    Ok(GeneratedRepairState {
        current_generation,
        generation_count: generations.len(),
        orphan_generations,
    })
}

fn inspect_generation_manifest(
    generation: &str,
    manifest_path: &Path,
    latest_snapshot: Option<&str>,
    issues: &mut Vec<RepairIssue>,
) -> Result<Option<GenerationManifest>> {
    let Some(manifest) = read_json::<GenerationManifest>(manifest_path, issues)? else {
        return Ok(None);
    };
    if manifest.schema != GENERATED_GENERATION_SCHEMA {
        issues.push(issue(
            "invalid_generation_manifest_schema",
            manifest_path.to_path_buf(),
            format!(
                "generation manifest has unsupported schema {}",
                manifest.schema
            ),
        ));
    }
    if manifest.generation != generation {
        issues.push(issue(
            "generation_manifest_id_mismatch",
            manifest_path.to_path_buf(),
            format!(
                "generation manifest id {} does not match directory {generation}",
                manifest.generation
            ),
        ));
    }
    if latest_snapshot.is_some_and(|snapshot| snapshot != manifest.snapshot) {
        issues.push(issue(
            "stale_generation_snapshot",
            manifest_path.to_path_buf(),
            format!(
                "generation {generation} was built from snapshot {}, latest canonical snapshot is {}",
                manifest.snapshot,
                latest_snapshot.unwrap_or("unknown")
            ),
        ));
    }
    Ok(Some(manifest))
}

fn inspect_current_generation_manifest(
    manifest: &GenerationManifest,
    current: Option<&CurrentGeneration>,
    manifest_path: &Path,
    issues: &mut Vec<RepairIssue>,
) {
    let Some(current) = current else {
        return;
    };
    if manifest.schema != GENERATED_GENERATION_SCHEMA {
        issues.push(issue(
            "invalid_current_generation_manifest_schema",
            manifest_path.to_path_buf(),
            format!(
                "current generation manifest has unsupported schema {}",
                manifest.schema
            ),
        ));
    }
    if manifest.generation != current.generation {
        issues.push(issue(
            "current_generation_manifest_id_mismatch",
            manifest_path.to_path_buf(),
            format!(
                "current generation manifest id {} does not match current pointer {}",
                manifest.generation, current.generation
            ),
        ));
    }
    if manifest.snapshot != current.snapshot {
        issues.push(issue(
            "current_generation_manifest_snapshot_mismatch",
            manifest_path.to_path_buf(),
            format!(
                "current generation manifest snapshot {} does not match current pointer {}",
                manifest.snapshot, current.snapshot
            ),
        ));
    }
}

fn list_directory_names(path: &Path) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(names),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };

    for entry in entries {
        let entry = entry.with_context(|| format!("failed to inspect {}", path.display()))?;
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?
            .is_dir()
        {
            names.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(names)
}

fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    issues: &mut Vec<RepairIssue>,
) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    match serde_json::from_str(&content) {
        Ok(value) => Ok(Some(value)),
        Err(error) => {
            issues.push(issue(
                "invalid_json",
                path.to_path_buf(),
                format!("failed to parse JSON: {error}"),
            ));
            Ok(None)
        }
    }
}

fn issue(code: &str, path: PathBuf, message: String) -> RepairIssue {
    RepairIssue {
        code: code.to_string(),
        path,
        message,
    }
}

fn remove_directory_inside(root: &Path, path: &Path) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;

    if !path.starts_with(&root) || path == root {
        anyhow::bail!(
            "refusing to remove {} outside cleanup root {}",
            path.display(),
            root.display()
        );
    }

    fs::remove_dir_all(&path).with_context(|| format!("failed to remove {}", path.display()))
}

fn split_retained(items: &[String], keep: usize) -> (&[String], &[String]) {
    let remove_count = items.len().saturating_sub(keep);
    items.split_at(remove_count)
}

fn is_generated_pointer_issue(issue: &RepairIssue) -> bool {
    matches!(
        issue.code.as_str(),
        "invalid_generated_current_schema"
            | "missing_current_generation"
            | "invalid_generated_current_path"
            | "invalid_generated_current_manifest_path"
            | "missing_current_generation_manifest"
            | "invalid_current_generation_manifest_schema"
            | "current_generation_manifest_id_mismatch"
            | "current_generation_manifest_snapshot_mismatch"
            | "stale_current_generation_snapshot"
            | "missing_generated_current"
    ) || (issue.code == "invalid_json"
        && issue
            .path
            .ends_with(Path::new(".athanor/generated/current.json")))
}

fn is_canonical_pointer_issue(issue: &RepairIssue) -> bool {
    matches!(
        issue.code.as_str(),
        "missing_latest_snapshot" | "missing_latest_pointer"
    ) || (issue.code == "invalid_json"
        && issue
            .path
            .ends_with(Path::new(".athanor/store/canonical/jsonl/latest.json")))
}

fn newest_valid_canonical_snapshot(canonical_root: &Path) -> Result<Option<String>> {
    let snapshots_root = canonical_root.join("snapshots");
    let snapshots = list_directory_names(&snapshots_root)?;
    let mut valid = Vec::new();

    for snapshot in snapshots {
        let manifest_path = snapshots_root.join(&snapshot).join("manifest.json");
        let Ok(content) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<CanonicalManifest>(&content) else {
            continue;
        };
        if manifest.schema == "athanor.canonical_snapshot.v1" && manifest.snapshot == snapshot {
            valid.push(snapshot);
        }
    }

    Ok(valid.into_iter().max())
}

fn write_canonical_latest(canonical_root: &Path, snapshot: &str) -> Result<()> {
    let latest = LatestSnapshot {
        snapshot: snapshot.to_string(),
    };
    replace_output_file(
        &canonical_root.join("latest.json"),
        &serde_json::to_string_pretty(&latest)
            .context("failed to serialize canonical latest pointer")?,
        "canonical latest pointer",
    )
    .context("failed to write canonical latest pointer")
}

#[cfg(test)]
mod tests {
    use athanor_core::KnowledgeStore;
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;

    use super::*;

    #[test]
    fn detects_orphan_and_stale_generated_generation() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        let generated_root = root.join(".athanor/generated");
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000002")).unwrap();
        fs::create_dir_all(generated_root.join("generations/00000001")).unwrap();
        fs::create_dir_all(generated_root.join("generations/00000002")).unwrap();
        fs::write(
            canonical_root.join("latest.json"),
            r#"{"snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000002/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("current.json"),
            r#"{"schema":"athanor.generated_current.v1","generation":"00000002","snapshot":"snap_jsonl_00000001","path":"generations/00000002","manifest":"generations/00000002/manifest.json"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000001/manifest.json"),
            r#"{"schema":"athanor.generated_generation.v1","generation":"00000001","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000002/manifest.json"),
            r#"{"schema":"athanor.generated_generation.v1","generation":"00000002","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();

        let report = inspect_repair(RepairInspectOptions { root: root.clone() }).unwrap();

        assert_eq!(report.status, RepairStatus::NeedsRepair);
        assert_eq!(
            report.canonical.latest_snapshot.as_deref(),
            Some("snap_jsonl_00000002")
        );
        assert_eq!(
            report.generated.current_generation.as_deref(),
            Some("00000002")
        );
        assert_eq!(report.generated.orphan_generations, vec!["00000001"]);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "stale_current_generation_snapshot")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_removes_orphan_artifact_directories_only() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        let generated_root = root.join(".athanor/generated");
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000001")).unwrap();
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000002")).unwrap();
        fs::create_dir_all(generated_root.join("generations/00000001")).unwrap();
        fs::create_dir_all(generated_root.join("generations/00000002")).unwrap();
        fs::write(
            canonical_root.join("latest.json"),
            r#"{"snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000001/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000002/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("current.json"),
            r#"{"schema":"athanor.generated_current.v1","generation":"00000002","snapshot":"snap_jsonl_00000002","path":"generations/00000002","manifest":"generations/00000002/manifest.json"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000001/manifest.json"),
            r#"{"schema":"athanor.generated_generation.v1","generation":"00000001","snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000002/manifest.json"),
            r#"{"schema":"athanor.generated_generation.v1","generation":"00000002","snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();

        let report = cleanup_repair(RepairCleanupOptions {
            root: root.clone(),
            dry_run: false,
            keep_canonical: 0,
            keep_generated: 0,
            generated_only: false,
        })
        .unwrap();

        assert_eq!(report.removed.len(), 2);
        assert!(
            !canonical_root
                .join("snapshots/snap_jsonl_00000001")
                .exists()
        );
        assert!(
            canonical_root
                .join("snapshots/snap_jsonl_00000002")
                .is_dir()
        );
        assert!(!generated_root.join("generations/00000001").exists());
        assert!(generated_root.join("generations/00000002").is_dir());
        assert!(report.remaining_issues.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_dry_run_reports_without_removing() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000001")).unwrap();
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000002")).unwrap();
        fs::write(
            canonical_root.join("latest.json"),
            r#"{"snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000001/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000002/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();

        let report = cleanup_repair(RepairCleanupOptions {
            root: root.clone(),
            dry_run: true,
            keep_canonical: 0,
            keep_generated: 0,
            generated_only: false,
        })
        .unwrap();

        assert_eq!(report.removed.len(), 1);
        assert!(
            canonical_root
                .join("snapshots/snap_jsonl_00000001")
                .is_dir()
        );
        assert!(
            canonical_root
                .join("snapshots/snap_jsonl_00000002")
                .is_dir()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_retains_newest_orphans_when_requested() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        let generated_root = root.join(".athanor/generated");
        for snapshot in [
            "snap_jsonl_00000001",
            "snap_jsonl_00000002",
            "snap_jsonl_00000003",
        ] {
            fs::create_dir_all(canonical_root.join("snapshots").join(snapshot)).unwrap();
            fs::write(
                canonical_root
                    .join("snapshots")
                    .join(snapshot)
                    .join("manifest.json"),
                format!(r#"{{"schema":"athanor.canonical_snapshot.v1","snapshot":"{snapshot}"}}"#),
            )
            .unwrap();
        }
        for generation in ["00000001", "00000002", "00000003"] {
            fs::create_dir_all(generated_root.join("generations").join(generation)).unwrap();
            fs::write(
                generated_root
                    .join("generations")
                    .join(generation)
                    .join("manifest.json"),
                format!(
                    r#"{{"schema":"athanor.generated_generation.v1","generation":"{generation}","snapshot":"snap_jsonl_00000003"}}"#
                ),
            )
            .unwrap();
        }
        fs::write(
            canonical_root.join("latest.json"),
            r#"{"snapshot":"snap_jsonl_00000003"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("current.json"),
            r#"{"schema":"athanor.generated_current.v1","generation":"00000003","snapshot":"snap_jsonl_00000003","path":"generations/00000003","manifest":"generations/00000003/manifest.json"}"#,
        )
        .unwrap();

        let report = cleanup_repair(RepairCleanupOptions {
            root: root.clone(),
            dry_run: false,
            keep_canonical: 1,
            keep_generated: 1,
            generated_only: false,
        })
        .unwrap();

        assert_eq!(report.removed.len(), 2);
        assert_eq!(report.retained.len(), 2);
        assert!(
            !canonical_root
                .join("snapshots/snap_jsonl_00000001")
                .exists()
        );
        assert!(
            canonical_root
                .join("snapshots/snap_jsonl_00000002")
                .is_dir()
        );
        assert!(
            canonical_root
                .join("snapshots/snap_jsonl_00000003")
                .is_dir()
        );
        assert!(!generated_root.join("generations/00000001").exists());
        assert!(generated_root.join("generations/00000002").is_dir());
        assert!(generated_root.join("generations/00000003").is_dir());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_generated_only_leaves_orphan_canonical_snapshots() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        let generated_root = root.join(".athanor/generated");
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000001")).unwrap();
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000002")).unwrap();
        fs::write(
            canonical_root.join("latest.json"),
            r#"{"snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000001/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000002/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();
        fs::create_dir_all(generated_root.join("generations/00000001")).unwrap();
        fs::create_dir_all(generated_root.join("generations/00000002")).unwrap();
        fs::write(
            generated_root.join("current.json"),
            r#"{"schema":"athanor.generated_current.v1","generation":"00000002","snapshot":"snap_jsonl_00000002","path":"generations/00000002","manifest":"generations/00000002/manifest.json"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000001/manifest.json"),
            r#"{"schema":"athanor.generated_generation.v1","generation":"00000001","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000002/manifest.json"),
            r#"{"schema":"athanor.generated_generation.v1","generation":"00000002","snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();

        let report = cleanup_repair(RepairCleanupOptions {
            root: root.clone(),
            dry_run: false,
            keep_canonical: 0,
            keep_generated: 0,
            generated_only: true,
        })
        .unwrap();

        assert_eq!(report.removed.len(), 1);
        assert_eq!(
            report.removed[0].kind,
            RepairCleanupRemovalKind::GeneratedGeneration
        );
        assert!(
            canonical_root
                .join("snapshots/snap_jsonl_00000001")
                .is_dir()
        );
        assert!(!generated_root.join("generations/00000001").exists());
        assert!(generated_root.join("generations/00000002").is_dir());

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn regenerate_repairs_stale_current_generation_pointer() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        let generated_root = root.join(".athanor/generated");
        let store = JsonlKnowledgeStore::new(&canonical_root);
        let snapshot = store
            .begin_snapshot(
                RepoId("repo_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();
        store.commit_snapshot(snapshot.clone()).await.unwrap();
        fs::create_dir_all(generated_root.join("generations/00000001")).unwrap();
        fs::write(
            generated_root.join("current.json"),
            r#"{"schema":"athanor.generated_current.v1","generation":"00000001","snapshot":"snap_jsonl_00000000","path":"generations/00000001","manifest":"generations/00000001/manifest.json"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000001/manifest.json"),
            r#"{"schema":"athanor.generated_generation.v1","generation":"00000001","snapshot":"snap_jsonl_00000000"}"#,
        )
        .unwrap();

        let composition = crate::test_runtime::composition();
        let report = regenerate_repair(
            RepairRegenerateOptions {
                root: root.clone(),
                dry_run: false,
            },
            &composition,
        )
        .await
        .unwrap();

        assert!(report.needed);
        let generated = report.generated.as_ref().unwrap();
        assert_eq!(generated.generation, "00000002");
        assert_eq!(generated.snapshot, snapshot.0);
        let pointer: CurrentGeneration =
            serde_json::from_str(&fs::read_to_string(generated_root.join("current.json")).unwrap())
                .unwrap();
        assert_eq!(pointer.generation, "00000002");
        assert_eq!(pointer.snapshot, snapshot.0);
        assert!(
            !report
                .remaining_issues
                .iter()
                .any(|issue| issue.code == "stale_current_generation_snapshot")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn regenerate_repairs_corrupt_current_generation_manifest() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        let generated_root = root.join(".athanor/generated");
        let store = JsonlKnowledgeStore::new(&canonical_root);
        let snapshot = store
            .begin_snapshot(
                RepoId("repo_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();
        store.commit_snapshot(snapshot.clone()).await.unwrap();
        fs::create_dir_all(generated_root.join("generations/00000001")).unwrap();
        fs::write(
            generated_root.join("current.json"),
            format!(
                r#"{{"schema":"athanor.generated_current.v1","generation":"00000001","snapshot":"{}","path":"generations/00000001","manifest":"generations/00000001/manifest.json"}}"#,
                snapshot.0
            ),
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000001/manifest.json"),
            r#"{"schema":"athanor.generated_generation.v1","generation":"00000009","snapshot":"snap_jsonl_00000000"}"#,
        )
        .unwrap();

        let inspection = inspect_repair(RepairInspectOptions { root: root.clone() }).unwrap();
        assert!(
            inspection
                .issues
                .iter()
                .any(|issue| { issue.code == "current_generation_manifest_id_mismatch" })
        );
        assert!(
            inspection
                .issues
                .iter()
                .any(|issue| { issue.code == "current_generation_manifest_snapshot_mismatch" })
        );

        let composition = crate::test_runtime::composition();
        let report = regenerate_repair(
            RepairRegenerateOptions {
                root: root.clone(),
                dry_run: false,
            },
            &composition,
        )
        .await
        .unwrap();

        assert!(report.needed);
        let generated = report.generated.as_ref().unwrap();
        assert_eq!(generated.generation, "00000002");
        assert_eq!(generated.snapshot, snapshot.0);
        assert!(!report.remaining_issues.iter().any(|issue| {
            matches!(
                issue.code.as_str(),
                "current_generation_manifest_id_mismatch"
                    | "current_generation_manifest_snapshot_mismatch"
            )
        }));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inspect_reports_invalid_current_pointer_paths() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        let generated_root = root.join(".athanor/generated");
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000001")).unwrap();
        fs::create_dir_all(generated_root.join("generations/00000001")).unwrap();
        fs::write(
            canonical_root.join("latest.json"),
            r#"{"snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000001/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("current.json"),
            r#"{"schema":"athanor.generated_current.v1","generation":"00000001","snapshot":"snap_jsonl_00000001","path":"../outside","manifest":"../outside/manifest.json"}"#,
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000001/manifest.json"),
            r#"{"schema":"athanor.generated_generation.v1","generation":"00000001","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();

        let report = inspect_repair(RepairInspectOptions { root: root.clone() }).unwrap();

        assert!(
            report
                .issues
                .iter()
                .any(|issue| { issue.code == "invalid_generated_current_path" })
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| { issue.code == "invalid_generated_current_manifest_path" })
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recover_canonical_rewrites_missing_latest_pointer_to_newest_valid_snapshot() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000001")).unwrap();
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000002")).unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000001/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000002/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000002"}"#,
        )
        .unwrap();

        let report = recover_canonical_repair(RepairRecoverCanonicalOptions {
            root: root.clone(),
            dry_run: false,
        })
        .unwrap();

        assert!(report.needed);
        assert_eq!(
            report.recovered_snapshot.as_deref(),
            Some("snap_jsonl_00000002")
        );
        let latest: LatestSnapshot =
            serde_json::from_str(&fs::read_to_string(canonical_root.join("latest.json")).unwrap())
                .unwrap();
        assert_eq!(latest.snapshot, "snap_jsonl_00000002");
        assert!(
            !report
                .remaining_issues
                .iter()
                .any(|issue| issue.code == "missing_latest_pointer")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recover_canonical_dry_run_does_not_write_pointer() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        fs::create_dir_all(canonical_root.join("snapshots/snap_jsonl_00000001")).unwrap();
        fs::write(
            canonical_root.join("snapshots/snap_jsonl_00000001/manifest.json"),
            r#"{"schema":"athanor.canonical_snapshot.v1","snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();

        let report = recover_canonical_repair(RepairRecoverCanonicalOptions {
            root: root.clone(),
            dry_run: true,
        })
        .unwrap();

        assert!(report.needed);
        assert_eq!(
            report.selected_snapshot.as_deref(),
            Some("snap_jsonl_00000001")
        );
        assert!(!canonical_root.join("latest.json").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pointer_issue_classification_uses_issue_path_for_invalid_json() {
        let canonical_issue = RepairIssue {
            code: "invalid_json".to_string(),
            path: PathBuf::from(".athanor/store/canonical/jsonl/latest.json"),
            message: "bad json".to_string(),
        };
        let generated_issue = RepairIssue {
            code: "invalid_json".to_string(),
            path: PathBuf::from(".athanor/generated/current.json"),
            message: "bad json".to_string(),
        };

        assert!(is_canonical_pointer_issue(&canonical_issue));
        assert!(!is_generated_pointer_issue(&canonical_issue));
        assert!(is_generated_pointer_issue(&generated_issue));
        assert!(!is_canonical_pointer_issue(&generated_issue));
    }

    #[tokio::test]
    async fn apply_recovers_regenerates_and_cleans_artifacts() {
        let root = temp_root();
        let canonical_root = root.join(".athanor/store/canonical/jsonl");
        let generated_root = root.join(".athanor/generated");
        let store = JsonlKnowledgeStore::new(&canonical_root);
        let old_snapshot = store
            .begin_snapshot(
                RepoId("repo_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();
        store.commit_snapshot(old_snapshot.clone()).await.unwrap();
        let new_snapshot = store
            .begin_snapshot(
                RepoId("repo_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: Some(old_snapshot.clone()),
                    working_tree: true,
                },
            )
            .await
            .unwrap();
        store.commit_snapshot(new_snapshot.clone()).await.unwrap();
        fs::remove_file(canonical_root.join("latest.json")).unwrap();
        fs::create_dir_all(generated_root.join("generations/00000001")).unwrap();
        fs::write(
            generated_root.join("current.json"),
            format!(
                r#"{{"schema":"athanor.generated_current.v1","generation":"00000001","snapshot":"{}","path":"generations/00000001","manifest":"generations/00000001/manifest.json"}}"#,
                old_snapshot.0
            ),
        )
        .unwrap();
        fs::write(
            generated_root.join("generations/00000001/manifest.json"),
            format!(
                r#"{{"schema":"athanor.generated_generation.v1","generation":"00000001","snapshot":"{}"}}"#,
                old_snapshot.0
            ),
        )
        .unwrap();

        let composition = crate::test_runtime::composition();
        let report = apply_repair(
            RepairApplyOptions {
                root: root.clone(),
                dry_run: false,
                keep_canonical: 0,
                keep_generated: 0,
                generated_only: false,
            },
            &composition,
        )
        .await
        .unwrap();

        assert_eq!(
            report.canonical.recovered_snapshot.as_deref(),
            Some(new_snapshot.0.as_str())
        );
        assert_eq!(
            report
                .generated
                .generated
                .as_ref()
                .map(|generation| generation.snapshot.as_str()),
            Some(new_snapshot.0.as_str())
        );
        assert!(
            !canonical_root
                .join("snapshots")
                .join(&old_snapshot.0)
                .exists()
        );
        assert!(
            canonical_root
                .join("snapshots")
                .join(&new_snapshot.0)
                .is_dir()
        );
        assert!(!generated_root.join("generations/00000001").exists());
        assert!(report.remaining_issues.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root() -> PathBuf {
        static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let counter = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "athanor-repair-test-{}-{}-{}",
            std::process::id(),
            counter,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
