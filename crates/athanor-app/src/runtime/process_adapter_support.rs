use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::config::{
    DEFAULT_EXTERNAL_PROCESS_MAX_STDERR_BYTES, DEFAULT_EXTERNAL_PROCESS_MAX_STDIN_BYTES,
    DEFAULT_EXTERNAL_PROCESS_MAX_STDOUT_BYTES, DEFAULT_EXTERNAL_PROCESS_TIMEOUT_MS,
};

use super::{AdapterProcessCommand, TrustedAdapterExecutable, plugin_hash};

#[derive(Debug, Clone)]
pub(super) struct ProcessCommand {
    pub(super) program: PathBuf,
    pub(super) args: Vec<String>,
    pub(super) working_dir: PathBuf,
    pub(super) clear_environment: bool,
    pub(super) limits: ProcessLimits,
    pub(super) expected: Vec<TrustedAdapterExecutable>,
    pub(super) fully_verified: Arc<AtomicBool>,
}

impl ProcessCommand {
    pub(super) fn from_manifest(
        manifest_dir: &Path,
        command: &AdapterProcessCommand,
    ) -> Result<Self> {
        Self::from_manifest_with_options(
            manifest_dir,
            command,
            false,
            ProcessLimits::default(),
            None,
        )
    }

    pub(super) fn from_manifest_with_options(
        manifest_dir: &Path,
        command: &AdapterProcessCommand,
        clear_environment: bool,
        limits: ProcessLimits,
        trusted_payload: Option<&[TrustedAdapterExecutable]>,
    ) -> Result<Self> {
        let program = resolve_manifest_program(manifest_dir, &command.program)?;
        let working_dir = manifest_dir.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize adapter manifest directory {}",
                manifest_dir.display()
            )
        })?;
        let expected = launch_digests(manifest_dir, command, trusted_payload)?;

        Ok(Self {
            program,
            args: command.args.clone(),
            working_dir,
            clear_environment,
            limits,
            expected,
            fully_verified: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(super) fn verify_unchanged(&self) -> Result<()> {
        if self.expected.is_empty() {
            #[cfg(test)]
            {
                return Ok(());
            }
            #[cfg(not(test))]
            bail!("external adapter has no launch digests; refuse to execute");
        }
        for expected in &self.expected {
            let content_size_bytes = plugin_hash::executable_size(&expected.program)?;
            if content_size_bytes != expected.content_size_bytes {
                bail!(
                    "external adapter payload changed after runtime assembly: {}; run ath plugins trust again and rebuild the runtime",
                    expected.program.display()
                );
            }
        }
        if self.fully_verified.load(Ordering::Acquire) {
            return Ok(());
        }
        for expected in &self.expected {
            let content_hash = plugin_hash::executable(&expected.program)?;
            if content_hash != expected.content_hash {
                bail!(
                    "external adapter payload changed after runtime assembly: {}; run ath plugins trust again and rebuild the runtime",
                    expected.program.display()
                );
            }
        }
        self.fully_verified.store(true, Ordering::Release);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProcessLimits {
    pub(super) timeout: Duration,
    pub(super) max_stdin_bytes: usize,
    pub(super) max_stdout_bytes: usize,
    pub(super) max_stderr_bytes: usize,
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(DEFAULT_EXTERNAL_PROCESS_TIMEOUT_MS),
            max_stdin_bytes: DEFAULT_EXTERNAL_PROCESS_MAX_STDIN_BYTES,
            max_stdout_bytes: DEFAULT_EXTERNAL_PROCESS_MAX_STDOUT_BYTES,
            max_stderr_bytes: DEFAULT_EXTERNAL_PROCESS_MAX_STDERR_BYTES,
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

pub(super) fn command_payload_files(
    manifest_dir: &Path,
    command: &AdapterProcessCommand,
) -> Result<Vec<PathBuf>> {
    let mut files = vec![resolve_manifest_program(manifest_dir, &command.program)?];
    for arg in &command.args {
        if let Some(path) = resolve_payload_arg(manifest_dir, arg)? {
            files.push(path);
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn launch_digests(
    manifest_dir: &Path,
    command: &AdapterProcessCommand,
    trusted_payload: Option<&[TrustedAdapterExecutable]>,
) -> Result<Vec<TrustedAdapterExecutable>> {
    let files = command_payload_files(manifest_dir, command)?;
    let current = files
        .into_iter()
        .map(|program| {
            let (content_hash, content_size_bytes) =
                plugin_hash::digest(&program, "adapter executable")?;
            Ok(TrustedAdapterExecutable {
                content_hash,
                content_size_bytes,
                program,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let Some(trusted_payload) = trusted_payload else {
        return Ok(current);
    };
    for digest in &current {
        let trusted = trusted_payload
            .iter()
            .find(|entry| entry.program == digest.program)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "external adapter payload {} is missing from the trust record; run ath plugins trust again",
                    digest.program.display()
                )
            })?;
        if trusted.content_hash != digest.content_hash
            || trusted.content_size_bytes != digest.content_size_bytes
        {
            bail!(
                "external adapter payload {} does not match the trusted digest; run ath plugins trust again",
                digest.program.display()
            );
        }
    }
    Ok(current)
}

fn resolve_payload_arg(manifest_dir: &Path, arg: &str) -> Result<Option<PathBuf>> {
    if arg.trim().is_empty() || arg.starts_with('-') {
        return Ok(None);
    }
    let path = PathBuf::from(arg);
    let candidate = if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    };
    let metadata = match fs::symlink_metadata(&candidate) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!(
            "adapter command argument {} must be a regular file, not a symlink or directory",
            candidate.display()
        );
    }
    let canonical = candidate.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize adapter command argument {}",
            candidate.display()
        )
    })?;
    Ok(Some(canonical))
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

    if path.is_relative() && !has_directory_separator(program) {
        bail!("adapter command program must be an explicit absolute or manifest-relative path");
    }

    if path.is_relative() && has_directory_separator(program) {
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

pub(super) fn extractor_matches_all(extensions: &BTreeSet<String>) -> bool {
    extensions.iter().any(|extension| extension == "*")
}

fn has_directory_separator(program: &str) -> bool {
    program.contains('/') || program.contains('\\')
}
