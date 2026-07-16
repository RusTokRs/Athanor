use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use athanor_core::{
    CoreError, CoreResult, OperationContext, OperationContextCancellation, SourceFile, SourceProvider,
};

const DISCOVERY_POLL_INTERVAL: usize = 64;
const HASH_POLL_BYTES: usize = 16 * 1024;

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
        discover_files(&self.root, None)
    }

    async fn discover_with_context(
        &self,
        context: &OperationContext,
    ) -> CoreResult<Vec<SourceFile>> {
        discover_files(&self.root, Some(context))
    }
}

fn discover_files(root: &Path, operation: Option<&OperationContext>) -> CoreResult<Vec<SourceFile>> {
    let mut poller = DiscoveryPoller::new(operation, DISCOVERY_POLL_INTERVAL)?;
    let root = root
        .canonicalize()
        .map_err(|err| CoreError::Adapter(format!("failed to canonicalize root: {err}")))?;
    let mut files = Vec::new();
    let mut pending = vec![root.clone()];

    while let Some(current) = pending.pop() {
        poller.step()?;
        let entries = fs::read_dir(&current).map_err(|err| {
            CoreError::Adapter(format!("failed to read {}: {err}", current.display()))
        })?;
        let mut child_directories = Vec::new();

        for entry in entries {
            poller.step()?;
            let entry = entry
                .map_err(|err| CoreError::Adapter(format!("failed to read dir entry: {err}")))?;
            let path = entry.path();
            let file_name = entry.file_name();

            if should_ignore(&file_name.to_string_lossy()) {
                continue;
            }

            if path.is_dir() {
                child_directories.push(path);
            } else if path.is_file() {
                let relative = path.strip_prefix(&root).map_err(|err| {
                    CoreError::Adapter(format!("failed to strip root prefix: {err}"))
                })?;
                if let Some(source) =
                    read_source_file_at_with_poller(&root, relative, &mut poller)?
                {
                    files.push(source);
                }
            }
        }

        child_directories.sort_by(|left, right| right.cmp(left));
        pending.extend(child_directories);
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    poller.finish()?;
    Ok(files)
}

pub fn read_source_file_at(root: &Path, relative: &Path) -> CoreResult<Option<SourceFile>> {
    let mut poller = DiscoveryPoller::new(None, DISCOVERY_POLL_INTERVAL)?;
    read_source_file_at_with_poller(root, relative, &mut poller)
}

pub fn read_source_file_at_with_operation_context(
    root: &Path,
    relative: &Path,
    operation: &OperationContext,
) -> CoreResult<Option<SourceFile>> {
    let mut poller = DiscoveryPoller::new(Some(operation), DISCOVERY_POLL_INTERVAL)?;
    let source = read_source_file_at_with_poller(root, relative, &mut poller)?;
    poller.finish()?;
    Ok(source)
}

fn read_source_file_at_with_poller(
    root: &Path,
    relative: &Path,
    poller: &mut DiscoveryPoller<'_>,
) -> CoreResult<Option<SourceFile>> {
    poller.step()?;
    let path = root.join(relative);
    if !path.is_file() {
        return Ok(None);
    }

    let content = fs::read(&path)
        .map_err(|err| CoreError::Adapter(format!("failed to read {}: {err}", path.display())))?;
    let content_hash = fnv1a64_with_poller(&content, poller)?;

    Ok(Some(SourceFile {
        path: normalize_path(relative),
        language_hint: language_hint(relative),
        content_hash: Some(format!("fnv1a64:{content_hash:016x}")),
        content: String::from_utf8(content).ok(),
    }))
}

struct DiscoveryPoller<'a> {
    operation: Option<&'a OperationContext>,
    interval: usize,
    remaining: usize,
}

impl<'a> DiscoveryPoller<'a> {
    fn new(operation: Option<&'a OperationContext>, interval: usize) -> CoreResult<Self> {
        if let Some(operation) = operation {
            operation.check_active()?;
        }
        let interval = interval.max(1);
        Ok(Self {
            operation,
            interval,
            remaining: interval,
        })
    }

    fn step(&mut self) -> CoreResult<()> {
        self.remaining -= 1;
        if self.remaining == 0 {
            if let Some(operation) = self.operation {
                operation.check_active()?;
            }
            self.remaining = self.interval;
        }
        Ok(())
    }

    fn finish(&self) -> CoreResult<()> {
        if let Some(operation) = self.operation {
            operation.check_active()?;
        }
        Ok(())
    }
}

fn fnv1a64_with_poller(bytes: &[u8], poller: &mut DiscoveryPoller<'_>) -> CoreResult<u64> {
    let mut hash = 0xcbf29ce484222325u64;

    for chunk in bytes.chunks(HASH_POLL_BYTES) {
        poller.step()?;
        for byte in chunk {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    Ok(hash)
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

#[cfg(test)]
mod tests {
    use std::io::Write;

    use athanor_core::CoreError;

    use super::*;

    #[tokio::test]
    async fn discovers_files_and_ignores_generated_dirs() {
        let root = test_root("discover");

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

    #[tokio::test]
    async fn context_discovery_rejects_pre_cancelled_operation() {
        let root = test_root("pre-cancelled");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        let operation = OperationContext::new("source-fs-pre-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        cancellation.cancel();

        let error = LocalFileSystemSource::new(&root)
            .discover_with_context(&operation)
            .await
            .expect_err("cancelled discovery must fail before traversal");

        assert!(matches!(error, CoreError::Cancelled(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn polling_stops_hashing_after_cancellation() {
        let operation = OperationContext::new("source-fs-hash-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        let mut poller = DiscoveryPoller::new(Some(&operation), 1).unwrap();
        cancellation.cancel();

        let error = fnv1a64_with_poller(&vec![1; HASH_POLL_BYTES * 2], &mut poller)
            .expect_err("hash loop must poll cancellation");

        assert!(matches!(error, CoreError::Cancelled(_)));
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

    #[test]
    fn reads_single_source_file_at_relative_path() {
        let root = test_root("single");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/app.tsx"),
            "export const App = () => <main />;\n",
        )
        .unwrap();

        let source = read_source_file_at(&root, Path::new("src/app.tsx"))
            .unwrap()
            .unwrap();

        assert_eq!(source.path, "src/app.tsx");
        assert_eq!(source.language_hint.as_deref(), Some("typescriptreact"));
        assert!(source.content_hash.is_some());
        assert_eq!(
            source.content.as_deref(),
            Some("export const App = () => <main />;\n")
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "athanor-source-fs-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
