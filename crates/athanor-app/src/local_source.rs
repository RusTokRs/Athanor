use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use athanor_core::SourceFile;

pub(crate) fn discover_source_files(root: &Path) -> Result<Vec<SourceFile>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let mut files = Vec::new();

    collect_files(&root, &root, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(files)
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<SourceFile>) -> Result<()> {
    let entries =
        fs::read_dir(current).with_context(|| format!("failed to read {}", current.display()))?;

    for entry in entries {
        let entry = entry.context("failed to read dir entry")?;
        let path = entry.path();
        let file_name = entry.file_name();

        if should_ignore(&file_name.to_string_lossy()) {
            continue;
        }

        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .with_context(|| format!("failed to strip root prefix for {}", path.display()))?;
            if let Some(source) = read_source_file_at(root, relative)? {
                files.push(source);
            }
        }
    }

    Ok(())
}

pub(crate) fn read_source_file_at(root: &Path, relative: &Path) -> Result<Option<SourceFile>> {
    let path = root.join(relative);
    if !path.is_file() {
        return Ok(None);
    }

    let content = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;

    Ok(Some(SourceFile {
        path: normalize_path(relative),
        language_hint: language_hint(relative),
        content_hash: Some(format!("fnv1a64:{:016x}", fnv1a64(&content))),
        content: String::from_utf8(content).ok(),
    }))
}

fn should_ignore(file_name: &str) -> bool {
    matches!(
        file_name,
        ".git" | ".athanor" | "target" | "node_modules" | ".idea" | ".vscode"
    )
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn language_hint(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| match extension {
            "rs" => "rust",
            "md" => "markdown",
            "toml" => "toml",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "js" | "mjs" | "cjs" => "javascript",
            "jsx" => "javascriptreact",
            "ts" | "mts" | "cts" => "typescript",
            "tsx" => "typescriptreact",
            "py" => "python",
            "go" => "go",
            "php" => "php",
            other => other,
        })
        .map(str::to_string)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}
