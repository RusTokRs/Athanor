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
    match (root.canonicalize(), joined.canonicalize()) {
        (Ok(root_canon), Ok(joined_canon)) => {
            let root_canon = normalize_canonical_path(root_canon);
            let joined_canon = normalize_canonical_path(joined_canon);
            if joined_canon.starts_with(&root_canon) {
                Some(joined_canon)
            } else {
                None
            }
        }
        (Ok(_), Err(_)) => Some(joined),
        _ => Some(joined),
    }
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
    let mut collected = Vec::new();
    let mut last = line_start;
    let mut truncated = false;
    for (index, line) in reader.lines().enumerate() {
        let number = index as u32 + 1;
        if number < line_start {
            continue;
        }
        if number > take_end {
            break;
        }
        let mut line = line.ok()?;
        if line.len() > MAX_EXCERPT_BYTES {
            let mut end = MAX_EXCERPT_BYTES;
            while end > 0 && !line.is_char_boundary(end) {
                end -= 1;
            }
            line.truncate(end);
            truncated = true;
        }
        collected.push(line);
        last = number;
    }
    if collected.is_empty() {
        return None;
    }
    truncated |= last < line_end;
    let mut text = collected.join("\n");
    if text.len() > MAX_EXCERPT_BYTES {
        let mut end = MAX_EXCERPT_BYTES;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        truncated = true;
    }
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
}
