use std::collections::HashMap;

use async_trait::async_trait;
use athanor_core::{CoreResult, ExtractInput, ExtractOutput, Extractor, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::json;

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
        let headings = markdown_headings(content);

        let page_key = StableKey(format!("doc://{}", input.source.path));
        let page_id = EntityId(format!(
            "ent_doc_page_{:016x}",
            stable_hash(page_key.0.as_bytes())
        ));
        let page = Entity {
            id: page_id.clone(),
            stable_key: page_key,
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
            language: Some(LanguageCode("markdown".to_string())),
            aliases: Vec::new(),
            ownership: ownership_for_file(&input.source.path),
            payload: json!({
                "content_hash": input.source.content_hash,
            }),
        };

        let mut entities = vec![page];
        let mut facts = Vec::new();
        let mut seen_slugs = HashMap::new();

        for heading in headings {
            let slug = unique_slug(&heading.title, &mut seen_slugs);
            let section_key = StableKey(format!("doc://{}#{}", input.source.path, slug));
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
                language: Some(LanguageCode("markdown".to_string())),
                aliases: Vec::new(),
                ownership: ownership_for_file(&input.source.path),
                payload: json!({
                    "level": heading.level,
                    "slug": slug,
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

        Ok(ExtractOutput { entities, facts })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownHeading {
    level: usize,
    title: String,
    line: u32,
}

fn markdown_headings(content: &str) -> Vec<MarkdownHeading> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut headings = Vec::new();
    let mut current: Option<MarkdownHeading> = None;
    for (event, range) in Parser::new_ext(content, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some(MarkdownHeading {
                    level: heading_level(level),
                    title: String::new(),
                    line: line_for_offset(content, range.start),
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
