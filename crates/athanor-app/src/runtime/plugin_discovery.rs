use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::{AdapterPluginManifest, DiscoveredAdapterPlugin};
use crate::adapter_contract::{
    ADAPTER_MANIFEST_SCHEMA_V1, normalize_adapter_manifest_schema, validate_adapter_manifest_schema,
};

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
    let mut manifest: AdapterPluginManifest = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    normalize_adapter_manifest_schema(&mut manifest)
        .with_context(|| format!("invalid adapter plugin manifest {}", path.display()))?;
    validate(&manifest)
        .with_context(|| format!("invalid adapter plugin manifest {}", path.display()))?;
    Ok(DiscoveredAdapterPlugin {
        manifest_path: path,
        manifest,
    })
}

pub(super) fn validate(manifest: &AdapterPluginManifest) -> Result<()> {
    validate_adapter_manifest_schema(&manifest.schema)?;
    if manifest.name.trim().is_empty() {
        anyhow::bail!("adapter plugin manifest name must not be empty");
    }
    if manifest.schema != ADAPTER_MANIFEST_SCHEMA_V1
        && manifest.schema != crate::adapter_contract::ADAPTER_MANIFEST_SCHEMA_LEGACY
    {
        anyhow::bail!(
            "unsupported adapter plugin manifest schema {}; expected {}",
            manifest.schema,
            ADAPTER_MANIFEST_SCHEMA_V1
        );
    }
    Ok(())
}
