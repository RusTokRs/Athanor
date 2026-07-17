use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterPluginManifest {
    pub schema: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub adapters: Vec<AdapterPluginEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterPluginEntry {
    pub id: String,
    pub kind: AdapterPluginKind,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub command: Option<AdapterProcessCommand>,
    #[serde(default)]
    pub supports_extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterProcessCommand {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterPluginKind {
    Source,
    Extractor,
    Linker,
    Checker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredAdapterPlugin {
    pub manifest_path: PathBuf,
    pub manifest: AdapterPluginManifest,
}

#[derive(Debug, Clone)]
pub struct AdapterTrustOptions {
    pub trust_path: PathBuf,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AdapterTrustListOptions {
    pub root: PathBuf,
    pub trust_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrustedAdapterPlugin {
    pub manifest_path: PathBuf,
    pub content_hash: String,
    #[serde(default)]
    pub executable_hashes: Vec<TrustedAdapterExecutable>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrustedAdapterExecutable {
    pub program: PathBuf,
    pub content_hash: String,
    pub content_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdapterTrustRegistry {
    pub schema: String,
    #[serde(default)]
    pub trusted_plugins: Vec<TrustedAdapterPlugin>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AdapterTrustStatus {
    pub manifest_path: PathBuf,
    pub name: String,
    pub version: Option<String>,
    pub has_external_process: bool,
    pub trusted: bool,
    pub content_hash: String,
    pub executable_hashes: Vec<TrustedAdapterExecutable>,
}

fn enabled_by_default() -> bool {
    true
}
