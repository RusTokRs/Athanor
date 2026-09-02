use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use athanor_app::{
    DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationOperationsPublicationOptions, DocumentationProfile,
    inspect_documentation_api_current, inspect_documentation_architecture_current,
    inspect_documentation_module_current, inspect_documentation_operations_current,
    inspect_documentation_operations_manifest, inspect_documentation_operations_validation,
    publish_documentation_operations_generation,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Entity, EntityId, EntityKind, Ownership, SnapshotId, SourceLocation, StableKey,
};
use serde_json::json;

#[test]
fn operations_current_manifest_and_validation_are_bounded_and_checksum_verified() {
    let project = TempProject::new("valid");
    let request = operations_request();
    let publication = publish_documentation_operations_generation(
        DocumentationOperationsPublicationOptions {
            root: project.root.clone(),
            force: false,
        },
        &request,
        &snapshot(),
    )
    .unwrap();

    let current = inspect_documentation_operations_current(&project.root).unwrap();
    assert_eq!(current.current.generation, publication.generation);
    assert_eq!(current.current.snapshot, request.snapshot);
    assert_eq!(current.current.profile, DocumentationProfile::Operations);

    let manifest = inspect_documentation_operations_manifest(&project.root).unwrap();
    assert_eq!(manifest.current, current.current);
    assert_eq!(manifest.manifest.documents.len(), 2);
    assert_eq!(manifest.generation_dir, publication.generation_dir);

    let validation = inspect_documentation_operations_validation(&project.root).unwrap();
    assert_eq!(validation.current, current.current);
    assert_eq!(validation.report.profile, DocumentationProfile::Operations);
    assert_eq!(validation.validation_path, publication.validation_report);
}

#[test]
fn operations_inspection_rejects_pointer_and_checksum_drift_and_other_profiles_fail_closed() {
    let project = TempProject::new("drift");
    let request = operations_request();
    let publication = publish_documentation_operations_generation(
        DocumentationOperationsPublicationOptions {
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

    let mut current: serde_json::Value =
        serde_json::from_slice(&fs::read(&publication.current_pointer).unwrap()).unwrap();
    current["path"] = serde_json::Value::String("../outside".to_string());
    fs::write(
        &publication.current_pointer,
        serde_json::to_string_pretty(&current).unwrap(),
    )
    .unwrap();
    assert!(
        inspect_documentation_operations_current(&project.root)
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
            "profile": "operations",
            "path": format!("generations/{}", publication.generation),
            "manifest": format!("generations/{}/manifest.json", publication.generation)
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(&publication.document, "# modified\n").unwrap();
    assert!(
        inspect_documentation_operations_manifest(&project.root)
            .unwrap_err()
            .to_string()
            .contains("checksum does not match manifest")
    );
}

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-operations-inspection".to_string())),
        entities: vec![Entity {
            id: EntityId("env-database".to_string()),
            stable_key: StableKey("env://DATABASE_URL".to_string()),
            kind: EntityKind::EnvVar,
            name: "DATABASE_URL".to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "config/runtime.env".to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: "config/runtime.env".to_string(),
            }],
            payload: json!({}),
        }],
        ..CanonicalSnapshot::default()
    }
}

fn operations_request() -> DocumentationGenerationRequest {
    DocumentationGenerationRequest::new(
        "snap-operations-inspection",
        DocumentationProfile::Operations,
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
            "athanor-operations-inspection-{label}-{}-{id}",
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
