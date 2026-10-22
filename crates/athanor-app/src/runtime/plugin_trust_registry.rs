use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use fs2::FileExt;
use tracing::warn;

use super::{AdapterTrustRegistry, TrustedAdapterPlugin};
use crate::adapter_contract::{
    ADAPTER_TRUST_REGISTRY_SCHEMA_V2, normalize_adapter_trust_registry_schema,
};

pub(super) fn load(path: &Path) -> Result<AdapterTrustRegistry> {
    let _lock = lock_trust_store(path, false)?;
    load_unlocked(path)
}

pub(super) fn update(
    path: &Path,
    mutate: impl FnOnce(&mut AdapterTrustRegistry) -> Result<()>,
) -> Result<AdapterTrustRegistry> {
    let _lock = lock_trust_store(path, true)?;
    let mut registry = load_unlocked(path)?;
    mutate(&mut registry)?;
    write_unlocked(path, &registry)?;
    Ok(registry)
}

fn load_unlocked(path: &Path) -> Result<AdapterTrustRegistry> {
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

fn write_unlocked(path: &Path, registry: &AdapterTrustRegistry) -> Result<()> {
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
    let mut staging_file = File::create(&staging).with_context(|| {
        format!(
            "failed to create staged adapter trust {}",
            staging.display()
        )
    })?;
    staging_file
        .write_all(format!("{}\n", serde_json::to_string_pretty(registry)?).as_bytes())
        .with_context(|| format!("failed to write staged adapter trust {}", staging.display()))?;
    staging_file
        .sync_all()
        .with_context(|| format!("failed to sync staged adapter trust {}", staging.display()))?;
    drop(staging_file);
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
    sync_published(path, parent);
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

fn lock_trust_store(path: &Path, exclusive: bool) -> Result<File> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("adapter trust path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create adapter trust directory {}",
            parent.display()
        )
    })?;
    let lock_path = trust_lock_path(path)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open adapter trust lock {}", lock_path.display()))?;
    if exclusive {
        file.lock_exclusive().with_context(|| {
            format!(
                "failed to lock adapter trust registry {}",
                lock_path.display()
            )
        })?;
    } else {
        file.lock_shared().with_context(|| {
            format!(
                "failed to lock adapter trust registry {}",
                lock_path.display()
            )
        })?;
    }
    Ok(file)
}

fn trust_lock_path(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("adapter trust path has no parent: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid adapter trust path: {}", path.display()))?;
    Ok(parent.join(format!(".{file_name}.lock")))
}

fn sync_published(path: &Path, parent: &Path) {
    if let Ok(published) = File::open(path)
        && let Err(error) = published.sync_all()
    {
        warn!(
            path = %path.display(),
            error = %error,
            "adapter trust registry was published but file sync failed"
        );
    }
    #[cfg(unix)]
    if let Ok(directory) = File::open(parent)
        && let Err(error) = directory.sync_all()
    {
        warn!(
            path = %parent.display(),
            error = %error,
            "adapter trust registry was published but directory sync failed"
        );
    }
}

fn sort(plugins: &mut [TrustedAdapterPlugin]) {
    plugins.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
}
