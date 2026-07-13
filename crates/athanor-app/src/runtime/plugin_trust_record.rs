use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::{DiscoveredAdapterPlugin, TrustedAdapterExecutable, TrustedAdapterPlugin, plugin_hash};

/// Builds the immutable record compared against the user trust registry.
pub(super) fn build(
    plugin: &DiscoveredAdapterPlugin,
    resolve_program: impl Fn(&Path, &str) -> Result<PathBuf>,
) -> Result<TrustedAdapterPlugin> {
    let manifest_dir = plugin.manifest_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "adapter manifest has no parent: {}",
            plugin.manifest_path.display()
        )
    })?;
    let mut executable_hashes = plugin
        .manifest
        .adapters
        .iter()
        .filter(|adapter| adapter.enabled)
        .filter_map(|adapter| adapter.command.as_ref())
        .map(|command| {
            let program = resolve_program(manifest_dir, &command.program)?;
            Ok(TrustedAdapterExecutable {
                content_hash: plugin_hash::executable(&program)?,
                content_size_bytes: plugin_hash::executable_size(&program)?,
                program,
            })
        })
        .collect::<Result<Vec<_>>>()?;
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
