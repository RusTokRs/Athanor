use std::collections::HashMap;

use athanor_domain::{Diagnostic, Entity, Relation, RelationKind};
use serde_json::Value;

use super::super::check::normalize_policy_path;

pub(crate) fn api_doc_path(editable_path: &str, endpoint: &Entity) -> String {
    let method = endpoint.payload["method"]
        .as_str()
        .unwrap_or("endpoint")
        .to_ascii_lowercase();
    let route = endpoint
        .payload
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or(endpoint.name.as_str());
    let slug = route
        .trim_matches('/')
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
    let slug = if slug.is_empty() {
        "root".to_string()
    } else {
        slug
    };
    format!(
        "{}/api/{}-{}.md",
        normalize_policy_path(editable_path),
        method,
        slug
    )
}

pub(crate) fn api_doc_content(
    snapshot: &str,
    endpoint: &Entity,
    diagnostic: &Diagnostic,
    path: &str,
    entities: &[Entity],
    relations: &[Relation],
) -> String {
    let method = endpoint.payload["method"]
        .as_str()
        .unwrap_or("UNKNOWN")
        .to_ascii_uppercase();
    let route = endpoint
        .payload
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or(endpoint.name.as_str());
    let summary = endpoint
        .payload
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("API endpoint");
    let context = api_doc_context(endpoint, entities, relations);
    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("id: doc://{path}\n"));
    content.push_str("kind: api_documentation\n");
    content.push_str("language: en\n");
    content.push_str("source_language: en\n");
    content.push_str("entities:\n");
    content.push_str(&format!("  - {}\n", endpoint.stable_key.0));
    content.push_str(&format!("last_verified_snapshot: {snapshot}\n"));
    content.push_str("status: verified\n");
    content.push_str("---\n\n");
    content.push_str(&format!("# {method} {route}\n\n{summary}\n\n"));
    content.push_str("## Contract\n\n");
    push_api_contract_lines(&mut content, endpoint, entities, relations);

    if let Some(handler) = context.handler {
        content.push_str("\n## Implementation\n\n");
        content.push_str(&format!("- Handler: `{}`\n", handler.stable_key.0));
        if let Some(source) = &handler.source {
            content.push_str(&format!("- Source: `{}`", source.path));
            if let Some(line) = source.line_start {
                content.push_str(&format!(":{line}"));
            }
            content.push('\n');
        }
    }
    if !context.request_schemas.is_empty() || !context.response_schemas.is_empty() {
        content.push_str("\n## Schemas\n\n");
        for schema in &context.request_schemas {
            content.push_str(&format!(
                "- Request: `{}` ({})\n",
                schema.schema.stable_key.0,
                schema.metadata()
            ));
        }
        for schema in &context.response_schemas {
            content.push_str(&format!(
                "- Response: `{}` ({})\n",
                schema.schema.stable_key.0,
                schema.metadata()
            ));
        }
    }
    if !context.examples.is_empty() {
        content.push_str("\n## Examples\n\n");
        for example in &context.examples {
            let direction = example.payload["direction"].as_str().unwrap_or("example");
            let media_type = example.payload["media_type"]
                .as_str()
                .unwrap_or("unknown media type");
            let name = example.payload["example_name"]
                .as_str()
                .unwrap_or(example.name.as_str());
            content.push_str(&format!(
                "- `{name}`: {direction} `{media_type}` via `{}`\n",
                example.stable_key.0
            ));
        }
    }
    content.push_str("\n## Evidence\n\n");
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
    content.push_str("\n## Notes\n\n");
    content.push_str(&format!(
        "Generated from diagnostic `{}`. Review this page before relying on it as user-facing documentation.\n",
        diagnostic.id.0
    ));
    content
}

pub(super) fn push_api_contract_lines(
    content: &mut String,
    endpoint: &Entity,
    entities: &[Entity],
    relations: &[Relation],
) {
    let method = endpoint.payload["method"]
        .as_str()
        .unwrap_or("UNKNOWN")
        .to_ascii_uppercase();
    let route = endpoint.payload["path"]
        .as_str()
        .unwrap_or(endpoint.name.as_str());
    content.push_str(&format!("- Method: `{method}`\n"));
    content.push_str(&format!("- Path: `{route}`\n"));
    if let Some(operation_id) = endpoint.payload["operation_id"].as_str() {
        content.push_str(&format!("- Operation ID: `{operation_id}`\n"));
    }
    let tags = string_array(&endpoint.payload["tags"]);
    if !tags.is_empty() {
        content.push_str(&format!("- Tags: {}\n", inline_code_list(&tags)));
    }
    let responses = string_array(&endpoint.payload["responses"]);
    if !responses.is_empty() {
        content.push_str(&format!(
            "- Declared responses: {}\n",
            inline_code_list(&responses)
        ));
    }
    if let Some(security) = endpoint
        .payload
        .get("security")
        .filter(|value| !value.is_null())
    {
        content.push_str(&format!("- Security: `{}`\n", compact_json(security)));
    }
    content.push_str(&format!(
        "- Canonical entity: `{}`\n",
        endpoint.stable_key.0
    ));

    let context = api_doc_context(endpoint, entities, relations);
    if let Some(handler) = context.handler {
        content.push_str(&format!("- Handler: `{}`", handler.stable_key.0));
        if let Some(source) = &handler.source {
            content.push_str(&format!(" from `{}`", source.path));
            if let Some(line) = source.line_start {
                content.push_str(&format!(":{line}"));
            }
        }
        content.push('\n');
    }
    for schema in &context.request_schemas {
        content.push_str(&format!(
            "- Request schema: `{}` ({})\n",
            schema.schema.stable_key.0,
            schema.metadata()
        ));
    }
    for schema in &context.response_schemas {
        content.push_str(&format!(
            "- Response schema: `{}` ({})\n",
            schema.schema.stable_key.0,
            schema.metadata()
        ));
    }
    for example in &context.examples {
        let direction = example.payload["direction"].as_str().unwrap_or("example");
        let media_type = example.payload["media_type"]
            .as_str()
            .unwrap_or("unknown media type");
        let name = example.payload["example_name"]
            .as_str()
            .unwrap_or(example.name.as_str());
        content.push_str(&format!(
            "- Example `{name}`: {direction} `{media_type}` via `{}`\n",
            example.stable_key.0
        ));
    }
}

pub(super) fn api_route_signature(endpoint: &Entity) -> Option<String> {
    let method = endpoint.payload["method"].as_str()?.to_ascii_uppercase();
    let path = endpoint.payload["path"].as_str()?;
    Some(format!("{method} {path}"))
}

struct ApiDocContext<'a> {
    handler: Option<&'a Entity>,
    request_schemas: Vec<ApiSchemaDocLink<'a>>,
    response_schemas: Vec<ApiSchemaDocLink<'a>>,
    examples: Vec<&'a Entity>,
}

struct ApiSchemaDocLink<'a> {
    schema: &'a Entity,
    media_type: Option<String>,
    status_code: Option<String>,
}

impl ApiSchemaDocLink<'_> {
    fn metadata(&self) -> String {
        let mut parts = Vec::new();
        if let Some(status_code) = &self.status_code {
            parts.push(format!("status `{status_code}`"));
        }
        if let Some(media_type) = &self.media_type {
            parts.push(format!("media `{media_type}`"));
        }
        if parts.is_empty() {
            "linked schema".to_string()
        } else {
            parts.join(", ")
        }
    }
}

fn api_doc_context<'a>(
    endpoint: &Entity,
    entities: &'a [Entity],
    relations: &[Relation],
) -> ApiDocContext<'a> {
    let by_id = entities
        .iter()
        .map(|entity| (&entity.id, entity))
        .collect::<HashMap<_, _>>();
    let mut context = ApiDocContext {
        handler: None,
        request_schemas: Vec::new(),
        response_schemas: Vec::new(),
        examples: Vec::new(),
    };
    for relation in relations {
        match relation.kind {
            RelationKind::ImplementedBy if relation.from == endpoint.id => {
                context.handler = by_id.get(&relation.to).copied();
            }
            RelationKind::SchemaForRequest if relation.from == endpoint.id => {
                if let Some(schema) = by_id.get(&relation.to).copied() {
                    context.request_schemas.push(ApiSchemaDocLink {
                        schema,
                        media_type: relation.payload["media_type"].as_str().map(str::to_string),
                        status_code: None,
                    });
                }
            }
            RelationKind::SchemaForResponse if relation.from == endpoint.id => {
                if let Some(schema) = by_id.get(&relation.to).copied() {
                    context.response_schemas.push(ApiSchemaDocLink {
                        schema,
                        media_type: relation.payload["media_type"].as_str().map(str::to_string),
                        status_code: relation.payload["status_code"].as_str().map(str::to_string),
                    });
                }
            }
            RelationKind::ExampleFor if relation.to == endpoint.id => {
                if let Some(example) = by_id.get(&relation.from).copied() {
                    context.examples.push(example);
                }
            }
            _ => {}
        }
    }
    context.request_schemas.sort_by(|left, right| {
        (&left.schema.stable_key.0, &left.media_type)
            .cmp(&(&right.schema.stable_key.0, &right.media_type))
    });
    context.response_schemas.sort_by(|left, right| {
        (
            &left.status_code,
            &left.schema.stable_key.0,
            &left.media_type,
        )
            .cmp(&(
                &right.status_code,
                &right.schema.stable_key.0,
                &right.media_type,
            ))
    });
    context
        .examples
        .sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
    context
}

fn string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn inline_code_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}
