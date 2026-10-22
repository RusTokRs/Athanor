//! Bounded source excerpts for one `explore` report.
//!
//! The Goal wants the agent to read the relevant slice in the same call that
//! returns the call path and blast radius. This module inlines only the
//! declaration range the snapshot already cites, clipped to a line and byte
//! bound, and refuses absolute paths, parent-directory escapes, missing files,
//! and non-UTF-8 contents instead of guessing.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};

use athanor_domain::SourceLocation;
use serde::{Deserialize, Serialize};

use crate::project_path::normalize_canonical_path;

/// Hard bound on lines inlined from one declaration.
pub const MAX_EXCERPT_LINES: usize = 32;
/// Hard bound on UTF-8 bytes inlined from one declaration.
const MAX_EXCERPT_BYTES: usize = 4_096;

/// One bounded source slice attached to an explore entry or path target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploreExcerpt {
    /// Repository-relative path the slice was read from.
    pub source_file: String,
    /// First inlined line, 1-based, inclusive.
    pub line_start: u32,
    /// Last inlined line, 1-based, inclusive.
    pub line_end: u32,
    /// The cited lines, joined by `\n`, clipped to the line and byte bounds.
    pub text: String,
    /// True when the declared range or the byte bound was clipped.
    pub truncated: bool,
}

/// Reads declaration ranges from the working tree, caching failed paths.
pub(super) struct ExcerptReader {
    root: PathBuf,
    failed: HashSet<String>,
    unread: usize,
}

impl ExcerptReader {
    pub(super) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            failed: HashSet::new(),
            unread: 0,
        }
    }

    /// Number of excerpt attempts that had a cited range but could not be read.
    pub(super) fn unread(&self) -> usize {
        self.unread
    }

    /// Inlines one cited range, or returns `None` when there is no bounded range
    /// or the file cannot be read.
    pub(super) fn take(&mut self, source: Option<&SourceLocation>) -> Option<ExploreExcerpt> {
        let source = source?;
        if source.path.is_empty() {
            return None;
        }
        let Some(line_start) = source.line_start.filter(|line| *line > 0) else {
            return None;
        };
        let line_end = source.line_end.unwrap_or(line_start).max(line_start);
        if self.failed.contains(&source.path) {
            self.unread += 1;
            return None;
        }
        let Some(path) = resolve_source_path(&self.root, &source.path) else {
            self.failed.insert(source.path.clone());
            self.unread += 1;
            return None;
        };
        match read_excerpt(&path, &source.path, line_start, line_end) {
            Some(excerpt) => Some(excerpt),
            None => {
                self.failed.insert(source.path.clone());
                self.unread += 1;
                None
            }
        }
    }
}

/// Resolves a repository-relative source path against the explore root.
///
/// Absolute paths, Windows prefixes, and `..` components are refused so an
/// excerpt never follows a citation out of the working tree.
fn resolve_source_path(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || relative.contains('\0') {
        return None;
    }
    let path = Path::new(relative);
    if path.is_absolute() || path.has_root() {
        return None;
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => return None,
        }
    }
    let joined = root.join(path);
    let root_canon = normalize_canonical_path(root.canonicalize().ok()?);
    let joined_canon = normalize_canonical_path(joined.canonicalize().ok()?);
    joined_canon
        .starts_with(&root_canon)
        .then_some(joined_canon)
}

fn read_excerpt(
    path: &Path,
    source_file: &str,
    line_start: u32,
    line_end: u32,
) -> Option<ExploreExcerpt> {
    if !path.is_file() {
        return None;
    }
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let cap_end = line_start.saturating_add(MAX_EXCERPT_LINES as u32 - 1);
    let take_end = line_end.min(cap_end);
    let mut text = String::new();
    let mut last = line_start;
    let mut has_lines = false;
    let mut truncated = false;
    for (index, line) in reader.lines().enumerate() {
        let number = index as u32 + 1;
        if number < line_start {
            continue;
        }
        if number > take_end {
            break;
        }
        let line = line.ok()?;
        let separator_bytes = usize::from(has_lines);
        let remaining = MAX_EXCERPT_BYTES - text.len();
        let available = remaining.saturating_sub(separator_bytes);
        if separator_bytes > remaining || (!line.is_empty() && available == 0) {
            truncated = true;
            break;
        }
        let mut end = line.len().min(available);
        while end > 0 && !line.is_char_boundary(end) {
            end -= 1;
        }
        if end == 0 && !line.is_empty() {
            truncated = true;
            break;
        }
        if has_lines {
            text.push('\n');
        }
        text.push_str(&line[..end]);
        has_lines = true;
        last = number;
        if end < line.len() {
            truncated = true;
            break;
        }
        if text.len() == MAX_EXCERPT_BYTES && number < take_end {
            truncated = true;
            break;
        }
    }
    if !has_lines {
        return None;
    }
    truncated |= last < line_end;
    Some(ExploreExcerpt {
        source_file: source_file.to_string(),
        line_start,
        line_end: last,
        text,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escaped_and_absolute_source_paths() {
        let root = Path::new("repo");
        assert!(resolve_source_path(root, "../secret.rs").is_none());
        assert!(resolve_source_path(root, "src/../../secret.rs").is_none());
        assert!(resolve_source_path(root, "/etc/passwd").is_none());
        assert!(resolve_source_path(root, "").is_none());
    }

    #[test]
    fn clips_an_over_long_declaration_range() {
        let root = std::env::temp_dir().join(format!(
            "athanor-explore-excerpt-clip-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut body = String::new();
        for index in 1..=40 {
            body.push_str(&format!("line{index}\n"));
        }
        std::fs::write(root.join("file.rs"), body).unwrap();

        let excerpt = read_excerpt(&root.join("file.rs"), "file.rs", 1, 40).unwrap();
        assert_eq!(excerpt.line_start, 1);
        assert_eq!(excerpt.line_end, MAX_EXCERPT_LINES as u32);
        assert!(excerpt.truncated);
        assert_eq!(excerpt.text.lines().count(), MAX_EXCERPT_LINES);
        assert!(excerpt.text.starts_with("line1\n"));
        assert!(excerpt.text.ends_with("line32"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn byte_clipping_reports_the_last_line_actually_inlined() {
        let root = std::env::temp_dir().join(format!(
            "athanor-explore-excerpt-bytes-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let first = "a".repeat(3_000);
        let second = "b".repeat(2_000);
        let third = "c".repeat(100);
        std::fs::write(
            root.join("file.rs"),
            format!("{first}\n{second}\n{third}\n"),
        )
        .unwrap();

        let excerpt = read_excerpt(&root.join("file.rs"), "file.rs", 1, 3).unwrap();

        assert_eq!(excerpt.line_end, 2);
        assert_eq!(excerpt.text.len(), MAX_EXCERPT_BYTES);
        assert!(excerpt.text.starts_with(&first));
        assert!(excerpt.text.ends_with(&"b".repeat(1_095)));
        assert!(!excerpt.text.contains('c'));
        assert!(excerpt.truncated);
        std::fs::remove_dir_all(root).unwrap();
    }
}
