use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, ProjectInput, Projector};
use athanor_domain::{Diagnostic, DiagnosticStatus, Entity};
use athanor_projector_support::{
    CanonicalProjectionPayload, publish_staged_directory, safe_filename, write_output_file,
};
use serde::Serialize;
use serde_json::json;

pub const WIKI_PROJECTION_SCHEMA: &str = "athanor.wiki_projection.v1";
pub const WIKI_MANIFEST_SCHEMA: &str = "athanor.wiki_manifest.v1";
pub const WIKI_FORMAT_VERSION: &str = "agent-wiki-v1";

pub type WikiProjectionPayload = CanonicalProjectionPayload;

#[derive(Debug, Default, Clone)]
pub struct MarkdownWikiProjector;

#[async_trait]
impl Projector for MarkdownWikiProjector {
    fn name(&self) -> &str {
        "markdown-wiki-projector"
    }

    async fn project(&self, input: ProjectInput) -> CoreResult<()> {
        let payload: WikiProjectionPayload = serde_json::from_value(input.payload)
            .map_err(|error| adapter_error(format!("invalid wiki projection payload: {error}")))?;
        if payload.schema != WIKI_PROJECTION_SCHEMA {
            return Err(adapter_error(format!(
                "unsupported wiki projection schema: {}",
                payload.schema
            )));
        }

        let target = PathBuf::from(input.target);
        write_projection(&target, &input.snapshot.0, payload)
    }
}

fn write_projection(
    target: &Path,
    snapshot: &str,
    mut payload: WikiProjectionPayload,
) -> CoreResult<()> {
    payload
        .entities
        .sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
    payload
        .facts
        .sort_by(|left, right| left.id.0.cmp(&right.id.0));
    payload
        .relations
        .sort_by(|left, right| left.id.0.cmp(&right.id.0));
    payload
        .diagnostics
        .sort_by(|left, right| left.id.0.cmp(&right.id.0));

    let entity_files = payload
        .entities
        .iter()
        .map(|entity| (entity.id.0.clone(), entity_filename(entity)))
        .collect::<HashMap<_, _>>();

    let open_diagnostics = payload
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .count();
    let manifest = json!({
        "schema": WIKI_MANIFEST_SCHEMA,
        "wiki_format_version": WIKI_FORMAT_VERSION,
        "snapshot": snapshot,
        "status": "complete",
        "entities": payload.entities.len(),
        "facts": payload.facts.len(),
        "relations": payload.relations.len(),
        "open_diagnostics": open_diagnostics,
    });
    let manifest = serde_json::to_string_pretty(&manifest)
        .map_err(|error| adapter_error(format!("serialize wiki manifest: {error}")))?;

    publish_staged_directory(target, "wiki", |staging| {
        write_output_file(
            &staging.join("index.md"),
            &render_index(snapshot, &payload, &entity_files),
        )?;
        for entity in &payload.entities {
            write_output_file(
                &staging.join("entities").join(entity_filename(entity)),
                &render_entity(snapshot, entity, &payload, &entity_files),
            )?;
        }
        for diagnostic in payload
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        {
            write_output_file(
                &staging
                    .join("diagnostics")
                    .join(diagnostic_filename(diagnostic)),
                &render_diagnostic(snapshot, diagnostic, &entity_files),
            )?;
        }
        write_output_file(&staging.join("manifest.json"), &manifest)
    })
}

fn render_index(
    snapshot: &str,
    payload: &WikiProjectionPayload,
    entity_files: &HashMap<String, String>,
) -> String {
    let open = payload
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .collect::<Vec<_>>();
    let mut output = frontmatter("wiki://index", "index", snapshot);
    output.push_str("# Athanor project wiki\n\n");
    output.push_str(&format!(
        "Canonical snapshot `{snapshot}` contains {} entities, {} facts, {} relations, and {} open diagnostics.\n\n",
        payload.entities.len(),
        payload.facts.len(),
        payload.relations.len(),
        open.len()
    ));
    output.push_str("## Entities\n\n");
    for entity in &payload.entities {
        let file = &entity_files[&entity.id.0];
        output.push_str(&format!(
            "- [{}](entities/{file}) — `{}`\n",
            inline_text(entity.title.as_deref().unwrap_or(&entity.name)),
            entity.stable_key.0
        ));
    }
    output.push_str("\n## Open diagnostics\n\n");
    if open.is_empty() {
        output.push_str("No open diagnostics.\n");
    } else {
        for diagnostic in open {
            output.push_str(&format!(
                "- [{}](diagnostics/{}) — `{}`\n",
                inline_text(&diagnostic.title),
                diagnostic_filename(diagnostic),
                serialized_name(&diagnostic.severity)
            ));
        }
    }
    output
}

fn render_entity(
    snapshot: &str,
    entity: &Entity,
    payload: &WikiProjectionPayload,
    entity_files: &HashMap<String, String>,
) -> String {
    let mut output = frontmatter(
        &entity.stable_key.0,
        &serialized_name(&entity.kind),
        snapshot,
    );
    output.push_str(&format!(
        "# {}\n\nStable key: `{}`\n\n",
        heading_text(entity.title.as_deref().unwrap_or(&entity.name)),
        entity.stable_key.0
    ));
    if let Some(source) = &entity.source {
        output.push_str(&format!(
            "Source: `{}`\n\n",
            source_location(&source.path, source.line_start)
        ));
    }

    output.push_str("## Facts\n\n");
    let facts = payload
        .facts
        .iter()
        .filter(|fact| fact.subject == entity.id || fact.object.as_ref() == Some(&entity.id))
        .collect::<Vec<_>>();
    if facts.is_empty() {
        output.push_str("No canonical facts.\n");
    } else {
        for fact in facts {
            output.push_str(&format!(
                "- `{}` (confidence {:.2})\n",
                serialized_name(&fact.kind),
                fact.confidence
            ));
        }
    }

    output.push_str("\n## Relations\n\n");
    let relations = payload
        .relations
        .iter()
        .filter(|relation| relation.from == entity.id || relation.to == entity.id)
        .collect::<Vec<_>>();
    if relations.is_empty() {
        output.push_str("No canonical relations.\n");
    } else {
        for relation in relations {
            let (direction, related_id) = if relation.from == entity.id {
                ("outgoing", &relation.to.0)
            } else {
                ("incoming", &relation.from.0)
            };
            let related = entity_files.get(related_id).map_or_else(
                || format!("`{related_id}`"),
                |file| format!("[{related_id}]({file})"),
            );
            output.push_str(&format!(
                "- {direction} `{}` → {related} (`{}`, confidence {:.2})\n",
                serialized_name(&relation.kind),
                serialized_name(&relation.status),
                relation.confidence
            ));
        }
    }

    output.push_str("\n## Open diagnostics\n\n");
    let diagnostics = payload.diagnostics.iter().filter(|diagnostic| {
        diagnostic.status == DiagnosticStatus::Open && diagnostic.entities.contains(&entity.id)
    });
    let mut found = false;
    for diagnostic in diagnostics {
        found = true;
        output.push_str(&format!(
            "- [{}](../diagnostics/{}) — `{}`\n",
            inline_text(&diagnostic.title),
            diagnostic_filename(diagnostic),
            serialized_name(&diagnostic.severity)
        ));
    }
    if !found {
        output.push_str("No open diagnostics.\n");
    }
    output
}

fn render_diagnostic(
    snapshot: &str,
    diagnostic: &Diagnostic,
    entity_files: &HashMap<String, String>,
) -> String {
    let mut output = frontmatter(
        &diagnostic.id.0,
        &serialized_name(&diagnostic.kind),
        snapshot,
    );
    output.push_str(&format!(
        "# {}\n\nSeverity: `{}`  \nStatus: `{}`\n\n{}\n\n",
        heading_text(&diagnostic.title),
        serialized_name(&diagnostic.severity),
        serialized_name(&diagnostic.status),
        diagnostic.message
    ));
    output.push_str("## Entities\n\n");
    if diagnostic.entities.is_empty() {
        output.push_str("No attached entities.\n");
    } else {
        for entity in &diagnostic.entities {
            let rendered = entity_files.get(&entity.0).map_or_else(
                || format!("`{}`", entity.0),
                |file| format!("[{}](../entities/{file})", entity.0),
            );
            output.push_str(&format!("- {rendered}\n"));
        }
    }
    output.push_str("\n## Evidence\n\n");
    let mut evidence_found = false;
    for evidence in &diagnostic.evidence {
        if let Some(source_file) = &evidence.source_file {
            evidence_found = true;
            output.push_str(&format!(
                "- `{}` (confidence {:.2})\n",
                source_location(source_file, evidence.line_start),
                evidence.confidence
            ));
        }
    }
    if !evidence_found {
        output.push_str("No file evidence.\n");
    }
    if let Some(fix) = &diagnostic.suggested_fix {
        output.push_str(&format!("\n## Suggested fix\n\n{}\n", fix));
    }
    output
}

fn frontmatter(id: &str, kind: &str, snapshot: &str) -> String {
    format!(
        "---\nid: {}\nkind: {}\nlanguage: neutral\nsnapshot: {}\n---\n\n",
        yaml_string(id),
        yaml_string(kind),
        yaml_string(snapshot)
    )
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

fn entity_filename(entity: &Entity) -> String {
    format!("{}.md", safe_filename(&entity.id.0))
}

fn diagnostic_filename(diagnostic: &Diagnostic) -> String {
    format!("{}.md", safe_filename(&diagnostic.id.0))
}

fn serialized_name(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn source_location(path: &str, line: Option<u32>) -> String {
    line.map_or_else(|| path.to_string(), |line| format!("{path}:{line}"))
}

fn heading_text(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_string()
}

fn inline_text(value: &str) -> String {
    heading_text(value).replace('[', "\\[").replace(']', "\\]")
}

fn adapter_error(message: String) -> CoreError {
    CoreError::Adapter(message)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use athanor_domain::{DiagnosticId, EntityId, EntityKind, Severity, SnapshotId, StableKey};

    use super::*;

    #[tokio::test]
    async fn writes_and_replaces_deterministic_wiki() {
        let root = std::env::temp_dir().join(format!(
            "athanor-wiki-projector-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("wiki");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("stale.md"), "stale").unwrap();

        let entity = Entity {
            id: EntityId("ent_auth".to_string()),
            stable_key: StableKey("api://POST:/login".to_string()),
            kind: EntityKind::ApiEndpoint,
            name: "login".to_string(),
            title: Some("Login".to_string()),
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        };
        let diagnostic = Diagnostic {
            id: DiagnosticId("diag_login_docs".to_string()),
            kind: athanor_domain::DiagnosticKind::ApiEndpointImplementedButNotDocumented,
            severity: Severity::Medium,
            status: DiagnosticStatus::Open,
            title: "Login is not documented".to_string(),
            message: "Add endpoint documentation.".to_string(),
            entities: vec![entity.id.clone()],
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            suggested_fix: Some("Document the endpoint.".to_string()),
            payload: json!({}),
        };
        let payload = WikiProjectionPayload {
            schema: WIKI_PROJECTION_SCHEMA.to_string(),
            entities: vec![entity],
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: vec![diagnostic],
        };
        let projector = MarkdownWikiProjector;
        projector
            .project(ProjectInput {
                snapshot: SnapshotId("snap_test".to_string()),
                target: target.to_string_lossy().into_owned(),
                payload: serde_json::to_value(payload).unwrap(),
            })
            .await
            .unwrap();

        assert!(!target.join("stale.md").exists());
        assert!(target.join("index.md").is_file());
        assert!(target.join("entities/ent_auth.md").is_file());
        assert!(target.join("diagnostics/diag_login_docs.md").is_file());
        let index = fs::read_to_string(target.join("index.md")).unwrap();
        assert!(index.contains("api://POST:/login"));
        assert!(index.contains("snap_test"));
        let manifest = fs::read_to_string(target.join("manifest.json")).unwrap();
        assert!(manifest.contains(WIKI_MANIFEST_SCHEMA));
        assert!(manifest.contains("\"open_diagnostics\": 1"));

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn rejects_unknown_payload_schema() {
        let error = MarkdownWikiProjector
            .project(ProjectInput {
                snapshot: SnapshotId("snap_test".to_string()),
                target: "unused".to_string(),
                payload: json!({
                    "schema": "unknown",
                    "entities": [],
                    "facts": [],
                    "relations": [],
                    "diagnostics": []
                }),
            })
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("unsupported wiki projection schema")
        );
    }
}
