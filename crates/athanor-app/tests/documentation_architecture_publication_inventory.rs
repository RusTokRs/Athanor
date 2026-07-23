use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use athanor_app::{
    ARCHITECTURE_DOCUMENT_PATH, CancellationToken, CurrentDocumentationGeneration,
    DOCUMENTATION_CURRENT_SCHEMA_V1, DOCUMENTATION_MANIFEST_PATH,
    DOCUMENTATION_VALIDATION_REPORT_PATH, DocumentationArchitecturePublicationOptions,
    DocumentationArchitecturePublicationStatus, DocumentationGenerationLimits,
    DocumentationGenerationManifest, DocumentationGenerationRequest, DocumentationProfile,
    DocumentationValidationReport, publish_documentation_architecture_generation,
    publish_documentation_architecture_generation_cancellable,
};
use athanor_core::CanonicalSnapshot;
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("fixtures/documentation_architecture_profile.v1.json");

#[test]
fn publication_is_immutable_checksum_bound_and_reuses_exact_current() {
    let project = TempProject::new("publish");
    let request = request();
    let snapshot = fixture_snapshot();

    let first = publish_documentation_architecture_generation(
        options(&project, false),
        &request,
        &snapshot,
    )
    .expect("publish first documentation generation");
    assert_eq!(
        first.status,
        DocumentationArchitecturePublicationStatus::Published
    );
    assert_eq!(first.generation, "00000001");
    assert!(first.generation_dir.is_dir());
    assert!(first.document.is_file());
    assert!(first.validation_report.is_file());
    assert!(first.manifest.is_file());

    let current: CurrentDocumentationGeneration = read_json(&first.current_pointer);
    assert_eq!(current.schema, DOCUMENTATION_CURRENT_SCHEMA_V1);
    assert_eq!(current.generation, first.generation);
    assert_eq!(current.snapshot, request.snapshot);
    assert_eq!(current.profile, DocumentationProfile::Architecture);
    assert_eq!(current.path, "generations/00000001");
    assert_eq!(current.manifest, "generations/00000001/manifest.json");

    let manifest: DocumentationGenerationManifest = read_json(&first.manifest);
    manifest
        .validate_for_request(&request)
        .expect("published manifest matches request");
    assert_eq!(manifest.documents.len(), 2);
    for document in &manifest.documents {
        let bytes = fs::read(first.generation_dir.join(&document.path)).unwrap();
        assert_eq!(sha256_hex(&bytes), document.sha256);
    }
    let validation: DocumentationValidationReport = read_json(&first.validation_report);
    assert_eq!(validation.snapshot, request.snapshot);

    let pointer_before = fs::read(&first.current_pointer).unwrap();
    let second = publish_documentation_architecture_generation(
        options(&project, false),
        &request,
        &snapshot,
    )
    .expect("reuse exact documentation generation");
    assert_eq!(
        second.status,
        DocumentationArchitecturePublicationStatus::UpToDate
    );
    assert_eq!(second.generation, first.generation);
    assert_eq!(generation_count(&project), 1);
    assert_eq!(fs::read(&second.current_pointer).unwrap(), pointer_before);
}

#[test]
fn incomplete_or_tampered_generation_is_never_reused() {
    let project = TempProject::new("tamper");
    let request = request();
    let snapshot = fixture_snapshot();

    let first = publish_documentation_architecture_generation(
        options(&project, false),
        &request,
        &snapshot,
    )
    .unwrap();
    let mut manifest: DocumentationGenerationManifest = read_json(&first.manifest);
    manifest
        .documents
        .retain(|document| document.path == DOCUMENTATION_VALIDATION_REPORT_PATH);
    fs::write(&first.manifest, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();

    let repaired_manifest = publish_documentation_architecture_generation(
        options(&project, false),
        &request,
        &snapshot,
    )
    .expect("incomplete manifest must create a new generation");
    assert_eq!(
        repaired_manifest.status,
        DocumentationArchitecturePublicationStatus::Published
    );
    assert_eq!(repaired_manifest.generation, "00000002");
    assert!(first.generation_dir.is_dir(), "immutable history was removed");

    fs::write(&repaired_manifest.document, "# tampered\n").unwrap();
    let repaired_document = publish_documentation_architecture_generation(
        options(&project, false),
        &request,
        &snapshot,
    )
    .expect("checksum mismatch must create a new generation");
    assert_eq!(repaired_document.generation, "00000003");
    assert_eq!(
        repaired_document.status,
        DocumentationArchitecturePublicationStatus::Published
    );

    let forced = publish_documentation_architecture_generation(
        options(&project, true),
        &request,
        &snapshot,
    )
    .expect("force must publish another immutable generation");
    assert_eq!(forced.generation, "00000004");
    assert_eq!(forced.status, DocumentationArchitecturePublicationStatus::Published);
    assert_eq!(generation_count(&project), 4);

    let current: CurrentDocumentationGeneration = read_json(&forced.current_pointer);
    assert_eq!(current.generation, forced.generation);
    assert!(forced.generation_dir.join(ARCHITECTURE_DOCUMENT_PATH).is_file());
    assert!(forced.generation_dir.join(DOCUMENTATION_MANIFEST_PATH).is_file());
}

#[test]
fn cancellation_preserves_the_existing_pointer_and_generation_set() {
    let project = TempProject::new("cancel");
    let request = request();
    let snapshot = fixture_snapshot();
    let first = publish_documentation_architecture_generation(
        options(&project, false),
        &request,
        &snapshot,
    )
    .unwrap();
    let pointer_before = fs::read(&first.current_pointer).unwrap();
    let generations_before = generation_count(&project);

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = publish_documentation_architecture_generation_cancellable(
        options(&project, true),
        &request,
        &snapshot,
        cancellation,
    )
    .expect_err("cancelled publication must fail");
    assert!(error.to_string().contains("operation cancelled"));
    assert_eq!(fs::read(&first.current_pointer).unwrap(), pointer_before);
    assert_eq!(generation_count(&project), generations_before);
}

#[test]
fn publication_content_is_deterministic_across_canonical_input_order() {
    let project = TempProject::new("order");
    let request = request();
    let snapshot = fixture_snapshot();
    let mut reversed = snapshot.clone();
    reversed.entities.reverse();
    reversed.facts.reverse();
    reversed.relations.reverse();
    reversed.diagnostics.reverse();

    let first = publish_documentation_architecture_generation(
        options(&project, false),
        &request,
        &snapshot,
    )
    .unwrap();
    let second = publish_documentation_architecture_generation(
        options(&project, true),
        &request,
        &reversed,
    )
    .unwrap();

    assert_eq!(fs::read(first.document).unwrap(), fs::read(second.document).unwrap());
    assert_eq!(
        fs::read(first.validation_report).unwrap(),
        fs::read(second.validation_report).unwrap()
    );
}

fn fixture_snapshot() -> CanonicalSnapshot {
    serde_json::from_str(FIXTURE).expect("valid canonical architecture fixture")
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

fn options(project: &TempProject, force: bool) -> DocumentationArchitecturePublicationOptions {
    DocumentationArchitecturePublicationOptions {
        root: project.root.clone(),
        force,
    }
}

fn generation_count(project: &TempProject) -> usize {
    let generations = project
        .root
        .join(".athanor/generated/documentation/generations");
    match fs::read_dir(generations) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .count(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => panic!("failed to inspect documentation generations: {error}"),
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_slice(&fs::read(path).unwrap())
        .unwrap_or_else(|error| panic!("invalid JSON at {}: {error}", path.display()))
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
            "athanor-documentation-publication-{label}-{}-{id}",
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
