use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::{
    AdapterTrustListOptions, AdapterTrustOptions, AdapterTrustStatus, DiscoveredAdapterPlugin,
    TrustedAdapterPlugin,
};
use crate::adapter_contract::{ADAPTER_TRUST_REPORT_SCHEMA_V1, VersionedAdapterTrustReport};

pub fn discover_adapter_plugins(root: impl AsRef<Path>) -> Result<Vec<DiscoveredAdapterPlugin>> {
    super::plugin_discovery::discover(root)
}

pub fn default_adapter_trust_path() -> Result<PathBuf> {
    super::plugin_trust_path::default_path()
}

pub fn list_adapter_plugin_trust(
    options: AdapterTrustListOptions,
) -> Result<VersionedAdapterTrustReport> {
    let root = options.root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize project root {}",
            options.root.display()
        )
    })?;
    let registry = super::plugin_trust_registry::load(&options.trust_path)?;
    let mut plugins = discover_adapter_plugins(root)?
        .into_iter()
        .map(|plugin| super::plugin_trust_status::status(&registry, &plugin, trusted_plugin_record))
        .collect::<Result<Vec<_>>>()?;
    plugins.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));

    Ok(VersionedAdapterTrustReport {
        schema: ADAPTER_TRUST_REPORT_SCHEMA_V1.to_string(),
        trust_path: options.trust_path,
        plugins,
    })
}

pub fn trust_adapter_plugin(options: AdapterTrustOptions) -> Result<VersionedAdapterTrustReport> {
    let plugin = super::plugin_discovery::read_manifest(options.manifest_path)?;
    let trusted = trusted_plugin_record(&plugin)?;
    let registry = super::plugin_trust_registry::update(&options.trust_path, |registry| {
        registry
            .trusted_plugins
            .retain(|entry| entry.manifest_path != trusted.manifest_path);
        registry.trusted_plugins.push(trusted.clone());
        registry
            .trusted_plugins
            .sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
        Ok(())
    })?;

    Ok(VersionedAdapterTrustReport {
        schema: ADAPTER_TRUST_REPORT_SCHEMA_V1.to_string(),
        trust_path: options.trust_path,
        plugins: vec![super::plugin_trust_status::status_from_record(
            &registry, &plugin, trusted,
        )?],
    })
}

pub fn untrust_adapter_plugin(options: AdapterTrustOptions) -> Result<VersionedAdapterTrustReport> {
    let requested = resolve_untrust_path(&options.manifest_path)?;
    let registry = super::plugin_trust_registry::update(&options.trust_path, |registry| {
        let matches = registry
            .trusted_plugins
            .iter()
            .enumerate()
            .filter(|(_, entry)| trust_path_matches(&entry.manifest_path, &requested))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => bail!(
                "adapter plugin manifest is not trusted: {}",
                requested.display()
            ),
            [index] => {
                registry.trusted_plugins.remove(*index);
            }
            _ => bail!(
                "adapter plugin untrust path {} matches multiple trust records; pass the canonical manifest path",
                requested.display()
            ),
        }
        Ok(())
    })?;

    let plugins = match super::plugin_discovery::read_manifest(options.manifest_path.clone()) {
        Ok(plugin) => vec![super::plugin_trust_status::status(
            &registry,
            &plugin,
            trusted_plugin_record,
        )?],
        Err(_) => vec![AdapterTrustStatus {
            manifest_path: requested,
            name: options.manifest_path.display().to_string(),
            version: None,
            has_external_process: false,
            trusted: false,
            content_hash: "unavailable".to_string(),
            executable_hashes: Vec::new(),
        }],
    };

    Ok(VersionedAdapterTrustReport {
        schema: ADAPTER_TRUST_REPORT_SCHEMA_V1.to_string(),
        trust_path: options.trust_path,
        plugins,
    })
}

pub(super) fn trusted_plugin_record(
    plugin: &DiscoveredAdapterPlugin,
) -> Result<TrustedAdapterPlugin> {
    super::plugin_trust_record::build(plugin)
}

fn resolve_untrust_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return path.canonicalize().with_context(|| {
            format!("failed to canonicalize adapter manifest {}", path.display())
        });
    }
    std::path::absolute(path).with_context(|| {
        format!(
            "failed to resolve adapter untrust path {} against the current directory",
            path.display()
        )
    })
}

fn trust_path_matches(stored: &Path, requested: &Path) -> bool {
    if stored == requested || stored.ends_with(requested) {
        return true;
    }
    let stored_tail = normal_components(stored);
    let requested_tail = normal_components(requested);
    !requested_tail.is_empty() && stored_tail.ends_with(&requested_tail)
}

fn normal_components(path: &Path) -> Vec<&OsStr> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part),
            _ => None,
        })
        .collect()
}
