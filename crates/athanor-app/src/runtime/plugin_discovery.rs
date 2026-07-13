use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::{ADAPTER_MANIFEST_SCHEMA, AdapterPluginManifest, DiscoveredAdapterPlugin};

/// Discovers adapter manifests from the project-local adapter and plugin directories.
pub(super) fn discover(root: impl AsRef<Path>) -> Result<Vec<DiscoveredAdapterPlugin>> {
    let root = root.as_ref();
    let mut manifest_paths = Vec::new();
    let adapters_dir = root.join(".athanor/adapters");
    let plugins_dir = root.join(".athanor/plugins");

    if adapters_dir.is_dir() {
        for entry in fs::read_dir(&adapters_dir)
            .with_context(|| format!("failed to read {}", adapters_dir.display()))?
        {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                manifest_paths.push(path);
            }
        }
    }

    if plugins_dir.is_dir() {
        for entry in fs::read_dir(&plugins_dir)
            .with_context(|| format!("failed to read {}", plugins_dir.display()))?
        {
            let path = entry?.path().join("athanor-adapter.json");
            if path.is_file() {
                manifest_paths.push(path);
            }
        }
    }

    manifest_paths.sort();
    manifest_paths.into_iter().map(read_manifest).collect()
}

pub(super) fn read_manifest(path: std::path::PathBuf) -> Result<DiscoveredAdapterPlugin> {
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: AdapterPluginManifest = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate(&manifest)
        .with_context(|| format!("invalid adapter plugin manifest {}", path.display()))?;
    Ok(DiscoveredAdapterPlugin {
        manifest_path: path,
        manifest,
    })
}

pub(super) fn validate(manifest: &AdapterPluginManifest) -> Result<()> {
    if manifest.schema != ADAPTER_MANIFEST_SCHEMA {
        anyhow::bail!(
            "unsupported adapter plugin manifest schema {}; expected {}",
            manifest.schema,
            ADAPTER_MANIFEST_SCHEMA
        );
    }
    if manifest.name.trim().is_empty() {
        anyhow::bail!("adapter plugin manifest name must not be empty");
    }
    Ok(())
}
