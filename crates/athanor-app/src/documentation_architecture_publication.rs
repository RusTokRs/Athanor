//! Atomic publication for deterministic architecture documentation.
//!
//! This owner publishes a supplied canonical snapshot into an isolated documentation generation
//! root. It deliberately does not load a store or add a CLI/daemon/MCP command.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshot;
use athanor_projector_support::{NewDirectoryPublication, replace_output_file, write_output_file};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ARCHITECTURE_DOCUMENT_MEDIA_TYPE, ARCHITECTURE_DOCUMENT_PATH, CancellationToken,
    DOCUMENTATION_DRAFT_SCHEMA_V1, DOCUMENTATION_VALIDATION_REPORT_SCHEMA_V1,
    DocumentationDocumentManifest, DocumentationGenerationManifest, DocumentationGenerationRequest,
    DocumentationGenerationStatus, DocumentationProfile, DocumentationValidationStatus,
    build_documentation_architecture_profile,
};

pub const DOCUMENTATION_CURRENT_SCHEMA_V1: &str = "athanor.documentation_current.v1";
pub const DOCUMENTATION_VALIDATION_REPORT_PATH: &str = "validation-report.json";
pub const DOCUMENTATION_MANIFEST_PATH: &str = "manifest.json";

#[derive(Debug, Clone)]
pub struct DocumentationArchitecturePublicationOptions {
    pub root: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationArchitecturePublicationStatus {
    Published,
    UpToDate,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentationArchitecturePublicationReport {
    pub status: DocumentationArchitecturePublicationStatus,
    pub root: PathBuf,
    pub generation: String,
    pub generation_dir: PathBuf,
    pub current_pointer: PathBuf,
    pub snapshot: String,
    pub manifest: PathBuf,
    pub document: PathBuf,
    pub validation_report: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CurrentDocumentationGeneration {
    pub schema: String,
    pub generation: String,
    pub snapshot: String,
    pub profile: DocumentationProfile,
    pub path: String,
    pub manifest: String,
}

/// Publishes an architecture document without cooperative cancellation.
pub fn publish_documentation_architecture_generation(
    options: DocumentationArchitecturePublicationOptions,
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationArchitecturePublicationReport> {
    publish_documentation_architecture_generation_inner(options, request, snapshot, None)
}

/// Publishes an architecture document while preserving the previous pointer on cancellation.
pub fn publish_documentation_architecture_generation_cancellable(
    options: DocumentationArchitecturePublicationOptions,
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
    cancellation: CancellationToken,
) -> Result<DocumentationArchitecturePublicationReport> {
    publish_documentation_architecture_generation_inner(
        options,
        request,
        snapshot,
        Some(cancellation),
    )
}

fn publish_documentation_architecture_generation_inner(
    options: DocumentationArchitecturePublicationOptions,
    request: &DocumentationGenerationRequest,
    snapshot: &CanonicalSnapshot,
    cancellation: Option<CancellationToken>,
) -> Result<DocumentationArchitecturePublicationReport> {
    check_cancelled(&cancellation)?;
    request
        .validate()
        .map_err(anyhow::Error::msg)
        .context("invalid documentation generation request")?;
    let root = options
        .root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", options.root.display()))?;
    let documentation_root = root.join(".athanor/generated/documentation");
    let generations_dir = documentation_root.join("generations");
    let current_pointer = documentation_root.join("current.json");

    if !options.force
        && let Some(current) = load_current_generation(&documentation_root, request)?
    {
        return Ok(report_for_current(
            DocumentationArchitecturePublicationStatus::UpToDate,
            root,
            current,
            current_pointer,
            &documentation_root,
        ));
    }

    let profile = build_documentation_architecture_profile(request, snapshot)
        .map_err(anyhow::Error::msg)
        .context("failed to build deterministic architecture profile")?;
    check_cancelled(&cancellation)?;

    let generation = next_generation_id(&generations_dir)?;
    let generation_dir = generations_dir.join(&generation);
    let publication = NewDirectoryPublication::new(&generation_dir, "documentation generation")
        .context("failed to prepare documentation generation")?;
    let staging = publication.staging_path();

    let validation_json = serde_json::to_string_pretty(&profile.validation_report)
        .context("failed to serialize documentation validation report")?;
    let manifest = DocumentationGenerationManifest {
        schema: DocumentationGenerationManifest::SCHEMA.to_string(),
        request_schema: DocumentationGenerationRequest::SCHEMA.to_string(),
        generation: generation.clone(),
        snapshot: request.snapshot.clone(),
        profile: request.profile,
        status: DocumentationGenerationStatus::Complete,
        effective_limits: request.limits,
        omitted: profile.context.omitted,
        documents: vec![
            DocumentationDocumentManifest {
                id: "architecture-overview".to_string(),
                path: ARCHITECTURE_DOCUMENT_PATH.to_string(),
                media_type: ARCHITECTURE_DOCUMENT_MEDIA_TYPE.to_string(),
                sha256: profile.document.sha256.clone(),
            },
            DocumentationDocumentManifest {
                id: "architecture-validation-report".to_string(),
                path: DOCUMENTATION_VALIDATION_REPORT_PATH.to_string(),
                media_type: "application/json".to_string(),
                sha256: sha256_hex(validation_json.as_bytes()),
            },
        ],
    };
    manifest
        .validate_for_request(request)
        .map_err(anyhow::Error::msg)
        .context("invalid documentation generation manifest")?;

    write_output_file(
        &staging.join(ARCHITECTURE_DOCUMENT_PATH),
        &profile.document.content,
    )
    .context("failed to write architecture Markdown")?;
    write_output_file(
        &staging.join(DOCUMENTATION_VALIDATION_REPORT_PATH),
        &validation_json,
    )
    .context("failed to write documentation validation report")?;
    write_output_file(
        &staging.join(DOCUMENTATION_MANIFEST_PATH),
        &serde_json::to_string_pretty(&manifest)
            .context("failed to serialize documentation manifest")?,
    )
    .context("failed to write documentation manifest")?;
    check_cancelled(&cancellation)?;

    publication
        .publish()
        .context("failed to publish immutable documentation generation")?;
    check_cancelled(&cancellation)?;

    let current = CurrentDocumentationGeneration {
        schema: DOCUMENTATION_CURRENT_SCHEMA_V1.to_string(),
        generation: generation.clone(),
        snapshot: request.snapshot.clone(),
        profile: request.profile,
        path: format!("generations/{generation}"),
        manifest: format!("generations/{generation}/{DOCUMENTATION_MANIFEST_PATH}"),
    };
    replace_output_file(
        &current_pointer,
        &serde_json::to_string_pretty(&current)
            .context("failed to serialize documentation current pointer")?,
        "documentation current pointer",
    )
    .context("failed to update documentation current pointer")?;

    Ok(report_for_current(
        DocumentationArchitecturePublicationStatus::Published,
        root,
        current,
        current_pointer,
        &documentation_root,
    ))
}

fn load_current_generation(
    documentation_root: &Path,
    request: &DocumentationGenerationRequest,
) -> Result<Option<CurrentDocumentationGeneration>> {
    let pointer_path = documentation_root.join("current.json");
    let Ok(pointer_source) = fs::read_to_string(&pointer_path) else {
        return Ok(None);
    };
    let current: CurrentDocumentationGeneration = match serde_json::from_str(&pointer_source) {
        Ok(current) => current,
        Err(_) => return Ok(None),
    };
    if current.schema != DOCUMENTATION_CURRENT_SCHEMA_V1
        || current.snapshot != request.snapshot
        || current.profile != request.profile
        || !is_normalized_generation_path(&current.path, &current.generation)
        || current.manifest
            != format!(
                "generations/{}/{DOCUMENTATION_MANIFEST_PATH}",
                current.generation
            )
    {
        return Ok(None);
    }

    let generation_dir = documentation_root.join(&current.path);
    let manifest_path = documentation_root.join(&current.manifest);
    if !generation_dir.is_dir() || !manifest_path.is_file() {
        return Ok(None);
    }
    let manifest: DocumentationGenerationManifest = match fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|source| serde_json::from_str(&source).ok())
    {
        Some(manifest) => manifest,
        None => return Ok(None),
    };
    if manifest.validate_for_request(request).is_err()
        || manifest.generation != current.generation
        || manifest.status != DocumentationGenerationStatus::Complete
    {
        return Ok(None);
    }

    for document in &manifest.documents {
        let path = generation_dir.join(&document.path);
        let Ok(content) = fs::read(&path) else {
            return Ok(None);
        };
        if sha256_hex(&content) != document.sha256 {
            return Ok(None);
        }
    }
    let report_path = generation_dir.join(DOCUMENTATION_VALIDATION_REPORT_PATH);
    let report: crate::DocumentationValidationReport = match fs::read_to_string(&report_path)
        .ok()
        .and_then(|source| serde_json::from_str(&source).ok())
    {
        Some(report) => report,
        None => return Ok(None),
    };
    if report.schema != DOCUMENTATION_VALIDATION_REPORT_SCHEMA_V1
        || report.draft_schema != DOCUMENTATION_DRAFT_SCHEMA_V1
        || report.snapshot != request.snapshot
        || report.profile != request.profile
        || report.status != DocumentationValidationStatus::Valid
    {
        return Ok(None);
    }
    Ok(Some(current))
}

fn report_for_current(
    status: DocumentationArchitecturePublicationStatus,
    root: PathBuf,
    current: CurrentDocumentationGeneration,
    current_pointer: PathBuf,
    documentation_root: &Path,
) -> DocumentationArchitecturePublicationReport {
    let generation_dir = documentation_root.join(&current.path);
    DocumentationArchitecturePublicationReport {
        status,
        root,
        generation: current.generation,
        manifest: documentation_root.join(current.manifest),
        document: generation_dir.join(ARCHITECTURE_DOCUMENT_PATH),
        validation_report: generation_dir.join(DOCUMENTATION_VALIDATION_REPORT_PATH),
        generation_dir,
        current_pointer,
        snapshot: current.snapshot,
    }
}

fn next_generation_id(generations_dir: &Path) -> Result<String> {
    let max = match fs::read_dir(generations_dir) {
        Ok(entries) => entries
            .filter_map(std::result::Result::ok)
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .filter_map(|name| name.parse::<u64>().ok())
            .max()
            .unwrap_or(0),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect documentation generations at {}",
                    generations_dir.display()
                )
            });
        }
    };
    Ok(format!(
        "{:08}",
        max.checked_add(1)
            .context("documentation generation number overflow")?
    ))
}

fn is_normalized_generation_path(path: &str, generation: &str) -> bool {
    path == format!("generations/{generation}")
        && generation.len() == 8
        && generation.bytes().all(|byte| byte.is_ascii_digit())
}

fn check_cancelled(cancellation: &Option<CancellationToken>) -> Result<()> {
    if let Some(cancellation) = cancellation {
        cancellation.check()?;
    }
    Ok(())
}

fn sha256_hex(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}
