use std::path::PathBuf;

use anyhow::{Context, Result, bail};

/// Resolves the per-user trust registry path without consulting adapter runtime state.
pub(super) fn default_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ATHANOR_ADAPTER_TRUST") {
        if path.is_empty() {
            bail!("ATHANOR_ADAPTER_TRUST must not be empty");
        }
        return absolute_path(PathBuf::from(path));
    }

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "cannot determine user home directory; set ATHANOR_ADAPTER_TRUST explicitly"
            )
        })?;
    Ok(PathBuf::from(home).join(".athanor/adapter-trust.json"))
}

fn absolute_path(path: PathBuf) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        bail!("adapter trust path must not be empty");
    }
    if path.is_absolute() {
        return Ok(path);
    }
    std::path::absolute(&path).with_context(|| {
        format!(
            "failed to resolve relative adapter trust path {} against the current directory",
            path.display()
        )
    })
}
