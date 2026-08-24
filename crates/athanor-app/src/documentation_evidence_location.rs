//! Canonical evidence-location normalization for deterministic documentation profiles.

use std::collections::BTreeSet;

use athanor_domain::{Entity, Evidence, SourceLocation};

use crate::DocumentationEvidenceLocation;

pub(crate) fn entity_evidence_locations(entity: &Entity) -> Vec<DocumentationEvidenceLocation> {
    let mut locations = entity
        .source
        .as_ref()
        .and_then(source_evidence_location)
        .into_iter()
        .collect::<Vec<_>>();
    locations.extend(
        entity
            .ownership
            .iter()
            .filter_map(|ownership| evidence_location(&ownership.source_file, None, None)),
    );
    deduplicate(locations)
}

#[allow(dead_code)]
pub(crate) fn evidence_locations<'a>(
    evidence: &[Evidence],
    ownership_paths: impl IntoIterator<Item = &'a String>,
) -> Vec<DocumentationEvidenceLocation> {
    let mut locations = evidence
        .iter()
        .filter_map(|evidence| {
            evidence
                .source_file
                .as_ref()
                .and_then(|path| evidence_location(path, evidence.line_start, evidence.line_end))
        })
        .collect::<Vec<_>>();
    locations.extend(
        ownership_paths
            .into_iter()
            .filter_map(|path| evidence_location(path, None, None)),
    );
    deduplicate(locations)
}

fn source_evidence_location(source: &SourceLocation) -> Option<DocumentationEvidenceLocation> {
    evidence_location(&source.path, source.line_start, source.line_end)
}

fn evidence_location(
    path: &str,
    start: Option<u32>,
    end: Option<u32>,
) -> Option<DocumentationEvidenceLocation> {
    let path = path.replace('\\', "/");
    if !is_portable_relative_path(&path) {
        return None;
    }
    let start_line = start.unwrap_or(1).max(1);
    Some(DocumentationEvidenceLocation {
        path,
        start_line,
        end_line: end.unwrap_or(start_line).max(start_line),
    })
}

fn is_portable_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\\')
        && path.split('/').all(|component| {
            !component.is_empty()
                && !matches!(component, "." | "..")
                && !component.ends_with('.')
                && !component.ends_with(' ')
                && !component
                    .chars()
                    .any(|character| character.is_control() || character == ':')
                && !is_windows_reserved(component)
        })
}

fn is_windows_reserved(component: &str) -> bool {
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

fn deduplicate(
    locations: Vec<DocumentationEvidenceLocation>,
) -> Vec<DocumentationEvidenceLocation> {
    locations
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use athanor_domain::{EntityId, EntityKind, Ownership, StableKey};
    use serde_json::json;

    use super::*;

    #[test]
    fn entity_locations_normalize_slashes_and_deduplicate_ownership() {
        let entity = Entity {
            id: EntityId("entity".to_string()),
            stable_key: StableKey("api://GET:/health".to_string()),
            kind: EntityKind::ApiEndpoint,
            name: "health".to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "api\\openapi.yaml".to_string(),
                line_start: Some(4),
                line_end: Some(6),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: "api/openapi.yaml".to_string(),
            }],
            payload: json!({}),
        };

        assert_eq!(
            entity_evidence_locations(&entity),
            vec![
                DocumentationEvidenceLocation {
                    path: "api/openapi.yaml".to_string(),
                    start_line: 1,
                    end_line: 1,
                },
                DocumentationEvidenceLocation {
                    path: "api/openapi.yaml".to_string(),
                    start_line: 4,
                    end_line: 6,
                },
            ]
        );
    }
}
