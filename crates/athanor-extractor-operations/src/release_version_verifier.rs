use athanor_core::ExtractInput;
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use serde_json::json;

use super::script_command_entity_id;

const RELEASE_VERIFIER_PATH: &str = "scripts/verify_release_version.py";
const CLI_INPUTS: [&str; 4] = ["--tag", "--changelog", "--notes-output", "manifests"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseVerifierMetadata {
    evidence_lines: Vec<u32>,
}

pub(super) fn is_release_version_verifier_path(path: &str) -> bool {
    path.replace('\\', "/") == RELEASE_VERIFIER_PATH
}

pub(super) fn extract_release_version_verifier(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    if !is_release_version_verifier_path(&input.source.path) {
        return;
    }
    let Some(metadata) = parse_release_version_verifier(content) else {
        return;
    };

    let stable_key = StableKey(format!(
        "script-command://{}#release-contract-verifier",
        input.source.path
    ));
    let entity_id = script_command_entity_id(&stable_key);
    let ownership = ownership_for_file(&input.source.path);

    let payload = json!({
        "command_kind": "release_contract_verifier",
        "entrypoint": RELEASE_VERIFIER_PATH,
        "cli_inputs": CLI_INPUTS,
        "tag_contract": "v<semver>",
        "manifest_version_source": "Cargo package.version",
        "requires_manifest_version_coherence": true,
        "changelog_heading_contract": "## [<version>] - <date>",
        "requires_single_changelog_section": true,
        "requires_substantive_release_notes": true,
        "writes_release_notes": true,
    });

    entities.push(Entity {
        id: entity_id.clone(),
        stable_key: stable_key.clone(),
        kind: EntityKind::ScriptCommand,
        name: "verify_release_version.py".to_string(),
        title: Some("Release version contract verifier".to_string()),
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(1),
            line_end: Some(1),
        }),
        language: Some(LanguageCode("python".to_string())),
        aliases: Vec::new(),
        ownership: ownership.clone(),
        payload: payload.clone(),
    });

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
            "source_kind": "release_version_verifier",
            "command_kind": payload["command_kind"],
            "entrypoint": payload["entrypoint"],
            "cli_inputs": payload["cli_inputs"],
            "tag_contract": payload["tag_contract"],
            "manifest_version_source": payload["manifest_version_source"],
            "requires_manifest_version_coherence": payload["requires_manifest_version_coherence"],
            "changelog_heading_contract": payload["changelog_heading_contract"],
            "requires_single_changelog_section": payload["requires_single_changelog_section"],
            "requires_substantive_release_notes": payload["requires_substantive_release_notes"],
            "writes_release_notes": payload["writes_release_notes"],
        }),
        evidence: metadata
            .evidence_lines
            .into_iter()
            .map(|line| evidence_for_file(&input.source.path, extractor, Some(line), Some(line)))
            .collect(),
        ownership,
        snapshot: input.snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    });
}

fn parse_release_version_verifier(content: &str) -> Option<ReleaseVerifierMetadata> {
    if content.lines().next()?.trim() != "#!/usr/bin/env python3" {
        return None;
    }

    let required_markers = [
        "parser.add_argument(\"--tag\"",
        "parser.add_argument(\"--changelog\"",
        "parser.add_argument(\"--notes-output\"",
        "\"manifests\",",
        "nargs=\"+\"",
        "package.get(\"version\")",
        "if not tag.startswith(\"v\"):",
        "if not SEMVER.fullmatch(tag_version):",
        "if version != tag_version",
        "if len(unique_versions) != 1:",
        "if release_date == \"Unreleased\":",
        "if not RELEASE_DATE.fullmatch(release_date):",
        "if len(matches) != 1:",
        "dated_heading = re.compile",
        "if not has_substantive_release_notes(note_lines):",
        "args.notes_output.write_text(",
    ];

    let mut evidence_lines = vec![1];
    for marker in required_markers {
        evidence_lines.push(find_line(content, marker)?);
    }
    evidence_lines.sort_unstable();
    evidence_lines.dedup();

    Some(ReleaseVerifierMetadata { evidence_lines })
}

fn find_line(content: &str, marker: &str) -> Option<u32> {
    content.lines().enumerate().find_map(|(index, line)| {
        line.trim()
            .contains(marker)
            .then_some((index + 1) as u32)
    })
}

#[cfg(test)]
mod tests {
    use athanor_core::{Extractor, SourceFile};
    use athanor_domain::{EntityKind, FactKind, RepoId, SnapshotId};

    use super::*;
    use crate::OperationsExtractor;

    const RELEASE_VERIFIER: &str = include_str!("../../../scripts/verify_release_version.py");

    #[test]
    fn recognizes_only_the_first_party_release_verifier_path() {
        assert!(is_release_version_verifier_path(
            "scripts/verify_release_version.py"
        ));
        assert!(!is_release_version_verifier_path(
            "tools/verify_release_version.py"
        ));
        assert!(!is_release_version_verifier_path("scripts/release.py"));
    }

    #[tokio::test]
    async fn projects_bounded_release_contract_metadata() {
        let extractor = OperationsExtractor;
        let source = SourceFile {
            path: RELEASE_VERIFIER_PATH.to_string(),
            language_hint: Some("python".to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(RELEASE_VERIFIER.to_string()),
        };
        assert!(extractor.supports(&source));

        let output = extractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source,
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.facts.len(), 1);

        let entity = &output.entities[0];
        assert_eq!(entity.kind, EntityKind::ScriptCommand);
        assert_eq!(
            entity.stable_key.0,
            "script-command://scripts/verify_release_version.py#release-contract-verifier"
        );
        assert_eq!(entity.payload["tag_contract"], json!("v<semver>"));
        assert_eq!(
            entity.payload["manifest_version_source"],
            json!("Cargo package.version")
        );
        assert_eq!(
            entity.payload["cli_inputs"],
            json!(["--tag", "--changelog", "--notes-output", "manifests"])
        );
        assert_eq!(
            entity.payload["changelog_heading_contract"],
            json!("## [<version>] - <date>")
        );
        assert_eq!(entity.payload["writes_release_notes"], json!(true));

        let serialized_payload = entity.payload.to_string();
        assert!(!serialized_payload.contains("v0.1.0"));
        assert!(!serialized_payload.contains("CHANGELOG.md"));
        assert!(!serialized_payload.contains("dist/release-notes.md"));

        let fact = &output.facts[0];
        assert_eq!(fact.kind, FactKind::SymbolDefined);
        assert_eq!(fact.value["source_kind"], json!("release_version_verifier"));
        assert!(!fact.evidence.is_empty());
        assert!(!fact.ownership.is_empty());
    }

    #[tokio::test]
    async fn stops_projection_when_a_required_contract_anchor_drifts() {
        let drifted = RELEASE_VERIFIER.replace(
            "if not tag.startswith(\"v\"):",
            "if not tag.startswith(\"release-\"):",
        );
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: RELEASE_VERIFIER_PATH.to_string(),
                    language_hint: Some("python".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(drifted),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.is_empty());
        assert!(output.facts.is_empty());
    }
}
