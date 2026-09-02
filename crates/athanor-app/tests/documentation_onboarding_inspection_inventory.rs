use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use athanor_app::{
    DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationOnboardingPublicationOptions, DocumentationProfile,
    inspect_documentation_api_current, inspect_documentation_architecture_current,
    inspect_documentation_module_current, inspect_documentation_onboarding_current,
    inspect_documentation_onboarding_manifest, inspect_documentation_onboarding_validation,
    inspect_documentation_operations_current, publish_documentation_onboarding_generation,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Entity, EntityId, EntityKind, Ownership, SnapshotId, SourceLocation, StableKey,
};
use serde_json::json;

#[test]
fn onboarding_current_manifest_and_validation_are_bounded_and_checksum_verified() {
    let project = TempProject::new("valid");
    let request = onboarding_request();
    let publication = publish_documentation_onboarding_generation(
        DocumentationOnboardingPublicationOptions {
            root: project.root.clone(),
            force: false,
        },
        &request,
        &snapshot(),
    )
    .unwrap();

    let current = inspect_documentation_onboarding_current(&project.root).unwrap();
    assert_eq!(current.current.generation, publication.generation);
    assert_eq!(current.current.snapshot, request.snapshot);
    assert_eq!(current.current.profile, DocumentationProfile::Onboarding);

    let manifest = inspect_documentation_onboarding_manifest(&project.root).unwrap();
    assert_eq!(manifest.current, current.current);
    assert_eq!(manifest.manifest.documents.len(), 2);
    assert_eq!(manifest.generation_dir, publication.generation_dir);

    let validation = inspect_documentation_onboarding_validation(&project.root).unwrap();
    assert_eq!(validation.current, current.current);
    assert_eq!(validation.report.profile, DocumentationProfile::Onboarding);
    assert_eq!(validation.validation_path, publication.validation_report);
}

#[test]
fn onboarding_inspection_rejects_pointer_and_checksum_drift_and_other_profiles_fail_closed() {
    let project = TempProject::new("drift");
    let request = onboarding_request();
    let publication = publish_documentation_onboarding_generation(
        DocumentationOnboardingPublicationOptions {
            root: project.root.clone(),
            force: false,
        },
        &request,
        &snapshot(),
    )
    .unwrap();

    assert!(
        inspect_documentation_architecture_current(&project.root)
            .unwrap_err()
            .to_string()
            .contains("not an architecture profile")
    );
    assert!(
        inspect_documentation_module_current(&project.root)
            .unwrap_err()
            .to_string()
            .contains("not a module profile")
    );
    assert!(
        inspect_documentation_api_current(&project.root)
            .unwrap_err()
            .to_string()
            .contains("not an API profile")
    );
    assert!(
        inspect_documentation_operations_current(&project.root)
            .unwrap_err()
            .to_string()
            .contains("not an operations profile")
    );

    let mut current: serde_json::Value =
        serde_json::from_slice(&fs::read(&publication.current_pointer).unwrap()).unwrap();
    current["path"] = serde_json::Value::String("../outside".to_string());
    fs::write(
        &publication.current_pointer,
        serde_json::to_string_pretty(&current).unwrap(),
    )
    .unwrap();
    assert!(
        inspect_documentation_onboarding_current(&project.root)
            .unwrap_err()
            .to_string()
            .contains("non-normalized generation path")
    );

    fs::write(
        &publication.current_pointer,
        serde_json::to_string_pretty(&json!({
            "schema": "athanor.documentation_current.v1",
            "generation": publication.generation,
            "snapshot": request.snapshot,
            "profile": "onboarding",
            "path": format!("generations/{}", publication.generation),
            "manifest": format!("generations/{}/manifest.json", publication.generation)
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(&publication.document, "# modified\n").unwrap();
    assert!(
        inspect_documentation_onboarding_manifest(&project.root)
            .unwrap_err()
            .to_string()
            .contains("checksum does not match manifest")
    );
}

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-onboarding-inspection".to_string())),
        entities: vec![Entity {
            id: EntityId("guide-readme".to_string()),
            stable_key: StableKey("doc://README.md".to_string()),
            kind: EntityKind::DocumentationPage,
            name: "README".to_string(),
            title: Some("Getting Started".to_string()),
            source: Some(SourceLocation {
                path: "README.md".to_string(),
                line_start: Some(1),
                line_end: Some(12),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: "README.md".to_string(),
            }],
            payload: json!({}),
        }],
        ..CanonicalSnapshot::default()
    }
}

fn onboarding_request() -> DocumentationGenerationRequest {
    DocumentationGenerationRequest::new(
        "snap-onboarding-inspection",
        DocumentationProfile::Onboarding,
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
            "athanor-onboarding-inspection-{label}-{}-{id}",
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
