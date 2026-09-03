use std::collections::BTreeMap;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoDenyPolicy {
    section: String,
    modes: BTreeMap<String, String>,
    list_counts: BTreeMap<String, usize>,
    line: u32,
}

pub(super) fn is_dependabot_config_path(path: &str) -> bool {
    is_github_dependabot_config_path(path) || is_cargo_deny_config_path(path)
}

fn is_github_dependabot_config_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        ".github/dependabot.yml" | ".github/dependabot.yaml"
    )
}

fn is_cargo_deny_config_path(path: &str) -> bool {
    path.replace('\\', "/").eq_ignore_ascii_case("deny.toml")
}

pub(super) fn extract_dependabot_config(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    if is_cargo_deny_config_path(&input.source.path) {
        extract_cargo_deny_config(extractor, input, file_id, content, entities, facts);
        return;
    }

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

fn extract_cargo_deny_config(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    for policy in parse_cargo_deny_policies(content) {
        let stable_key = StableKey(format!(
            "config://{}#cargo-deny:{}",
            input.source.path,
            sanitize_key_fragment(&policy.section)
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
            name: policy.section.clone(),
            title: Some(format!("cargo-deny {} policy", policy.section)),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(policy.line),
                line_end: Some(policy.line),
            }),
            language: Some(LanguageCode("toml".to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "feature_kind": "dependency_policy",
                "policy_tool": "cargo-deny",
                "section": policy.section,
                "modes": policy.modes,
                "list_counts": policy.list_counts,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_cargo_deny_policy_defined_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            )),
            kind: FactKind::SymbolDefined,
            subject: entity_id,
            object: Some(file_id.clone()),
            value: json!({
                "stable_key": stable_key.0,
                "path": input.source.path,
                "source_kind": "cargo_deny",
                "section": policy.section,
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

fn parse_cargo_deny_policies(content: &str) -> Vec<CargoDenyPolicy> {
    let Ok(root_value) = toml::from_str::<toml::Value>(content) else {
        return Vec::new();
    };
    let Some(root) = root_value.as_table() else {
        return Vec::new();
    };

    ["advisories", "licenses", "bans", "sources"]
        .into_iter()
        .filter_map(|section| {
            let table = root.get(section)?.as_table()?;
            let mut modes = BTreeMap::new();
            let mut list_counts = BTreeMap::new();

            for key in cargo_deny_mode_keys(section) {
                if let Some(value) = table.get(*key) {
                    if let Some(value) = cargo_deny_scalar_text(value) {
                        modes.insert((*key).to_string(), value);
                    }
                }
            }

            for key in cargo_deny_list_keys(section) {
                if let Some(values) = table.get(*key).and_then(toml::Value::as_array) {
                    list_counts.insert((*key).to_string(), values.len());
                }
            }

            Some(CargoDenyPolicy {
                section: section.to_string(),
                modes,
                list_counts,
                line: cargo_deny_section_line(content, section).unwrap_or(1),
            })
        })
        .collect()
}

fn cargo_deny_mode_keys(section: &str) -> &'static [&'static str] {
    match section {
        "advisories" => &["version", "unmaintained"],
        "licenses" => &["version"],
        "bans" => &["multiple-versions", "wildcards", "highlight"],
        "sources" => &["unknown-registry", "unknown-git"],
        _ => &[],
    }
}

fn cargo_deny_list_keys(section: &str) -> &'static [&'static str] {
    match section {
        "advisories" => &["db-urls", "ignore"],
        "licenses" => &["allow", "exceptions"],
        "bans" => &["allow", "deny", "skip", "skip-tree"],
        "sources" => &["allow-registry", "allow-git"],
        _ => &[],
    }
}

fn cargo_deny_scalar_text(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(value) => Some(value.clone()),
        toml::Value::Integer(value) => Some(value.to_string()),
        toml::Value::Boolean(value) => Some(value.to_string()),
        _ => None,
    }
}

fn cargo_deny_section_line(content: &str, section: &str) -> Option<u32> {
    let header = format!("[{section}]");
    content.lines().enumerate().find_map(|(index, line)| {
        (line.trim() == header).then_some(u32::try_from(index + 1).unwrap_or(u32::MAX))
    })
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

    use super::{
        is_dependabot_config_path, parse_cargo_deny_policies, parse_dependabot_update_policies,
    };
    use crate::OperationsExtractor;

    #[test]
    fn recognizes_only_bounded_dependency_policy_paths() {
        assert!(is_dependabot_config_path(".github/dependabot.yml"));
        assert!(is_dependabot_config_path(".github/dependabot.yaml"));
        assert!(is_dependabot_config_path("deny.toml"));
        assert!(!is_dependabot_config_path("dependabot.yml"));
        assert!(!is_dependabot_config_path("config/deny.toml"));
        assert!(!is_dependabot_config_path(".github/workflows/dependabot.yml"));
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

        assert!(parse_dependabot_update_policies(
            "version: 1\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n"
        )
        .is_empty());
    }

    #[test]
    fn parses_only_bounded_cargo_deny_policy_summaries() {
        let policies = parse_cargo_deny_policies(
            "[advisories]\nversion = 2\nunmaintained = \"workspace\"\nignore = [\"RUSTSEC-1\"]\n\n[licenses]\nversion = 2\nallow = [\"MIT\", \"Apache-2.0\"]\nexceptions = [{ allow = [\"BUSL-1.1\"], name = \"example\" }]\n\n[bans]\nmultiple-versions = \"warn\"\nwildcards = \"allow\"\ndeny = []\n\n[sources]\nunknown-registry = \"deny\"\nunknown-git = \"deny\"\nallow-registry = [\"registry\"]\n",
        );
        assert_eq!(policies.len(), 4);
        assert_eq!(policies[0].section, "advisories");
        assert_eq!(policies[0].modes.get("unmaintained").map(String::as_str), Some("workspace"));
        assert_eq!(policies[0].list_counts.get("ignore"), Some(&1));
        assert_eq!(policies[1].list_counts.get("allow"), Some(&2));
        assert_eq!(policies[2].modes.get("multiple-versions").map(String::as_str), Some("warn"));
        assert_eq!(policies[3].modes.get("unknown-registry").map(String::as_str), Some("deny"));
        assert_eq!(policies[3].line, 16);
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

    #[tokio::test]
    async fn operations_extractor_projects_cargo_deny_sections_without_raw_lists() {
        let source = SourceFile {
            path: "deny.toml".to_string(),
            language_hint: Some("toml".to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(
                "[advisories]\nversion = 2\nunmaintained = \"workspace\"\nignore = [\"RUSTSEC-SECRET\"]\n\n[licenses]\nversion = 2\nallow = [\"MIT\", \"Apache-2.0\"]\n\n[bans]\nmultiple-versions = \"warn\"\nwildcards = \"allow\"\n\n[sources]\nunknown-registry = \"deny\"\nunknown-git = \"deny\"\nallow-registry = [\"private-registry\"]\n"
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
        assert_eq!(policies.len(), 4);
        assert_eq!(
            policies[0].stable_key.0,
            "config://deny.toml#cargo-deny:advisories"
        );
        assert!(policies.iter().all(|entity| {
            entity.payload.get("feature_kind")
                == Some(&serde_json::Value::String("dependency_policy".to_string()))
                && !entity.payload.to_string().contains("RUSTSEC-SECRET")
                && !entity.payload.to_string().contains("private-registry")
        }));
        assert_eq!(output.facts.len(), 4);
        assert!(output.facts.iter().all(|fact| {
            fact.value.get("source_kind")
                == Some(&serde_json::Value::String("cargo_deny".to_string()))
                && !fact.evidence.is_empty()
                && !fact.ownership.is_empty()
        }));
    }
}
