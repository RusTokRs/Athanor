//! Admission checks for discovered plugins that execute external processes.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use tracing::warn;

use super::process_adapter_support::{
    resolve_external_process_allowlist, resolve_manifest_program,
};
use super::{AdapterTrustRegistry, DiscoveredAdapterPlugin, TrustedAdapterExecutable};

/// Verifies external-process opt-in, the command allowlist, and the user's trust record.
///
/// Returns the trusted launch digests that adapter registration must pin to each process command.
/// Plugins without enabled external commands need no trust record and return `None`.
pub(super) fn bind_external_plugin(
    root: &Path,
    allow_external_process: bool,
    allowed_external_process_programs: &[PathBuf],
    adapter_trust_path: Option<&Path>,
    plugin: &DiscoveredAdapterPlugin,
    trust_registry: &mut Option<AdapterTrustRegistry>,
) -> Result<Option<Vec<TrustedAdapterExecutable>>> {
    let external = plugin
        .manifest
        .adapters
        .iter()
        .filter(|adapter| adapter.enabled && adapter.command.is_some())
        .collect::<Vec<_>>();
    if external.is_empty() {
        return Ok(None);
    }
    if !allow_external_process {
        bail!(
            "external process adapters are disabled; set [adapters] allow_external_process = true to enable plugin `{}`",
            plugin.manifest.name
        );
    }
    let allowlist = resolve_external_process_allowlist(root, allowed_external_process_programs)?;
    let manifest_dir = plugin.manifest_path.parent().unwrap_or(root);
    for adapter in &external {
        let command = adapter
            .command
            .as_ref()
            .expect("external adapters were already filtered to entries that declare command");
        let resolved = resolve_manifest_program(manifest_dir, &command.program)?;
        if !allowlist.contains(&resolved) {
            bail!(
                "external process adapter `{}` command {} is not in [adapters].external_process_allowlist",
                adapter.id,
                resolved.display()
            );
        }
    }
    if trust_registry.is_none() {
        let trust_path = match adapter_trust_path {
            Some(path) => path.to_path_buf(),
            None => super::default_adapter_trust_path()?,
        };
        *trust_registry = Some(super::plugin_trust_registry::load(&trust_path)?);
    }
    let registry = trust_registry
        .as_ref()
        .expect("adapter trust registry was loaded for external adapters");
    let trusted = super::trust::trusted_plugin_record(plugin)?;
    if !super::plugin_trust_status::record_matches(registry, &trusted) {
        bail!(
            "external process adapter plugin `{}` is not trusted; run `ath plugins trust {}` to trust this manifest version",
            plugin.manifest.name,
            plugin.manifest_path.display()
        );
    }
    for adapter in external {
        warn!(
            plugin = %plugin.manifest.name,
            adapter = %adapter.id,
            "external process adapter execution is enabled"
        );
    }
    Ok(Some(trusted.executable_hashes))
}
