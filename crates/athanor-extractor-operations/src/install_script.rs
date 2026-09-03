use athanor_core::ExtractInput;
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use serde_json::json;

use super::script_command_entity_id;

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstallScriptMetadata {
    manifest_line: u32,
    install_targets: Vec<(String, u32)>,
    checksum_tools: Vec<(String, u32)>,
}

pub(super) fn is_root_install_script_path(path: &str) -> bool {
    path.replace('\\', "/") == "install.sh"
}

pub(super) fn extract_install_script(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    if !is_root_install_script_path(&input.source.path) {
        return;
    }
    let Some(metadata) = parse_install_script(content) else {
        return;
    };

    let stable_key = StableKey(format!("script-command://{}#installer", input.source.path));
    let entity_id = script_command_entity_id(&stable_key);
    let ownership = ownership_for_file(&input.source.path);
    let install_targets = metadata
        .install_targets
        .iter()
        .map(|(target, _)| target.as_str())
        .collect::<Vec<_>>();
    let checksum_tools = metadata
        .checksum_tools
        .iter()
        .map(|(tool, _)| tool.as_str())
        .collect::<Vec<_>>();

    entities.push(Entity {
        id: entity_id.clone(),
        stable_key: stable_key.clone(),
        kind: EntityKind::ScriptCommand,
        name: "install.sh".to_string(),
        title: Some("Athanor install.sh entry point".to_string()),
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(1),
            line_end: Some(1),
        }),
        language: Some(LanguageCode("shell".to_string())),
        aliases: Vec::new(),
        ownership: ownership.clone(),
        payload: json!({
            "command_kind": "first_party_install_script",
            "entrypoint": "install.sh",
            "install_targets": install_targets,
            "checksum_manifest": "SHA256SUMS",
            "checksum_tools": checksum_tools,
            "security_anchor": "checksum_verification",
        }),
    });

    let mut evidence_lines = vec![1, metadata.manifest_line];
    evidence_lines.extend(metadata.install_targets.iter().map(|(_, line)| *line));
    evidence_lines.extend(metadata.checksum_tools.iter().map(|(_, line)| *line));
    evidence_lines.sort_unstable();
    evidence_lines.dedup();

    facts.push(Fact {
        id: FactId(format!(
            "fact_script_command_defined_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        )),
        kind: FactKind::SymbolDefined,
        subject: entity_id,
        object: Some(file_id.clone()),
        value: json!({
            "stable_key": stable_key.0,
            "path": input.source.path,
            "source_kind": "install_script",
            "entrypoint": "install.sh",
            "install_targets": install_targets,
            "checksum_manifest": "SHA256SUMS",
            "checksum_tools": checksum_tools,
            "security_anchor": "checksum_verification",
        }),
        evidence: evidence_lines
            .into_iter()
            .map(|line| evidence_for_file(&input.source.path, extractor, Some(line), Some(line)))
            .collect(),
        ownership,
        snapshot: input.snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    });
}

fn parse_install_script(content: &str) -> Option<InstallScriptMetadata> {
    let first_line = content.lines().next()?.trim();
    if !is_shell_shebang(first_line) {
        return None;
    }

    let mut manifest_line = None;
    let mut install_targets = Vec::new();
    let mut checksum_tools = Vec::new();

    for (index, raw_line) in content.lines().enumerate() {
        let line = (index + 1) as u32;
        let trimmed = raw_line.trim();

        if matches!(trimmed, "manifest=\"SHA256SUMS\"" | "manifest='SHA256SUMS'") {
            manifest_line.get_or_insert(line);
            continue;
        }

        if trimmed == "sha256sum -c \"$manifest\"" {
            checksum_tools.push(("sha256sum".to_string(), line));
            continue;
        }
        if trimmed == "shasum -a 256 -c \"$manifest\"" {
            checksum_tools.push(("shasum".to_string(), line));
            continue;
        }

        if let Some(target) = parse_install_target(trimmed) {
            install_targets.push((target, line));
        }
    }

    install_targets.sort_by(|left, right| left.0.cmp(&right.0));
    install_targets.dedup_by(|left, right| left.0 == right.0);
    checksum_tools.sort_by(|left, right| left.0.cmp(&right.0));
    checksum_tools.dedup_by(|left, right| left.0 == right.0);

    let manifest_line = manifest_line?;
    let target_names = install_targets
        .iter()
        .map(|(target, _)| target.as_str())
        .collect::<Vec<_>>();
    if target_names != ["ath", "athd"] || checksum_tools.is_empty() {
        return None;
    }

    Some(InstallScriptMetadata {
        manifest_line,
        install_targets,
        checksum_tools,
    })
}

fn is_shell_shebang(line: &str) -> bool {
    if !line.starts_with("#!") {
        return false;
    }
    let command = line.trim_start_matches("#!").trim();
    command == "/bin/sh"
        || command == "/usr/bin/sh"
        || command
            .split_whitespace()
            .next()
            .is_some_and(|program| program.ends_with("/env"))
            && command.split_whitespace().nth(1) == Some("sh")
}

fn parse_install_target(line: &str) -> Option<String> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 5 || parts[..3] != ["install", "-m", "0755"] {
        return None;
    }
    let target = parts[3];
    if !matches!(target, "ath" | "athd") {
        return None;
    }
    let destination = parts[4].trim_matches(['\'', '"']);
    (destination == format!("$prefix/{target}")).then(|| target.to_string())
}

#[cfg(test)]
mod tests {
    use athanor_core::{Extractor, SourceFile};
    use athanor_domain::{EntityKind, FactKind, RepoId, SnapshotId};

    use super::*;
    use crate::OperationsExtractor;

    const INSTALL_SCRIPT: &str = "#!/usr/bin/env sh\nset -eu\n\nscript_dir=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\ncd \"$script_dir\"\n\nmanifest=\"SHA256SUMS\"\nif [ ! -f \"$manifest\" ]; then\n  exit 1\nfi\n\nfor binary in ath athd; do\n  test -f \"$binary\"\ndone\n\nif command -v sha256sum >/dev/null 2>&1; then\n  sha256sum -c \"$manifest\"\nelif command -v shasum >/dev/null 2>&1; then\n  shasum -a 256 -c \"$manifest\"\nfi\n\nprefix=\"${ATHANOR_INSTALL_DIR:-$HOME/.local/bin}\"\nmkdir -p \"$prefix\"\ninstall -m 0755 ath \"$prefix/ath\"\ninstall -m 0755 athd \"$prefix/athd\"\n";

    #[test]
    fn recognizes_only_the_root_install_script() {
        assert!(is_root_install_script_path("install.sh"));
        assert!(!is_root_install_script_path("scripts/install.sh"));
        assert!(!is_root_install_script_path("install.ps1"));
    }

    #[tokio::test]
    async fn projects_bounded_install_targets_and_checksum_metadata() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "install.sh".to_string(),
                    language_hint: Some("shell".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(INSTALL_SCRIPT.to_string()),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 1);
        let entity = &output.entities[0];
        assert_eq!(entity.kind, EntityKind::ScriptCommand);
        assert_eq!(entity.stable_key.0, "script-command://install.sh#installer");
        assert_eq!(entity.payload["entrypoint"], json!("install.sh"));
        assert_eq!(entity.payload["install_targets"], json!(["ath", "athd"]));
        assert_eq!(entity.payload["checksum_manifest"], json!("SHA256SUMS"));
        assert_eq!(entity.payload["checksum_tools"], json!(["sha256sum", "shasum"]));
        assert_eq!(entity.payload["security_anchor"], json!("checksum_verification"));
        assert!(entity.payload.get("prefix").is_none());
        assert!(entity.payload.get("environment").is_none());

        assert_eq!(output.facts.len(), 1);
        let fact = &output.facts[0];
        assert_eq!(fact.kind, FactKind::SymbolDefined);
        assert_eq!(fact.value["source_kind"], json!("install_script"));
        assert!(!fact.ownership.is_empty());
        assert_eq!(
            fact.evidence
                .iter()
                .filter_map(|evidence| evidence.line_start)
                .collect::<Vec<_>>(),
            vec![1, 7, 17, 19, 24, 25]
        );
    }

    #[tokio::test]
    async fn does_not_project_nested_install_scripts_as_first_party_installer_metadata() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "scripts/install.sh".to_string(),
                    language_hint: Some("shell".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(INSTALL_SCRIPT.to_string()),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.is_empty());
        assert!(output.facts.is_empty());
    }
}
