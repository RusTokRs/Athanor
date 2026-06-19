use std::collections::HashMap;

use async_trait::async_trait;
use athanor_core::{CoreResult, ExtractInput, ExtractOutput, Extractor, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, stable_hash};
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
            title: first_heading(content).map(str::to_string),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(1),
                line_end: line_count(content),
            }),
            language: Some(LanguageCode("markdown".to_string())),
            aliases: Vec::new(),
            payload: json!({
                "content_hash": input.source.content_hash,
            }),
        };

        let mut entities = vec![page];
        let mut facts = Vec::new();
        let mut seen_slugs = HashMap::new();

        for heading in markdown_headings(content) {
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
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            let level = trimmed
                .chars()
                .take_while(|character| *character == '#')
                .count();

            if level == 0 || level > 6 || !trimmed[level..].starts_with(' ') {
                return None;
            }

            let title = trimmed[level..].trim().to_string();

            if title.is_empty() {
                return None;
            }

            Some(MarkdownHeading {
                level,
                title,
                line: (index + 1) as u32,
            })
        })
        .collect()
}

fn first_heading(content: &str) -> Option<&str> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
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
}
