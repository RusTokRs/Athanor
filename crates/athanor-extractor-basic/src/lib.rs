use async_trait::async_trait;
use athanor_core::{CoreResult, ExtractInput, ExtractOutput, Extractor, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Evidence, EvidenceStatus, Fact, FactId, FactKind, LanguageCode,
    Ownership, SourceLocation, StableKey,
};
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct FileExtractor;

#[async_trait]
impl Extractor for FileExtractor {
    fn name(&self) -> &'static str {
        "file"
    }

    fn supports(&self, _source: &SourceFile) -> bool {
        true
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let entity = file_entity(&input.source, &input.snapshot.0);
        let fact = Fact {
            id: FactId(format!(
                "fact_file_discovered_{:016x}",
                stable_hash(entity.stable_key.0.as_bytes())
            )),
            kind: FactKind::FileDiscovered,
            subject: entity.id.clone(),
            object: None,
            value: json!({
                "path": input.source.path,
                "content_hash": input.source.content_hash,
                "language_hint": input.source.language_hint,
            }),
            evidence: vec![evidence_for_file(&entity.name, self.name(), None, None)],
            ownership: ownership_for_file(&entity.name),
            snapshot: input.snapshot,
            extractor: self.name().to_string(),
            confidence: 1.0,
        };

        Ok(ExtractOutput {
            entities: vec![entity],
            facts: vec![fact],
            diagnostics: Vec::new(),
        })
    }
}

pub fn file_entity(source: &SourceFile, snapshot: &str) -> Entity {
    let stable_key = StableKey(format!("file://{}", source.path));

    Entity {
        id: EntityId(format!(
            "ent_file_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        )),
        stable_key,
        kind: EntityKind::File,
        name: source.path.clone(),
        title: None,
        source: Some(SourceLocation {
            path: source.path.clone(),
            line_start: None,
            line_end: None,
        }),
        language: source
            .language_hint
            .as_ref()
            .map(|language| LanguageCode(language.clone())),
        aliases: Vec::new(),
        ownership: ownership_for_file(&source.path),
        payload: json!({
            "snapshot": snapshot,
            "content_hash": source.content_hash,
            "has_text_content": source.content.is_some(),
        }),
    }
}

pub fn ownership_for_file(path: &str) -> Vec<Ownership> {
    vec![Ownership {
        source_file: path.to_string(),
    }]
}

pub fn evidence_for_file(
    path: &str,
    extractor: &str,
    line_start: Option<u32>,
    line_end: Option<u32>,
) -> Evidence {
    Evidence {
        source_file: Some(path.to_string()),
        line_start,
        line_end,
        extractor: Some(extractor.to_string()),
        commit_hash: None,
        confidence: 1.0,
        status: EvidenceStatus::Verified,
    }
}

pub fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

#[cfg(test)]
mod tests {
    use athanor_domain::{RepoId, SnapshotId};

    use super::*;

    #[tokio::test]
    async fn file_extractor_emits_file_entity_and_fact() {
        let extractor = FileExtractor;
        let output = extractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "src/lib.rs".to_string(),
                    language_hint: Some("rust".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some("pub fn hello() {}".to_string()),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.entities[0].stable_key.0, "file://src/lib.rs");
        assert_eq!(output.facts.len(), 1);
        assert_eq!(output.facts[0].kind, FactKind::FileDiscovered);
    }
}
