//! Read-only catalog of published documentation generations.
//!
//! The catalog is the only source the publication routes answer from. It reads
//! `.athanor/generated/documentation`, validates every manifest it can, and refuses to serve a file
//! that the manifest does not declare. Nothing here writes to disk.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use athanor_core::{OperationContext, OperationContextCancellation};
use serde::Serialize;
use sha2::{Digest, Sha256};

use athanor_app::{
    CurrentDocumentationGeneration, DOCUMENTATION_CURRENT_SCHEMA_V1, DocumentationDraft,
    DocumentationGenerationLimits, DocumentationGenerationManifest, DocumentationOmittedCounts,
    DocumentationProfile, DocumentationValidationReport, DocumentationValidationStatus,
};

/// Stable identifier of the generation catalog transport.
pub(super) const SERVE_CATALOG_SCHEMA_V1: &str = "athanor.documentation_serve_catalog.v1";
/// Documentation root relative to the project root.
pub(super) const DOCUMENTATION_ROOT_RELATIVE: &str = ".athanor/generated/documentation";
/// Published manifest file name inside one generation directory.
pub(super) const MANIFEST_FILE: &str = "manifest.json";
/// Atomic current-generation pointer inside the documentation root.
pub(super) const CURRENT_FILE: &str = "current.json";
/// Validation report file name inside one generation directory.
pub(super) const VALIDATION_REPORT_FILE: &str = "validation-report.json";
/// Largest pointer or validation metadata file the catalog will read.
const MAX_METADATA_BYTES: u64 = 1024 * 1024;
/// Sixteen descriptors fit comfortably in this bound, limiting catalog memory independently of bodies.
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
/// A v1 profile publishes two files; leave a small bounded allowance for additive artifacts.
const MAX_DOCUMENTS_PER_GENERATION: usize = 16;

/// Hard bounds applied while reading the catalog.
#[derive(Debug, Clone, Copy, Serialize)]
pub(super) struct CatalogLimits {
    pub(super) max_generations: usize,
    pub(super) max_document_bytes: u64,
}

/// One served document, after its declared checksum matched.
#[derive(Debug, Clone)]
pub(super) struct ServedDocument {
    pub(super) bytes: Vec<u8>,
    pub(super) media_type: String,
}

/// Why one generation document could not be served.
#[derive(Debug, Clone)]
pub(super) enum CatalogError {
    /// The generation or the declared document does not exist.
    NotFound(String),
    /// The file exceeds the configured serve limit.
    TooLarge(String),
    /// The file is not a readable regular file inside its generation directory.
    Unreadable(String),
    /// The file content does not match the checksum its manifest declares.
    ChecksumMismatch(String),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatalogError::NotFound(message)
            | CatalogError::TooLarge(message)
            | CatalogError::Unreadable(message)
            | CatalogError::ChecksumMismatch(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CatalogError {}

/// One declared document of a validated generation.
#[derive(Debug, Clone, Serialize)]
pub(super) struct DocumentReport {
    pub(super) id: String,
    pub(super) path: String,
    pub(super) media_type: String,
    pub(super) sha256: String,
    /// True when the document body is Markdown the server can render.
    pub(super) markdown: bool,
}

/// Validation summary of one generation, when the report is present and matches the manifest.
#[derive(Debug, Clone, Serialize)]
pub(super) struct ValidationSummary {
    pub(super) status: String,
    pub(super) diagnostics: usize,
}

/// One loadable generation.
#[derive(Debug, Clone)]
pub(super) struct Generation {
    pub(super) id: String,
    pub(super) dir: PathBuf,
    pub(super) manifest: DocumentationGenerationManifest,
    pub(super) documents: Vec<DocumentReport>,
    pub(super) validation: Option<ValidationSummary>,
    pub(super) current: bool,
}

/// One directory that looks like a generation but did not validate.
#[derive(Debug, Clone, Serialize)]
pub(super) struct InvalidGeneration {
    pub(super) generation: String,
    pub(super) reason: String,
}

/// Serializable catalog of loadable generations.
#[derive(Debug, Clone, Serialize)]
pub(super) struct CatalogReport {
    pub(super) schema: String,
    pub(super) root: PathBuf,
    pub(super) documentation_root: PathBuf,
    pub(super) current: Option<String>,
    pub(super) current_error: Option<String>,
    pub(super) limits: CatalogLimits,
    pub(super) returned: usize,
    pub(super) omitted_generations: usize,
    pub(super) generations: Vec<GenerationReport>,
    pub(super) invalid_generations: Vec<InvalidGeneration>,
}

/// Serializable summary of one generation.
#[derive(Debug, Clone, Serialize)]
pub(super) struct GenerationReport {
    pub(super) generation: String,
    pub(super) snapshot: String,
    pub(super) profile: String,
    pub(super) status: String,
    pub(super) current: bool,
    pub(super) effective_limits: DocumentationGenerationLimits,
    pub(super) omitted: DocumentationOmittedCounts,
    pub(super) validation: Option<ValidationSummary>,
    pub(super) documents: Vec<DocumentReport>,
}

/// Loaded documentation catalog for one request.
#[derive(Debug, Clone)]
pub(super) struct Catalog {
    root: PathBuf,
    documentation_root: PathBuf,
    generations: Vec<Generation>,
    invalid: Vec<InvalidGeneration>,
    omitted: usize,
    current: Option<String>,
    current_error: Option<String>,
    limits: CatalogLimits,
}

impl Catalog {
    /// Reads and validates every generation the configured bounds allow.
    pub(super) fn load(root: &Path, limits: CatalogLimits) -> Result<Self> {
        Self::load_internal(root, limits, None)
    }

    /// Checks request cancellation/deadlines while enumerating names and inspecting generations.
    pub(super) fn load_for_operation(
        root: &Path,
        limits: CatalogLimits,
        operation: &OperationContext,
    ) -> Result<Self> {
        Self::load_internal(root, limits, Some(operation))
    }

    fn load_internal(
        root: &Path,
        limits: CatalogLimits,
        operation: Option<&OperationContext>,
    ) -> Result<Self> {
        check_active(operation)?;
        let root = root
            .canonicalize()
            .context("failed to resolve the served project root")?;
        let documentation_root = root.join(DOCUMENTATION_ROOT_RELATIVE);
        let mut invalid = Vec::new();
        let mut generations = Vec::new();
        let mut omitted = 0usize;
        let confined_root = confined_directory(&root, &documentation_root)?;
        let generations_root = match confined_root.as_deref() {
            Some(confined_root) => {
                confined_directory(confined_root, &confined_root.join("generations"))?
            }
            None => None,
        };

        if let Some(generations_root) = generations_root {
            // Retain only the newest bounded candidates while walking the directory. Staging and
            // unrelated names are not published generation ids and are never inspected.
            let mut candidates = BTreeMap::new();
            for entry in fs::read_dir(&generations_root)
                .context("failed to read the documentation generations directory")?
            {
                check_active(operation)?;
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        if invalid.len() < limits.max_generations {
                            invalid.push(InvalidGeneration {
                                generation: "unreadable-entry".to_string(),
                                reason: format!("failed to read a generations entry: {error}"),
                            });
                        } else {
                            omitted += 1;
                        }
                        continue;
                    }
                };
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some(number) = name
                    .parse::<u64>()
                    .ok()
                    .filter(|number| *number > 0 && name == format!("{number:08}"))
                else {
                    continue;
                };
                candidates.insert(number, (name, entry.path()));
                if candidates.len() > limits.max_generations {
                    candidates.pop_first();
                    omitted += 1;
                }
            }
            for (_, (id, dir)) in candidates.into_iter().rev() {
                check_active(operation)?;
                match load_generation(&id, &dir, limits) {
                    Ok(generation) => generations.push(generation),
                    Err(error) => invalid.push(InvalidGeneration {
                        generation: id,
                        reason: format!("{error:#}"),
                    }),
                }
            }
        }
        invalid.sort_by(|left, right| right.generation.cmp(&left.generation));

        check_active(operation)?;
        let (current, current_error) = load_current_pointer(&documentation_root, &generations);
        check_active(operation)?;
        if let Some(current) = current.as_deref() {
            for generation in &mut generations {
                generation.current = generation.id == current;
            }
        }

        Ok(Self {
            root,
            documentation_root,
            generations,
            invalid,
            omitted,
            current,
            current_error,
            limits,
        })
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn documentation_root(&self) -> &Path {
        &self.documentation_root
    }

    pub(super) fn generations(&self) -> &[Generation] {
        &self.generations
    }

    pub(super) fn invalid(&self) -> &[InvalidGeneration] {
        &self.invalid
    }

    pub(super) fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    pub(super) fn generation(&self, id: &str) -> Option<&Generation> {
        self.generations
            .iter()
            .find(|generation| generation.id == id)
    }

    /// Reads one declared document and verifies the checksum its manifest states.
    pub(super) fn read_document(
        &self,
        generation_id: &str,
        document_path: &str,
        max_bytes: u64,
    ) -> Result<ServedDocument, CatalogError> {
        let generation = self.generation(generation_id).ok_or_else(|| {
            CatalogError::NotFound(format!("unknown generation `{generation_id}`"))
        })?;
        read_generation_document(generation, document_path, max_bytes)
    }

    /// Re-serializes one validated manifest; the served bytes are the validated contract.
    pub(super) fn manifest_json(&self, generation_id: &str) -> Result<String, CatalogError> {
        let generation = self.generation(generation_id).ok_or_else(|| {
            CatalogError::NotFound(format!("unknown generation `{generation_id}`"))
        })?;
        serde_json::to_string_pretty(&generation.manifest).map_err(|error| {
            CatalogError::Unreadable(format!(
                "failed to serialize the documentation manifest: {error}"
            ))
        })
    }

    pub(super) fn report(&self) -> CatalogReport {
        CatalogReport {
            schema: SERVE_CATALOG_SCHEMA_V1.to_string(),
            root: self.root.clone(),
            documentation_root: self.documentation_root.clone(),
            current: self.current.clone(),
            current_error: self.current_error.clone(),
            limits: self.limits,
            returned: self.generations.len(),
            omitted_generations: self.omitted,
            generations: self
                .generations
                .iter()
                .map(|generation| GenerationReport {
                    generation: generation.id.clone(),
                    snapshot: generation.manifest.snapshot.clone(),
                    profile: profile_label(generation.manifest.profile).to_string(),
                    status: generation_status_label(&generation.manifest),
                    current: generation.current,
                    effective_limits: generation.manifest.effective_limits,
                    omitted: generation.manifest.omitted,
                    validation: generation.validation.clone(),
                    documents: generation.documents.clone(),
                })
                .collect(),
            invalid_generations: self.invalid.clone(),
        }
    }
}

fn check_active(operation: Option<&OperationContext>) -> Result<()> {
    if let Some(operation) = operation {
        operation.check_active()?;
    }
    Ok(())
}

fn load_generation(id: &str, dir: &Path, limits: CatalogLimits) -> Result<Generation> {
    if fs::symlink_metadata(dir)?.file_type().is_symlink() || !dir.is_dir() {
        bail!("a published generation must be a regular directory, not a symlink");
    }
    let dir = dir
        .canonicalize()
        .context("failed to resolve the generation directory")?;
    let manifest_path = dir.join(MANIFEST_FILE);
    let source = read_bounded_text(&dir, &manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest: DocumentationGenerationManifest = serde_json::from_str(&source)
        .with_context(|| format!("failed to parse {MANIFEST_FILE}"))?;
    manifest
        .validate()
        .map_err(|error| anyhow!("invalid documentation manifest: {error}"))?;
    if manifest.generation != id {
        bail!(
            "manifest generation `{}` does not match its directory `{id}`",
            manifest.generation
        );
    }
    if manifest.documents.len() > MAX_DOCUMENTS_PER_GENERATION {
        bail!("a served generation may declare at most {MAX_DOCUMENTS_PER_GENERATION} documents");
    }
    let documents = manifest
        .documents
        .iter()
        .map(|document| DocumentReport {
            id: document.id.clone(),
            path: document.path.clone(),
            media_type: document.media_type.clone(),
            sha256: document.sha256.clone(),
            markdown: document.media_type == "text/markdown",
        })
        .collect();
    let mut generation = Generation {
        id: id.to_string(),
        dir,
        manifest,
        documents,
        validation: None,
        current: false,
    };
    generation.validation = load_validation(&generation, limits);
    Ok(generation)
}

fn read_generation_document(
    generation: &Generation,
    document_path: &str,
    max_bytes: u64,
) -> Result<ServedDocument, CatalogError> {
    let document = generation
        .documents
        .iter()
        .find(|document| document.path == document_path)
        .ok_or_else(|| {
            CatalogError::NotFound(format!(
                "generation `{}` does not declare `{document_path}`",
                generation.id
            ))
        })?;
    let bytes = read_confined_bytes(
        &generation.dir,
        &generation.dir.join(&document.path),
        max_bytes,
    )?;
    if sha256_hex(&bytes) != document.sha256 {
        return Err(CatalogError::ChecksumMismatch(format!(
            "`{document_path}` does not match the checksum its manifest declares"
        )));
    }
    Ok(ServedDocument {
        bytes,
        media_type: content_type_for(&document.media_type),
    })
}

fn load_validation(generation: &Generation, limits: CatalogLimits) -> Option<ValidationSummary> {
    if !generation.documents.iter().any(|document| {
        document.path == VALIDATION_REPORT_FILE && document.media_type == "application/json"
    }) {
        return None;
    }
    let served = read_generation_document(
        generation,
        VALIDATION_REPORT_FILE,
        limits.max_document_bytes.min(MAX_METADATA_BYTES),
    )
    .ok()?;
    let report: DocumentationValidationReport = serde_json::from_slice(&served.bytes).ok()?;
    if report.schema != DocumentationValidationReport::SCHEMA
        || report.draft_schema != DocumentationDraft::SCHEMA
        || report.snapshot != generation.manifest.snapshot
        || report.profile != generation.manifest.profile
    {
        return None;
    }
    let status = match report.status {
        DocumentationValidationStatus::Valid => "valid",
        DocumentationValidationStatus::Invalid => "invalid",
    };
    Some(ValidationSummary {
        status: status.to_string(),
        diagnostics: report.diagnostics.len(),
    })
}

fn load_current_pointer(
    documentation_root: &Path,
    generations: &[Generation],
) -> (Option<String>, Option<String>) {
    let pointer = documentation_root.join(CURRENT_FILE);
    if let Err(error) = fs::symlink_metadata(&pointer) {
        return if error.kind() == std::io::ErrorKind::NotFound {
            (None, None)
        } else {
            (
                None,
                Some(format!(
                    "failed to inspect the current documentation pointer: {error}"
                )),
            )
        };
    }
    let source = match read_bounded_text(documentation_root, &pointer, MAX_METADATA_BYTES) {
        Ok(source) => source,
        Err(error) => {
            return (
                None,
                Some(format!(
                    "failed to read the current documentation pointer: {error:#}"
                )),
            );
        }
    };
    let current: CurrentDocumentationGeneration = match serde_json::from_str(&source) {
        Ok(current) => current,
        Err(error) => {
            return (
                None,
                Some(format!(
                    "failed to parse the current documentation pointer: {error}"
                )),
            );
        }
    };
    if current.schema != DOCUMENTATION_CURRENT_SCHEMA_V1 {
        return (
            None,
            Some("the current documentation pointer carries an unsupported schema".to_string()),
        );
    }
    let Some(generation) = generations
        .iter()
        .find(|generation| generation.id == current.generation)
    else {
        return (
            None,
            Some(format!(
                "the current pointer names generation `{}` which is invalid, absent, or outside the catalog bound",
                current.generation
            )),
        );
    };
    if current.snapshot != generation.manifest.snapshot
        || current.profile != generation.manifest.profile
        || current.path != format!("generations/{}", current.generation)
        || current.manifest != format!("generations/{}/{MANIFEST_FILE}", current.generation)
    {
        return (None, Some("the current documentation pointer does not match the generation identity or layout".to_string()));
    }
    (Some(current.generation), None)
}

fn confined_directory(root: &Path, path: &Path) -> Result<Option<PathBuf>> {
    let canonical = match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to resolve {}", path.display()));
        }
    };
    if !canonical.starts_with(root) || !canonical.is_dir() {
        bail!(
            "{} is not a directory confined to {}",
            path.display(),
            root.display()
        );
    }
    Ok(Some(canonical))
}

fn read_confined_bytes(root: &Path, path: &Path, limit: u64) -> Result<Vec<u8>, CatalogError> {
    let canonical = path.canonicalize().map_err(|error| {
        CatalogError::NotFound(format!("failed to resolve {}: {error}", path.display()))
    })?;
    if !canonical.starts_with(root) {
        return Err(CatalogError::Unreadable(format!(
            "{} resolves outside its publication directory",
            path.display()
        )));
    }
    let metadata =
        fs::metadata(&canonical).map_err(|error| CatalogError::Unreadable(error.to_string()))?;
    if !metadata.is_file() {
        return Err(CatalogError::Unreadable(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    if metadata.len() > limit {
        return Err(CatalogError::TooLarge(format!(
            "{} exceeds the {limit} byte serve limit",
            path.display()
        )));
    }
    let file =
        fs::File::open(&canonical).map_err(|error| CatalogError::Unreadable(error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| CatalogError::Unreadable(error.to_string()))?;
    // Metadata is only a preflight: enforce the bound on the bytes actually read as well.
    if bytes.len() as u64 > limit {
        return Err(CatalogError::TooLarge(format!(
            "{} exceeds the {limit} byte serve limit",
            path.display()
        )));
    }
    Ok(bytes)
}

fn read_bounded_text(root: &Path, path: &Path, limit: u64) -> Result<String> {
    let canonical_root = root
        .canonicalize()
        .context("failed to resolve the publication directory")?;
    let bytes = read_confined_bytes(&canonical_root, path, limit)?;
    String::from_utf8(bytes).with_context(|| format!("{} is not valid UTF-8", path.display()))
}

fn content_type_for(media_type: &str) -> String {
    match media_type {
        "text/markdown" => "text/markdown; charset=utf-8".to_string(),
        "application/json" => "application/json; charset=utf-8".to_string(),
        _ => "text/plain; charset=utf-8".to_string(),
    }
}

pub(super) fn profile_label(profile: DocumentationProfile) -> &'static str {
    match profile {
        DocumentationProfile::Architecture => "architecture",
        DocumentationProfile::Module => "module",
        DocumentationProfile::Api => "api",
        DocumentationProfile::Operations => "operations",
        DocumentationProfile::Onboarding => "onboarding",
    }
}

fn generation_status_label(manifest: &DocumentationGenerationManifest) -> String {
    // The v1 contract has one complete status; the label stays derived from the contract.
    serde_json::to_value(manifest.status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
