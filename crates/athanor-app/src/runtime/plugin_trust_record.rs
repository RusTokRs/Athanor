use anyhow::{Context, Result};

use super::process_adapter_support::command_payload_files;
use super::{DiscoveredAdapterPlugin, TrustedAdapterExecutable, TrustedAdapterPlugin, plugin_hash};

/// Builds the immutable record compared against the user trust registry.
pub(super) fn build(plugin: &DiscoveredAdapterPlugin) -> Result<TrustedAdapterPlugin> {
    let manifest_dir = plugin.manifest_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "adapter manifest has no parent: {}",
            plugin.manifest_path.display()
        )
    })?;
    let mut executable_hashes = Vec::new();
    for adapter in plugin
        .manifest
        .adapters
        .iter()
        .filter(|adapter| adapter.enabled)
    {
        let Some(command) = adapter.command.as_ref() else {
            continue;
        };
        for program in command_payload_files(manifest_dir, command)? {
            let (content_hash, content_size_bytes) =
                plugin_hash::digest(&program, "adapter executable")?;
            executable_hashes.push(TrustedAdapterExecutable {
                content_hash,
                content_size_bytes,
                program,
            });
        }
    }
    executable_hashes.sort_by(|left, right| left.program.cmp(&right.program));
    executable_hashes.dedup_by(|left, right| left.program == right.program);
    Ok(TrustedAdapterPlugin {
        manifest_path: plugin.manifest_path.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize adapter manifest {}",
                plugin.manifest_path.display()
            )
        })?,
        content_hash: plugin_hash::manifest(&plugin.manifest_path)?,
        executable_hashes,
    })
}
