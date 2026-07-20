use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::{
    AdapterTrustListOptions, AdapterTrustOptions, DiscoveredAdapterPlugin, TrustedAdapterPlugin,
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
    let mut registry = super::plugin_trust_registry::load(&options.trust_path)?;
    let trusted = trusted_plugin_record(&plugin)?;

    registry
        .trusted_plugins
        .retain(|entry| entry.manifest_path != trusted.manifest_path);
    registry.trusted_plugins.push(trusted);
    sort_trusted_plugins(&mut registry.trusted_plugins);
    super::plugin_trust_registry::write(&options.trust_path, &registry)?;

    Ok(VersionedAdapterTrustReport {
        schema: ADAPTER_TRUST_REPORT_SCHEMA_V1.to_string(),
        trust_path: options.trust_path,
        plugins: vec![super::plugin_trust_status::status(
            &registry,
            &plugin,
            trusted_plugin_record,
        )?],
    })
}

pub fn untrust_adapter_plugin(options: AdapterTrustOptions) -> Result<VersionedAdapterTrustReport> {
    let plugin = super::plugin_discovery::read_manifest(options.manifest_path)?;
    let mut registry = super::plugin_trust_registry::load(&options.trust_path)?;
    let manifest_path = plugin.manifest_path.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize adapter manifest {}",
            plugin.manifest_path.display()
        )
    })?;
    let previous_len = registry.trusted_plugins.len();
    registry
        .trusted_plugins
        .retain(|entry| entry.manifest_path != manifest_path);
    if registry.trusted_plugins.len() == previous_len {
        bail!(
            "adapter plugin manifest is not trusted: {}",
            manifest_path.display()
        );
    }
    super::plugin_trust_registry::write(&options.trust_path, &registry)?;

    Ok(VersionedAdapterTrustReport {
        schema: ADAPTER_TRUST_REPORT_SCHEMA_V1.to_string(),
        trust_path: options.trust_path,
        plugins: vec![super::plugin_trust_status::status(
            &registry,
            &plugin,
            trusted_plugin_record,
        )?],
    })
}

pub(super) fn trusted_plugin_record(
    plugin: &DiscoveredAdapterPlugin,
) -> Result<TrustedAdapterPlugin> {
    super::plugin_trust_record::build(
        plugin,
        super::process_adapter_support::resolve_manifest_program,
    )
}

fn sort_trusted_plugins(plugins: &mut [TrustedAdapterPlugin]) {
    plugins.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
}
