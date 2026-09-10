use athanor_core::ExtractInput;
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use serde_json::json;

use super::{sanitize_key_fragment, yaml_key_line};

#[derive(Debug, Clone, PartialEq, Eq)]
struct IssueForm {
    name: String,
    description: Option<String>,
    labels: Vec<String>,
    body_items: Vec<IssueFormItem>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IssueFormItem {
    index: usize,
    kind: String,
    label: Option<String>,
    required: bool,
    line: u32,
}

pub(super) fn is_github_issue_form_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let Some(filename) = normalized.strip_prefix(".github/issue_template/") else {
        return false;
    };
    !filename.is_empty()
        && !filename.contains('/')
        && (filename.ends_with(".yml") || filename.ends_with(".yaml"))
}

pub(super) fn extract_github_issue_form(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    if !is_github_issue_form_path(&input.source.path) {
        return;
    }
    let Some(form) = parse_issue_form(content) else {
        return;
    };

    let stable_key = StableKey(format!(
        "issue-form://{}#{}",
        input.source.path,
        sanitize_key_fragment(&form.name)
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
        name: form.name.clone(),
        title: Some(format!("GitHub issue form {}", form.name)),
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(form.line),
            line_end: Some(form.line),
        }),
        language: Some(LanguageCode("yaml".to_string())),
        aliases: Vec::new(),
        ownership: ownership.clone(),
        payload: json!({
            "feature_kind": "github_issue_form",
            "name": form.name,
            "description": form.description,
            "labels": form.labels,
            "body_item_count": form.body_items.len(),
            "required_item_count": form.body_items.iter().filter(|item| item.required).count(),
            "item_kinds": form.body_items.iter().map(|item| item.kind.as_str()).collect::<Vec<_>>(),
        }),
    });

    facts.push(Fact {
        id: FactId(format!(
            "fact_github_issue_form_defined_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        )),
        kind: FactKind::SymbolDefined,
        subject: entity_id.clone(),
        object: Some(file_id.clone()),
        value: json!({
            "stable_key": stable_key.0,
            "path": input.source.path,
            "source_kind": "github_issue_form",
            "name": form.name,
            "body_item_count": form.body_items.len(),
        }),
        evidence: vec![evidence_for_file(
            &input.source.path,
            extractor,
            Some(form.line),
            Some(form.line),
        )],
        ownership,
        snapshot: input.snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    });

    for item in form.body_items {
        let item_key = StableKey(format!(
            "issue-form://{}#{}:item:{}",
            input.source.path, 
            sanitize_key_fragment(&form.name),
            item.index
        ));
        let item_id = EntityId(format!(
            "ent_feature_{:016x}",
            stable_hash(item_key.0.as_bytes())
        ));
        let item_ownership = ownership_for_file(&input.source.path);

        entities.push(Entity {
            id: item_id.clone(),
            stable_key: item_key.clone(),
            kind: EntityKind::Feature,
            name: item
                .label
                .clone()
                .unwrap_or_else(|| format!("item {}", item.index)),
            title: Some(format!("Issue form item {}", item.index)),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(item.line),
                line_end: Some(item.line),
            }),
            language: Some(LanguageCode("yaml".to_string())),
            aliases: Vec::new(),
            ownership: item_ownership.clone(),
            payload: json!({
                "feature_kind": "github_issue_form_item",
                "form": form.name,
                "index": item.index,
                "item_type": item.kind,
                "label": item.label,
                "required": item.required,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_github_issue_form_item_defined_{:016x}",
                stable_hash(item_key.0.as_bytes())
            )),
            kind: FactKind::SymbolDefined,
            subject: item_id,
            object: Some(file_id.clone()),
            value: json!({
                "stable_key": item_key.0,
                "path": input.source.path,
                "source_kind": "github_issue_form",
                "form": form.name,
                "index": item.index,
                "item_type": item.kind,
                "required": item.required,
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(item.line),
                Some(item.line),
            )],
            ownership: item_ownership,
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });
    }
}

fn parse_issue_form(content: &str) -> Option<IssueForm> {
    let root = serde_yaml_ng::from_str::<serde_json::Value>(content).ok()?;
    let object = root.as_object()?;
    let name = object.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    let description = object
        .get("description")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let labels = object
        .get("labels")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let body_items = object
        .get("body")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, value)| {
            let item = value.as_object()?;
            let kind = item.get("type")?.as_str()?.trim();
            if kind.is_empty() {
                return None;
            }
            let attributes = item.get("attributes").and_then(serde_json::Value::as_object);
            let label = attributes
                .and_then(|attributes| attributes.get("label"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            let required = item
                .get("validations")
                .and_then(serde_json::Value::as_object)
                .and_then(|validations| validations.get("required"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Some(IssueFormItem {
                index: index + 1,
                kind: kind.to_string(),
                label,
                required,
                line: yaml_key_line(content, "body").unwrap_or(1),
            })
        })
        .collect::<Vec<_>>();

    if body_items.is_empty() {
        return None;
    }

    Some(IssueForm {
        name: name.to_string(),
        description,
        labels,
        body_items,
        line: yaml_key_line(content, "name").unwrap_or(1),
    })
}

#[cfg(test)]
mod tests {
    use athanor_core::{Extractor, SourceFile};
    use athanor_domain::{EntityKind, FactKind, RepoId, SnapshotId};

    use super::*;
    use crate::OperationsExtractor;

    #[test]
    fn recognizes_only_issue_form_paths() {
        assert!(is_github_issue_form_path(
            ".github/ISSUE_TEMPLATE/bug_report.yml"
        ));
        assert!(is_github_issue_form_path(
            ".github/issue_template/feature_request.yaml"
        ));
        assert!(!is_github_issue_form_path(".github/dependabot.yml"));
        assert!(!is_github_issue_form_path(".github/ISSUE_TEMPLATE/config.yml"));
        assert!(!is_github_issue_form_path(".github/ISSUE_TEMPLATE/nested/form.yml"));
        assert!(!is_github_issue_form_path(".github/ISSUE_TEMPLATE/"));
        assert!(!is_github_issue_form_path("github/ISSUE_TEMPLATE/form.yml"));
    }

    #[test]
    fn rejects_empty_form_name() {
        assert!(parse_issue_form("name: \\nbody:\n  - type: textarea\n").is_none());
    }

    #[test]
    fn parses_bounded_form_structure() {
        let form = parse_issue_form(
            "name: Bug report\ndescription: Reproducible defect\nlabels: [bug]\nbody:\n  - type: textarea\n    attributes:\n      label: Reproduction\n    validations:\n      required: true\n  - type: input\n    attributes:\n      label: Version\n    validations:\n      required: true\n",
        )
        .unwrap();

        assert_eq!(form.name, "Bug report");
        assert_eq!(form.labels, vec!["bug"]);
        assert_eq!(form.body_items.len(), 2);
        assert_eq!(form.body_items[0].kind, "textarea");
        assert!(form.body_items[0].required);
        assert_eq!(form.body_items[1].label.as_deref(), Some("Version"));
        assert!(form.body_items[1].required);
    }

    #[tokio::test]
    async fn projects_form_and_items_with_evidence() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: ".github/ISSUE_TEMPLATE/bug_report.yml".to_string(),
                    language_hint: Some("yaml".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "name: Bug report\ndescription: Reproducible defect\nlabels: [bug]\nbody:\n  - type: textarea\n    attributes:\n      label: Reproduction\n    validations:\n      required: true\n".to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let forms = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Feature)
            .collect::<Vec<_>>();
        assert_eq!(forms.len(), 2);
        assert_eq!(output.facts.len(), 2);
        assert!(output
            .facts
            .iter()
            .all(|fact| fact.kind == FactKind::SymbolDefined && !fact.evidence.is_empty()));
    }
}
