use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use athanor_core::{
    CheckInput, Checker, CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, LinkInput,
    Linker, OperationContext, ProcessLimits as CoreProcessLimits, ProcessRequest, SourceFile,
    SourceProvider,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use super::process_adapter_support::{
    ProcessCommand, ProcessLimits, normalize_extension, process_output_excerpt,
};
use super::{
    CancellableProcessRunner, SharedProcessRunner, current_process_execution_context,
    default_process_runner,
};
use crate::CancellationToken;

pub(super) fn source(
    id: String,
    command: ProcessCommand,
    root: PathBuf,
    runner: SharedProcessRunner,
) -> Box<dyn SourceProvider> {
    Box::new(ProcessSource {
        id,
        command,
        root,
        runner,
    })
}

pub(super) fn extractor(
    id: String,
    command: ProcessCommand,
    supports_extensions: BTreeSet<String>,
    runner: SharedProcessRunner,
) -> Box<dyn Extractor> {
    Box::new(ProcessExtractor {
        id,
        command,
        supports_extensions,
        runner,
    })
}

pub(super) fn linker(
    id: String,
    command: ProcessCommand,
    runner: SharedProcessRunner,
) -> Box<dyn Linker> {
    Box::new(ProcessLinker {
        id,
        command,
        runner,
    })
}

pub(super) fn checker(
    id: String,
    command: ProcessCommand,
    runner: SharedProcessRunner,
) -> Box<dyn Checker> {
    Box::new(ProcessChecker {
        id,
        command,
        runner,
    })
}

struct ProcessExtractor {
    id: String,
    command: ProcessCommand,
    supports_extensions: BTreeSet<String>,
    runner: SharedProcessRunner,
}

struct ProcessSource {
    id: String,
    command: ProcessCommand,
    root: PathBuf,
    runner: SharedProcessRunner,
}

#[derive(Serialize)]
struct SourceDiscoverInput<'a> {
    root: &'a Path,
}

struct ProcessLinker {
    id: String,
    command: ProcessCommand,
    runner: SharedProcessRunner,
}

struct ProcessChecker {
    id: String,
    command: ProcessCommand,
    runner: SharedProcessRunner,
}

#[async_trait::async_trait]
impl SourceProvider for ProcessSource {
    fn name(&self) -> &str {
        &self.id
    }

    async fn discover(&self) -> CoreResult<Vec<SourceFile>> {
        run(
            self.runner.as_ref(),
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
        run(
            self.runner.as_ref(),
            "extractor",
            &self.id,
            &self.command,
            &input,
        )
        .await
    }
}

#[async_trait::async_trait]
impl Linker for ProcessLinker {
    fn name(&self) -> &str {
        &self.id
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<athanor_domain::Relation>> {
        run(
            self.runner.as_ref(),
            "linker",
            &self.id,
            &self.command,
            &input,
        )
        .await
    }
}

#[async_trait::async_trait]
impl Checker for ProcessChecker {
    fn name(&self) -> &str {
        &self.id
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<athanor_domain::Diagnostic>> {
        run(
            self.runner.as_ref(),
            "checker",
            &self.id,
            &self.command,
            &input,
        )
        .await
    }
}

async fn run<I, O>(
    runner: &dyn CancellableProcessRunner,
    adapter_kind: &str,
    adapter_id: &str,
    command: &ProcessCommand,
    input: &I,
) -> CoreResult<O>
where
    I: Serialize,
    O: for<'de> Deserialize<'de>,
{
    let context = current_process_execution_context();
    let operation = context.as_ref().map(|context| &context.operation);
    let cancellation = context
        .as_ref()
        .and_then(|context| context.cancellation.as_ref());
    run_with_limits_using(
        runner,
        adapter_kind,
        adapter_id,
        command,
        input,
        ProcessLimits::default(),
        operation,
        cancellation,
    )
    .await
}

#[allow(dead_code)]
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
    let runner = default_process_runner();
    run_with_limits_using(
        runner.as_ref(),
        adapter_kind,
        adapter_id,
        command,
        input,
        limits,
        None,
        cancellation,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_with_limits_using<I, O>(
    runner: &dyn CancellableProcessRunner,
    adapter_kind: &str,
    adapter_id: &str,
    command: &ProcessCommand,
    input: &I,
    limits: ProcessLimits,
    operation: Option<&OperationContext>,
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
    let output = runner
        .run_with_operation_context(
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
            operation,
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;

    use athanor_core::{ProcessOutput, ProcessRunner};
    use serde_json::json;

    use super::*;

    struct RecordingRunner {
        requests: Mutex<Vec<ProcessRequest>>,
        output: ProcessOutput,
        saw_operation: AtomicBool,
        saw_cancellation: AtomicBool,
    }

    impl RecordingRunner {
        fn new(output: ProcessOutput) -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                output,
                saw_operation: AtomicBool::new(false),
                saw_cancellation: AtomicBool::new(false),
            }
        }
    }

    #[async_trait::async_trait]
    impl ProcessRunner for RecordingRunner {
        async fn run(&self, request: ProcessRequest) -> CoreResult<ProcessOutput> {
            self.requests.lock().unwrap().push(request);
            Ok(self.output.clone())
        }
    }

    #[async_trait::async_trait]
    impl CancellableProcessRunner for RecordingRunner {
        async fn run_with_operation_context(
            &self,
            request: ProcessRequest,
            operation: Option<&OperationContext>,
            cancellation: Option<&CancellationToken>,
        ) -> CoreResult<ProcessOutput> {
            self.saw_operation
                .store(operation.is_some(), Ordering::Release);
            self.saw_cancellation
                .store(cancellation.is_some(), Ordering::Release);
            self.run(request).await
        }
    }

    #[tokio::test]
    async fn injected_runner_receives_request_and_execution_context() {
        let runner = RecordingRunner::new(output(br#"{"ok":true}"#.to_vec()));
        let operation = OperationContext::new("external-adapter-test");
        let cancellation = CancellationToken::new();
        let limits = ProcessLimits {
            timeout: Duration::from_millis(123),
            max_stdin_bytes: 456,
            max_stdout_bytes: 789,
            max_stderr_bytes: 321,
        };

        let value: serde_json::Value = run_with_limits_using(
            &runner,
            "extractor",
            "test",
            &command(),
            &json!({ "input": 42 }),
            limits,
            Some(&operation),
            Some(&cancellation),
        )
        .await
        .unwrap();

        assert_eq!(value, json!({ "ok": true }));
        assert!(runner.saw_operation.load(Ordering::Acquire));
        assert!(runner.saw_cancellation.load(Ordering::Acquire));
        let requests = runner.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].label, "external extractor test");
        assert_eq!(requests[0].stdin, b"{\"input\":42}\n");
        assert_eq!(requests[0].limits.timeout_ms, 123);
        assert_eq!(requests[0].limits.max_stdout_bytes, 789);
    }

    #[tokio::test]
    async fn injected_runner_nonzero_exit_keeps_stable_adapter_error() {
        let runner = RecordingRunner::new(ProcessOutput {
            success: false,
            exit_code: Some(7),
            stdout: Vec::new(),
            stderr: b"failure".to_vec(),
            stdout_truncated: false,
            stderr_truncated: false,
        });

        let error = run_with_limits_using::<_, serde_json::Value>(
            &runner,
            "checker",
            "test",
            &command(),
            &json!({}),
            ProcessLimits::default(),
            None,
            None,
        )
        .await
        .expect_err("nonzero adapter exit must fail");

        assert!(matches!(error, CoreError::Adapter(_)));
        assert!(error.to_string().contains("exit code: 7"));
        assert!(error.to_string().contains("failure"));
    }

    #[tokio::test]
    async fn injected_runner_truncation_fails_before_protocol_decode() {
        let runner = RecordingRunner::new(ProcessOutput {
            success: true,
            exit_code: Some(0),
            stdout: b"{}".to_vec(),
            stderr: Vec::new(),
            stdout_truncated: true,
            stderr_truncated: false,
        });

        let error = run_with_limits_using::<_, serde_json::Value>(
            &runner,
            "source",
            "test",
            &command(),
            &json!({}),
            ProcessLimits::default(),
            None,
            None,
        )
        .await
        .expect_err("truncated stdout must fail closed");

        assert!(matches!(error, CoreError::Adapter(_)));
        assert!(error.to_string().contains("stdout exceeded"));
    }

    #[tokio::test]
    async fn injected_runner_invalid_json_keeps_protocol_classification() {
        let runner = RecordingRunner::new(output(b"not-json".to_vec()));

        let error = run_with_limits_using::<_, serde_json::Value>(
            &runner,
            "linker",
            "test",
            &command(),
            &json!({}),
            ProcessLimits::default(),
            None,
            None,
        )
        .await
        .expect_err("invalid adapter JSON must remain a protocol error");

        assert!(matches!(error, CoreError::AdapterProtocol(_)));
    }

    fn command() -> ProcessCommand {
        ProcessCommand {
            program: PathBuf::from("/fake/adapter"),
            args: vec!["--json".to_string()],
            working_dir: PathBuf::from("/fake"),
            clear_environment: true,
            expected_content_hash: None,
            expected_content_size_bytes: None,
        }
    }

    fn output(stdout: Vec<u8>) -> ProcessOutput {
        ProcessOutput {
            success: true,
            exit_code: Some(0),
            stdout,
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }
}
