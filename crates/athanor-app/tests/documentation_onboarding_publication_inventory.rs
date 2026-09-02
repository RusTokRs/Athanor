use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use athanor_app::{
    CancellationToken, CurrentDocumentationGeneration, DOCUMENTATION_CURRENT_SCHEMA_V1,
    DocumentationGenerationLimits, DocumentationGenerationManifest, DocumentationGenerationRequest,
    DocumentationOnboardingPublicationOptions, DocumentationOnboardingPublicationStatus,
    DocumentationProfile, DocumentationValidationReport, ONBOARDING_DOCUMENT_PATH,
    publish_documentation_onboarding_generation,
    publish_documentation_onboarding_generation_cancellable,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Entity, EntityId, EntityKind, Ownership, SnapshotId, SourceLocation, StableKey,
};
use serde_json::json;
use sha2::{Digest, Sha256};

#[test]
fn onboarding_publication_is_immutable_checksum_bound_and_reuses_exact_current() {
    let project = TempProject::new("publish");
    let request = request();
    let snapshot = snapshot();

    let first =
        publish_documentation_onboarding_generation(options(&project, false), &request, &snapshot)
            .expect("publish first onboarding documentation generation");
    assert_eq!(
        first.status,
        DocumentationOnboardingPublicationStatus::Published
    );
    assert_eq!(first.generation, "00000001");
    assert!(first.document.is_file());
    assert!(first.validation_report.is_file());
    assert!(first.manifest.is_file());

    let current: CurrentDocumentationGeneration = read_json(&first.current_pointer);
    assert_eq!(current.schema, DOCUMENTATION_CURRENT_SCHEMA_V1);
    assert_eq!(current.snapshot, request.snapshot);
    assert_eq!(current.profile, DocumentationProfile::Onboarding);

    let manifest: DocumentationGenerationManifest = read_json(&first.manifest);
    manifest
        .validate_for_request(&request)
        .expect("published manifest matches onboarding request");
    assert_eq!(manifest.documents.len(), 2);
    assert!(
        manifest
            .documents
            .iter()
            .any(|document| document.path == ONBOARDING_DOCUMENT_PATH)
    );
    for document in &manifest.documents {
        let bytes = fs::read(first.generation_dir.join(&document.path)).unwrap();
        assert_eq!(sha256_hex(&bytes), document.sha256);
    }
    let validation: DocumentationValidationReport = read_json(&first.validation_report);
    assert_eq!(validation.profile, DocumentationProfile::Onboarding);

    let pointer_before = fs::read(&first.current_pointer).unwrap();
    let second =
        publish_documentation_onboarding_generation(options(&project, false), &request, &snapshot)
            .expect("reuse exact onboarding generation");
    assert_eq!(
        second.status,
        DocumentationOnboardingPublicationStatus::UpToDate
    );
    assert_eq!(second.generation, first.generation);
    assert_eq!(fs::read(&second.current_pointer).unwrap(), pointer_before);
}

#[test]
fn tamper_force_and_cancellation_preserve_immutable_onboarding_lifecycle() {
    let project = TempProject::new("lifecycle");
    let request = request();
    let snapshot = snapshot();
    let first =
        publish_documentation_onboarding_generation(options(&project, false), &request, &snapshot)
            .unwrap();

    fs::write(&first.document, "# tampered\n").unwrap();
    let repaired =
        publish_documentation_onboarding_generation(options(&project, false), &request, &snapshot)
            .expect("tamper must publish a new immutable generation");
    assert_eq!(repaired.generation, "00000002");
    assert_eq!(
        repaired.status,
        DocumentationOnboardingPublicationStatus::Published
    );
    assert!(first.generation_dir.is_dir());

    let forced =
        publish_documentation_onboarding_generation(options(&project, true), &request, &snapshot)
            .expect("force must publish another generation");
    assert_eq!(forced.generation, "00000003");

    let pointer_before = fs::read(&forced.current_pointer).unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = publish_documentation_onboarding_generation_cancellable(
        options(&project, true),
        &request,
        &snapshot,
        cancellation,
    )
    .expect_err("cancelled onboarding publication must fail");
    assert!(error.to_string().contains("operation cancelled"));
    assert_eq!(fs::read(&forced.current_pointer).unwrap(), pointer_before);
}

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-onboarding-publication".to_string())),
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
            payload: json!({"documentation_kind": "project_overview"}),
        }],
        ..CanonicalSnapshot::default()
    }
}

fn request() -> DocumentationGenerationRequest {
    DocumentationGenerationRequest::new(
        "snap-onboarding-publication",
        DocumentationProfile::Onboarding,
        DocumentationGenerationLimits {
            max_entities: 16,
            max_facts: 16,
            max_relations: 16,
            max_diagnostics: 8,
        },
    )
}

fn options(project: &TempProject, force: bool) -> DocumentationOnboardingPublicationOptions {
    DocumentationOnboardingPublicationOptions {
        root: project.root.clone(),
        force,
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn sha256_hex(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "athanor-onboarding-publication-{label}-{}-{id}",
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
