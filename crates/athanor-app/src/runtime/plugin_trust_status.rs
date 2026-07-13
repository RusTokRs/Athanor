use anyhow::{Context, Result};

use super::{
    AdapterTrustRegistry, AdapterTrustStatus, DiscoveredAdapterPlugin, TrustedAdapterPlugin,
    plugin_hash,
};

pub(super) fn is_trusted(
    registry: &AdapterTrustRegistry,
    plugin: &DiscoveredAdapterPlugin,
    record: impl Fn(&DiscoveredAdapterPlugin) -> Result<TrustedAdapterPlugin>,
) -> Result<bool> {
    let trusted = record(plugin)?;
    Ok(registry.trusted_plugins.iter().any(|entry| {
        entry.manifest_path == trusted.manifest_path
            && entry.content_hash == trusted.content_hash
            && entry.executable_hashes == trusted.executable_hashes
    }))
}

pub(super) fn status(
    registry: &AdapterTrustRegistry,
    plugin: &DiscoveredAdapterPlugin,
    record: impl Fn(&DiscoveredAdapterPlugin) -> Result<TrustedAdapterPlugin>,
) -> Result<AdapterTrustStatus> {
    let trusted_record = record(plugin)?;
    let trusted = registry.trusted_plugins.iter().any(|entry| {
        entry.manifest_path == trusted_record.manifest_path
            && entry.content_hash == trusted_record.content_hash
            && entry.executable_hashes == trusted_record.executable_hashes
    });
    Ok(AdapterTrustStatus {
        manifest_path: plugin.manifest_path.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize adapter manifest {}",
                plugin.manifest_path.display()
            )
        })?,
        name: plugin.manifest.name.clone(),
        version: plugin.manifest.version.clone(),
        has_external_process: plugin
            .manifest
            .adapters
            .iter()
            .any(|adapter| adapter.enabled && adapter.command.is_some()),
        trusted,
        content_hash: plugin_hash::manifest(&plugin.manifest_path)?,
        executable_hashes: trusted_record.executable_hashes,
    })
}
