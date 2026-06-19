use async_trait::async_trait;
use athanor_core::{CoreResult, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityKind, Evidence, EvidenceStatus, Relation, RelationId, RelationKind,
    RelationStatus, SnapshotId,
};
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct MarkdownContainmentLinker;

#[async_trait]
impl Linker for MarkdownContainmentLinker {
    fn name(&self) -> &'static str {
        "markdown-containment"
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
        let mut relations = Vec::new();
        let files = input
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::File)
            .collect::<Vec<_>>();
        let pages = input
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::DocumentationPage)
            .collect::<Vec<_>>();
        let sections = input
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::DocumentationSection)
            .collect::<Vec<_>>();

        for page in pages {
            if let Some(file) = matching_file(page, &files) {
                relations.push(contains_relation(
                    &input.snapshot,
                    file,
                    page,
                    self.name(),
                    "file_contains_documentation_page",
                ));
            }

            for section in sections
                .iter()
                .copied()
                .filter(|section| same_source_path(page, section))
            {
                relations.push(contains_relation(
                    &input.snapshot,
                    page,
                    section,
                    self.name(),
                    "documentation_page_contains_section",
                ));
            }
        }

        Ok(relations)
    }
}

fn matching_file<'a>(page: &Entity, files: &'a [&Entity]) -> Option<&'a Entity> {
    let page_path = page.source.as_ref().map(|source| source.path.as_str())?;

    files.iter().copied().find(|file| {
        file.source
            .as_ref()
            .is_some_and(|source| source.path == page_path)
    })
}

fn same_source_path(left: &Entity, right: &Entity) -> bool {
    left.source
        .as_ref()
        .zip(right.source.as_ref())
        .is_some_and(|(left, right)| left.path == right.path)
}

fn contains_relation(
    snapshot: &SnapshotId,
    from: &Entity,
    to: &Entity,
    linker: &str,
    reason: &str,
) -> Relation {
    Relation {
        id: RelationId(format!(
            "rel_contains_{:016x}",
            stable_hash(format!("{}->{}", from.stable_key.0, to.stable_key.0).as_bytes())
        )),
        kind: RelationKind::Contains,
        from: from.id.clone(),
        to: to.id.clone(),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence: vec![evidence_for_entities(from, to, linker)],
        snapshot: snapshot.clone(),
        payload: json!({
            "from": from.stable_key.0,
            "to": to.stable_key.0,
            "reason": reason,
        }),
    }
}

fn evidence_for_entities(from: &Entity, to: &Entity, linker: &str) -> Evidence {
    let source = to.source.as_ref().or(from.source.as_ref());

    Evidence {
        source_file: source.map(|source| source.path.clone()),
        line_start: source.and_then(|source| source.line_start),
        line_end: source.and_then(|source| source.line_end),
        extractor: Some(linker.to_string()),
        commit_hash: None,
        confidence: 1.0,
        status: EvidenceStatus::Verified,
    }
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

#[cfg(test)]
mod tests {
    use athanor_domain::{EntityId, LanguageCode, SourceLocation, StableKey};

    use super::*;

    #[tokio::test]
    async fn links_markdown_file_page_and_sections() {
        let file = entity(
            "ent_file_docs_auth",
            "file://docs/auth.md",
            EntityKind::File,
            "docs/auth.md",
        );
        let page = entity(
            "ent_doc_page_auth",
            "doc://docs/auth.md",
            EntityKind::DocumentationPage,
            "docs/auth.md",
        );
        let section = entity(
            "ent_doc_section_login",
            "doc://docs/auth.md#login",
            EntityKind::DocumentationSection,
            "docs/auth.md",
        );

        let relations = MarkdownContainmentLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![file.clone(), page.clone(), section.clone()],
                facts: Vec::new(),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 2);
        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::Contains
                && relation.from == file.id
                && relation.to == page.id
        }));
        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::Contains
                && relation.from == page.id
                && relation.to == section.id
        }));
    }

    fn entity(id: &str, stable_key: &str, kind: EntityKind, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: path.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("markdown".to_string())),
            aliases: Vec::new(),
            payload: json!({}),
        }
    }
}
