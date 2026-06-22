use athanor_core::{CoreError, CoreResult};
use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MarkdownFrontmatter {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub language: Option<String>,
    pub source_language: Option<String>,
    pub documentation_layer: Option<DocumentationLayer>,
    #[serde(default)]
    pub concepts: Vec<String>,
    #[serde(default)]
    pub entities: Vec<String>,
    pub last_verified_snapshot: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationLayer {
    Editable,
    Generated,
}

impl DocumentationLayer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Editable => "editable",
            Self::Generated => "generated",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedMarkdownFrontmatter {
    pub metadata: Option<MarkdownFrontmatter>,
    pub body_offset: usize,
}

pub fn parse_markdown_frontmatter(content: &str) -> CoreResult<ParsedMarkdownFrontmatter> {
    let mut lines = content.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Ok(ParsedMarkdownFrontmatter::default());
    };
    if trimmed_line(first) != "---" {
        return Ok(ParsedMarkdownFrontmatter::default());
    }

    let yaml_start = first.len();
    let mut cursor = yaml_start;
    for line in lines {
        let line_start = cursor;
        cursor += line.len();
        if trimmed_line(line) == "---" {
            let yaml = &content[yaml_start..line_start];
            let metadata = if yaml.trim().is_empty() {
                MarkdownFrontmatter::default()
            } else {
                serde_yaml_ng::from_str(yaml).map_err(|error| {
                    CoreError::InvalidInput(format!("invalid Markdown frontmatter: {error}"))
                })?
            };
            return Ok(ParsedMarkdownFrontmatter {
                metadata: Some(metadata),
                body_offset: cursor,
            });
        }
    }

    Err(CoreError::InvalidInput(
        "Markdown frontmatter is missing its closing `---` delimiter".to_string(),
    ))
}

fn trimmed_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_returns_body_offset() {
        let content = "---\r\nid: doc://docs/auth.md\r\nlanguage: en\r\ndocumentation_layer: editable\r\n---\r\n# Auth\r\n";
        let parsed = parse_markdown_frontmatter(content).unwrap();
        let metadata = parsed.metadata.unwrap();

        assert_eq!(metadata.id.as_deref(), Some("doc://docs/auth.md"));
        assert_eq!(metadata.language.as_deref(), Some("en"));
        assert_eq!(
            metadata.documentation_layer,
            Some(DocumentationLayer::Editable)
        );
        assert_eq!(&content[parsed.body_offset..], "# Auth\r\n");
    }

    #[test]
    fn leaves_documents_without_frontmatter_unchanged() {
        let parsed = parse_markdown_frontmatter("# Auth\n").unwrap();
        assert_eq!(parsed, ParsedMarkdownFrontmatter::default());
    }

    #[test]
    fn rejects_unclosed_frontmatter() {
        let error =
            parse_markdown_frontmatter("---\nid: doc://docs/auth.md\n# Auth\n").unwrap_err();
        assert!(error.to_string().contains("closing `---`"));
    }
}
