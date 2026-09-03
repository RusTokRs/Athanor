use athanor_core::ExtractInput;
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use serde_json::json;

use super::{sanitize_key_fragment, yaml_key_line};

#[derive(Debug, Clone, PartialEq, Eq)]
struct DependabotUpdatePolicy {
    package_ecosystem: String,
    directory: String,
    schedule_interval: Option<String>,
    target_branch: Option<String>,
    line: u32,
}

pub(super) fn is_dependabot_config_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        ".github/dependabot.yml" | ".github/dependabot.yaml"
    )
}

pub(super) fn extract_dependabot_config(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    for policy in parse_dependabot_update_policies(content) {
        let stable_key = StableKey(format!(
            "config://{}#dependabot:{}:{}",
            input.source.path,
            sanitize_key_fragment(&policy.package_ecosystem),
            sanitize_key_fragment(&policy.directory)
        ));
        let entity_id = EntityId(format!(
            "ent_feature_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        ));
        let ownership = ownership_for_file(&input.source.path);

        entities.push(Entity {
            id: entity_id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::Feature,
            name: format!("{} {}", policy.package_ecosystem, policy.directory),
            title: Some(format!(
                "Dependabot {} updates for {}",
                policy.package_ecosystem, policy.directory
            )),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(policy.line),
                line_end: Some(policy.line),
            }),
            language: Some(LanguageCode("yaml".to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "feature_kind": "dependency_update_policy",
                "package_ecosystem": policy.package_ecosystem,
                "directory": policy.directory,
                "schedule_interval": policy.schedule_interval,
                "target_branch": policy.target_branch,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_dependabot_policy_defined_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            )),
            kind: FactKind::SymbolDefined,
            subject: entity_id,
            object: Some(file_id.clone()),
            value: json!({
                "stable_key": stable_key.0,
                "path": input.source.path,
                "source_kind": "dependabot",
                "package_ecosystem": policy.package_ecosystem,
                "directory": policy.directory,
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(policy.line),
                Some(policy.line),
            )],
            ownership,
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });
    }
}

fn parse_dependabot_update_policies(content: &str) -> Vec<DependabotUpdatePolicy> {
    let Ok(root_value) = serde_yaml_ng::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let Some(root) = root_value.as_object() else {
        return Vec::new();
    };
    if root.get("version").and_then(serde_json::Value::as_u64) != Some(2) {
        return Vec::new();
    }

    let update_lines = dependabot_update_lines(content);
    let fallback_line = yaml_key_line(content, "updates").unwrap_or(1);
    root.get("updates")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, value)| {
            let update = value.as_object()?;
            let package_ecosystem = update.get("package-ecosystem")?.as_str()?.trim();
            let directory = update.get("directory")?.as_str()?.trim();
            if package_ecosystem.is_empty() || directory.is_empty() {
                return None;
            }

            Some(DependabotUpdatePolicy {
                package_ecosystem: package_ecosystem.to_string(),
                directory: directory.to_string(),
                schedule_interval: update
                    .get("schedule")
                    .and_then(serde_json::Value::as_object)
                    .and_then(|schedule| schedule.get("interval"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                target_branch: update
                    .get("target-branch")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                line: update_lines.get(index).copied().unwrap_or(fallback_line),
            })
        })
        .collect()
}

fn dependabot_update_lines(content: &str) -> Vec<u32> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            line.contains("package-ecosystem:")
                .then_some(u32::try_from(index + 1).unwrap_or(u32::MAX))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use athanor_core::{ExtractInput, Extractor, SourceFile};
    use athanor_domain::{EntityKind, RepoId, SnapshotId};

    use super::{is_dependabot_config_path, parse_dependabot_update_policies};
    use crate::OperationsExtractor;

    #[test]
    fn recognizes_only_github_dependabot_config_paths() {
        assert!(is_dependabot_config_path(".github/dependabot.yml"));
        assert!(is_dependabot_config_path(".github/dependabot.yaml"));
        assert!(!is_dependabot_config_path("dependabot.yml"));
        assert!(!is_dependabot_config_path(
            ".github/workflows/dependabot.yml"
        ));
    }

    #[test]
    fn parses_version_two_update_policies_and_rejects_other_versions() {
        let policies = parse_dependabot_update_policies(
            "version: 2\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n    schedule:\n      interval: weekly\n  - package-ecosystem: github-actions\n    directory: /\n    target-branch: main\n    schedule:\n      interval: monthly\n",
        );
        assert_eq!(policies.len(), 2);
        assert_eq!(policies[0].package_ecosystem, "cargo");
        assert_eq!(policies[0].directory, "/");
        assert_eq!(policies[0].schedule_interval.as_deref(), Some("weekly"));
        assert_eq!(policies[0].line, 3);
        assert_eq!(policies[1].target_branch.as_deref(), Some("main"));
        assert_eq!(policies[1].line, 7);

        assert!(
            parse_dependabot_update_policies(
                "version: 1\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n"
            )
            .is_empty()
        );
    }

    #[tokio::test]
    async fn operations_extractor_projects_dependabot_policies_without_generic_yaml_fields() {
        let source = SourceFile {
            path: ".github/dependabot.yml".to_string(),
            language_hint: Some("yaml".to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(
                "version: 2\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n    schedule:\n      interval: weekly\n    registries: [private]\n  - package-ecosystem: github-actions\n    directory: /\n    schedule:\n      interval: weekly\n"
                    .to_string(),
            ),
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

        let policies = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Feature)
            .collect::<Vec<_>>();
        assert_eq!(policies.len(), 2);
        assert_eq!(
            policies[0].stable_key.0,
            "config://.github/dependabot.yml#dependabot:cargo:-"
        );
        assert_eq!(
            policies[1].stable_key.0,
            "config://.github/dependabot.yml#dependabot:github-actions:-"
        );
        assert!(policies.iter().all(|entity| {
            entity.payload.get("feature_kind")
                == Some(&serde_json::Value::String(
                    "dependency_update_policy".to_string(),
                ))
                && entity.payload.get("registries").is_none()
        }));
        assert_eq!(output.facts.len(), 2);
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }
}
