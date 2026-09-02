use std::collections::BTreeMap;

use super::{EnvDeclaration, file_name, is_env_name};

pub(super) fn is_powershell_script_path(path: &str) -> bool {
    file_name(path).to_ascii_lowercase().ends_with(".ps1")
}

pub(super) fn parse_powershell_env_references(content: &str) -> BTreeMap<String, EnvDeclaration> {
    let mut references = BTreeMap::new();

    for (index, line) in content.lines().enumerate() {
        let line_number = (index + 1) as u32;
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        collect_env_references(line, "$env:", line_number, &mut references);
        collect_env_references(line, "${env:", line_number, &mut references);
    }

    references
}

fn collect_env_references(
    line: &str,
    marker: &str,
    line_number: u32,
    references: &mut BTreeMap<String, EnvDeclaration>,
) {
    let lowercase = line.to_ascii_lowercase();
    let marker = marker.to_ascii_lowercase();
    let mut search_from = 0;

    while let Some(relative_start) = lowercase[search_from..].find(&marker) {
        let value_start = search_from + relative_start + marker.len();
        let value = &line[value_start..];
        let name_len = value
            .char_indices()
            .take_while(|(_, character)| character.is_ascii_alphanumeric() || *character == '_')
            .map(|(offset, character)| offset + character.len_utf8())
            .last()
            .unwrap_or(0);

        if name_len == 0 {
            search_from = value_start;
            continue;
        }

        let name = &value[..name_len];
        if is_env_name(name) {
            references
                .entry(name.to_string())
                .or_insert(EnvDeclaration {
                    line: line_number,
                    has_value: false,
                });
        }

        search_from = value_start + name_len;
    }
}

#[cfg(test)]
mod tests {
    use athanor_domain::{EntityKind, FactKind, RepoId, SnapshotId};

    use super::*;
    use crate::{ExtractInput, Extractor, OperationsExtractor, SourceFile};

    #[tokio::test]
    async fn extracts_powershell_environment_references_without_values() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "scripts/install.ps1".to_string(),
                    language_hint: Some("ps1".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "$install = \"$env:LOCALAPPDATA\\Athanor\"\n$temp = $Env:RUNNER_TEMP\n$home = ${env:ATHANOR_HOME}\n$local = $not_environment\n# $env:IGNORED_COMMENT\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let env_keys = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::EnvVar)
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            env_keys,
            vec![
                "env://ATHANOR_HOME",
                "env://LOCALAPPDATA",
                "env://RUNNER_TEMP"
            ]
        );
        assert!(output.entities.iter().all(|entity| {
            entity.payload.get("value").is_none()
                && entity.payload["has_default"] == serde_json::json!(false)
                && entity.payload["value_redacted"] == serde_json::json!(false)
        }));
        assert!(output.facts.iter().all(|fact| {
            fact.kind == FactKind::EnvVarUsed
                && fact.value["mechanism"] == serde_json::json!("powershell")
                && fact.value["source_kind"] == serde_json::json!("powershell")
                && !fact.evidence.is_empty()
                && !fact.ownership.is_empty()
        }));
    }

    #[test]
    fn recognizes_powershell_script_paths() {
        assert!(is_powershell_script_path("install.ps1"));
        assert!(is_powershell_script_path(
            "scripts/collect-windows-daemon-diagnostics.PS1"
        ));
        assert!(!is_powershell_script_path("scripts/install.sh"));
    }
}
