use std::path::PathBuf;

use anyhow::{Result, bail};

/// Resolves the per-user trust registry path without consulting adapter runtime state.
pub(super) fn default_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ATHANOR_ADAPTER_TRUST") {
        if path.is_empty() {
            bail!("ATHANOR_ADAPTER_TRUST must not be empty");
        }
        return Ok(PathBuf::from(path));
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
