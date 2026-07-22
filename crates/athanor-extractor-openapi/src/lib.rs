//! OpenAPI extractor boundary with canonical protocol identity normalization.

use async_trait::async_trait;
use athanor_core::{
    CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, InvalidationPolicy, SourceFile,
};
use athanor_domain::{Entity, EntityId, EntityKind, LanguageCode, SourceLocation, StableKey};
use athanor_extractor_basic::{ownership_for_file, stable_hash};
use serde_json::{Value, json};

mod implementation;

/// Extracts OpenAPI contracts and guarantees the shared API endpoint protocol discriminator.
#[derive(Debug, Clone, Default)]
pub struct OpenApiExtractor;

#[async_trait]
impl Extractor for OpenApiExtractor {
    fn name(&self) -> &str {
        "openapi"
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        let delegate = implementation::OpenApiExtractor;
        delegate.invalidation_policy()
    }

    fn supports(&self, source: &SourceFile) -> bool {
        let delegate = implementation::OpenApiExtractor;
        delegate.supports(source)
            || (!is_test_fixture_path(&source.path)
                && source
                    .content
                    .as_deref()
                    .is_some_and(has_parameter_components))
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let parameter_entities = repository_parameter_entities(&input.source);
        let delegate = implementation::OpenApiExtractor;
        let mut output = if delegate.supports(&input.source) {
            delegate.extract(input).await?
        } else {
            ExtractOutput::default()
        };
        output.entities.extend(parameter_entities);
        canonicalize_endpoint_protocol(&mut output)?;
        Ok(output)
    }
}

fn canonicalize_endpoint_protocol(output: &mut ExtractOutput) -> CoreResult<()> {
    for endpoint in output
        .entities
        .iter_mut()
        .filter(|entity| entity.kind == EntityKind::ApiEndpoint)
    {
        let payload = endpoint.payload.as_object_mut().ok_or_else(|| {
            CoreError::Adapter(format!(
                "OpenAPI endpoint {} has a non-object payload",
                endpoint.stable_key.0
            ))
        })?;
        match payload.get("protocol") {
            None => {
                payload.insert("protocol".to_string(), Value::String("openapi".to_string()));
            }
            Some(Value::String(protocol)) if protocol == "openapi" => {}
            Some(protocol) => {
                return Err(CoreError::Adapter(format!(
                    "OpenAPI endpoint {} has conflicting protocol identity {protocol}",
                    endpoint.stable_key.0
                )));
            }
        }
    }
    Ok(())
}

fn has_parameter_components(content: &str) -> bool {
    normalized_document(content)
        .and_then(|root| {
            root.get("components")?
                .get("parameters")?
                .as_object()
                .map(|parameters| !parameters.is_empty())
        })
        .unwrap_or(false)
}

fn repository_parameter_entities(source: &SourceFile) -> Vec<Entity> {
    let Some(content) = source.content.as_deref() else {
        return Vec::new();
    };
    let Some(root) = normalized_document(content) else {
        return Vec::new();
    };
    let Some(parameters) = root
        .get("components")
        .and_then(|components| components.get("parameters"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };

    parameters
        .iter()
        .filter_map(|(component_name, parameter)| {
            let normalized = normalize_parameter_component(parameter)?;
            let identity = format!("{}\0{component_name}", source.path);
            let hash = stable_hash(identity.as_bytes());
            let reference = format!("#/components/parameters/{component_name}");
            Some(Entity {
                id: EntityId(format!("ent_api_parameter_{hash:016x}")),
                stable_key: StableKey(format!("api-parameter://{}#{component_name}", source.path)),
                kind: EntityKind::ApiSchema,
                name: component_name.clone(),
                title: None,
                source: Some(SourceLocation {
                    path: source.path.clone(),
                    line_start: key_line(content, component_name),
                    line_end: key_line(content, component_name),
                }),
                language: Some(LanguageCode("openapi".to_string())),
                aliases: vec![reference.clone()],
                ownership: ownership_for_file(&source.path),
                payload: json!({
                    "protocol": "openapi",
                    "schema_kind": "parameter",
                    "reference": reference,
                    "parameter": normalized,
                }),
            })
        })
        .collect()
}

fn normalize_parameter_component(parameter: &Value) -> Option<Value> {
    let object = parameter.as_object()?;
    if object.contains_key("$ref") {
        return None;
    }
    let name = object.get("name")?.as_str()?;
    let location = object.get("in")?.as_str()?.to_ascii_lowercase();
    let required = location == "path"
        || object
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    Some(json!({
        "name": name,
        "location": location,
        "required": required,
        "schema": object.get("schema").cloned(),
    }))
}

fn normalized_document(content: &str) -> Option<Value> {
    serde_json::from_str::<Value>(content)
        .ok()
        .or_else(|| serde_yaml_ng::from_str::<Value>(content).ok())
}

fn is_test_fixture_path(path: &str) -> bool {
    path.split('/')
        .collect::<Vec<_>>()
        .windows(2)
        .any(|parts| matches!(parts, ["tests", "fixtures"] | ["test", "fixtures"]))
}

fn key_line(content: &str, key: &str) -> Option<u32> {
    content
        .lines()
        .position(|line| line.contains(key))
        .map(|index| (index + 1) as u32)
}

#[cfg(test)]
mod tests {
    use athanor_domain::{EntityKind, RepoId, SnapshotId};

    use super::*;

    #[tokio::test]
    async fn emitted_endpoints_have_canonical_openapi_protocol() {
        let output = OpenApiExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "service.openapi.json".to_string(),
                    language_hint: Some("json".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"{"openapi":"3.0.3","info":{"title":"API","version":"1"},"paths":{"/users/{id}":{"get":{"operationId":"getUser","responses":{"200":{"description":"ok"}}}}}}"#
                            .to_string(),
                    ),
                },
            })
            .await
            .expect("extract OpenAPI endpoint");

        let endpoints = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ApiEndpoint)
            .collect::<Vec<_>>();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].payload["protocol"], "openapi");
        assert_eq!(endpoints[0].payload["operation_id"], "getUser");
    }

    #[tokio::test]
    async fn partial_component_document_emits_repository_parameter_entities() {
        let source = SourceFile {
            path: "api/parameters.yaml".to_string(),
            language_hint: Some("yaml".to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(
                r#"
components:
  parameters:
    UserId:
      name: id
      in: path
      required: true
      schema:
        type: string
"#
                .to_string(),
            ),
        };
        assert!(OpenApiExtractor.supports(&source));
        let output = OpenApiExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source,
            })
            .await
            .expect("extract parameter component");
        let parameter = output
            .entities
            .iter()
            .find(|entity| entity.payload["schema_kind"] == "parameter")
            .expect("parameter entity");
        assert_eq!(parameter.name, "UserId");
        assert_eq!(parameter.payload["parameter"]["name"], "id");
        assert_eq!(parameter.payload["parameter"]["location"], "path");
        assert_eq!(parameter.payload["parameter"]["required"], true);
        assert_eq!(parameter.payload["parameter"]["schema"]["type"], "string");
    }
}
