use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use athanor_app::{
    DocumentationApiPublicationOptions, DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationProfile, inspect_documentation_api_current, inspect_documentation_api_manifest,
    inspect_documentation_api_validation, inspect_documentation_architecture_current,
    inspect_documentation_module_current, publish_documentation_api_generation,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{Entity, EntityId, EntityKind, Ownership, SnapshotId, SourceLocation, StableKey};
use serde_json::json;

#[test]
fn api_current_manifest_and_validation_are_bounded_and_checksum_verified() {
    let project = TempProject::new("valid");
    let request = api_request();
    let publication = publish_documentation_api_generation(
        DocumentationApiPublicationOptions {
            root: project.root.clone(),
            force: false,
        },
        &request,
        &snapshot(),
    )
    .unwrap();

    let current = inspect_documentation_api_current(&project.root).unwrap();
    assert_eq!(current.current.generation, publication.generation);
    assert_eq!(current.current.snapshot, request.snapshot);
    assert_eq!(current.current.profile, DocumentationProfile::Api);

    let manifest = inspect_documentation_api_manifest(&project.root).unwrap();
    assert_eq!(manifest.current, current.current);
    assert_eq!(manifest.manifest.documents.len(), 2);
    assert_eq!(manifest.generation_dir, publication.generation_dir);

    let validation = inspect_documentation_api_validation(&project.root).unwrap();
    assert_eq!(validation.current, current.current);
    assert_eq!(validation.report.snapshot, request.snapshot);
    assert_eq!(validation.report.profile, DocumentationProfile::Api);
    assert_eq!(validation.validation_path, publication.validation_report);
}

#[test]
fn api_inspection_rejects_pointer_layout_and_artifact_checksum_drift() {
    let project = TempProject::new("drift");
    let request = api_request();
    let publication = publish_documentation_api_generation(
        DocumentationApiPublicationOptions {
            root: project.root.clone(),
            force: false,
        },
        &request,
        &snapshot(),
    )
    .unwrap();

    let mut current: serde_json::Value =
        serde_json::from_slice(&fs::read(&publication.current_pointer).unwrap()).unwrap();
    current["path"] = serde_json::Value::String("../outside".to_string());
    fs::write(
        &publication.current_pointer,
        serde_json::to_string_pretty(&current).unwrap(),
    )
    .unwrap();
    let pointer_error = inspect_documentation_api_current(&project.root).unwrap_err();
    assert!(
        pointer_error
            .to_string()
            .contains("non-normalized generation path")
    );

    fs::write(
        &publication.current_pointer,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "athanor.documentation_current.v1",
            "generation": publication.generation,
            "snapshot": request.snapshot,
            "profile": "api",
            "path": format!("generations/{}", publication.generation),
            "manifest": format!("generations/{}/manifest.json", publication.generation)
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(&publication.document, "# modified\n").unwrap();
    let checksum_error = inspect_documentation_api_manifest(&project.root).unwrap_err();
    assert!(
        checksum_error
            .to_string()
            .contains("checksum does not match manifest")
    );
}

#[test]
fn current_inspection_is_fail_closed_from_api_to_other_profiles() {
    let project = TempProject::new("profile-isolation");
    publish_documentation_api_generation(
        DocumentationApiPublicationOptions {
            root: project.root.clone(),
            force: false,
        },
        &api_request(),
        &snapshot(),
    )
    .unwrap();

    let architecture_error = inspect_documentation_architecture_current(&project.root).unwrap_err();
    assert!(
        architecture_error
            .to_string()
            .contains("not an architecture profile")
    );
    let module_error = inspect_documentation_module_current(&project.root).unwrap_err();
    assert!(module_error.to_string().contains("not a module profile"));
}

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-api-inspection".to_string())),
        entities: vec![Entity {
            id: EntityId("endpoint-health".to_string()),
            stable_key: StableKey("api://GET:/health".to_string()),
            kind: EntityKind::ApiEndpoint,
            name: "health".to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "api/openapi.yaml".to_string(),
                line_start: Some(4),
                line_end: Some(8),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: "api/openapi.yaml".to_string(),
            }],
            payload: json!({"method": "GET", "path": "/health"}),
        }],
        facts: Vec::new(),
        relations: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn api_request() -> DocumentationGenerationRequest {
    DocumentationGenerationRequest::new(
        "snap-api-inspection",
        DocumentationProfile::Api,
        DocumentationGenerationLimits {
            max_entities: 16,
            max_facts: 16,
            max_relations: 16,
            max_diagnostics: 8,
        },
    )
}

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "athanor-api-inspection-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
