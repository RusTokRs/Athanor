use std::collections::BTreeSet;

use athanor_domain::{Entity, Evidence, EvidenceStatus};
use serde_json::Value;

use super::model::ChangeMapAnnotation;

pub(super) fn entity_files(entity: &Entity) -> Vec<String> {
    let mut files = entity
        .ownership
        .iter()
        .map(|ownership| ownership.source_file.clone())
        .collect::<BTreeSet<_>>();
    if let Some(source) = &entity.source {
        files.insert(source.path.clone());
    }
    files.into_iter().collect()
}

pub(super) fn entity_evidence(entity: &Entity) -> Vec<Evidence> {
    if let Some(source) = &entity.source {
        return vec![Evidence {
            source_file: Some(source.path.clone()),
            line_start: source.line_start,
            line_end: source.line_end,
            extractor: Some("canonical_entity".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Verified,
        }];
    }
    entity
        .ownership
        .iter()
        .map(|ownership| Evidence {
            source_file: Some(ownership.source_file.clone()),
            line_start: None,
            line_end: None,
            extractor: Some("canonical_ownership".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Verified,
        })
        .collect()
}

pub(super) fn dedupe_evidence(evidence: &mut Vec<Evidence>) {
    evidence.sort_by_key(|item| {
        (
            item.source_file.clone(),
            item.line_start,
            item.line_end,
            item.extractor.clone(),
        )
    });
    evidence.dedup_by(|left, right| {
        left.source_file == right.source_file
            && left.line_start == right.line_start
            && left.line_end == right.line_end
            && left.extractor == right.extractor
    });
}

pub(super) fn annotations_from_payload(payload: &Value, message: &str) -> Vec<ChangeMapAnnotation> {
    let Some(schema) = payload.get("schema").and_then(Value::as_str) else {
        return Vec::new();
    };
    let source = schema.split('.').next().unwrap_or("adapter");
    if matches!(source, "athanor" | "") {
        return Vec::new();
    }
    vec![ChangeMapAnnotation {
        source: source.to_string(),
        schema: schema.to_string(),
        message: format!("{source} adapter context from {message}"),
    }]
}
