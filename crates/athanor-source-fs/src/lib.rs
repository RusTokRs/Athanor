use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, SourceFile, SourceProvider};

#[derive(Debug, Clone)]
pub struct LocalFileSystemSource {
    root: PathBuf,
}

impl LocalFileSystemSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[async_trait]
impl SourceProvider for LocalFileSystemSource {
    fn name(&self) -> &'static str {
        "source-fs"
    }

    async fn discover(&self) -> CoreResult<Vec<SourceFile>> {
        let root = self
            .root
            .canonicalize()
            .map_err(|err| CoreError::Adapter(format!("failed to canonicalize root: {err}")))?;
        let mut files = Vec::new();

        collect_files(&root, &root, &mut files)?;
        files.sort_by(|left, right| left.path.cmp(&right.path));

        Ok(files)
    }
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<SourceFile>) -> CoreResult<()> {
    let entries = fs::read_dir(current).map_err(|err| {
        CoreError::Adapter(format!("failed to read {}: {err}", current.display()))
    })?;

    for entry in entries {
        let entry =
            entry.map_err(|err| CoreError::Adapter(format!("failed to read dir entry: {err}")))?;
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
                .map_err(|err| CoreError::Adapter(format!("failed to strip root prefix: {err}")))?;
            let content = fs::read(&path).map_err(|err| {
                CoreError::Adapter(format!("failed to read {}: {err}", path.display()))
            })?;

            files.push(SourceFile {
                path: normalize_path(relative),
                language_hint: language_hint(relative),
                content_hash: Some(format!("fnv1a64:{:016x}", fnv1a64(&content))),
                content: String::from_utf8(content).ok(),
            });
        }
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[tokio::test]
    async fn discovers_files_and_ignores_generated_dirs() {
        let root = std::env::temp_dir().join(format!(
            "athanor-source-fs-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();

        fs::File::create(root.join(".git/ignored")).unwrap();
        let mut file = fs::File::create(root.join("src/lib.rs")).unwrap();
        writeln!(file, "pub fn hello() {{}}").unwrap();

        let source = LocalFileSystemSource::new(&root);
        let files = source.discover().await.unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/lib.rs");
        assert_eq!(files[0].language_hint.as_deref(), Some("rust"));
        assert!(files[0].content_hash.is_some());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn maps_javascript_and_typescript_extensions() {
        assert_eq!(
            language_hint(Path::new("src/app.js")).as_deref(),
            Some("javascript")
        );
        assert_eq!(
            language_hint(Path::new("src/app.jsx")).as_deref(),
            Some("javascriptreact")
        );
        assert_eq!(
            language_hint(Path::new("src/app.mjs")).as_deref(),
            Some("javascript")
        );
        assert_eq!(
            language_hint(Path::new("src/app.cjs")).as_deref(),
            Some("javascript")
        );
        assert_eq!(
            language_hint(Path::new("src/app.ts")).as_deref(),
            Some("typescript")
        );
        assert_eq!(
            language_hint(Path::new("src/app.tsx")).as_deref(),
            Some("typescriptreact")
        );
        assert_eq!(
            language_hint(Path::new("src/app.mts")).as_deref(),
            Some("typescript")
        );
        assert_eq!(
            language_hint(Path::new("src/app.cts")).as_deref(),
            Some("typescript")
        );
    }
}
