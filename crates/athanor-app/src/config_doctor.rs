use std::path::Path;

use anyhow::Result;

use crate::config::{
    CONFIG_DOCTOR_SCHEMA, ConfigDoctorCheck, ConfigDoctorReport, ConfigReportOptions,
    ExternalProcessSandboxProfile, ProjectConfig, StorageMode, load_config,
};
use crate::discover_adapter_plugins;

pub fn doctor_project_config(options: ConfigReportOptions) -> Result<ConfigDoctorReport> {
    doctor_project_config_with_store_support(options, false)
}

pub fn doctor_project_config_with_store_support(
    options: ConfigReportOptions,
    surreal_storage_available: bool,
) -> Result<ConfigDoctorReport> {
    let config = load_config(&options.root)?;
    let (storage_status, storage_detail) = match config.storage.mode {
        StorageMode::Jsonl => (
            "available",
            "JSONL storage is compiled into this build".to_string(),
        ),
        StorageMode::SurrealEmbedded | StorageMode::SurrealMemory if surreal_storage_available => (
            "available",
            "SurrealDB storage is compiled into this build".to_string(),
        ),
        StorageMode::SurrealEmbedded | StorageMode::SurrealMemory => (
            "requires-feature",
            "SurrealDB storage modes require the ath store-surreal feature".to_string(),
        ),
    };
    let external_process_sandbox = match config.adapters.external_process_sandbox {
        ExternalProcessSandboxProfile::Disabled => "disabled",
        ExternalProcessSandboxProfile::CleanEnvironment => "clean_environment",
    };
    let (adapter_status, adapter_detail) = doctor_external_process_adapters(&options.root, &config);

    Ok(ConfigDoctorReport {
        schema: CONFIG_DOCTOR_SCHEMA,
        root: options.root,
        config,
        checks: vec![
            ConfigDoctorCheck {
                name: "storage_backend",
                status: storage_status.to_string(),
                detail: storage_detail,
            },
            ConfigDoctorCheck {
                name: "external_process_adapters",
                status: adapter_status,
                detail: adapter_detail,
            },
            ConfigDoctorCheck {
                name: "external_process_sandbox",
                status: external_process_sandbox.to_string(),
                detail: "clean_environment clears inherited environment variables only; it is not OS-level filesystem, network, or CPU isolation".to_string(),
            },
        ],
    })
}

fn doctor_external_process_adapters(root: &Path, config: &ProjectConfig) -> (String, String) {
    let plugins = match discover_adapter_plugins(root) {
        Ok(plugins) => plugins,
        Err(error) => {
            return (
                "error".to_string(),
                format!("failed to discover adapter plugins: {error}"),
            );
        }
    };
    let external = plugins
        .iter()
        .filter(|plugin| {
            plugin
                .manifest
                .adapters
                .iter()
                .any(|adapter| adapter.enabled && adapter.command.is_some())
        })
        .count();
    if external == 0 {
        let disabled = if config.adapters.disabled.is_empty() {
            "none".to_string()
        } else {
            config.adapters.disabled.join(", ")
        };
        return (
            if config.adapters.allow_external_process {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
            format!(
                "0 external process plugins discovered; disabled built-in adapters: {disabled}"
            ),
        );
    }
    if !config.adapters.allow_external_process {
        return (
            "blocked".to_string(),
            format!(
                "{external} external process plugin(s) present but [adapters].allow_external_process is false"
            ),
        );
    }
    if config.adapters.external_process_allowlist.is_empty() {
        return (
            "blocked".to_string(),
            format!(
                "{external} external process plugin(s) present but [adapters].external_process_allowlist is empty"
            ),
        );
    }
    (
        "enabled".to_string(),
        format!("{external} external process plugin(s) present; execution still requires trust"),
    )
}
