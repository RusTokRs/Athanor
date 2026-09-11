use athanor_core::{ExtractInput, Extractor, SourceFile};
use athanor_domain::{EntityKind, RepoId, SnapshotId};
use athanor_extractor_openapi::OpenApiExtractor;

const EXAMPLE: &str = include_str!("../../../docs/examples/auth.openapi.yaml");

#[tokio::test]
async fn first_party_openapi_example_produces_api_profile_entities() {
    let output = OpenApiExtractor
        .extract(ExtractInput {
            repo: RepoId("repo_test".to_string()),
            snapshot: SnapshotId("snap_test".to_string()),
            source: SourceFile {
                path: "docs/examples/auth.openapi.yaml".to_string(),
                language_hint: Some("yaml".to_string()),
                content_hash: Some("auth-example".to_string()),
                content: Some(EXAMPLE.to_string()),
            },
        })
        .await
        .expect("extract first-party OpenAPI example");

    let endpoints = output
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::ApiEndpoint)
        .collect::<Vec<_>>();
    let schemas = output
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::ApiSchema)
        .collect::<Vec<_>>();
    let examples = output
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::ApiExample)
        .collect::<Vec<_>>();

    assert_eq!(endpoints.len(), 1);
    assert_eq!(endpoints[0].payload["protocol"], "openapi");
    assert_eq!(endpoints[0].payload["operation_id"], "login");
    assert_eq!(schemas.len(), 2);
    assert_eq!(examples.len(), 2);
}
