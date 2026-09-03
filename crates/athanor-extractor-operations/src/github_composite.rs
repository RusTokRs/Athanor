use super::{GithubActionsStep, parse_github_actions_step, yaml_key_line};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GithubCompositeAction {
    pub(super) name: Option<String>,
    pub(super) line: u32,
    pub(super) steps: Vec<GithubActionsStep>,
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

pub(super) fn parse_github_composite_action(content: &str) -> Option<GithubCompositeAction> {
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
    use super::*;
    use crate::GithubActionsStepKind;

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
}
