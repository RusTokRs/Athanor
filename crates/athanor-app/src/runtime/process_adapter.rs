use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use athanor_core::{
    CheckInput, Checker, CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, LinkInput,
    Linker, ProcessLimits as CoreProcessLimits, ProcessRequest, SourceFile, SourceProvider,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use super::process_adapter_support::{
    ProcessCommand, ProcessLimits, normalize_extension, process_output_excerpt,
};
use super::{CancellationToken, PROCESS_CANCELLATION, TokioProcessRunner};

pub(super) fn source(
    id: String,
    command: ProcessCommand,
    root: PathBuf,
) -> Box<dyn SourceProvider> {
    Box::new(ProcessSource { id, command, root })
}

pub(super) fn extractor(
    id: String,
    command: ProcessCommand,
    supports_extensions: BTreeSet<String>,
) -> Box<dyn Extractor> {
    Box::new(ProcessExtractor {
        id,
        command,
        supports_extensions,
    })
}

pub(super) fn linker(id: String, command: ProcessCommand) -> Box<dyn Linker> {
    Box::new(ProcessLinker { id, command })
}

pub(super) fn checker(id: String, command: ProcessCommand) -> Box<dyn Checker> {
    Box::new(ProcessChecker { id, command })
}

struct ProcessExtractor {
    id: String,
    command: ProcessCommand,
    supports_extensions: BTreeSet<String>,
}

struct ProcessSource {
    id: String,
    command: ProcessCommand,
    root: PathBuf,
}

#[derive(Serialize)]
struct SourceDiscoverInput<'a> {
    root: &'a Path,
}

struct ProcessLinker {
    id: String,
    command: ProcessCommand,
}

struct ProcessChecker {
    id: String,
    command: ProcessCommand,
}

#[async_trait::async_trait]
impl SourceProvider for ProcessSource {
    fn name(&self) -> &str {
        &self.id
    }

    async fn discover(&self) -> CoreResult<Vec<SourceFile>> {
        run(
            "source",
            &self.id,
            &self.command,
            &SourceDiscoverInput { root: &self.root },
        )
        .await
    }
}

#[async_trait::async_trait]
impl Extractor for ProcessExtractor {
    fn name(&self) -> &str {
        &self.id
    }

    fn supports(&self, source: &SourceFile) -> bool {
        self.supports_extensions.is_empty()
            || Path::new(&source.path)
                .extension()
                .and_then(|extension| extension.to_str())
                .map(normalize_extension)
                .is_some_and(|extension| self.supports_extensions.contains(&extension))
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        run("extractor", &self.id, &self.command, &input).await
    }
}

#[async_trait::async_trait]
impl Linker for ProcessLinker {
    fn name(&self) -> &str {
        &self.id
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<athanor_domain::Relation>> {
        run("linker", &self.id, &self.command, &input).await
    }
}

#[async_trait::async_trait]
impl Checker for ProcessChecker {
    fn name(&self) -> &str {
        &self.id
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<athanor_domain::Diagnostic>> {
        run("checker", &self.id, &self.command, &input).await
    }
}

async fn run<I, O>(
    adapter_kind: &str,
    adapter_id: &str,
    command: &ProcessCommand,
    input: &I,
) -> CoreResult<O>
where
    I: Serialize,
    O: for<'de> Deserialize<'de>,
{
    let cancellation = PROCESS_CANCELLATION.try_with(Clone::clone).ok().flatten();
    run_with_limits(
        adapter_kind,
        adapter_id,
        command,
        input,
        ProcessLimits::default(),
        cancellation.as_ref(),
    )
    .await
}

pub(super) async fn run_with_limits<I, O>(
    adapter_kind: &str,
    adapter_id: &str,
    command: &ProcessCommand,
    input: &I,
    limits: ProcessLimits,
    cancellation: Option<&CancellationToken>,
) -> CoreResult<O>
where
    I: Serialize,
    O: for<'de> Deserialize<'de>,
{
    command.verify_unchanged().map_err(|error| {
        CoreError::Adapter(format!(
            "refusing external {adapter_kind} {adapter_id}: {error}"
        ))
    })?;
    let mut input = serde_json::to_vec(input).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to serialize input for external {adapter_kind} {adapter_id}: {error}"
        ))
    })?;
    input.push(b'\n');
    let output = TokioProcessRunner
        .run_cancellable(
            ProcessRequest {
                label: format!("external {adapter_kind} {adapter_id}"),
                program: command.program.clone(),
                args: command.args.clone(),
                working_dir: command.working_dir.clone(),
                clear_environment: command.clear_environment,
                stdin: input,
                limits: CoreProcessLimits {
                    timeout_ms: limits.timeout.as_millis() as u64,
                    max_stdin_bytes: limits.max_stdin_bytes,
                    max_stdout_bytes: limits.max_stdout_bytes,
                    max_stderr_bytes: limits.max_stderr_bytes,
                },
            },
            cancellation,
        )
        .await?;
    let stdout = process_output_excerpt(&output.stdout);
    let stderr = process_output_excerpt(&output.stderr);
    if !stdout.is_empty() {
        debug!(adapter_kind, adapter_id, stdout = %stdout, "external process adapter stdout");
    }
    if !stderr.is_empty() {
        warn!(adapter_kind, adapter_id, stderr = %stderr, "external process adapter stderr");
    }
    if !output.success {
        return Err(CoreError::Adapter(format!(
            "external {adapter_kind} {adapter_id} exited with {}; stderr: {}",
            output.exit_code.map_or_else(
                || "unknown status".to_string(),
                |code| format!("exit code: {code}")
            ),
            stderr
        )));
    }
    if output.stdout_truncated {
        return Err(CoreError::Adapter(format!(
            "external {adapter_kind} {adapter_id} stdout exceeded {} bytes",
            limits.max_stdout_bytes
        )));
    }
    if output.stderr_truncated {
        return Err(CoreError::Adapter(format!(
            "external {adapter_kind} {adapter_id} stderr exceeded {} bytes",
            limits.max_stderr_bytes
        )));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        CoreError::AdapterProtocol(format!(
            "failed to parse output from external {adapter_kind} {adapter_id}: {error}"
        ))
    })
}
