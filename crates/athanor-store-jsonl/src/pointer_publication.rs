use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use athanor_core::{CoreError, CoreResult};
use serde::Serialize;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static INJECT_BACKUP_CLEANUP_FAILURE: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn publish_json<T: Serialize>(
    target: &Path,
    value: &T,
    staging_prefix: &str,
    subject: &str,
) -> CoreResult<()> {
    let parent = target.parent().ok_or_else(|| {
        CoreError::Adapter(format!("{subject} path {} has no parent", target.display()))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to create {subject} directory {}: {error}",
            parent.display()
        ))
    })?;

    let staging = parent.join(format!("{staging_prefix}{}", unique_suffix()));
    let content = serde_json::to_vec_pretty(value)
        .map_err(|error| CoreError::Adapter(format!("failed to serialize {subject}: {error}")))?;
    fs::write(&staging, content)
        .map_err(|error| CoreError::Adapter(format!("failed to stage {subject}: {error}")))?;

    if let Err(error) = replace_file(&staging, target, subject) {
        let _ = fs::remove_file(&staging);
        return Err(error);
    }
    Ok(())
}

pub(crate) fn replace_file(staging: &Path, target: &Path, subject: &str) -> CoreResult<()> {
    if !target.exists() {
        return fs::rename(staging, target)
            .map_err(|error| CoreError::Adapter(format!("failed to publish {subject}: {error}")));
    }

    let backup = target.with_extension(format!("json.backup-{}", unique_suffix()));
    fs::rename(target, &backup).map_err(|error| {
        CoreError::Adapter(format!("failed to stage previous {subject}: {error}"))
    })?;

    if let Err(error) = fs::rename(staging, target) {
        let restore_error = fs::rename(&backup, target).err();
        let _ = fs::remove_file(staging);
        return Err(CoreError::Adapter(match restore_error {
            Some(restore_error) => format!(
                "failed to publish {subject}: {error}; failed to restore previous value: {restore_error}"
            ),
            None => format!("failed to publish {subject}: {error}"),
        }));
    }

    cleanup_backup_after_commit(&backup, subject);
    Ok(())
}

fn cleanup_backup_after_commit(backup: &Path, subject: &str) {
    #[cfg(test)]
    if INJECT_BACKUP_CLEANUP_FAILURE.with(Cell::get) {
        eprintln!(
            "warning: JSONL {subject} was published but backup cleanup failed (injected): {}",
            backup.display()
        );
        return;
    }

    if let Err(error) = fs::remove_file(backup) {
        eprintln!(
            "warning: JSONL {subject} was published but backup cleanup failed for {}: {error}",
            backup.display()
        );
    }
}

pub(crate) fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_commit_cleanup_failure_keeps_new_pointer_published() {
        let root = test_root("cleanup");
        fs::create_dir_all(&root).unwrap();
        let target = root.join("latest.json");
        let staging = root.join(".latest.json.staging-test");
        fs::write(&target, "old").unwrap();
        fs::write(&staging, "new").unwrap();

        INJECT_BACKUP_CLEANUP_FAILURE.with(|flag| flag.set(true));
        let result = replace_file(&staging, &target, "canonical latest pointer");
        INJECT_BACKUP_CLEANUP_FAILURE.with(|flag| flag.set(false));

        result.unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert!(fs::read_dir(&root).unwrap().filter_map(Result::ok).any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("latest.json.backup-"))
        }));
        fs::remove_dir_all(root).unwrap();
    }

    fn test_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "athanor-jsonl-pointer-{label}-{}",
            unique_suffix()
        ))
    }
}
