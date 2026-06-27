use std::collections::HashMap;

mod frontmatter;

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::json;

use frontmatter::{DocumentationLayer, MarkdownFrontmatter, parse_markdown_frontmatter};

#[derive(Debug, Clone, Default)]
pub struct MarkdownExtractor;

#[async_trait]
impl Extractor for MarkdownExtractor {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn supports(&self, source: &SourceFile) -> bool {
        source.language_hint.as_deref() == Some("markdown")
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };
        let parsed = parse_markdown_frontmatter(content)?;
        let metadata = parsed.metadata.unwrap_or_default();
        let headings = markdown_headings(content, parsed.body_offset);
        let operation_steps = markdown_operation_steps(content, parsed.body_offset);
        let page_key = page_stable_key(&input.source.path, &metadata)?;
        let language = metadata
            .language
            .as_deref()
            .map(validate_non_empty_language)
            .transpose()?
            .map_or_else(
                || LanguageCode("markdown".to_string()),
                |language| LanguageCode(language.to_string()),
            );
        let documentation_layer = metadata
            .documentation_layer
            .unwrap_or_else(|| inferred_documentation_layer(&input.source.path));

        let page_id = EntityId(format!(
            "ent_doc_page_{:016x}",
            stable_hash(page_key.0.as_bytes())
        ));
        let page = Entity {
            id: page_id.clone(),
            stable_key: page_key.clone(),
            kind: EntityKind::DocumentationPage,
            name: input.source.path.clone(),
            title: headings
                .iter()
                .find(|heading| heading.level == 1)
                .map(|heading| heading.title.clone()),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(1),
                line_end: line_count(content),
            }),
            language: Some(language.clone()),
            aliases: Vec::new(),
            ownership: ownership_for_file(&input.source.path),
            payload: json!({
                "content_hash": input.source.content_hash,
                "frontmatter_present": parsed.body_offset > 0,
                "frontmatter_fields": parsed.fields,
                "documentation_layer": documentation_layer.as_str(),
                "documentation_kind": metadata.kind.as_deref(),
                "source_language": metadata.source_language.as_deref(),
                "concepts": &metadata.concepts,
                "entities": &metadata.entities,
                "last_verified_snapshot": metadata.last_verified_snapshot.as_deref(),
                "status": metadata.status.as_deref(),
            }),
        };

        let mut entities = vec![page];
        if is_runbook_document(metadata.kind.as_deref()) {
            let runbook_key = runbook_stable_key(&page_key);
            entities.push(Entity {
                id: EntityId(format!(
                    "ent_runbook_{:016x}",
                    stable_hash(runbook_key.0.as_bytes())
                )),
                stable_key: runbook_key.clone(),
                kind: EntityKind::Runbook,
                name: input.source.path.clone(),
                title: headings
                    .iter()
                    .find(|heading| heading.level == 1)
                    .map(|heading| heading.title.clone()),
                source: Some(SourceLocation {
                    path: input.source.path.clone(),
                    line_start: Some(1),
                    line_end: line_count(content),
                }),
                language: Some(language.clone()),
                aliases: Vec::new(),
                ownership: ownership_for_file(&input.source.path),
                payload: json!({
                    "documentation_page": page_key.0,
                    "documentation_kind": metadata.kind.as_deref(),
                    "operation_targets": &metadata.entities,
                    "last_verified_snapshot": metadata.last_verified_snapshot.as_deref(),
                    "status": metadata.status.as_deref(),
                }),
            });

            for step in &operation_steps {
                let step_key = StableKey(format!("{}#step-{}", runbook_key.0, step.sequence));
                entities.push(Entity {
                    id: EntityId(format!(
                        "ent_operation_step_{:016x}",
                        stable_hash(step_key.0.as_bytes())
                    )),
                    stable_key: step_key,
                    kind: EntityKind::OperationStep,
                    name: step.text.clone(),
                    title: Some(step.text.clone()),
                    source: Some(SourceLocation {
                        path: input.source.path.clone(),
                        line_start: Some(step.line_start),
                        line_end: Some(step.line_end),
                    }),
                    language: Some(language.clone()),
                    aliases: Vec::new(),
                    ownership: ownership_for_file(&input.source.path),
                    payload: json!({
                        "runbook": runbook_key.0,
                        "documentation_page": page_key.0,
                        "sequence": step.sequence,
                        "text": step.text,
                    }),
                });
            }
        }
        let mut facts = Vec::new();
        let mut seen_slugs = HashMap::new();

        for heading in headings {
            let slug = unique_slug(&heading.title, &mut seen_slugs);
            let section_key = StableKey(format!("{}#{}", page_key.0, slug));
            let section_id = EntityId(format!(
                "ent_doc_section_{:016x}",
                stable_hash(section_key.0.as_bytes())
            ));

            entities.push(Entity {
                id: section_id.clone(),
                stable_key: section_key.clone(),
                kind: EntityKind::DocumentationSection,
                name: heading.title.clone(),
                title: Some(heading.title.clone()),
                source: Some(SourceLocation {
                    path: input.source.path.clone(),
                    line_start: Some(heading.line),
                    line_end: Some(heading.line),
                }),
                language: Some(language.clone()),
                aliases: Vec::new(),
                ownership: ownership_for_file(&input.source.path),
                payload: json!({
                    "level": heading.level,
                    "slug": slug,
                    "documentation_page": page_key.0,
                    "documentation_layer": documentation_layer.as_str(),
                    "documentation_kind": metadata.kind.as_deref(),
                }),
            });

            facts.push(Fact {
                id: FactId(format!(
                    "fact_doc_section_found_{:016x}",
                    stable_hash(section_key.0.as_bytes())
                )),
                kind: FactKind::DocSectionFound,
                subject: page_id.clone(),
                object: Some(section_id),
                value: json!({
                    "title": heading.title,
                    "level": heading.level,
                    "slug": slug,
                }),
                evidence: vec![evidence_for_file(
                    &input.source.path,
                    self.name(),
                    Some(heading.line),
                    Some(heading.line),
                )],
                ownership: ownership_for_file(&input.source.path),
                snapshot: input.snapshot.clone(),
                extractor: self.name().to_string(),
                confidence: 1.0,
            });
        }

        Ok(ExtractOutput {
            entities,
            facts,
            diagnostics: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownHeading {
    level: usize,
    title: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownOperationStep {
    sequence: usize,
    text: String,
    line_start: u32,
    line_end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownOperationStepDraft {
    sequence: usize,
    text: String,
    line_start: u32,
    line_end: u32,
}

fn page_stable_key(path: &str, metadata: &MarkdownFrontmatter) -> CoreResult<StableKey> {
    let Some(id) = metadata.id.as_deref() else {
        return Ok(StableKey(format!("doc://{path}")));
    };
    if id.is_empty() || id.trim() != id || !id.starts_with("doc://") || id.contains('#') {
        return Err(CoreError::InvalidInput(format!(
            "Markdown frontmatter id must be a non-empty `doc://` page key without a fragment: {id:?}"
        )));
    }
    Ok(StableKey(id.to_string()))
}

fn is_runbook_document(kind: Option<&str>) -> bool {
    matches!(kind, Some("runbook" | "operations_runbook"))
}

fn runbook_stable_key(page_key: &StableKey) -> StableKey {
    StableKey(format!(
        "runbook://{}",
        page_key.0.strip_prefix("doc://").unwrap_or(&page_key.0)
    ))
}

fn validate_non_empty_language(language: &str) -> CoreResult<&str> {
    if language.is_empty() || language.trim() != language {
        return Err(CoreError::InvalidInput(
            "Markdown frontmatter language must be a non-empty trimmed code".to_string(),
        ));
    }
    Ok(language)
}

fn inferred_documentation_layer(path: &str) -> DocumentationLayer {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with(".athanor/generated/") {
        DocumentationLayer::Generated
    } else {
        DocumentationLayer::Editable
    }
}

fn markdown_headings(content: &str, body_offset: usize) -> Vec<MarkdownHeading> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut headings = Vec::new();
    let mut current: Option<MarkdownHeading> = None;
    let body_offset = markdown_body_offset(content, body_offset);
    let body = &content[body_offset..];
    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some(MarkdownHeading {
                    level: heading_level(level),
                    title: String::new(),
                    line: line_for_offset(content, body_offset + range.start),
                });
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(heading) = &mut current {
                    heading.title.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(heading) = &mut current
                    && !heading.title.ends_with(' ')
                {
                    heading.title.push(' ');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(mut heading) = current.take() {
                    heading.title = heading.title.trim().to_string();
                    if !heading.title.is_empty() {
                        headings.push(heading);
                    }
                }
            }
            _ => {}
        }
    }
    headings
}

fn markdown_operation_steps(content: &str, body_offset: usize) -> Vec<MarkdownOperationStep> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut ordered_list_depth = 0usize;
    let mut current: Option<MarkdownOperationStepDraft> = None;
    let mut steps = Vec::new();
    let body_offset = markdown_body_offset(content, body_offset);
    let body = &content[body_offset..];

    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        let line_start = line_for_offset(content, body_offset + range.start);
        let line_end = line_for_offset(content, body_offset + range.end);
        match event {
            Event::Start(Tag::List(Some(_))) => {
                ordered_list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                ordered_list_depth = ordered_list_depth.saturating_sub(1);
            }
            Event::Start(Tag::Item) if ordered_list_depth > 0 && current.is_none() => {
                current = Some(MarkdownOperationStepDraft {
                    sequence: steps.len() + 1,
                    text: String::new(),
                    line_start,
                    line_end,
                });
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(step) = &mut current {
                    if !step.text.is_empty() && !step.text.ends_with(' ') {
                        step.text.push(' ');
                    }
                    step.text.push_str(text.trim());
                    step.line_end = line_end;
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(step) = &mut current
                    && !step.text.ends_with(' ')
                {
                    step.text.push(' ');
                    step.line_end = line_end;
                }
            }
            Event::End(TagEnd::Item) => {
                if let Some(mut step) = current.take() {
                    step.text = step.text.trim().to_string();
                    if !step.text.is_empty() {
                        steps.push(MarkdownOperationStep {
                            sequence: step.sequence,
                            text: step.text,
                            line_start: step.line_start,
                            line_end: step.line_end,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    steps
}

fn markdown_body_offset(content: &str, body_offset: usize) -> usize {
    let offset = body_offset.min(content.len());
    if content[offset..].starts_with('\u{feff}') {
        offset + '\u{feff}'.len_utf8()
    } else {
        offset
    }
}

fn heading_level(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn line_for_offset(content: &str, offset: usize) -> u32 {
    content.as_bytes()[..offset.min(content.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count() as u32
        + 1
}

fn line_count(content: &str) -> Option<u32> {
    let count = content.lines().count();
    (count > 0).then_some(count as u32)
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for character in input.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            previous_dash = false;
        } else if character.is_alphanumeric() {
            let mut buffer = [0; 4];
            for byte in character.encode_utf8(&mut buffer).as_bytes() {
                slug.push('%');
                slug.push_str(&format!("{byte:02x}"));
            }
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();

    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

fn unique_slug(input: &str, seen: &mut HashMap<String, usize>) -> String {
    let base = slugify(input);
    let count = seen.entry(base.clone()).or_default();
    *count += 1;

    if *count == 1 {
        base
    } else {
        format!("{base}-{}", *count)
    }
}

#[cfg(test)]
mod tests {
    use athanor_domain::{RepoId, SnapshotId};

    use super::*;

    #[tokio::test]
    async fn markdown_extractor_emits_page_sections_and_facts() {
        let extractor = MarkdownExtractor;
        let output = extractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "docs/auth.md".to_string(),
                    language_hint: Some("markdown".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some("# Auth\n\n## Login\n\nText".to_string()),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 3);
        assert_eq!(output.facts.len(), 2);
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "doc://docs/auth.md#login"
                && entity.kind == EntityKind::DocumentationSection
        }));
    }

    #[tokio::test]
    async fn markdown_extractor_disambiguates_repeated_headings() {
        let extractor = MarkdownExtractor;
        let output = extractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "docs/auth.md".to_string(),
                    language_hint: Some("markdown".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some("## Login\n\n## Login".to_string()),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "doc://docs/auth.md#login"
                && entity.kind == EntityKind::DocumentationSection
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "doc://docs/auth.md#login-2"
                && entity.kind == EntityKind::DocumentationSection
        }));
    }

    #[tokio::test]
    async fn markdown_extractor_percent_encodes_non_ascii_slugs() {
        let extractor = MarkdownExtractor;
        let output = extractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "docs/auth.md".to_string(),
                    language_hint: Some("markdown".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some("## Авторизация".to_string()),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity
                .stable_key
                .0
                .starts_with("doc://docs/auth.md#%d0%b0%d0%b2")
        }));
    }

    #[tokio::test]
    async fn ignores_heading_syntax_inside_fenced_code() {
        let output = extract("# Real\n\n```md\n# Not a heading\n```\n\n## Section").await;

        let sections = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::DocumentationSection)
            .collect::<Vec<_>>();
        assert_eq!(sections.len(), 2);
        assert!(!sections.iter().any(|entity| entity.name == "Not a heading"));
    }

    #[tokio::test]
    async fn extracts_setext_and_formatted_headings_with_source_lines() {
        let output = extract("Page title\n==========\n\n## Login with *tokens* and `code`").await;

        let page = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::DocumentationPage)
            .unwrap();
        assert_eq!(page.title.as_deref(), Some("Page title"));
        let section = output
            .entities
            .iter()
            .find(|entity| entity.name == "Login with tokens and code")
            .unwrap();
        assert_eq!(section.source.as_ref().unwrap().line_start, Some(4));
        assert_eq!(
            section.stable_key.0,
            "doc://docs/auth.md#login-with-tokens-and-code"
        );
    }

    #[tokio::test]
    async fn extracts_headings_after_utf8_bom() {
        let output = extract("\u{feff}# Documentation `rustok-tax`\n\n## Purpose\n").await;
        let page = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::DocumentationPage)
            .unwrap();
        let h1 = output
            .entities
            .iter()
            .find(|entity| {
                entity.kind == EntityKind::DocumentationSection
                    && entity.name == "Documentation rustok-tax"
            })
            .unwrap();

        assert_eq!(page.title.as_deref(), Some("Documentation rustok-tax"));
        assert_eq!(h1.source.as_ref().unwrap().line_start, Some(1));
    }

    #[tokio::test]
    async fn applies_frontmatter_identity_language_and_documentation_metadata() {
        let content = r#"---
id: doc://product/authentication
kind: api_documentation
language: en
source_language: ru
documentation_layer: editable
concepts:
  - concept://authentication
entities:
  - api://POST:/login
last_verified_snapshot: snap_reference
status: verified
---
# Authentication

## Login
"#;
        let output = extract(content).await;
        let page = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::DocumentationPage)
            .unwrap();
        let section = output
            .entities
            .iter()
            .find(|entity| entity.name == "Login")
            .unwrap();

        assert_eq!(page.stable_key.0, "doc://product/authentication");
        assert_eq!(page.language.as_ref().unwrap().0, "en");
        assert_eq!(page.payload["frontmatter_present"], true);
        assert_eq!(page.payload["documentation_layer"], "editable");
        assert_eq!(page.payload["documentation_kind"], "api_documentation");
        assert_eq!(page.payload["source_language"], "ru");
        assert_eq!(page.payload["concepts"][0], "concept://authentication");
        assert_eq!(page.payload["entities"][0], "api://POST:/login");
        assert_eq!(page.payload["last_verified_snapshot"], "snap_reference");
        assert_eq!(page.payload["status"], "verified");
        assert_eq!(section.stable_key.0, "doc://product/authentication#login");
        assert_eq!(section.language.as_ref().unwrap().0, "en");
        assert_eq!(section.source.as_ref().unwrap().line_start, Some(16));
        assert!(
            !output
                .entities
                .iter()
                .any(|entity| entity.name == "status: verified")
        );
    }

    #[tokio::test]
    async fn emits_runbook_entity_for_runbook_frontmatter() {
        let content = r#"---
id: doc://docs/operations/deploy
kind: runbook
entities:
  - script-command://Makefile#target:deploy
---
# Deploy
"#;
        let output = extract(content).await;
        let runbook = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Runbook)
            .unwrap();

        assert_eq!(runbook.stable_key.0, "runbook://docs/operations/deploy");
        assert_eq!(
            runbook.payload["operation_targets"][0],
            "script-command://Makefile#target:deploy"
        );
        assert_eq!(
            runbook.payload["documentation_page"],
            "doc://docs/operations/deploy"
        );
    }

    #[tokio::test]
    async fn emits_operation_steps_for_runbook_ordered_list_items() {
        let content = r#"---
id: doc://docs/operations/deploy
kind: runbook
entities:
  - script-command://Makefile#target:deploy
---
# Deploy

1. Build the release
2. Run `make deploy`

- This note is not an operation step.
"#;
        let output = extract(content).await;
        let steps = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::OperationStep)
            .collect::<Vec<_>>();

        assert_eq!(steps.len(), 2);
        assert_eq!(
            steps[0].stable_key.0,
            "runbook://docs/operations/deploy#step-1"
        );
        assert_eq!(steps[0].payload["sequence"], 1);
        assert_eq!(
            steps[0].payload["runbook"],
            "runbook://docs/operations/deploy"
        );
        assert_eq!(steps[1].name, "Run make deploy");
    }

    #[tokio::test]
    async fn rejects_invalid_frontmatter_page_identity() {
        let error = MarkdownExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "docs/auth.md".to_string(),
                    language_hint: Some("markdown".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some("---\nid: auth-page\n---\n# Auth\n".to_string()),
                },
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("must be a non-empty `doc://`"));
    }

    #[tokio::test]
    async fn defaults_source_markdown_to_editable_documentation() {
        let output = extract("# Auth\n").await;
        let page = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::DocumentationPage)
            .unwrap();

        assert_eq!(page.payload["documentation_layer"], "editable");
        assert_eq!(page.payload["frontmatter_present"], false);
    }

    async fn extract(content: &str) -> ExtractOutput {
        MarkdownExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "docs/auth.md".to_string(),
                    language_hint: Some("markdown".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(content.to_string()),
                },
            })
            .await
            .unwrap()
    }
}
