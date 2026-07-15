use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_domain::{GenerationId, SnapshotId};
use serde::Deserialize;
use serde_json::Value;

mod legacy {
    include!("repair.rs");
}

pub use legacy::{
    CanonicalRepairState, GeneratedRepairState, RepairApplyOptions, RepairApplyReport,
    RepairCleanupOptions, RepairCleanupRemoval, RepairCleanupRemovalKind, RepairCleanupReport,
    RepairCleanupRetained, RepairInspectOptions, RepairInspectReport, RepairIssue,
    RepairRecoverCanonicalOptions, RepairRecoverCanonicalReport, RepairRegenerateOptions,
    RepairRegenerateReport, RepairStatus, recover_canonical_repair, regenerate_repair,
};

const POINTER_PATH: &str = ".athanor/state/index-current.json";
const JOURNAL_PATH: &str = ".athanor/state/index-current-publication.json";
const JOURNAL_SCHEMA: &str = "athanor.index_current_publication.v1";
const GENERATIONS_PATH: &str = ".athanor/generated/index-generations";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PointerDocument {
    schema: String,
    generation: String,
    snapshot: String,
    read_model: String,
    index_state: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalDocument {
    schema: String,
    generation: String,
    snapshot: String,
}

/// Extends the established canonical/generated inspection with index-current validation.
pub fn inspect_repair(options: RepairInspectOptions) -> Result<RepairInspectReport> {
    let mut report = legacy::inspect_repair(options)?;
    inspect_index_current(
        &report.root,
        report.canonical.latest_snapshot.as_deref(),
        &mut report.issues,
    )?;
    report.schema = "athanor.repair_inspect.v2".to_string();
    report.status = if report.issues.is_empty() {
        RepairStatus::Clean
    } else {
        RepairStatus::NeedsRepair
    };
    Ok(report)
}

/// Legacy cleanup remains authoritative for canonical and `ath generate` artifacts. Immutable index
/// generations are report-only until a dedicated retention flag ships. Unsafe pointer or journal
/// state blocks cleanup so a referenced snapshot cannot be removed accidentally.
pub fn cleanup_repair(options: RepairCleanupOptions) -> Result<RepairCleanupReport> {
    let preflight = inspect_repair(RepairInspectOptions {
        root: options.root.clone(),
    })?;
    if let Some(blocker) = preflight.issues.iter().find(|issue| cleanup_blocker(issue)) {
        bail!(
            "refusing repair cleanup while index publication state is unsafe: {} ({})",
            blocker.message,
            blocker.path.display()
        );
    }

    let mut report = legacy::cleanup_repair(options)?;
    let inspection = inspect_repair(RepairInspectOptions {
        root: report.root.clone(),
    })?;
    report.schema = "athanor.repair_cleanup.v2".to_string();
    report.remaining_issues = inspection.issues.clone();
    report.inspection = inspection;
    Ok(report)
}

pub async fn apply_repair(options: RepairApplyOptions) -> Result<RepairApplyReport> {
    let canonical = recover_canonical_repair(RepairRecoverCanonicalOptions {
        root: options.root.clone(),
        dry_run: options.dry_run,
    })?;
    let root = canonical.root.clone();
    let generated = regenerate_repair(RepairRegenerateOptions {
        root: root.clone(),
        dry_run: options.dry_run,
    })
    .await?;
    let cleanup = cleanup_repair(RepairCleanupOptions {
        root: root.clone(),
        dry_run: options.dry_run,
        keep_canonical: options.keep_canonical,
        keep_generated: options.keep_generated,
        generated_only: options.generated_only,
    })?;

    Ok(RepairApplyReport {
        schema: "athanor.repair_apply.v2".to_string(),
        root,
        dry_run: options.dry_run,
        canonical,
        generated,
        remaining_issues: cleanup.remaining_issues.clone(),
        cleanup,
    })
}

fn inspect_index_current(
    root: &Path,
    latest_snapshot: Option<&str>,
    issues: &mut Vec<RepairIssue>,
) -> Result<()> {
    let mut protected = BTreeSet::new();
    let pointer_path = root.join(POINTER_PATH);
    if let Some(pointer) =
        read_document::<PointerDocument>(&pointer_path, "invalid_index_current_json", issues)?
    {
        protected.insert(pointer.generation.clone());
        inspect_pointer(root, &pointer_path, &pointer, latest_snapshot, issues);
    }

    let journal_path = root.join(JOURNAL_PATH);
    if let Some(journal) = read_document::<JournalDocument>(
        &journal_path,
        "invalid_index_current_publication_json",
        issues,
    )? {
        protected.insert(journal.generation.clone());
        inspect_journal(&journal_path, &journal, issues);
    }

    let read_generations = list_directories(&root.join(GENERATIONS_PATH))?;
    let state_generations = list_state_generations(root)?;
    for generation in read_generations.union(&state_generations) {
        let read_exists = read_generations.contains(generation);
        let state_exists = state_generations.contains(generation);
        if !read_exists || !state_exists {
            issues.push(issue(
                "incomplete_index_generation",
                generation_path(root, generation),
                format!(
                    "index generation {generation} is incomplete: read_model={read_exists}, index_state={state_exists}"
                ),
            ));
        }
        if !protected.contains(generation) {
            issues.push(issue(
                "orphan_index_generation",
                generation_path(root, generation),
                format!(
                    "immutable index generation {generation} is unpointed; it is reported but retained until explicit index retention is enabled"
                ),
            ));
        }
    }
    Ok(())
}

fn inspect_pointer(
    root: &Path,
    path: &Path,
    pointer: &PointerDocument,
    latest_snapshot: Option<&str>,
    issues: &mut Vec<RepairIssue>,
) {
    if pointer.schema != crate::index_current::INDEX_CURRENT_SCHEMA {
        issues.push(issue(
            "invalid_index_current_schema",
            path.to_path_buf(),
            format!("unsupported index current schema {}", pointer.schema),
        ));
    }
    let expected = GenerationId::for_snapshot(&SnapshotId(pointer.snapshot.clone()));
    if pointer.generation != expected.as_str() {
        issues.push(issue(
            "index_current_generation_mismatch",
            path.to_path_buf(),
            format!(
                "index current generation {} does not match snapshot {}",
                pointer.generation, pointer.snapshot
            ),
        ));
    }
    if latest_snapshot.is_some_and(|latest| latest != pointer.snapshot) {
        issues.push(issue(
            "stale_index_current_snapshot",
            path.to_path_buf(),
            format!(
                "index current points to snapshot {}, latest canonical snapshot is {}",
                pointer.snapshot,
                latest_snapshot.unwrap_or("unknown")
            ),
        ));
    }

    let expected_read = format!(
        ".athanor/generated/index-generations/{}/jsonl",
        pointer.generation
    );
    let expected_state = format!(".athanor/state/index-state-{}.json", pointer.generation);
    if pointer.read_model != expected_read {
        issues.push(issue(
            "invalid_index_current_read_model_path",
            path.to_path_buf(),
            format!("index current read-model path must be {expected_read}"),
        ));
    }
    if pointer.index_state != expected_state {
        issues.push(issue(
            "invalid_index_current_state_path",
            path.to_path_buf(),
            format!("index current state path must be {expected_state}"),
        ));
    }

    inspect_identity(
        &root.join(&expected_read).join("manifest.json"),
        crate::read_model::JSONL_MANIFEST_SCHEMA,
        &pointer.snapshot,
        &pointer.generation,
        "index_current_manifest",
        issues,
    );
    inspect_identity(
        &root.join(&expected_state),
        crate::index_state::INDEX_STATE_SCHEMA,
        &pointer.snapshot,
        &pointer.generation,
        "index_current_state",
        issues,
    );
}

fn inspect_journal(path: &Path, journal: &JournalDocument, issues: &mut Vec<RepairIssue>) {
    if journal.schema != JOURNAL_SCHEMA {
        issues.push(issue(
            "invalid_index_current_publication_schema",
            path.to_path_buf(),
            format!(
                "unsupported index current publication schema {}",
                journal.schema
            ),
        ));
    }
    let expected = GenerationId::for_snapshot(&SnapshotId(journal.snapshot.clone()));
    if journal.generation != expected.as_str() {
        issues.push(issue(
            "index_current_publication_generation_mismatch",
            path.to_path_buf(),
            format!(
                "pending generation {} does not match snapshot {}",
                journal.generation, journal.snapshot
            ),
        ));
    }
    issues.push(issue(
        "pending_index_current_publication",
        path.to_path_buf(),
        "index current publication recovery is pending; run indexing recovery before cleanup"
            .to_string(),
    ));
}

fn inspect_identity(
    path: &Path,
    expected_schema: &str,
    expected_snapshot: &str,
    expected_generation: &str,
    prefix: &str,
    issues: &mut Vec<RepairIssue>,
) {
    if !path.is_file() {
        issues.push(issue(
            &format!("missing_{prefix}"),
            path.to_path_buf(),
            format!("selected index artifact {} is missing", path.display()),
        ));
        return;
    }
    let value: Value = match fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
    {
        Some(value) => value,
        None => {
            issues.push(issue(
                &format!("invalid_{prefix}_json"),
                path.to_path_buf(),
                "selected index artifact is not valid JSON".to_string(),
            ));
            return;
        }
    };
    let matches = value.get("schema").and_then(Value::as_str) == Some(expected_schema)
        && value.get("snapshot").and_then(Value::as_str) == Some(expected_snapshot)
        && value.get("generation").and_then(Value::as_str) == Some(expected_generation);
    if !matches {
        issues.push(issue(
            &format!("{prefix}_identity_mismatch"),
            path.to_path_buf(),
            format!(
                "selected index artifact must identify schema={expected_schema}, snapshot={expected_snapshot}, generation={expected_generation}"
            ),
        ));
    }
}

fn read_document<T: serde::de::DeserializeOwned>(
    path: &Path,
    code: &str,
    issues: &mut Vec<RepairIssue>,
) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read repair artifact {}", path.display()))?;
    match serde_json::from_str(&content) {
        Ok(value) => Ok(Some(value)),
        Err(error) => {
            issues.push(issue(code, path.to_path_buf(), error.to_string()));
            Ok(None)
        }
    }
}

fn list_directories(path: &Path) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(names),
        Err(error) => return Err(error).context(format!("failed to inspect {}", path.display())),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            names.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(names)
}

fn list_state_generations(root: &Path) -> Result<BTreeSet<String>> {
    let state = root.join(".athanor/state");
    let mut generations = BTreeSet::new();
    let entries = match fs::read_dir(&state) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(generations),
        Err(error) => return Err(error).context(format!("failed to inspect {}", state.display())),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(generation) = name
            .strip_prefix("index-state-")
            .and_then(|name| name.strip_suffix(".json"))
            && generation.starts_with("gen_")
        {
            generations.insert(generation.to_string());
        }
    }
    Ok(generations)
}

fn generation_path(root: &Path, generation: &str) -> PathBuf {
    root.join(GENERATIONS_PATH).join(generation)
}

fn issue(code: &str, path: PathBuf, message: String) -> RepairIssue {
    RepairIssue {
        code: code.to_string(),
        path,
        message,
    }
}

fn cleanup_blocker(issue: &RepairIssue) -> bool {
    issue.code == "pending_index_current_publication"
        || issue.code == "incomplete_index_generation"
        || issue.code == "stale_index_current_snapshot"
        || issue.code.starts_with("invalid_index_current")
        || issue.code.starts_with("missing_index_current")
        || issue.code.starts_with("index_current_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_pointer_generation_is_clean() {
        let root = test_root("valid");
        write_generation(&root, "snap_test", true);
        let report = inspect_repair(RepairInspectOptions { root: root.clone() }).unwrap();
        assert_eq!(report.schema, "athanor.repair_inspect.v2");
        assert_eq!(report.status, RepairStatus::Clean);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn orphan_generation_is_reported_and_retained() {
        let root = test_root("orphan");
        write_generation(&root, "snap_orphan", false);
        let report = cleanup_repair(RepairCleanupOptions {
            root: root.clone(),
            dry_run: false,
            keep_canonical: 0,
            keep_generated: 0,
            generated_only: false,
        })
        .unwrap();
        assert!(
            report
                .remaining_issues
                .iter()
                .any(|issue| issue.code == "orphan_index_generation")
        );
        assert!(generation_path(&root, "gen_snap_orphan").is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pending_publication_blocks_cleanup() {
        let root = test_root("pending");
        fs::create_dir_all(root.join(".athanor/state")).unwrap();
        fs::write(
            root.join(JOURNAL_PATH),
            r#"{"schema":"athanor.index_current_publication.v1","snapshot":"snap_pending","generation":"gen_snap_pending"}"#,
        )
        .unwrap();
        let error = cleanup_repair(RepairCleanupOptions {
            root: root.clone(),
            dry_run: false,
            keep_canonical: 0,
            keep_generated: 0,
            generated_only: false,
        })
        .expect_err("pending publication must block cleanup");
        assert!(error.to_string().contains("refusing repair cleanup"));
        fs::remove_dir_all(root).unwrap();
    }

    fn write_generation(root: &Path, snapshot: &str, pointer: bool) {
        let generation = format!("gen_{snapshot}");
        let read_model = generation_path(root, &generation).join("jsonl");
        fs::create_dir_all(&read_model).unwrap();
        fs::create_dir_all(root.join(".athanor/state")).unwrap();
        fs::write(
            read_model.join("manifest.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": crate::read_model::JSONL_MANIFEST_SCHEMA,
                "snapshot": snapshot,
                "generation": generation
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(".athanor/state")
                .join(format!("index-state-gen_{snapshot}.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": crate::index_state::INDEX_STATE_SCHEMA,
                "snapshot": snapshot,
                "generation": format!("gen_{snapshot}"),
                "files": {}
            }))
            .unwrap(),
        )
        .unwrap();
        if pointer {
            fs::write(
                root.join(POINTER_PATH),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "schema": crate::index_current::INDEX_CURRENT_SCHEMA,
                    "generation": format!("gen_{snapshot}"),
                    "snapshot": snapshot,
                    "read_model": format!(
                        ".athanor/generated/index-generations/gen_{snapshot}/jsonl"
                    ),
                    "index_state": format!(".athanor/state/index-state-gen_{snapshot}.json")
                }))
                .unwrap(),
            )
            .unwrap();
        }
    }

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("athanor-repair-index-{label}-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
