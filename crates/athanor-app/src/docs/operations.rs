use athanor_domain::{Diagnostic, Entity, EntityKind};

use super::check::normalize_policy_path;

pub(super) fn env_doc_path(editable_path: &str, env_var: &Entity) -> String {
    let name = env_var
        .stable_key
        .0
        .strip_prefix("env://")
        .unwrap_or(env_var.name.as_str());
    let slug = slug_for_path(name, "variable");
    format!(
        "{}/operations/env-{}.md",
        normalize_policy_path(editable_path),
        slug
    )
}

pub(super) fn env_doc_content(
    snapshot: &str,
    env_var: &Entity,
    diagnostic: &Diagnostic,
    path: &str,
) -> String {
    let name = env_var
        .stable_key
        .0
        .strip_prefix("env://")
        .unwrap_or(env_var.name.as_str());
    let mut content = String::new();
    push_frontmatter(
        &mut content,
        path,
        "operations_documentation",
        &env_var.stable_key.0,
        snapshot,
    );
    content.push_str(&format!("# Environment Variable `{name}`\n\n"));
    content.push_str("## Purpose\n\n");
    content.push_str("Document the runtime purpose, expected format, and default behavior for this environment variable.\n\n");
    content.push_str("## Contract\n\n");
    content.push_str(&format!("- Variable: `{name}`\n"));
    content.push_str(&format!(
        "- Canonical entity: `{}`\n\n",
        env_var.stable_key.0
    ));
    push_evidence(&mut content, diagnostic);
    content.push_str("\n## Notes\n\n");
    content.push_str(&format!(
        "Generated from diagnostic `{}`. Review this page before relying on it as operational documentation.\n",
        diagnostic.id.0
    ));
    content
}

pub(super) fn operation_doc_diagnostic_shape(
    diagnostic: &Diagnostic,
) -> Option<(EntityKind, &'static str, &'static str, &'static str)> {
    match diagnostic
        .payload
        .get("scope")
        .and_then(serde_json::Value::as_str)
    {
        Some("scripts") => Some((
            EntityKind::ScriptCommand,
            "script_command",
            "script",
            "Script Command",
        )),
        Some("deployment") => Some((
            EntityKind::DockerService,
            "deployment",
            "deployment",
            "Deployment Resource",
        )),
        Some("env") => Some((
            EntityKind::Feature,
            "config_key",
            "config",
            "Runtime Config Key",
        )),
        Some("runbooks") => Some((EntityKind::Runbook, "runbook", "runbook", "Runbook")),
        _ => None,
    }
}

pub(super) fn operation_doc_path(
    editable_path: &str,
    prefix: &str,
    entity: &Entity,
) -> String {
    let slug = slug_for_path(&entity.stable_key.0, entity.name.as_str());
    format!(
        "{}/operations/{}-{}.md",
        normalize_policy_path(editable_path),
        prefix,
        slug
    )
}

pub(super) fn operation_doc_content(
    snapshot: &str,
    entity: &Entity,
    diagnostic: &Diagnostic,
    path: &str,
    title_prefix: &str,
) -> String {
    let title = if entity.kind == EntityKind::Feature {
        entity.name.as_str()
    } else {
        entity.title.as_deref().unwrap_or(entity.name.as_str())
    };
    let mut content = String::new();
    push_frontmatter(
        &mut content,
        path,
        "operations_documentation",
        &entity.stable_key.0,
        snapshot,
    );
    content.push_str(&format!("# {title_prefix} `{title}`\n\n"));
    content.push_str("## Purpose\n\n");
    content.push_str("Document when this operational item is used, who owns it, and what successful execution or operation means.\n\n");
    content.push_str("## Contract\n\n");
    content.push_str(&format!("- Canonical entity: `{}`\n", entity.stable_key.0));
    content.push_str(&format!(
        "- Entity kind: `{}`\n",
        serialized_entity_kind(&entity.kind)
    ));
    if let Some(source) = &entity.source {
        content.push_str(&format!("- Source: `{}`", source.path));
        if let Some(line) = source.line_start {
            content.push_str(&format!(":{line}"));
        }
        content.push('\n');
    }
    content.push_str("\n## Procedure\n\n");
    if entity.kind == EntityKind::Runbook {
        content.push_str("1. Review the runbook frontmatter and confirm operational targets.\n");
        content.push_str("2. Add ordered operation steps that Athanor can extract.\n");
        content.push_str(
            "3. Re-index and verify `ath check runbooks` no longer reports this diagnostic.\n\n",
        );
    } else {
        content.push_str("1. Review the source definition and confirm prerequisites.\n");
        content.push_str("2. Execute or operate this item according to the project runbook.\n");
        content.push_str("3. Record outcomes, rollback notes, and follow-up actions.\n\n");
    }
    push_evidence(&mut content, diagnostic);
    content.push_str("\n## Notes\n\n");
    content.push_str(&format!(
        "Generated from diagnostic `{}`. Review this page before relying on it as operational documentation.\n",
        diagnostic.id.0
    ));
    content
}

fn slug_for_path(input: &str, fallback: &str) -> String {
    let slug = input
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug
    }
}

fn serialized_entity_kind(kind: &EntityKind) -> &'static str {
    match kind {
        EntityKind::ScriptCommand => "script_command",
        EntityKind::DockerService => "docker_service",
        EntityKind::EnvVar => "env_var",
        EntityKind::Feature => "feature",
        EntityKind::Runbook => "runbook",
        EntityKind::OperationStep => "operation_step",
        _ => "other",
    }
}

fn push_frontmatter(
    content: &mut String,
    path: &str,
    kind: &str,
    stable_key: &str,
    snapshot: &str,
) {
    content.push_str("---\n");
    content.push_str(&format!("id: doc://{path}\n"));
    content.push_str(&format!("kind: {kind}\n"));
    content.push_str("language: en\n");
    content.push_str("source_language: en\n");
    content.push_str("entities:\n");
    content.push_str(&format!("  - {stable_key}\n"));
    content.push_str(&format!("last_verified_snapshot: {snapshot}\n"));
    content.push_str("status: verified\n");
    content.push_str("---\n\n");
}

fn push_evidence(content: &mut String, diagnostic: &Diagnostic) {
    content.push_str("## Evidence\n\n");
    if diagnostic.evidence.is_empty() {
        content.push_str("- `unknown source`\n");
    } else {
        for evidence in &diagnostic.evidence {
            let source = evidence.source_file.as_deref().unwrap_or("unknown source");
            let line = evidence
                .line_start
                .map_or_else(String::new, |line| format!(":{line}"));
            content.push_str(&format!("- `{source}{line}`\n"));
        }
    }
}
