//! Versioned contracts for evidence-backed documentation generation.
//!
//! These types define the bounded application/projector boundary. They deliberately do not
//! implement planning, composition, rendering, publication, or provider access.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const DOCUMENTATION_GENERATION_REQUEST_SCHEMA_V1: &str =
    "athanor.documentation_generation_request.v1";
pub const DOCUMENTATION_GENERATION_MANIFEST_SCHEMA_V1: &str =
    "athanor.documentation_generation_manifest.v1";
pub const DOCUMENTATION_GENERATION_LIMIT_MAX: usize = 100_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationProfile {
    Architecture,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationGenerationLimits {
    pub max_entities: usize,
    pub max_facts: usize,
    pub max_relations: usize,
    pub max_diagnostics: usize,
}

impl DocumentationGenerationLimits {
    fn validate(self) -> Result<(), DocumentationContractError> {
        for (field, value) in [
            ("max_entities", self.max_entities),
            ("max_facts", self.max_facts),
            ("max_relations", self.max_relations),
            ("max_diagnostics", self.max_diagnostics),
        ] {
            if !(1..=DOCUMENTATION_GENERATION_LIMIT_MAX).contains(&value) {
                return Err(DocumentationContractError(format!(
                    "documentation generation limit {field} must be between 1 and {DOCUMENTATION_GENERATION_LIMIT_MAX}"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationGenerationRequest {
    pub schema: String,
    pub snapshot: String,
    pub profile: DocumentationProfile,
    pub limits: DocumentationGenerationLimits,
}

impl DocumentationGenerationRequest {
    pub const SCHEMA: &'static str = DOCUMENTATION_GENERATION_REQUEST_SCHEMA_V1;

    pub fn new(
        snapshot: impl Into<String>,
        profile: DocumentationProfile,
        limits: DocumentationGenerationLimits,
    ) -> Self {
        Self {
            schema: Self::SCHEMA.to_string(),
            snapshot: snapshot.into(),
            profile,
            limits,
        }
    }

    pub fn validate(&self) -> Result<(), DocumentationContractError> {
        validate_schema("documentation generation request", &self.schema, Self::SCHEMA)?;
        validate_non_empty("snapshot", &self.snapshot)?;
        self.limits.validate()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationGenerationStatus {
    Complete,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationOmittedCounts {
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationDocumentManifest {
    pub id: String,
    pub path: String,
    pub media_type: String,
    pub sha256: String,
}

impl DocumentationDocumentManifest {
    fn validate(&self) -> Result<(), DocumentationContractError> {
        validate_non_empty("document id", &self.id)?;
        validate_non_empty("document media_type", &self.media_type)?;
        validate_relative_output_path(&self.path)?;
        validate_sha256(&self.sha256)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationGenerationManifest {
    pub schema: String,
    pub request_schema: String,
    pub generation: String,
    pub snapshot: String,
    pub profile: DocumentationProfile,
    pub status: DocumentationGenerationStatus,
    pub effective_limits: DocumentationGenerationLimits,
    pub omitted: DocumentationOmittedCounts,
    pub documents: Vec<DocumentationDocumentManifest>,
}

impl DocumentationGenerationManifest {
    pub const SCHEMA: &'static str = DOCUMENTATION_GENERATION_MANIFEST_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), DocumentationContractError> {
        validate_schema("documentation generation manifest", &self.schema, Self::SCHEMA)?;
        validate_schema(
            "documentation generation manifest request",
            &self.request_schema,
            DocumentationGenerationRequest::SCHEMA,
        )?;
        validate_non_empty("generation", &self.generation)?;
        validate_non_empty("snapshot", &self.snapshot)?;
        self.effective_limits.validate()?;
        if self.documents.is_empty() {
            return Err(DocumentationContractError(
                "documentation generation manifest must contain at least one document".to_string(),
            ));
        }

        let mut ids = BTreeSet::new();
        let mut portable_paths = BTreeSet::new();
        for document in &self.documents {
            document.validate()?;
            if !ids.insert(document.id.as_str()) {
                return Err(DocumentationContractError(format!(
                    "duplicate documentation document id {}",
                    document.id
                )));
            }
            if !portable_paths.insert(document.path.to_ascii_lowercase()) {
                return Err(DocumentationContractError(format!(
                    "duplicate portable documentation document path {}",
                    document.path
                )));
            }
        }
        Ok(())
    }

    pub fn validate_for_request(
        &self,
        request: &DocumentationGenerationRequest,
    ) -> Result<(), DocumentationContractError> {
        request.validate()?;
        self.validate()?;
        if self.snapshot != request.snapshot {
            return Err(DocumentationContractError(format!(
                "manifest snapshot {} does not match request snapshot {}",
                self.snapshot, request.snapshot
            )));
        }
        if self.profile != request.profile {
            return Err(DocumentationContractError(
                "manifest profile does not match request profile".to_string(),
            ));
        }
        if self.effective_limits != request.limits {
            return Err(DocumentationContractError(
                "manifest effective_limits do not match request limits".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationContractError(pub String);

impl fmt::Display for DocumentationContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for DocumentationContractError {}

fn validate_schema(
    owner: &str,
    actual: &str,
    expected: &str,
) -> Result<(), DocumentationContractError> {
    if actual == expected {
        Ok(())
    } else {
        Err(DocumentationContractError(format!(
            "{owner} schema {actual} does not match {expected}"
        )))
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), DocumentationContractError> {
    if value.is_empty() || value.trim() != value {
        Err(DocumentationContractError(format!(
            "documentation generation {field} must be non-empty and trimmed"
        )))
    } else {
        Ok(())
    }
}

fn validate_relative_output_path(path: &str) -> Result<(), DocumentationContractError> {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') || path.contains('\\') {
        return Err(invalid_output_path(path));
    }

    for component in path.split('/') {
        if component.is_empty()
            || matches!(component, "." | "..")
            || component.ends_with(['.', ' '])
            || component
                .chars()
                .any(|character| character.is_control() || character == ':')
            || is_windows_reserved_component(component)
        {
            return Err(invalid_output_path(path));
        }
    }
    Ok(())
}

fn invalid_output_path(path: &str) -> DocumentationContractError {
    DocumentationContractError(format!(
        "documentation document path {path} must be a normalized portable relative slash path"
    ))
}

fn is_windows_reserved_component(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or_default();
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "CLOCK$"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn validate_sha256(value: &str) -> Result<(), DocumentationContractError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(DocumentationContractError(
            "documentation document sha256 must be 64 lowercase hexadecimal characters"
                .to_string(),
        ))
    }
}
