use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use tracing::warn;

use super::{AdapterTrustRegistry, TrustedAdapterPlugin};
use crate::adapter_contract::{
    ADAPTER_TRUST_REGISTRY_SCHEMA_V2, normalize_adapter_trust_registry_schema,
};

pub(super) fn load(path: &Path) -> Result<AdapterTrustRegistry> {
    if !path.exists() {
        return Ok(AdapterTrustRegistry {
            schema: ADAPTER_TRUST_REGISTRY_SCHEMA_V2.to_string(),
            trusted_plugins: Vec::new(),
        });
    }

    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut registry: AdapterTrustRegistry = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    normalize_adapter_trust_registry_schema(&mut registry)
        .with_context(|| format!("unsupported adapter trust registry in {}", path.display()))?;
    for entry in &registry.trusted_plugins {
        if !entry.manifest_path.is_absolute() || entry.content_hash.trim().is_empty() {
            bail!(
                "trusted adapter manifest {} has an invalid trust record",
                entry.manifest_path.display()
            );
        }
        for executable in &entry.executable_hashes {
            if !executable.program.is_absolute() || executable.content_hash.trim().is_empty() {
                bail!(
                    "trusted adapter manifest {} has invalid executable trust record",
                    entry.manifest_path.display()
                );
            }
        }
    }
    sort(&mut registry.trusted_plugins);
    Ok(registry)
}

pub(super) fn write(path: &Path, registry: &AdapterTrustRegistry) -> Result<()> {
    if registry.schema != ADAPTER_TRUST_REGISTRY_SCHEMA_V2 {
        bail!(
            "refusing to write adapter trust registry with schema `{}`; expected `{}`",
            registry.schema,
            ADAPTER_TRUST_REGISTRY_SCHEMA_V2
        );
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("adapter trust path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create adapter trust directory {}",
            parent.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid adapter trust path: {}", path.display()))?;
    let staging = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let backup = parent.join(format!(".{file_name}.backup-{}", std::process::id()));
    let _ = fs::remove_file(&staging);
    let _ = fs::remove_file(&backup);
    fs::write(
        &staging,
        format!("{}\n", serde_json::to_string_pretty(registry)?),
    )
    .with_context(|| format!("failed to write staged adapter trust {}", staging.display()))?;
    if path.exists() {
        fs::rename(path, &backup).with_context(|| {
            format!("failed to stage previous adapter trust {}", path.display())
        })?;
    }
    if let Err(error) = fs::rename(&staging, path) {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&staging);
        return Err(error)
            .with_context(|| format!("failed to publish adapter trust {}", path.display()));
    }
    if backup.exists()
        && let Err(error) = fs::remove_file(&backup)
    {
        warn!(
            backup = %backup.display(),
            error = %error,
            "adapter trust registry was published but backup cleanup failed"
        );
    }
    Ok(())
}

fn sort(plugins: &mut [TrustedAdapterPlugin]) {
    plugins.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
}
