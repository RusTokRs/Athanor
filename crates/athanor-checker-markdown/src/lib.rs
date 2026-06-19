use async_trait::async_trait;
use athanor_core::{CheckInput, Checker, CoreResult};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityKind, Evidence,
    EvidenceStatus, RelationKind, Severity, SnapshotId,
};
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

        Ok(diagnostics)
    }
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
        snapshot: snapshot.clone(),
        suggested_fix: None,
        payload: json!({
            "entity": entity.stable_key.0,
        }),
    }
}

fn evidence_for_entity(entity: &Entity, checker: &str) -> Evidence {
    let source = entity.source.as_ref();

    Evidence {
        source_file: source.map(|source| source.path.clone()),
        line_start: source.and_then(|source| source.line_start),
        line_end: source.and_then(|source| source.line_end),
        extractor: Some(checker.to_string()),
        commit_hash: None,
        confidence: 1.0,
        status: EvidenceStatus::Verified,
    }
}

fn diagnostic_slug(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::EmptyDocumentationPage => "empty_documentation_page",
        DiagnosticKind::DocumentationPageMissingTitle => "documentation_page_missing_title",
        _ => "diagnostic",
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
                entities: vec![page],
                facts: Vec::new(),
                relations: Vec::new(),
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
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        };

        let diagnostics = MarkdownStructureChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: vec![page, section],
                facts: Vec::new(),
                relations: vec![relation],
            })
            .await
            .unwrap();

        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::EmptyDocumentationPage)
        );
    }

    fn page_entity(title: Option<&str>) -> Entity {
        Entity {
            id: EntityId("ent_page".to_string()),
            stable_key: StableKey("doc://docs/auth.md".to_string()),
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
            payload: json!({}),
        }
    }
}
