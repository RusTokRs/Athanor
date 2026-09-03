use std::collections::BTreeMap;

use athanor_core::ExtractInput;
use athanor_domain::{Entity, EntityId, Fact, StableKey};
use serde_json::json;

use super::{
    EnvDeclaration, GithubActionsStep, GithubActionsStepKind, extract_env_declarations,
    parse_github_actions_step, push_script_command_entity_and_fact, yaml_key_line,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubCompositeAction {
    name: Option<String>,
    line: u32,
    steps: Vec<GithubActionsStep>,
}

pub(super) fn is_github_composite_action_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    if !normalized.starts_with(".github/actions/") {
        return false;
    }
    matches!(
        normalized.rsplit('/').next().unwrap_or_default(),
        "action.yml" | "action.yaml"
    )
}

pub(super) fn extract_github_composite_action(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let Some(action) = parse_github_composite_action(content) else {
        return;
    };

    let action_name = action
        .name
        .clone()
        .unwrap_or_else(|| "composite action".to_string());
    let action_key = StableKey(format!(
        "script-command://{}#github-actions:composite",
        input.source.path
    ));
    push_script_command_entity_and_fact(
        extractor,
        input,
        file_id,
        action_key,
        action_name.clone(),
        Some(format!("GitHub composite action {action_name}")),
        action.line,
        "github_actions",
        json!({
            "command_kind": "github_composite_action",
            "action": action_name,
        }),
        json!({
            "path": input.source.path,
            "source_kind": "github_actions",
            "action_kind": "composite",
        }),
        entities,
        facts,
    );

    let mut environment = BTreeMap::<String, EnvDeclaration>::new();
    for step in action.steps {
        for (name, declaration) in &step.environment {
            environment
                .entry(name.clone())
                .or_insert_with(|| declaration.clone());
        }

        let (step_kind, value) = match &step.kind {
            GithubActionsStepKind::Run(command) => ("run", command.as_str()),
            GithubActionsStepKind::Uses(action) => ("uses", action.as_str()),
        };
        let step_key = StableKey(format!(
            "script-command://{}#github-actions:composite:step:{}:{}",
            input.source.path, step.index, step_kind
        ));
        let step_name = step
            .name
            .clone()
            .unwrap_or_else(|| format!("composite step {}", step.index));
        push_script_command_entity_and_fact(
            extractor,
            input,
            file_id,
            step_key,
            step_name.clone(),
            Some(format!("GitHub composite {step_kind} step {step_name}")),
            step.line,
            "github_actions",
            json!({
                "command_kind": "github_composite_step",
                "step": step.index,
                "step_name": &step.name,
                "step_kind": step_kind,
                "value": value,
            }),
            json!({
                "path": input.source.path,
                "source_kind": "github_actions",
                "action_kind": "composite",
                "step": step.index,
                "step_kind": step_kind,
            }),
            entities,
            facts,
        );
    }

    extract_env_declarations(
        extractor,
        input,
        file_id,
        "github_actions",
        environment,
        entities,
        facts,
    );
}

fn parse_github_composite_action(content: &str) -> Option<GithubCompositeAction> {
    let root_value = serde_yaml_ng::from_str::<serde_json::Value>(content).ok()?;
    let root = root_value.as_object()?;
    let runs = root.get("runs")?.as_object()?;
    let using = runs.get("using")?.as_str()?;
    if !using.eq_ignore_ascii_case("composite") {
        return None;
    }

    let steps = runs
        .get("steps")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, value)| parse_github_actions_step(content, index + 1, value))
        .collect::<Vec<_>>();

    Some(GithubCompositeAction {
        name: root
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        line: yaml_key_line(content, "name")
            .or_else(|| yaml_key_line(content, "runs"))
            .unwrap_or(1),
        steps,
    })
}

#[cfg(test)]
mod tests {
    use athanor_core::{ExtractInput, Extractor, SourceFile};
    use athanor_domain::{EntityKind, RepoId, SnapshotId};

    use super::*;
    use crate::OperationsExtractor;

    #[test]
    fn recognizes_only_first_party_action_metadata_paths() {
        assert!(is_github_composite_action_path(
            ".github/actions/setup-rust/action.yml"
        ));
        assert!(is_github_composite_action_path(
            ".github/actions/cache/action.yaml"
        ));
        assert!(!is_github_composite_action_path(
            ".github/workflows/action.yml"
        ));
        assert!(!is_github_composite_action_path(
            ".github/actions/setup-rust/action.json"
        ));
    }

    #[test]
    fn parses_composite_run_and_uses_steps_but_rejects_javascript_actions() {
        let action = parse_github_composite_action(
            "name: Setup\nruns:\n  using: composite\n  steps:\n    - name: Checkout\n      uses: actions/checkout@v7\n    - name: Build\n      env:\n        CARGO_TERM_COLOR: always\n      run: cargo build\n      shell: bash\n",
        )
        .unwrap();

        assert_eq!(action.name.as_deref(), Some("Setup"));
        assert_eq!(action.steps.len(), 2);
        assert!(matches!(action.steps[0].kind, GithubActionsStepKind::Uses(_)));
        assert!(matches!(action.steps[1].kind, GithubActionsStepKind::Run(_)));
        assert!(action.steps[1].environment.contains_key("CARGO_TERM_COLOR"));

        assert!(parse_github_composite_action(
            "name: JavaScript action\nruns:\n  using: node24\n  main: index.js\n"
        )
        .is_none());
    }

    #[tokio::test]
    async fn operations_extractor_projects_composite_action_steps_and_env_without_values() {
        let source = SourceFile {
            path: ".github/actions/setup-rust/action.yml".to_string(),
            language_hint: Some("yaml".to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(
                "name: Setup Rust\nruns:\n  using: composite\n  steps:\n    - name: Checkout\n      uses: actions/checkout@v7\n    - name: Build\n      env:\n        CARGO_TERM_COLOR: always\n      run: cargo build\n      shell: bash\n"
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

        let command_keys = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ScriptCommand)
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            command_keys,
            vec![
                "script-command://.github/actions/setup-rust/action.yml#github-actions:composite",
                "script-command://.github/actions/setup-rust/action.yml#github-actions:composite:step:1:uses",
                "script-command://.github/actions/setup-rust/action.yml#github-actions:composite:step:2:run",
            ]
        );
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::EnvVar
                && entity.stable_key.0 == "env://CARGO_TERM_COLOR"
                && entity.payload.get("value").is_none()
        }));
        assert_eq!(output.facts.len(), 4);
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }
}
