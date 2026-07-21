//! OpenAPI extractor boundary with canonical protocol identity normalization.

use async_trait::async_trait;
use athanor_core::{
    CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, InvalidationPolicy, SourceFile,
};
use athanor_domain::EntityKind;
use serde_json::Value;

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
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let delegate = implementation::OpenApiExtractor;
        let mut output = delegate.extract(input).await?;
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
                payload.insert(
                    "protocol".to_string(),
                    Value::String("openapi".to_string()),
                );
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
}
