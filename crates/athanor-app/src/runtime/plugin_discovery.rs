use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::model::AdapterPluginKind;
use super::{AdapterPluginManifest, DiscoveredAdapterPlugin};
use crate::adapter_contract::{
    ADAPTER_MANIFEST_SCHEMA_V1, normalize_adapter_manifest_schema, validate_adapter_manifest_schema,
};

/// Discovers adapter manifests from the project-local adapter and plugin directories.
pub(super) fn discover(root: impl AsRef<Path>) -> Result<Vec<DiscoveredAdapterPlugin>> {
    let root = root.as_ref();
    let mut manifest_paths = Vec::new();
    collect_adapter_dir_manifests(&root.join(".athanor/adapters"), &mut manifest_paths)?;
    collect_plugin_dir_manifests(&root.join(".athanor/plugins"), &mut manifest_paths)?;
    manifest_paths.sort();
    manifest_paths.into_iter().map(read_manifest).collect()
}

fn collect_adapter_dir_manifests(
    adapters_dir: &Path,
    manifest_paths: &mut Vec<PathBuf>,
) -> Result<()> {
    if !is_regular_dir(adapters_dir, "adapter manifest directory")? {
        return Ok(());
    }
    for entry in fs::read_dir(adapters_dir)
        .with_context(|| format!("failed to read {}", adapters_dir.display()))?
    {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            && is_regular_file(&path, "adapter manifest")?
        {
            manifest_paths.push(path);
        }
    }
    Ok(())
}

fn collect_plugin_dir_manifests(
    plugins_dir: &Path,
    manifest_paths: &mut Vec<PathBuf>,
) -> Result<()> {
    if !is_regular_dir(plugins_dir, "adapter plugin directory")? {
        return Ok(());
    }
    for entry in fs::read_dir(plugins_dir)
        .with_context(|| format!("failed to read {}", plugins_dir.display()))?
    {
        let plugin_dir = entry?.path();
        if let Some(path) = plugin_bundle_manifest(&plugin_dir)? {
            manifest_paths.push(path);
        }
    }
    Ok(())
}

fn plugin_bundle_manifest(plugin_dir: &Path) -> Result<Option<PathBuf>> {
    if !is_regular_dir(plugin_dir, "adapter plugin directory")? {
        return Ok(None);
    }
    let mut found = None;
    for entry in fs::read_dir(plugin_dir)
        .with_context(|| format!("failed to read {}", plugin_dir.display()))?
    {
        let entry = entry?;
        if !entry
            .file_name()
            .eq_ignore_ascii_case("athanor-adapter.json")
        {
            continue;
        }
        let path = entry.path();
        if !is_regular_file(&path, "adapter manifest")? {
            continue;
        }
        if found.replace(path).is_some() {
            bail!(
                "adapter plugin directory {} contains multiple athanor-adapter.json manifests",
                plugin_dir.display()
            );
        }
    }
    Ok(found)
}

pub(super) fn read_manifest(path: PathBuf) -> Result<DiscoveredAdapterPlugin> {
    if !is_regular_file(&path, "adapter manifest")? {
        bail!("adapter manifest must be a regular file: {}", path.display());
    }
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
        bail!("adapter plugin manifest name must not be empty");
    }
    if manifest.schema != ADAPTER_MANIFEST_SCHEMA_V1 {
        bail!(
            "unsupported adapter plugin manifest schema {}; expected {}",
            manifest.schema,
            ADAPTER_MANIFEST_SCHEMA_V1
        );
    }
    let mut ids = BTreeSet::new();
    for adapter in &manifest.adapters {
        let id = adapter.id.trim();
        if id.is_empty() {
            bail!("adapter plugin `{}` has an empty adapter id", manifest.name);
        }
        if id != adapter.id {
            bail!(
                "adapter plugin `{}` adapter id `{}` must not include surrounding whitespace",
                manifest.name,
                adapter.id
            );
        }
        if !ids.insert(adapter.id.as_str()) {
            bail!(
                "adapter plugin `{}` declares duplicate adapter id {}",
                manifest.name,
                adapter.id
            );
        }
        if adapter.kind != AdapterPluginKind::Extractor && !adapter.supports_extensions.is_empty() {
            bail!(
                "adapter {} in plugin `{}` may not set supports_extensions unless kind is extractor",
                adapter.id,
                manifest.name
            );
        }
        if adapter.kind == AdapterPluginKind::Extractor
            && adapter.command.is_some()
            && adapter.supports_extensions.is_empty()
        {
            bail!(
                "external extractor {} in plugin `{}` must declare supports_extensions (use \"*\" to match all files)",
                adapter.id,
                manifest.name
            );
        }
    }
    Ok(())
}

fn is_regular_dir(path: &Path, label: &str) -> Result<bool> {
    let Some(metadata) = symlink_metadata_if_exists(path, label)? else {
        return Ok(false);
    };
    if metadata.file_type().is_symlink() {
        bail!("{label} must not be a symlink: {}", path.display());
    }
    Ok(metadata.is_dir())
}

fn is_regular_file(path: &Path, label: &str) -> Result<bool> {
    let Some(metadata) = symlink_metadata_if_exists(path, label)? else {
        return Ok(false);
    };
    if metadata.file_type().is_symlink() {
        bail!("{label} must not be a symlink: {}", path.display());
    }
    Ok(metadata.is_file())
}

fn symlink_metadata_if_exists(path: &Path, label: &str) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| {
            format!("failed to inspect {label} {}", path.display())
        }),
    }
}
