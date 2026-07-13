use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::daemon::{
    DAEMON_ENDPOINT_SCHEMA, DAEMON_ENDPOINT_SCHEMA_V2, DAEMON_PROTOCOL_VERSION,
    DAEMON_PROTOCOL_VERSION_V2, DaemonEndpoint,
};

pub(super) fn read(path: &Path) -> Result<DaemonEndpoint> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let endpoint: DaemonEndpoint = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if endpoint.schema != DAEMON_ENDPOINT_SCHEMA && endpoint.schema != DAEMON_ENDPOINT_SCHEMA_V2 {
        bail!("unsupported daemon endpoint schema `{}`", endpoint.schema);
    }
    if endpoint.protocol_version != DAEMON_PROTOCOL_VERSION
        && endpoint.protocol_version != DAEMON_PROTOCOL_VERSION_V2
    {
        bail!(
            "unsupported daemon protocol version `{}`",
            endpoint.protocol_version
        );
    }
    Ok(endpoint)
}

pub(super) fn write(path: &Path, endpoint: &DaemonEndpoint) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("daemon endpoint has no parent"))?;
    let staging = parent.join(format!(".endpoint.json.tmp-{}", std::process::id()));
    let content = serde_json::to_string_pretty(endpoint)?;
    fs::write(&staging, format!("{content}\n"))
        .with_context(|| format!("failed to write {}", staging.display()))?;
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to replace {}", path.display()))?;
    }
    fs::rename(&staging, path).with_context(|| format!("failed to publish {}", path.display()))
}
