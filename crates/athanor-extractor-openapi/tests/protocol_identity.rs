use athanor_core::{ExtractInput, Extractor, SourceFile};
use athanor_domain::{EntityKind, RepoId, SnapshotId};
use athanor_extractor_openapi::OpenApiExtractor;

#[tokio::test]
async fn public_extractor_emits_canonical_openapi_protocol() {
    let output = OpenApiExtractor
        .extract(ExtractInput {
            repo: RepoId("repo_protocol_contract".to_string()),
            snapshot: SnapshotId("snap_protocol_contract".to_string()),
            source: SourceFile {
                path: "contract.openapi.json".to_string(),
                language_hint: Some("json".to_string()),
                content_hash: Some("hash".to_string()),
                content: Some(
                    r#"{"openapi":"3.0.3","info":{"title":"Contract","version":"1"},"paths":{"/users/{id}":{"get":{"operationId":"getUser","responses":{"200":{"description":"ok"}}}}}}"#
                        .to_string(),
                ),
            },
        })
        .await
        .expect("extract public OpenAPI contract");

    let endpoint = output
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::ApiEndpoint)
        .expect("OpenAPI endpoint");
    assert_eq!(endpoint.stable_key.0, "api://GET:/users/{id}");
    assert_eq!(endpoint.payload["protocol"], "openapi");
    assert_eq!(endpoint.payload["operation_id"], "getUser");
}
