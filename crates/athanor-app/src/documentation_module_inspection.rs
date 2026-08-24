//! Validated inspection of the current module documentation generation.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    CurrentDocumentationGeneration, DOCUMENTATION_CURRENT_SCHEMA_V1, DOCUMENTATION_DRAFT_SCHEMA_V1,
    DOCUMENTATION_MANIFEST_PATH, DOCUMENTATION_VALIDATION_REPORT_PATH,
    DOCUMENTATION_VALIDATION_REPORT_SCHEMA_V1, DocumentationGenerationManifest,
    DocumentationGenerationRequest, DocumentationGenerationStatus, DocumentationProfile,
    DocumentationValidationReport, DocumentationValidationStatus, MODULE_DOCUMENT_MEDIA_TYPE,
    MODULE_DOCUMENT_PATH,
};

const MODULE_DOCUMENT_ID: &str = "module-overview";
const MODULE_VALIDATION_REPORT_ID: &str = "module-validation-report";
const JSON_MEDIA_TYPE: &str = "application/json";

#[derive(Debug, Clone, Serialize)]
pub struct DocumentationModuleCurrentInspection {
    pub root: PathBuf,
    pub current_pointer: PathBuf,
    pub current: CurrentDocumentationGeneration,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentationModuleManifestInspection {
    pub root: PathBuf,
    pub generation_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub current: CurrentDocumentationGeneration,
    pub manifest: DocumentationGenerationManifest,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentationModuleValidationInspection {
    pub root: PathBuf,
    pub validation_path: PathBuf,
    pub current: CurrentDocumentationGeneration,
    pub report: DocumentationValidationReport,
}

pub fn inspect_documentation_module_current(
    root: impl AsRef<Path>,
) -> Result<DocumentationModuleCurrentInspection> {
    let resolved = resolve_current(root.as_ref())?;
    Ok(DocumentationModuleCurrentInspection {
        root: resolved.root,
        current_pointer: resolved.current_pointer,
        current: resolved.current,
    })
}

pub fn inspect_documentation_module_manifest(
    root: impl AsRef<Path>,
) -> Result<DocumentationModuleManifestInspection> {
    let resolved = resolve_current(root.as_ref())?;
    let manifest = load_validated_manifest(&resolved)?;
    Ok(DocumentationModuleManifestInspection {
        root: resolved.root,
        generation_dir: resolved.generation_dir,
        manifest_path: resolved.manifest_path,
        current: resolved.current,
        manifest,
    })
}

pub fn inspect_documentation_module_validation(
    root: impl AsRef<Path>,
) -> Result<DocumentationModuleValidationInspection> {
    let resolved = resolve_current(root.as_ref())?;
    let manifest = load_validated_manifest(&resolved)?;
    let validation_path = confined_existing_file(
        &resolved.documentation_root,
        &resolved
            .generation_dir
            .join(DOCUMENTATION_VALIDATION_REPORT_PATH),
        "documentation validation report",
    )?;
    let report: DocumentationValidationReport = read_json(&validation_path)?;
    if report.schema != DOCUMENTATION_VALIDATION_REPORT_SCHEMA_V1
        || report.draft_schema != DOCUMENTATION_DRAFT_SCHEMA_V1
        || report.snapshot != resolved.current.snapshot
        || report.profile != DocumentationProfile::Module
        || report.status != DocumentationValidationStatus::Valid
    {
        bail!("documentation validation report identity or status is invalid");
    }
    let descriptor = manifest
        .documents
        .iter()
        .find(|document| document.id == MODULE_VALIDATION_REPORT_ID)
        .context("documentation manifest omits module validation report")?;
    if sha256_file(&validation_path)? != descriptor.sha256 {
        bail!("documentation validation report checksum does not match manifest");
    }
    Ok(DocumentationModuleValidationInspection {
        root: resolved.root,
        validation_path,
        current: resolved.current,
        report,
    })
}

struct ResolvedCurrent {
    root: PathBuf,
    documentation_root: PathBuf,
    current_pointer: PathBuf,
    generation_dir: PathBuf,
    manifest_path: PathBuf,
    current: CurrentDocumentationGeneration,
}

fn resolve_current(root: &Path) -> Result<ResolvedCurrent> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let documentation_root = root.join(".athanor/generated/documentation");
    let current_pointer = documentation_root.join("current.json");
    let current: CurrentDocumentationGeneration =
        read_json(&current_pointer).with_context(|| {
            format!(
                "failed to read current documentation pointer in {}",
                root.display()
            )
        })?;
    if current.schema != DOCUMENTATION_CURRENT_SCHEMA_V1 {
        bail!(
            "unsupported documentation current schema {}",
            current.schema
        );
    }
    if current.profile != DocumentationProfile::Module {
        bail!("current documentation generation is not a module profile");
    }
    if current.generation.len() != 8
        || !current.generation.bytes().all(|byte| byte.is_ascii_digit())
        || current.path != format!("generations/{}", current.generation)
        || current.manifest
            != format!(
                "generations/{}/{DOCUMENTATION_MANIFEST_PATH}",
                current.generation
            )
    {
        bail!("documentation current pointer contains a non-normalized generation path");
    }
    let documentation_root = documentation_root
        .canonicalize()
        .context("documentation generation root does not exist")?;
    let generation_dir = confined_existing_dir(
        &documentation_root,
        &documentation_root.join(&current.path),
        "documentation generation",
    )?;
    let manifest_path = confined_existing_file(
        &documentation_root,
        &documentation_root.join(&current.manifest),
        "documentation manifest",
    )?;
    Ok(ResolvedCurrent {
        root,
        documentation_root,
        current_pointer,
        generation_dir,
        manifest_path,
        current,
    })
}

fn load_validated_manifest(resolved: &ResolvedCurrent) -> Result<DocumentationGenerationManifest> {
    let manifest: DocumentationGenerationManifest = read_json(&resolved.manifest_path)?;
    let request = DocumentationGenerationRequest::new(
        manifest.snapshot.clone(),
        manifest.profile,
        manifest.effective_limits,
    );
    manifest
        .validate_for_request(&request)
        .map_err(anyhow::Error::msg)
        .context("invalid documentation generation manifest")?;
    if manifest.generation != resolved.current.generation
        || manifest.snapshot != resolved.current.snapshot
        || manifest.profile != DocumentationProfile::Module
        || manifest.profile != resolved.current.profile
        || manifest.status != DocumentationGenerationStatus::Complete
        || manifest.documents.len() != 2
    {
        bail!("documentation manifest does not match current generation identity");
    }
    let document = manifest
        .documents
        .iter()
        .find(|document| document.id == MODULE_DOCUMENT_ID)
        .context("documentation manifest omits module Markdown")?;
    let validation = manifest
        .documents
        .iter()
        .find(|document| document.id == MODULE_VALIDATION_REPORT_ID)
        .context("documentation manifest omits module validation report")?;
    if document.path != MODULE_DOCUMENT_PATH
        || document.media_type != MODULE_DOCUMENT_MEDIA_TYPE
        || validation.path != DOCUMENTATION_VALIDATION_REPORT_PATH
        || validation.media_type != JSON_MEDIA_TYPE
    {
        bail!("documentation manifest contains an unsupported module artifact layout");
    }
    for descriptor in &manifest.documents {
        let path = confined_existing_file(
            &resolved.documentation_root,
            &resolved.generation_dir.join(&descriptor.path),
            "documentation artifact",
        )?;
        if sha256_file(&path)? != descriptor.sha256 {
            bail!(
                "documentation artifact {} checksum does not match manifest",
                descriptor.path
            );
        }
    }
    Ok(manifest)
}

fn confined_existing_dir(root: &Path, path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("{label} does not exist at {}", path.display()))?;
    if !canonical.starts_with(root) || !canonical.is_dir() {
        bail!("{label} escapes the documentation generation root");
    }
    Ok(canonical)
}

fn confined_existing_file(root: &Path, path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("{label} does not exist at {}", path.display()))?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        bail!("{label} escapes the documentation generation root");
    }
    Ok(canonical)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let source = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&source).with_context(|| format!("invalid JSON at {}", path.display()))
}

fn sha256_file(path: &Path) -> Result<String> {
    let content = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(content)))
}
