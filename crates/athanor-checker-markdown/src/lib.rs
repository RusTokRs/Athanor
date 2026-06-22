use std::collections::BTreeMap;

use async_trait::async_trait;
use athanor_core::{CheckInput, Checker, CoreResult};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityKind, Evidence,
    EvidenceStatus, Ownership, RelationKind, Severity, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct MarkdownStructureChecker;

#[async_trait]
impl Checker for MarkdownStructureChecker {
    fn name(&self) -> &'static str {
        "markdown-structure"
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        for page in input
            .affected
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::DocumentationPage)
        {
            if page
                .title
                .as_ref()
                .is_none_or(|title| title.trim().is_empty())
            {
                diagnostics.push(diagnostic(
                    page,
                    DiagnosticKind::DocumentationPageMissingTitle,
                    Severity::Medium,
                    "Documentation page has no top-level title",
                    "Markdown documentation page should start with a clear H1 title.",
                    self.name(),
                    &input.snapshot,
                ));
            }

            let section_count = input
                .affected
                .relations
                .iter()
                .filter(|relation| {
                    relation.kind == RelationKind::Contains && relation.from == page.id
                })
                .count();

            if section_count == 0 {
                diagnostics.push(diagnostic(
                    page,
                    DiagnosticKind::EmptyDocumentationPage,
                    Severity::Low,
                    "Documentation page has no sections",
                    "Markdown documentation page does not expose any heading sections.",
                    self.name(),
                    &input.snapshot,
                ));
            }
        }

        if !input.affected.entities.is_empty() {
            let candidate_ownership = ownership_for_candidates(&input.entities);
            let pages = input
                .entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::DocumentationPage)
                .collect::<Vec<_>>();
            let mut entities_by_stable_key = BTreeMap::<&str, Vec<&Entity>>::new();
            for entity in input.entities.iter() {
                entities_by_stable_key
                    .entry(&entity.stable_key.0)
                    .or_default()
                    .push(entity);
            }

            for page in &pages {
                for (reference_type, stable_key) in declared_references(page) {
                    match entities_by_stable_key
                        .get(stable_key)
                        .map(Vec::len)
                        .unwrap_or(0)
                    {
                        0 => diagnostics.push(unresolved_reference_diagnostic(
                            page,
                            reference_type,
                            stable_key,
                            self.name(),
                            &input.snapshot,
                            candidate_ownership.clone(),
                        )),
                        1 => {}
                        _ => diagnostics.push(ambiguous_reference_diagnostic(
                            page,
                            reference_type,
                            stable_key,
                            self.name(),
                            &input.snapshot,
                            candidate_ownership.clone(),
                        )),
                    }
                }
            }

            let mut pages_by_stable_key = BTreeMap::<&str, Vec<&Entity>>::new();
            for page in pages {
                pages_by_stable_key
                    .entry(&page.stable_key.0)
                    .or_default()
                    .push(page);
            }
            for (stable_key, duplicates) in pages_by_stable_key {
                if duplicates.len() > 1 {
                    diagnostics.push(duplicate_document_id_diagnostic(
                        stable_key,
                        &duplicates,
                        self.name(),
                        &input.snapshot,
                        candidate_ownership.clone(),
                    ));
                }
            }
        }

        Ok(diagnostics)
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

fn unresolved_reference_diagnostic(
    page: &Entity,
    reference_type: &str,
    stable_key: &str,
    checker: &str,
    snapshot: &SnapshotId,
    ownership: Vec<Ownership>,
) -> Diagnostic {
    let id_material = format!(
        "broken_documentation_reference\0{}\0{reference_type}\0{stable_key}",
        page.stable_key.0
    );
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_broken_doc_reference_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: DiagnosticKind::DocumentationReferenceUnresolved,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Documentation frontmatter reference is unresolved".to_string(),
        message: format!(
            "The {reference_type} frontmatter entry `{stable_key}` does not match a canonical entity."
        ),
        entities: vec![page.id.clone()],
        evidence: vec![evidence_for_entity_with_status(
            page,
            checker,
            EvidenceStatus::Missing,
        )],
        ownership,
        snapshot: snapshot.clone(),
        suggested_fix: Some(format!(
            "Correct `{stable_key}` or remove it from the `{reference_type}` frontmatter list."
        )),
        payload: json!({
            "reason": "unresolved_frontmatter_reference",
            "documentation_page": page.stable_key.0,
            "reference_type": reference_type,
            "reference": stable_key,
        }),
    }
}

fn ambiguous_reference_diagnostic(
    page: &Entity,
    reference_type: &str,
    stable_key: &str,
    checker: &str,
    snapshot: &SnapshotId,
    ownership: Vec<Ownership>,
) -> Diagnostic {
    let id_material = format!(
        "ambiguous_documentation_reference\0{}\0{reference_type}\0{stable_key}",
        page.stable_key.0
    );
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_ambiguous_doc_reference_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: DiagnosticKind::DocumentationReferenceUnresolved,
        severity: Severity::High,
        status: DiagnosticStatus::Open,
        title: "Documentation frontmatter reference is ambiguous".to_string(),
        message: format!(
            "The {reference_type} frontmatter entry `{stable_key}` matches multiple canonical entities."
        ),
        entities: vec![page.id.clone()],
        evidence: vec![evidence_for_entity_with_status(
            page,
            checker,
            EvidenceStatus::Conflicting,
        )],
        ownership,
        snapshot: snapshot.clone(),
        suggested_fix: Some(format!(
            "Make `{stable_key}` unique before using it in the `{reference_type}` frontmatter list."
        )),
        payload: json!({
            "reason": "ambiguous_frontmatter_reference",
            "documentation_page": page.stable_key.0,
            "reference_type": reference_type,
            "reference": stable_key,
        }),
    }
}

fn duplicate_document_id_diagnostic(
    stable_key: &str,
    pages: &[&Entity],
    checker: &str,
    snapshot: &SnapshotId,
    ownership: Vec<Ownership>,
) -> Diagnostic {
    let mut entity_ids = pages.iter().map(|page| page.id.clone()).collect::<Vec<_>>();
    entity_ids.dedup();
    let sources = pages
        .iter()
        .filter_map(|page| page.source.as_ref().map(|source| source.path.clone()))
        .collect::<Vec<_>>();
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_duplicate_doc_id_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        kind: DiagnosticKind::DuplicateDocumentationId,
        severity: Severity::High,
        status: DiagnosticStatus::Open,
        title: "Documentation page identity is duplicated".to_string(),
        message: format!(
            "The stable key `{stable_key}` is declared by multiple Markdown pages: {}.",
            sources.join(", ")
        ),
        entities: entity_ids,
        evidence: pages
            .iter()
            .map(|page| evidence_for_entity_with_status(page, checker, EvidenceStatus::Conflicting))
            .collect(),
        ownership,
        snapshot: snapshot.clone(),
        suggested_fix: Some(
            "Assign a unique fragment-free `doc://` id to each Markdown page.".to_string(),
        ),
        payload: json!({
            "reason": "duplicate_documentation_id",
            "stable_key": stable_key,
            "sources": sources,
        }),
    }
}

fn ownership_for_candidates(entities: &[Entity]) -> Vec<Ownership> {
    let mut ownership = Vec::new();
    for owner in entities.iter().flat_map(|entity| entity.ownership.iter()) {
        if !ownership
            .iter()
            .any(|existing: &Ownership| existing.source_file == owner.source_file)
        {
            ownership.push(owner.clone());
        }
    }
    ownership
}

fn diagnostic(
    entity: &Entity,
    kind: DiagnosticKind,
    severity: Severity,
    title: &str,
    message: &str,
    checker: &str,
    snapshot: &SnapshotId,
) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_{}_{:016x}",
            diagnostic_slug(&kind),
            stable_hash(entity.stable_key.0.as_bytes())
        )),
        kind,
        severity,
        status: DiagnosticStatus::Open,
        title: title.to_string(),
        message: message.to_string(),
        entities: vec![entity.id.clone()],
        evidence: vec![evidence_for_entity(entity, checker)],
        ownership: entity.ownership.clone(),
        snapshot: snapshot.clone(),
        suggested_fix: None,
        payload: json!({
            "entity": entity.stable_key.0,
        }),
    }
}

fn evidence_for_entity(entity: &Entity, checker: &str) -> Evidence {
    evidence_for_entity_with_status(entity, checker, EvidenceStatus::Verified)
}

fn evidence_for_entity_with_status(
    entity: &Entity,
    checker: &str,
    status: EvidenceStatus,
) -> Evidence {
    let source = entity.source.as_ref();

    Evidence {
        source_file: source.map(|source| source.path.clone()),
        line_start: source.and_then(|source| source.line_start),
        line_end: source.and_then(|source| source.line_end),
        extractor: Some(checker.to_string()),
        commit_hash: None,
        confidence: 1.0,
        status,
    }
}

fn diagnostic_slug(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::EmptyDocumentationPage => "empty_documentation_page",
        DiagnosticKind::DocumentationPageMissingTitle => "documentation_page_missing_title",
        _ => "diagnostic",
    }
}

#[cfg(test)]
mod tests {
    use athanor_domain::{
        EntityId, LanguageCode, Relation, RelationId, RelationStatus, SnapshotId, SourceLocation,
        StableKey,
    };

    use super::*;

    #[tokio::test]
    async fn reports_page_without_title() {
        let page = page_entity(None);

        let diagnostics = MarkdownStructureChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![page.clone()].into(),
                facts: Vec::new().into(),
                relations: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(vec![page], Vec::new()),
            })
            .await
            .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::DocumentationPageMissingTitle
        }));
    }

    #[tokio::test]
    async fn does_not_report_empty_page_when_section_relation_exists() {
        let page = page_entity(Some("Auth"));
        let section = Entity {
            id: EntityId("ent_section".to_string()),
            stable_key: StableKey("doc://docs/auth.md#login".to_string()),
            kind: EntityKind::DocumentationSection,
            name: "Login".to_string(),
            title: Some("Login".to_string()),
            source: Some(SourceLocation {
                path: "docs/auth.md".to_string(),
                line_start: Some(3),
                line_end: Some(3),
            }),
            language: Some(LanguageCode("markdown".to_string())),
            aliases: Vec::new(),
            ownership: athanor_extractor_basic::ownership_for_file("docs/auth.md"),
            payload: json!({}),
        };
        let relation = Relation {
            id: RelationId("rel_contains".to_string()),
            kind: RelationKind::Contains,
            from: page.id.clone(),
            to: section.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: athanor_extractor_basic::ownership_for_file("docs/auth.md"),
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        };

        let diagnostics = MarkdownStructureChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![page.clone(), section.clone()].into(),
                facts: Vec::new().into(),
                relations: vec![relation.clone()].into(),
                affected: athanor_core::AffectedSubset::from_extracted(
                    vec![page, section],
                    Vec::new(),
                )
                .with_relations(vec![relation]),
            })
            .await
            .unwrap();

        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::EmptyDocumentationPage)
        );
    }

    #[tokio::test]
    async fn limits_diagnostics_to_affected_pages() {
        let affected_page =
            page_entity_with_id("ent_affected_page", "doc://docs/affected.md", None);
        let unaffected_page =
            page_entity_with_id("ent_unaffected_page", "doc://docs/unaffected.md", None);

        let diagnostics = MarkdownStructureChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![affected_page.clone(), unaffected_page.clone()].into(),
                facts: Vec::new().into(),
                relations: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(
                    vec![affected_page.clone()],
                    Vec::new(),
                ),
            })
            .await
            .unwrap();

        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.entities == vec![affected_page.id.clone()] })
        );
    }

    #[tokio::test]
    async fn reports_unresolved_frontmatter_reference_with_candidate_ownership() {
        let mut page = page_entity(Some("Auth"));
        page.payload = json!({ "entities": ["api://POST:/missing"] });
        let candidate = entity_with_kind(
            "ent_candidate",
            "api://POST:/other",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );

        let diagnostics = MarkdownStructureChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![page.clone(), candidate].into(),
                facts: Vec::new().into(),
                relations: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(
                    vec![page.clone()],
                    Vec::new(),
                ),
            })
            .await
            .unwrap();

        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.payload["reason"] == "unresolved_frontmatter_reference")
            .unwrap();
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::DocumentationReferenceUnresolved
        );
        assert_eq!(diagnostic.payload["reference"], "api://POST:/missing");
        assert_eq!(diagnostic.ownership.len(), 2);
        assert_eq!(diagnostic.evidence[0].status, EvidenceStatus::Missing);
    }

    #[tokio::test]
    async fn accepts_resolved_frontmatter_reference() {
        let mut page = page_entity(Some("Auth"));
        page.payload = json!({ "entities": ["api://POST:/login"] });
        let endpoint = entity_with_kind(
            "ent_endpoint",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
        );

        let diagnostics = MarkdownStructureChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![page.clone(), endpoint].into(),
                facts: Vec::new().into(),
                relations: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(vec![page], Vec::new()),
            })
            .await
            .unwrap();

        assert!(!diagnostics.iter().any(|diagnostic| {
            diagnostic.payload["reason"] == "unresolved_frontmatter_reference"
        }));
    }

    #[tokio::test]
    async fn reports_ambiguous_frontmatter_reference() {
        let mut page = page_entity(Some("Auth"));
        page.payload = json!({ "entities": ["api://POST:/login"] });
        let first = entity_with_kind(
            "ent_endpoint_first",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "first.openapi.yaml",
        );
        let second = entity_with_kind(
            "ent_endpoint_second",
            "api://POST:/login",
            EntityKind::ApiEndpoint,
            "second.openapi.yaml",
        );

        let diagnostics = MarkdownStructureChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![page.clone(), first, second].into(),
                facts: Vec::new().into(),
                relations: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(vec![page], Vec::new()),
            })
            .await
            .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.payload["reason"] == "ambiguous_frontmatter_reference"
                && diagnostic.evidence[0].status == EvidenceStatus::Conflicting
        }));
    }

    #[tokio::test]
    async fn reports_duplicate_documentation_page_identity() {
        let first = page_entity_with_id("ent_duplicate", "doc://shared", Some("First"));
        let mut second = page_entity_with_id("ent_duplicate", "doc://shared", Some("Second"));
        second.source.as_mut().unwrap().path = "docs/second.md".to_string();
        second.ownership = athanor_extractor_basic::ownership_for_file("docs/second.md");

        let diagnostics = MarkdownStructureChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![first.clone(), second.clone()].into(),
                facts: Vec::new().into(),
                relations: Vec::new().into(),
                affected: athanor_core::AffectedSubset::from_extracted(
                    vec![first, second],
                    Vec::new(),
                ),
            })
            .await
            .unwrap();

        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.payload["reason"] == "duplicate_documentation_id")
            .unwrap();
        assert_eq!(diagnostic.kind, DiagnosticKind::DuplicateDocumentationId);
        assert_eq!(diagnostic.severity, Severity::High);
        assert_eq!(diagnostic.evidence.len(), 2);
        assert_eq!(diagnostic.ownership.len(), 2);
    }

    fn page_entity(title: Option<&str>) -> Entity {
        page_entity_with_id("ent_page", "doc://docs/auth.md", title)
    }

    fn page_entity_with_id(id: &str, stable_key: &str, title: Option<&str>) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind: EntityKind::DocumentationPage,
            name: "docs/auth.md".to_string(),
            title: title.map(str::to_string),
            source: Some(SourceLocation {
                path: "docs/auth.md".to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("markdown".to_string())),
            aliases: Vec::new(),
            ownership: athanor_extractor_basic::ownership_for_file("docs/auth.md"),
            payload: json!({}),
        }
    }

    fn entity_with_kind(id: &str, stable_key: &str, kind: EntityKind, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: stable_key.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: athanor_extractor_basic::ownership_for_file(path),
            payload: json!({}),
        }
    }
}
