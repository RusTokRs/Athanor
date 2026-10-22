use anyhow::Result;

use super::{
    AdapterTrustRegistry, AdapterTrustStatus, DiscoveredAdapterPlugin, TrustedAdapterPlugin,
};

pub(super) fn record_matches(
    registry: &AdapterTrustRegistry,
    trusted: &TrustedAdapterPlugin,
) -> bool {
    registry.trusted_plugins.iter().any(|entry| {
        entry.manifest_path == trusted.manifest_path
            && entry.content_hash == trusted.content_hash
            && entry.executable_hashes == trusted.executable_hashes
    })
}

pub(super) fn status(
    registry: &AdapterTrustRegistry,
    plugin: &DiscoveredAdapterPlugin,
    record: impl Fn(&DiscoveredAdapterPlugin) -> Result<TrustedAdapterPlugin>,
) -> Result<AdapterTrustStatus> {
    status_from_record(registry, plugin, record(plugin)?)
}

pub(super) fn status_from_record(
    registry: &AdapterTrustRegistry,
    plugin: &DiscoveredAdapterPlugin,
    trusted_record: TrustedAdapterPlugin,
) -> Result<AdapterTrustStatus> {
    let trusted = record_matches(registry, &trusted_record);
    Ok(AdapterTrustStatus {
        manifest_path: trusted_record.manifest_path,
        name: plugin.manifest.name.clone(),
        version: plugin.manifest.version.clone(),
        has_external_process: plugin
            .manifest
            .adapters
            .iter()
            .any(|adapter| adapter.enabled && adapter.command.is_some()),
        trusted,
        content_hash: trusted_record.content_hash,
        executable_hashes: trusted_record.executable_hashes,
    })
}
