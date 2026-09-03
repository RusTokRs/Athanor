use std::collections::BTreeMap;

use athanor_core::ExtractInput;
use athanor_domain::{Entity, EntityId, Fact, StableKey};
use serde_json::json;

use super::{EnvDeclaration, extract_env_declarations, push_script_command_entity_and_fact};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellInstaller {
    line: u32,
    required_binaries: Vec<String>,
    installed_binaries: Vec<String>,
    checksum_tools: Vec<String>,
    checksum_manifest: String,
    environment: BTreeMap<String, EnvDeclaration>,
}

pub(super) fn is_first_party_shell_installer_path(path: &str) -> bool {
    path.replace('\\', "/") == "install.sh"
}

pub(super) fn extract_first_party_shell_installer(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    if !is_first_party_shell_installer_path(&input.source.path) {
        return;
    }
    let Some(installer) = parse_first_party_shell_installer(content) else {
        return;
    };

    let stable_key = StableKey(format!(
        "script-command://{}#athanor-installer",
        input.source.path
    ));
    push_script_command_entity_and_fact(
        extractor,
        input,
        file_id,
        stable_key,
        "install".to_string(),
        Some("Athanor release installer".to_string()),
        installer.line,
        "shell",
        json!({
            "command_kind": "athanor_shell_installer",
            "required_binaries": installer.required_binaries,
            "installed_binaries": installer.installed_binaries,
            "checksum_tools": installer.checksum_tools,
            "checksum_manifest": installer.checksum_manifest,
        }),
        json!({
            "path": input.source.path,
            "source_kind": "shell_installer",
            "installer_kind": "release",
        }),
        entities,
        facts,
    );

    extract_env_declarations(
        extractor,
        input,
        file_id,
        "shell_installer",
        installer.environment,
        entities,
        facts,
    );
}

fn parse_first_party_shell_installer(content: &str) -> Option<ShellInstaller> {
    let checksum_manifest = quoted_assignment(content, "manifest")?;
    if checksum_manifest != "SHA256SUMS" {
        return None;
    }

    let (line, required_binaries) = installer_required_binaries(content)?;
    if required_binaries != ["ath", "athd"] {
        return None;
    }

    let installed_binaries = installed_release_binaries(content);
    if installed_binaries != required_binaries {
        return None;
    }

    let checksum_tools = checksum_verification_tools(content);
    if checksum_tools != ["sha256sum", "shasum"] {
        return None;
    }

    let mut environment = BTreeMap::new();
    for name in ["ATHANOR_INSTALL_DIR", "HOME"] {
        let line = shell_reference_line(content, name)?;
        environment.insert(
            name.to_string(),
            EnvDeclaration {
                line,
                has_value: false,
            },
        );
    }

    Some(ShellInstaller {
        line,
        required_binaries,
        installed_binaries,
        checksum_tools,
        checksum_manifest,
        environment,
    })
}

fn quoted_assignment(content: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=\"");
    content.lines().find_map(|line| {
        let value = line.trim().strip_prefix(&prefix)?.strip_suffix('"')?;
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn installer_required_binaries(content: &str) -> Option<(u32, Vec<String>)> {
    content.lines().enumerate().find_map(|(index, line)| {
        let values = line
            .trim()
            .strip_prefix("for binary in ")?
            .strip_suffix("; do")?
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        (!values.is_empty()).then_some((u32::try_from(index + 1).unwrap_or(u32::MAX), values))
    })
}

fn installed_release_binaries(content: &str) -> Vec<String> {
    let mut binaries = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("install -m 0755 ")?;
            let binary = rest.split_whitespace().next()?;
            matches!(binary, "ath" | "athd").then(|| binary.to_string())
        })
        .collect::<Vec<_>>();
    binaries.sort();
    binaries.dedup();
    binaries
}

fn checksum_verification_tools(content: &str) -> Vec<String> {
    let mut tools = Vec::new();
    for line in content.lines().map(str::trim) {
        if line.starts_with("sha256sum -c ") {
            tools.push("sha256sum".to_string());
        } else if line.starts_with("shasum -a 256 -c ") {
            tools.push("shasum".to_string());
        }
    }
    tools.sort();
    tools.dedup();
    tools
}

fn shell_reference_line(content: &str, name: &str) -> Option<u32> {
    let braced = format!("${{{name}");
    let plain = format!("${name}");
    content.lines().enumerate().find_map(|(index, line)| {
        (line.contains(&braced) || line.contains(&plain))
            .then_some(u32::try_from(index + 1).unwrap_or(u32::MAX))
    })
}

#[cfg(test)]
mod tests {
    use athanor_core::{ExtractInput, Extractor, SourceFile};
    use athanor_domain::{EntityKind, RepoId, SnapshotId};

    use super::*;
    use crate::OperationsExtractor;

    const INSTALLER: &str = "#!/usr/bin/env sh\nset -eu\nmanifest=\"SHA256SUMS\"\nfor binary in ath athd; do\n  test -f \"$binary\"\ndone\nif command -v sha256sum >/dev/null 2>&1; then\n  sha256sum -c \"$manifest\"\nelif command -v shasum >/dev/null 2>&1; then\n  shasum -a 256 -c \"$manifest\"\nfi\nprefix=\"${ATHANOR_INSTALL_DIR:-$HOME/.local/bin}\"\ninstall -m 0755 ath \"$prefix/ath\"\ninstall -m 0755 athd \"$prefix/athd\"\n";

    #[test]
    fn recognizes_only_root_first_party_installer_path() {
        assert!(is_first_party_shell_installer_path("install.sh"));
        assert!(!is_first_party_shell_installer_path("scripts/install.sh"));
        assert!(!is_first_party_shell_installer_path("install.bash"));
    }

    #[test]
    fn parses_only_bounded_release_installer_contract() {
        let installer = parse_first_party_shell_installer(INSTALLER).unwrap();
        assert_eq!(installer.required_binaries, ["ath", "athd"]);
        assert_eq!(installer.installed_binaries, ["ath", "athd"]);
        assert_eq!(installer.checksum_tools, ["sha256sum", "shasum"]);
        assert_eq!(installer.checksum_manifest, "SHA256SUMS");
        assert_eq!(installer.environment.len(), 2);
        assert!(parse_first_party_shell_installer("#!/bin/sh\necho install\n").is_none());
    }

    #[tokio::test]
    async fn operations_extractor_projects_installer_without_environment_defaults() {
        let source = SourceFile {
            path: "install.sh".to_string(),
            language_hint: Some("sh".to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(INSTALLER.to_string()),
        };
        let extractor = OperationsExtractor;
        assert!(extractor.supports(&source));

        let output = extractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source,
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ScriptCommand
                && entity.stable_key.0 == "script-command://install.sh#athanor-installer"
                && entity.payload["command_kind"] == json!("athanor_shell_installer")
                && entity.payload["required_binaries"] == json!(["ath", "athd"])
                && entity.payload["checksum_manifest"] == json!("SHA256SUMS")
        }));
        let env_keys = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::EnvVar)
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(env_keys, vec!["env://ATHANOR_INSTALL_DIR", "env://HOME"]);
        assert!(output.entities.iter().all(|entity| {
            entity.payload.get("value").is_none()
                && !entity.payload.to_string().contains(".local/bin")
        }));
        assert_eq!(output.facts.len(), 3);
        assert!(output.facts.iter().all(|fact| {
            !fact.evidence.is_empty()
                && !fact.ownership.is_empty()
                && !fact.value.to_string().contains(".local/bin")
        }));
    }
}
