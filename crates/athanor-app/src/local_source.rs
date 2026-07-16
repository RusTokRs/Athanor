use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use athanor_core::{OperationContext, OperationContextCancellation, SourceFile};

const DISCOVERY_POLL_INTERVAL: usize = 64;
const HASH_POLL_BYTES: usize = 16 * 1024;

pub(crate) fn discover_source_files(root: &Path) -> Result<Vec<SourceFile>> {
    discover_source_files_inner(root, None)
}

pub(crate) fn discover_source_files_with_operation_context(
    root: &Path,
    operation: &OperationContext,
) -> Result<Vec<SourceFile>> {
    discover_source_files_inner(root, Some(operation))
}

fn discover_source_files_inner(
    root: &Path,
    operation: Option<&OperationContext>,
) -> Result<Vec<SourceFile>> {
    let mut poller = DiscoveryPoller::new(operation, DISCOVERY_POLL_INTERVAL)?;
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let mut files = Vec::new();
    let mut pending = vec![root.clone()];

    while let Some(current) = pending.pop() {
        poller.step()?;
        let entries =
            fs::read_dir(&current).with_context(|| format!("failed to read {}", current.display()))?;
        let mut child_directories = Vec::new();

        for entry in entries {
            poller.step()?;
            let entry = entry.context("failed to read dir entry")?;
            let path = entry.path();
            let file_name = entry.file_name();

            if should_ignore(&file_name.to_string_lossy()) {
                continue;
            }

            if path.is_dir() {
                child_directories.push(path);
            } else if path.is_file() {
                let relative = path.strip_prefix(&root).with_context(|| {
                    format!("failed to strip root prefix for {}", path.display())
                })?;
                if let Some(source) = read_source_file_at_with_poller(&root, relative, &mut poller)? {
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

pub(crate) fn read_source_file_at(root: &Path, relative: &Path) -> Result<Option<SourceFile>> {
    let mut poller = DiscoveryPoller::new(None, DISCOVERY_POLL_INTERVAL)?;
    read_source_file_at_with_poller(root, relative, &mut poller)
}

fn read_source_file_at_with_poller(
    root: &Path,
    relative: &Path,
    poller: &mut DiscoveryPoller<'_>,
) -> Result<Option<SourceFile>> {
    poller.step()?;
    let path = root.join(relative);
    if !path.is_file() {
        return Ok(None);
    }
    let content = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
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
    fn new(operation: Option<&'a OperationContext>, interval: usize) -> Result<Self> {
        if let Some(operation) = operation {
            operation.check_active().map_err(anyhow::Error::new)?;
        }
        let interval = interval.max(1);
        Ok(Self {
            operation,
            interval,
            remaining: interval,
        })
    }

    fn step(&mut self) -> Result<()> {
        self.remaining -= 1;
        if self.remaining == 0 {
            if let Some(operation) = self.operation {
                operation.check_active().map_err(anyhow::Error::new)?;
            }
            self.remaining = self.interval;
        }
        Ok(())
    }

    fn finish(&self) -> Result<()> {
        if let Some(operation) = self.operation {
            operation.check_active().map_err(anyhow::Error::new)?;
        }
        Ok(())
    }
}

fn fnv1a64_with_poller(bytes: &[u8], poller: &mut DiscoveryPoller<'_>) -> Result<u64> {
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
    use athanor_core::CoreError;

    use super::*;

    #[test]
    fn pre_cancelled_discovery_fails_before_project_access() {
        let operation = OperationContext::new("local-source-pre-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        cancellation.cancel();

        let error = discover_source_files_with_operation_context(Path::new("."), &operation)
            .expect_err("cancelled discovery must fail before canonicalization");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    #[test]
    fn hashing_observes_cooperative_cancellation() {
        let operation = OperationContext::new("local-source-hash-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        let mut poller = DiscoveryPoller::new(Some(&operation), 1).unwrap();
        cancellation.cancel();

        let error = fnv1a64_with_poller(&vec![1; HASH_POLL_BYTES * 2], &mut poller)
            .expect_err("hashing must observe cancellation");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }
}
