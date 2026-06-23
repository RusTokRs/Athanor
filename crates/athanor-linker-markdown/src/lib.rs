use async_trait::async_trait;
use athanor_core::{CoreResult, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityKind, Evidence, EvidenceStatus, Ownership, Relation, RelationId, RelationKind,
    RelationStatus, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
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
        let affected_paths = input
            .affected
            .entities
            .iter()
            .filter_map(|entity| entity.source.as_ref().map(|source| source.path.as_str()))
            .collect::<std::collections::HashSet<_>>();
        let pages = input
            .entities
            .iter()
            .filter(|entity| {
                entity.kind == EntityKind::DocumentationPage
                    && entity
                        .source
                        .as_ref()
                        .is_some_and(|source| affected_paths.contains(source.path.as_str()))
            })
            .collect::<Vec<_>>();
        let sections = input
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::DocumentationSection)
            .collect::<Vec<_>>();
        let runbooks = input
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Runbook)
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

            for runbook in runbooks
                .iter()
                .copied()
                .filter(|runbook| same_source_path(page, runbook))
            {
                relations.push(contains_relation(
                    &input.snapshot,
                    page,
                    runbook,
                    self.name(),
                    "documentation_page_contains_runbook",
                ));
            }
        }

        let affected_ids = input
            .affected
            .entities
            .iter()
            .map(|entity| &entity.id)
            .collect::<std::collections::HashSet<_>>();
        let all_pages = input
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::DocumentationPage);
        for page in all_pages {
            for (reference_type, stable_key) in declared_references(page) {
                let targets = input
                    .entities
                    .iter()
                    .filter(|entity| entity.stable_key.0 == stable_key)
                    .collect::<Vec<_>>();
                if let [target] = targets.as_slice() {
                    let target = *target;
                    if affected_ids.contains(&page.id) || affected_ids.contains(&target.id) {
                        relations.push(documents_relation(
                            &input.snapshot,
                            page,
                            target,
                            self.name(),
                            reference_type,
                        ));
                    }
                }
            }
        }

        Ok(relations)
    }
}

fn declared_references(entity: &Entity) -> Vec<(&'static str, &str)> {
    ["entities", "concepts"]
        .into_iter()
        .flat_map(|field| {
            entity
                .payload
                .get(field)
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(move |stable_key| (field, stable_key))
        })
        .collect()
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
        ownership: ownership_for_entities(from, to),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": from.stable_key.0,
            "to": to.stable_key.0,
            "reason": reason,
        }),
    }
}

fn documents_relation(
    snapshot: &SnapshotId,
    page: &Entity,
    target: &Entity,
    linker: &str,
    reference_type: &str,
) -> Relation {
    let id_material = format!(
        "documents\0{}\0{}\0{reference_type}",
        page.stable_key.0, target.stable_key.0
    );
    Relation {
        id: RelationId(format!(
            "rel_documents_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: RelationKind::Documents,
        from: page.id.clone(),
        to: target.id.clone(),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence: vec![evidence_for_declared_reference(page, linker)],
        ownership: ownership_for_entities(page, target),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": page.stable_key.0,
            "to": target.stable_key.0,
            "reason": "markdown_frontmatter_reference",
            "reference_type": reference_type,
        }),
    }
}

fn ownership_for_entities(from: &Entity, to: &Entity) -> Vec<Ownership> {
    let mut ownership = from.ownership.clone();

    for owner in &to.ownership {
        if !ownership
            .iter()
            .any(|existing| existing.source_file == owner.source_file)
        {
            ownership.push(owner.clone());
        }
    }

    ownership
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

fn evidence_for_declared_reference(page: &Entity, linker: &str) -> Evidence {
    let source = page.source.as_ref();
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
                entities: vec![file.clone(), page.clone(), section.clone()].into(),
                facts: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(
                    vec![file.clone(), page.clone(), section.clone()],
                    Vec::new(),
                ),
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

    #[tokio::test]
    async fn limits_links_to_affected_source_paths() {
        let auth_file = entity(
            "ent_file_auth",
            "file://docs/auth.md",
            EntityKind::File,
            "docs/auth.md",
        );
        let auth_page = entity(
            "ent_doc_page_auth",
            "doc://docs/auth.md",
            EntityKind::DocumentationPage,
            "docs/auth.md",
        );
        let billing_file = entity(
            "ent_file_billing",
            "file://docs/billing.md",
            EntityKind::File,
            "docs/billing.md",
        );
        let billing_page = entity(
            "ent_doc_page_billing",
            "doc://docs/billing.md",
            EntityKind::DocumentationPage,
            "docs/billing.md",
        );

        let relations = MarkdownContainmentLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![
                    auth_file.clone(),
                    auth_page.clone(),
                    billing_file.clone(),
                    billing_page.clone(),
                ]
                .into(),
                facts: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(
                    vec![auth_file.clone(), auth_page.clone()],
                    Vec::new(),
                ),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].from, auth_file.id);
        assert_eq!(relations[0].to, auth_page.id);
    }

    #[tokio::test]
    async fn links_explicit_frontmatter_references_as_verified_documents_relations() {
        let mut page = entity(
            "ent_doc_page_auth",
            "doc://docs/auth.md",
            EntityKind::DocumentationPage,
            "docs/auth.md",
        );
        page.payload = json!({
            "entities": ["api://POST:/login"],
            "concepts": []
        });
        let endpoint = entity(
            "ent_endpoint_login",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );

        let relations = MarkdownContainmentLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![page.clone(), endpoint.clone()].into(),
                facts: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(
                    vec![page.clone()],
                    Vec::new(),
                ),
            })
            .await
            .unwrap();

        let relation = relations
            .iter()
            .find(|relation| relation.kind == RelationKind::Documents)
            .unwrap();
        assert_eq!(relation.from, page.id);
        assert_eq!(relation.to, endpoint.id);
        assert_eq!(relation.status, RelationStatus::Verified);
        assert_eq!(relation.payload["reference_type"], "entities");
        assert_eq!(relation.ownership.len(), 2);
        assert_eq!(
            relation.evidence[0].source_file.as_deref(),
            Some("docs/auth.md")
        );
    }

    #[tokio::test]
    async fn rebuilds_explicit_relation_when_only_target_is_affected() {
        let mut page = entity(
            "ent_doc_page_auth",
            "doc://docs/auth.md",
            EntityKind::DocumentationPage,
            "docs/auth.md",
        );
        page.payload = json!({ "entities": ["api://POST:/login"] });
        let endpoint = entity(
            "ent_endpoint_login",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );

        let relations = MarkdownContainmentLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![page, endpoint.clone()].into(),
                facts: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(vec![endpoint], Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(
            relations
                .iter()
                .filter(|relation| relation.kind == RelationKind::Documents)
                .count(),
            1
        );
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
            ownership: athanor_extractor_basic::ownership_for_file(path),
            payload: json!({}),
        }
    }
}
