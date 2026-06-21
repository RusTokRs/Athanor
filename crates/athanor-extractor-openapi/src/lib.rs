use std::path::Path;

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde_json::{Map, Value, json};

mod parser;

use parser::{has_openapi_root_marker, parse_openapi_document};

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
        let document = parse_openapi_document(content, &input.source.path)?;
        let root = document.root.as_object().ok_or_else(|| {
            CoreError::Adapter(format!(
                "normalized OpenAPI document {} must have an object root",
                input.source.path
            ))
        })?;
        let version = document.version.as_str();
        let parser_backend = document.backend.name();
        let source_context = OpenApiSourceContext {
            path: &input.source.path,
            version,
            parser_backend,
        };

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
                        source_context,
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
                let entity = schema_entity(source_context, name, schema, line);
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

#[derive(Debug, Clone, Copy)]
struct OpenApiSourceContext<'a> {
    path: &'a str,
    version: &'a str,
    parser_backend: &'a str,
}

fn endpoint_entity(
    source: OpenApiSourceContext<'_>,
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
    let request_schemas = request_schema_references(operation);
    let response_schemas = response_schema_references(operation);
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
            path: source.path.to_string(),
            line_start: line,
            line_end: line,
        }),
        language: Some(LanguageCode("openapi".to_string())),
        aliases,
        ownership: ownership_for_file(source.path),
        payload: json!({
            "openapi_version": source.version,
            "parser_backend": source.parser_backend,
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
            "request_schemas": request_schemas,
            "responses": responses,
            "response_schemas": response_schemas,
            "security": operation.get("security").cloned(),
        }),
    }
}

fn request_schema_references(operation: &Map<String, Value>) -> Vec<Value> {
    let Some(content) = operation
        .get("requestBody")
        .and_then(Value::as_object)
        .and_then(|request_body| request_body.get("content"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };

    media_schema_references(content)
        .into_iter()
        .map(|(media_type, reference)| {
            json!({
                "media_type": media_type,
                "reference": reference,
            })
        })
        .collect()
}

fn response_schema_references(operation: &Map<String, Value>) -> Vec<Value> {
    let Some(responses) = operation.get("responses").and_then(Value::as_object) else {
        return Vec::new();
    };

    responses
        .iter()
        .flat_map(|(status_code, response)| {
            response
                .get("content")
                .and_then(Value::as_object)
                .map(media_schema_references)
                .unwrap_or_default()
                .into_iter()
                .map(move |(media_type, reference)| {
                    json!({
                        "status_code": status_code,
                        "media_type": media_type,
                        "reference": reference,
                    })
                })
        })
        .collect()
}

fn media_schema_references(content: &Map<String, Value>) -> Vec<(String, String)> {
    content
        .iter()
        .flat_map(|(media_type, media)| {
            let mut references = Vec::new();
            if let Some(schema) = media.get("schema") {
                collect_schema_references(schema, &mut references);
            }
            references
                .into_iter()
                .map(|reference| (media_type.clone(), reference))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn collect_schema_references(schema: &Value, references: &mut Vec<String>) {
    match schema {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str)
                && !references.iter().any(|existing| existing == reference)
            {
                references.push(reference.to_string());
            }
            for value in object.values() {
                collect_schema_references(value, references);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_schema_references(value, references);
            }
        }
        _ => {}
    }
}

fn schema_entity(
    source: OpenApiSourceContext<'_>,
    name: &str,
    schema: &Value,
    line: Option<u32>,
) -> Entity {
    let stable_key = StableKey(format!("api-schema://{}#{name}", source.path));

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
            path: source.path.to_string(),
            line_start: line,
            line_end: line,
        }),
        language: Some(LanguageCode("openapi".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file(source.path),
        payload: json!({
            "openapi_version": source.version,
            "parser_backend": source.parser_backend,
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

        assert_eq!(output.entities.len(), 5);
        assert_eq!(output.facts.len(), 5);
        let endpoint = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "api://POST:/login")
            .unwrap();
        assert_eq!(endpoint.stable_key.0, "api://POST:/login");
        assert_eq!(endpoint.name, "login");
        assert_eq!(endpoint.payload["responses"][0], "200");
        assert_eq!(
            endpoint.payload["request_schemas"][0]["reference"],
            "#/components/schemas/LoginRequest"
        );
        assert_eq!(
            endpoint.payload["response_schemas"][0]["reference"],
            "#/components/schemas/LoginResponse"
        );
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

    #[tokio::test]
    async fn preserves_canonical_contract_across_version_and_format_corpus() {
        let cases = [
            (
                "specs/compat-3.0.openapi.yaml",
                include_str!("../tests/fixtures/compat-3.0.yaml.fixture"),
                "3.0.3",
                "legacy-value",
            ),
            (
                "specs/compat-3.0.openapi.json",
                include_str!("../tests/fixtures/compat-3.0.json.fixture"),
                "3.0.3",
                "legacy-value",
            ),
            (
                "specs/compat-3.1.openapi.yaml",
                include_str!("../tests/fixtures/compat-3.1.yaml.fixture"),
                "3.1.0",
                "oas3",
            ),
            (
                "specs/compat-3.1.openapi.json",
                include_str!("../tests/fixtures/compat-3.1.json.fixture"),
                "3.1.0",
                "oas3",
            ),
            (
                "specs/compat-3.1.1.openapi.yaml",
                include_str!("../tests/fixtures/compat-3.1.1.yaml.fixture"),
                "3.1.1",
                "oas3",
            ),
            (
                "specs/compat-3.1.1.openapi.json",
                include_str!("../tests/fixtures/compat-3.1.1.json.fixture"),
                "3.1.1",
                "oas3",
            ),
        ];

        for (path, content, version, backend) in cases {
            let output = OpenApiExtractor
                .extract(input(path, content))
                .await
                .unwrap();
            let endpoint = output
                .entities
                .iter()
                .find(|entity| entity.kind == EntityKind::ApiEndpoint)
                .unwrap();
            let schema = output
                .entities
                .iter()
                .find(|entity| entity.kind == EntityKind::ApiSchema)
                .unwrap();

            assert_eq!(endpoint.stable_key.0, "api://POST:/compat");
            assert_eq!(endpoint.payload["openapi_version"], version);
            assert_eq!(endpoint.payload["parser_backend"], backend);
            assert_eq!(
                endpoint.payload["request_schemas"][0]["reference"],
                "#/components/schemas/CompatRequest"
            );
            assert_eq!(schema.name, "CompatRequest");
            assert!(schema.stable_key.0.ends_with("#CompatRequest"));
            assert_eq!(schema.payload["parser_backend"], backend);
            assert!(output.facts.iter().all(|fact| !fact.evidence.is_empty()));
        }
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
