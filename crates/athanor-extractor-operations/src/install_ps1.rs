use athanor_core::ExtractInput;
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use serde_json::json;

use super::script_command_entity_id;

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstallPs1Metadata {
    manifest_line: u32,
    expected_files_line: u32,
    checksum_line: u32,
}

pub(super) fn is_root_install_ps1_path(path: &str) -> bool {
    path.replace('\\', "/") == "install.ps1"
}

pub(super) fn extract_install_ps1(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    if !is_root_install_ps1_path(&input.source.path) {
        return;
    }
    let Some(metadata) = parse_install_ps1(content) else {
        return;
    };

    let stable_key = StableKey(format!(
        "script-command://{}#installer-powershell",
        input.source.path
    ));
    let entity_id = script_command_entity_id(&stable_key);
    let ownership = ownership_for_file(&input.source.path);

    entities.push(Entity {
        id: entity_id.clone(),
        stable_key: stable_key.clone(),
        kind: EntityKind::ScriptCommand,
        name: "install.ps1".to_string(),
        title: Some("Athanor install.ps1 entry point".to_string()),
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(1),
            line_end: Some(1),
        }),
        language: Some(LanguageCode("powershell".to_string())),
        aliases: Vec::new(),
        ownership: ownership.clone(),
        payload: json!({
            "command_kind": "first_party_install_script",
            "entrypoint": "install.ps1",
            "install_targets": ["ath.exe", "athd.exe"],
            "checksum_manifest": "SHA256SUMS",
            "checksum_tool": "Get-FileHash",
            "security_anchor": "checksum_verification",
        }),
    });

    let mut evidence_lines = vec![
        1,
        metadata.manifest_line,
        metadata.expected_files_line,
        metadata.checksum_line,
    ];
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
            "entrypoint": "install.ps1",
            "install_targets": ["ath.exe", "athd.exe"],
            "checksum_manifest": "SHA256SUMS",
            "checksum_tool": "Get-FileHash",
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

fn parse_install_ps1(content: &str) -> Option<InstallPs1Metadata> {
    if content.lines().next()?.trim() != "param(" {
        return None;
    }

    let mut manifest_line = None;
    let mut expected_files_line = None;
    let mut checksum_line = None;

    for (index, raw_line) in content.lines().enumerate() {
        let line = (index + 1) as u32;
        match raw_line.trim() {
            "$ManifestPath = Join-Path $ScriptDir \"SHA256SUMS\"" => manifest_line = Some(line),
            "$ExpectedFiles = @(\"ath.exe\", \"athd.exe\")" => expected_files_line = Some(line),
            "$ActualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $SourcePath).Hash.ToLowerInvariant()" => {
                checksum_line = Some(line)
            }
            _ => {}
        }
    }

    Some(InstallPs1Metadata {
        manifest_line: manifest_line?,
        expected_files_line: expected_files_line?,
        checksum_line: checksum_line?,
    })
}

#[cfg(test)]
mod tests {
    use athanor_core::{Extractor, SourceFile};
    use athanor_domain::{EntityKind, FactKind, RepoId, SnapshotId};

    use super::*;
    use crate::OperationsExtractor;

    #[test]
    fn recognizes_only_the_root_install_ps1() {
        assert!(is_root_install_ps1_path("install.ps1"));
        assert!(!is_root_install_ps1_path("scripts/install.ps1"));
        assert!(!is_root_install_ps1_path("install.sh"));
    }

    #[tokio::test]
    async fn projects_bounded_installer_and_checksum_metadata() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "install.ps1".to_string(),
                    language_hint: Some("powershell".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(include_str!("../../../../install.ps1").to_string()),
                },
            })
            .await
            .unwrap();

        let command_entities = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ScriptCommand)
            .collect::<Vec<_>>();
        assert_eq!(command_entities.len(), 1);

        let entity = command_entities[0];
        assert_eq!(
            entity.stable_key.0,
            "script-command://install.ps1#installer-powershell"
        );
        assert_eq!(entity.payload["entrypoint"], json!("install.ps1"));
        assert_eq!(
            entity.payload["install_targets"],
            json!(["ath.exe", "athd.exe"])
        );
        assert_eq!(entity.payload["checksum_manifest"], json!("SHA256SUMS"));
        assert_eq!(entity.payload["checksum_tool"], json!("Get-FileHash"));
        assert_eq!(
            entity.payload["security_anchor"],
            json!("checksum_verification")
        );
        assert!(entity.payload.get("install_dir").is_none());
        assert!(entity.payload.get("environment").is_none());

        let command_facts = output
            .facts
            .iter()
            .filter(|fact| fact.kind == FactKind::SymbolDefined)
            .collect::<Vec<_>>();
        assert_eq!(command_facts.len(), 1);
        assert!(!command_facts[0].evidence.is_empty());
        assert!(!command_facts[0].ownership.is_empty());
    }

    #[tokio::test]
    async fn rejects_script_with_drifted_checksum_contract() {
        let content = include_str!("../../../../install.ps1").replace(
            "Get-FileHash -Algorithm SHA256",
            "Get-FileHash -Algorithm MD5",
        );
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "install.ps1".to_string(),
                    language_hint: Some("powershell".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(content),
                },
            })
            .await
            .unwrap();

        assert!(output
            .entities
            .iter()
            .all(|entity| entity.kind != EntityKind::ScriptCommand));
        assert!(output
            .facts
            .iter()
            .all(|fact| fact.kind != FactKind::SymbolDefined));
    }
}
