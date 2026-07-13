use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::{AdapterProcessCommand, plugin_hash};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProcessCommand {
    pub(super) program: PathBuf,
    pub(super) args: Vec<String>,
    pub(super) working_dir: PathBuf,
    pub(super) clear_environment: bool,
    pub(super) expected_content_hash: Option<String>,
    pub(super) expected_content_size_bytes: Option<u64>,
}

impl ProcessCommand {
    pub(super) fn from_manifest(
        manifest_dir: &Path,
        command: &AdapterProcessCommand,
    ) -> Result<Self> {
        Self::from_manifest_with_sandbox(manifest_dir, command, false)
    }

    pub(super) fn from_manifest_with_sandbox(
        manifest_dir: &Path,
        command: &AdapterProcessCommand,
        clear_environment: bool,
    ) -> Result<Self> {
        let program = resolve_manifest_program(manifest_dir, &command.program)?;
        let working_dir = manifest_dir.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize adapter manifest directory {}",
                manifest_dir.display()
            )
        })?;

        Ok(Self {
            expected_content_hash: Some(plugin_hash::executable(&program)?),
            expected_content_size_bytes: Some(plugin_hash::executable_size(&program)?),
            program,
            args: command.args.clone(),
            working_dir,
            clear_environment,
        })
    }

    pub(super) fn verify_unchanged(&self) -> Result<()> {
        let (Some(expected_content_hash), Some(expected_content_size_bytes)) = (
            self.expected_content_hash.as_ref(),
            self.expected_content_size_bytes,
        ) else {
            // Test-only direct construction of this private type predates launch verification.
            return Ok(());
        };
        let content_hash = plugin_hash::executable(&self.program)?;
        let content_size_bytes = plugin_hash::executable_size(&self.program)?;
        if content_hash != *expected_content_hash
            || content_size_bytes != expected_content_size_bytes
        {
            bail!(
                "external adapter executable changed after runtime assembly: {}; run ath plugins trust again and rebuild the runtime",
                self.program.display()
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ProcessLimits {
    pub(super) timeout: Duration,
    pub(super) max_stdin_bytes: usize,
    pub(super) max_stdout_bytes: usize,
    pub(super) max_stderr_bytes: usize,
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_stdin_bytes: 8 * 1024 * 1024,
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
        }
    }
}

pub(super) fn process_output_excerpt(bytes: &[u8]) -> String {
    const MAX_PROCESS_OUTPUT_LOG_CHARS: usize = 4096;

    let value = String::from_utf8_lossy(bytes);
    let trimmed = value.trim();
    let mut excerpt = trimmed
        .chars()
        .take(MAX_PROCESS_OUTPUT_LOG_CHARS)
        .collect::<String>();

    if trimmed.chars().count() > MAX_PROCESS_OUTPUT_LOG_CHARS {
        excerpt.push_str("...");
    }

    excerpt
}

pub(super) fn resolve_manifest_program(manifest_dir: &Path, program: &str) -> Result<PathBuf> {
    if program.trim().is_empty() {
        bail!("adapter command program must not be empty");
    }

    let path = PathBuf::from(program);
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!("adapter command program must not contain parent directory components");
    }

    if path.is_relative() && !(program.contains('/') || program.contains('\\')) {
        bail!("adapter command program must be an explicit absolute or manifest-relative path");
    }

    if path.is_relative() && (program.contains('/') || program.contains('\\')) {
        let base = manifest_dir
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", manifest_dir.display()))?;
        let resolved = manifest_dir
            .join(path)
            .canonicalize()
            .with_context(|| format!("failed to canonicalize adapter command program {program}"))?;
        if !resolved.starts_with(&base) {
            bail!(
                "adapter command program {} escapes manifest directory {}",
                resolved.display(),
                base.display()
            );
        }
        Ok(resolved)
    } else {
        path.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize adapter command program {}",
                path.display()
            )
        })
    }
}

pub(super) fn resolve_external_process_allowlist(
    root: &Path,
    programs: &[PathBuf],
) -> Result<BTreeSet<PathBuf>> {
    programs
        .iter()
        .map(|program| {
            let path = if program.is_absolute() {
                program.clone()
            } else {
                root.join(program)
            };
            path.canonicalize().with_context(|| {
                format!(
                    "failed to canonicalize external process allowlist entry {}",
                    path.display()
                )
            })
        })
        .collect()
}

pub(super) fn normalize_extension(extension: impl AsRef<str>) -> String {
    extension
        .as_ref()
        .trim_start_matches('.')
        .to_ascii_lowercase()
}
