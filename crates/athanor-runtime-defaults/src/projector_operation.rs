use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};

static PUBLICATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn publish_projector_output_cancellable(
    target: &Path,
    output_kind: &str,
    is_cancelled: &dyn Fn() -> bool,
    build: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    ensure_not_cancelled(is_cancelled)?;
    let parent = target.parent().ok_or_else(|| {
        anyhow::anyhow!("{output_kind} target has no parent: {}", target.display())
    })?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create {output_kind} output parent {}", parent.display()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid {output_kind} target: {}", target.display()))?;
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        PUBLICATION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let staging = parent.join(format!(".{name}.operation-{suffix}"));
    let backup = parent.join(format!(".{name}.operation-backup-{suffix}"));
    remove_path_if_exists(&staging)?;
    remove_path_if_exists(&backup)?;

    if let Err(error) = build(&staging) {
        let _ = remove_path_if_exists(&staging);
        return Err(error);
    }
    if let Err(error) = ensure_not_cancelled(is_cancelled) {
        let _ = remove_path_if_exists(&staging);
        return Err(error);
    }

    let had_existing = target.exists();
    if had_existing {
        fs::rename(target, &backup).with_context(|| {
            format!(
                "stage previous {output_kind} output {}",
                target.display()
            )
        })?;
    }
    if let Err(error) = fs::rename(&staging, target) {
        if had_existing {
            let _ = fs::rename(&backup, target);
        }
        let _ = remove_path_if_exists(&staging);
        return Err(error).with_context(|| {
            format!("publish {output_kind} output {}", target.display())
        });
    }
    if had_existing {
        let _ = remove_path_if_exists(&backup);
    }
    Ok(())
}

fn ensure_not_cancelled(is_cancelled: &dyn Fn() -> bool) -> Result<()> {
    if is_cancelled() {
        bail!("operation cancelled");
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("remove output directory {}", path.display()))?;
    } else if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("remove output file {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn late_cancellation_preserves_previous_directory() {
        let root = test_root("late-cancel");
        let target = root.join("report");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("previous.txt"), "previous").unwrap();
        let cancelled = Cell::new(false);

        let error = publish_projector_output_cancellable(
            &target,
            "test report",
            &|| cancelled.get(),
            |staging| {
                fs::create_dir_all(staging)?;
                for page in 0..4 {
                    fs::write(staging.join(format!("page-{page}.txt")), "page")?;
                    if page == 2 {
                        cancelled.set(true);
                    }
                }
                Ok(())
            },
        )
        .expect_err("late cancellation must reject staged output");

        assert!(error.to_string().contains("operation cancelled"));
        assert_eq!(
            fs::read_to_string(target.join("previous.txt")).unwrap(),
            "previous"
        );
        assert!(!target.join("page-0.txt").exists());
        assert_eq!(
            fs::read_dir(&root)
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_name().to_string_lossy().contains("operation-"))
                .count(),
            0
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_publication_replaces_previous_directory() {
        let root = test_root("replace");
        let target = root.join("report");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("stale.txt"), "stale").unwrap();

        publish_projector_output_cancellable(&target, "test report", &|| false, |staging| {
            fs::create_dir_all(staging)?;
            fs::write(staging.join("current.txt"), "current")?;
            Ok(())
        })
        .unwrap();

        assert!(!target.join("stale.txt").exists());
        assert_eq!(
            fs::read_to_string(target.join("current.txt")).unwrap(),
            "current"
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "athanor-runtime-projector-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
