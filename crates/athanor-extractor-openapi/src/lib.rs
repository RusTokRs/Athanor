use std::path::Path;

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde_json::{Map, Value, json};

const HTTP_METHODS: &[&str] = &[
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

#[derive(Debug, Clone, Default)]
pub struct OpenApiExtractor;

#[async_trait]
impl Extractor for OpenApiExtractor {
    fn name(&self) -> &'static str {
        "openapi"
    }

    fn supports(&self, source: &SourceFile) -> bool {
        is_openapi_file_name(&source.path)
            || (matches!(source.language_hint.as_deref(), Some("yaml" | "json"))
                && source
                    .content
                    .as_deref()
                    .is_some_and(has_openapi_root_marker))
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };
        let document = parse_document(content, &input.source.path)?;
        let root = document.as_object().ok_or_else(|| {
            CoreError::Adapter(format!(
                "OpenAPI document {} must have an object root",
                input.source.path
            ))
        })?;
        let version = root.get("openapi").and_then(Value::as_str).ok_or_else(|| {
            if let Some(swagger) = root.get("swagger").and_then(Value::as_str) {
                CoreError::Adapter(format!(
                    "OpenAPI document {} uses unsupported Swagger version {swagger}; expected OpenAPI 3.x",
                    input.source.path
                ))
            } else {
                CoreError::Adapter(format!(
                    "OpenAPI document {} is missing the openapi version",
                    input.source.path
                ))
            }
        })?;
        if !version.starts_with("3.") {
            return Err(CoreError::Adapter(format!(
                "OpenAPI document {} uses unsupported version {version}; expected 3.x",
                input.source.path
            )));
        }

        let file_id = file_entity(&input.source, &input.snapshot.0).id;
        let mut entities = Vec::new();
        let mut facts = Vec::new();

        if let Some(paths) = root.get("paths").and_then(Value::as_object) {
            for (path, path_item) in paths {
                let Some(path_item) = path_item.as_object() else {
                    continue;
                };
                let path_parameter_count = array_len(path_item.get("parameters"));
                for method in HTTP_METHODS {
                    let Some(operation) = path_item.get(*method).and_then(Value::as_object) else {
                        continue;
                    };
                    let line = operation_line(content, path, method);
                    let endpoint = endpoint_entity(
                        &input.source.path,
                        version,
                        method,
                        path,
                        operation,
                        path_parameter_count,
                        line,
                    );
                    facts.push(declaration_fact(
                        self.name(),
                        &input,
                        &endpoint,
                        &file_id,
                        FactKind::RouteDeclared,
                        "route",
                        line,
                    ));
                    entities.push(endpoint);
                }
            }
        }

        if let Some(schemas) = root
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
        {
            for (name, schema) in schemas {
                let line = key_line(content, name);
                let entity = schema_entity(&input.source.path, version, name, schema, line);
                facts.push(declaration_fact(
                    self.name(),
                    &input,
                    &entity,
                    &file_id,
                    FactKind::Other("api_schema_declared".to_string()),
                    "schema",
                    line,
                ));
                entities.push(entity);
            }
        }

        Ok(ExtractOutput { entities, facts })
    }
}

fn endpoint_entity(
    source_path: &str,
    version: &str,
    method: &str,
    path: &str,
    operation: &Map<String, Value>,
    path_parameter_count: usize,
    line: Option<u32>,
) -> Entity {
    let method = method.to_uppercase();
    let stable_key = StableKey(format!("api://{method}:{path}"));
    let operation_id = string_value(operation.get("operationId"));
    let summary = string_value(operation.get("summary"));
    let tags = string_array(operation.get("tags"));
    let responses = operation
        .get("responses")
        .and_then(Value::as_object)
        .map(|responses| responses.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let aliases = operation_id.iter().cloned().collect();

    Entity {
        id: EntityId(format!(
            "ent_api_endpoint_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        )),
        stable_key,
        kind: EntityKind::ApiEndpoint,
        name: operation_id
            .clone()
            .unwrap_or_else(|| format!("{method} {path}")),
        title: summary.clone(),
        source: Some(SourceLocation {
            path: source_path.to_string(),
            line_start: line,
            line_end: line,
        }),
        language: Some(LanguageCode("openapi".to_string())),
        aliases,
        ownership: ownership_for_file(source_path),
        payload: json!({
            "openapi_version": version,
            "method": method,
            "path": path,
            "operation_id": operation_id,
            "summary": summary,
            "description": string_value(operation.get("description")),
            "tags": tags,
            "deprecated": operation.get("deprecated").and_then(Value::as_bool).unwrap_or(false),
            "operation_parameter_count": array_len(operation.get("parameters")),
            "path_parameter_count": path_parameter_count,
            "has_request_body": operation.contains_key("requestBody"),
            "responses": responses,
            "security": operation.get("security").cloned(),
        }),
    }
}

fn schema_entity(
    source_path: &str,
    version: &str,
    name: &str,
    schema: &Value,
    line: Option<u32>,
) -> Entity {
    let stable_key = StableKey(format!("api-schema://{source_path}#{name}"));

    Entity {
        id: EntityId(format!(
            "ent_api_schema_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        )),
        stable_key,
        kind: EntityKind::ApiSchema,
        name: name.to_string(),
        title: schema
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string),
        source: Some(SourceLocation {
            path: source_path.to_string(),
            line_start: line,
            line_end: line,
        }),
        language: Some(LanguageCode("openapi".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file(source_path),
        payload: json!({
            "openapi_version": version,
            "schema": schema,
        }),
    }
}

fn declaration_fact(
    extractor: &str,
    input: &ExtractInput,
    entity: &Entity,
    file_id: &EntityId,
    kind: FactKind,
    declaration_kind: &str,
    line: Option<u32>,
) -> Fact {
    let id_material = format!("{}\0{}", entity.stable_key.0, input.source.path);
    Fact {
        id: FactId(format!(
            "fact_openapi_declared_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        subject: entity.id.clone(),
        object: Some(file_id.clone()),
        value: json!({
            "declaration_kind": declaration_kind,
            "stable_key": entity.stable_key.0,
            "path": input.source.path,
        }),
        evidence: vec![evidence_for_file(&input.source.path, extractor, line, line)],
        ownership: ownership_for_file(&input.source.path),
        snapshot: input.snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    }
}

fn parse_document(content: &str, path: &str) -> CoreResult<Value> {
    let yaml = serde_yaml::from_str::<serde_yaml::Value>(content).map_err(|error| {
        CoreError::Adapter(format!("failed to parse OpenAPI document {path}: {error}"))
    })?;
    serde_json::to_value(yaml).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to normalize OpenAPI document {path}: {error}"
        ))
    })
}

fn is_openapi_file_name(path: &str) -> bool {
    let Some(name) = Path::new(path).file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "openapi.yaml"
            | "openapi.yml"
            | "openapi.json"
            | "swagger.yaml"
            | "swagger.yml"
            | "swagger.json"
    ) || name.contains(".openapi.")
}

fn has_openapi_root_marker(content: &str) -> bool {
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(content) else {
        return false;
    };
    value.as_mapping().is_some_and(|mapping| {
        mapping
            .keys()
            .any(|key| key.as_str().is_some_and(|key| key == "openapi"))
    })
}

fn operation_line(content: &str, path: &str, method: &str) -> Option<u32> {
    let path_index = content.lines().position(|line| line.contains(path))?;
    content
        .lines()
        .enumerate()
        .skip(path_index)
        .find(|(_, line)| {
            let line = line.trim_start();
            line.starts_with(&format!("{method}:")) || line.starts_with(&format!("\"{method}\""))
        })
        .map(|(index, _)| (index + 1) as u32)
        .or(Some((path_index + 1) as u32))
}

fn key_line(content: &str, key: &str) -> Option<u32> {
    content
        .lines()
        .position(|line| line.contains(key))
        .map(|index| (index + 1) as u32)
}

fn string_value(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn array_len(value: Option<&Value>) -> usize {
    value.and_then(Value::as_array).map_or(0, Vec::len)
}

#[cfg(test)]
mod tests {
    use athanor_domain::{FactKind, RepoId, SnapshotId};

    use super::*;

    #[tokio::test]
    async fn extracts_yaml_operations_and_component_schemas() {
        let output = OpenApiExtractor
            .extract(input(
                "openapi.yaml",
                include_str!("../tests/fixtures/basic.openapi.yaml"),
            ))
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 4);
        assert_eq!(output.facts.len(), 4);
        let endpoint = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "api://POST:/login")
            .unwrap();
        assert_eq!(endpoint.stable_key.0, "api://POST:/login");
        assert_eq!(endpoint.name, "login");
        assert_eq!(endpoint.payload["responses"][0], "200");
        assert!(endpoint.source.as_ref().unwrap().line_start.is_some());
        assert!(output.facts.iter().all(|fact| {
            !fact.evidence.is_empty()
                && !fact.ownership.is_empty()
                && fact
                    .object
                    .as_ref()
                    .is_some_and(|id| id.0.starts_with("ent_file_"))
        }));
        assert!(
            output
                .facts
                .iter()
                .any(|fact| fact.kind == FactKind::RouteDeclared)
        );
    }

    #[tokio::test]
    async fn extracts_json_openapi_documents() {
        let output = OpenApiExtractor
            .extract(input(
                "specs/service.openapi.json",
                r#"{"openapi":"3.0.3","info":{"title":"API","version":"1"},"paths":{"/health":{"get":{"responses":{"204":{"description":"OK"}}}}}}"#,
            ))
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.entities[0].stable_key.0, "api://GET:/health");
        assert_eq!(output.entities[0].name, "GET /health");
    }

    #[test]
    fn ignores_unrelated_yaml_and_json_files() {
        let source = SourceFile {
            path: "config.yaml".to_string(),
            language_hint: Some("yaml".to_string()),
            content_hash: None,
            content: Some("name: example".to_string()),
        };

        assert!(!OpenApiExtractor.supports(&source));
    }

    #[tokio::test]
    async fn rejects_unsupported_openapi_versions() {
        let error = OpenApiExtractor
            .extract(input(
                "swagger.yaml",
                "swagger: '2.0'\ninfo: { title: API, version: '1' }\npaths: {}",
            ))
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("unsupported Swagger version 2.0")
        );
    }

    fn input(path: &str, content: &str) -> ExtractInput {
        ExtractInput {
            repo: RepoId("repo_test".to_string()),
            snapshot: SnapshotId("snap_test".to_string()),
            source: SourceFile {
                path: path.to_string(),
                language_hint: Some("yaml".to_string()),
                content_hash: Some("hash".to_string()),
                content: Some(content.to_string()),
            },
        }
    }
}
