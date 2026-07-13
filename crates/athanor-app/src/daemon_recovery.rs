use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub(super) fn cleanup_known_staging_artifacts(root: &Path) -> Result<()> {
    let roots = [
        root.join(".athanor/store/canonical/jsonl"),
        root.join(".athanor/generated"),
        root.join(".athanor/generated/current"),
    ];
    for directory in roots {
        cleanup_staging_directory(&directory)?;
    }
    Ok(())
}

fn cleanup_staging_directory(directory: &Path) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", directory.display()));
        }
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with('.') && (name.contains(".tmp-") || name.contains(".backup-"))) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove stale staging {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale staging {}", path.display()))?;
        }
    }
    Ok(())
}
