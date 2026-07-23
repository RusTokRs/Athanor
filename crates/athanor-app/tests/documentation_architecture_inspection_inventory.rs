use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use athanor_app::{
    DocumentationArchitecturePublicationOptions, DocumentationGenerationLimits,
    DocumentationGenerationRequest, DocumentationProfile, inspect_documentation_architecture_current,
    inspect_documentation_architecture_manifest, inspect_documentation_architecture_validation,
    publish_documentation_architecture_generation,
};
use athanor_core::CanonicalSnapshot;

const FIXTURE: &str = include_str!("fixtures/documentation_architecture_profile.v1.json");

#[test]
fn current_manifest_and_validation_are_bounded_and_checksum_verified() {
    let project = TempProject::new("valid");
    let request = request();
    let publication = publish_documentation_architecture_generation(
        DocumentationArchitecturePublicationOptions {
            root: project.root.clone(),
            force: false,
        },
        &request,
        &snapshot(),
    )
    .unwrap();

    let current = inspect_documentation_architecture_current(&project.root).unwrap();
    assert_eq!(current.current.generation, publication.generation);
    assert_eq!(current.current.snapshot, request.snapshot);

    let manifest = inspect_documentation_architecture_manifest(&project.root).unwrap();
    assert_eq!(manifest.current, current.current);
    assert_eq!(manifest.manifest.documents.len(), 2);
    assert_eq!(manifest.generation_dir, publication.generation_dir);

    let validation = inspect_documentation_architecture_validation(&project.root).unwrap();
    assert_eq!(validation.current, current.current);
    assert_eq!(validation.report.snapshot, request.snapshot);
    assert_eq!(validation.validation_path, publication.validation_report);
}

#[test]
fn inspection_rejects_pointer_layout_and_artifact_checksum_drift() {
    let project = TempProject::new("drift");
    let request = request();
    let publication = publish_documentation_architecture_generation(
        DocumentationArchitecturePublicationOptions {
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
    let pointer_error = inspect_documentation_architecture_current(&project.root).unwrap_err();
    assert!(pointer_error.to_string().contains("non-normalized generation path"));

    fs::write(
        &publication.current_pointer,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "athanor.documentation_current.v1",
            "generation": publication.generation,
            "snapshot": request.snapshot,
            "profile": "architecture",
            "path": format!("generations/{}", publication.generation),
            "manifest": format!("generations/{}/manifest.json", publication.generation)
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(&publication.document, "# modified\n").unwrap();
    let checksum_error = inspect_documentation_architecture_manifest(&project.root).unwrap_err();
    assert!(checksum_error.to_string().contains("checksum does not match manifest"));
}

fn snapshot() -> CanonicalSnapshot {
    serde_json::from_str(FIXTURE).unwrap()
}

fn request() -> DocumentationGenerationRequest {
    DocumentationGenerationRequest::new(
        "snap-architecture-0001",
        DocumentationProfile::Architecture,
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
            "athanor-documentation-inspection-{label}-{}-{id}",
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
