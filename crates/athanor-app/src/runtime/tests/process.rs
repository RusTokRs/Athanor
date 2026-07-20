use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use athanor_core::{CoreError, ProcessLimits as CoreProcessLimits, ProcessRequest, ProcessRunner};
use serde_json::Value;

use super::super::{
    AdapterProcessCommand, TokioProcessRunner, process_adapter,
    process_adapter_support::{ProcessCommand, ProcessLimits},
};
#[cfg(unix)]
use super::fixtures::sh_path;
use super::fixtures::{failing_command, sleep_command, stdout_bytes_command, test_working_dir};
use crate::CancellationToken;

#[test]
fn rejects_bare_and_parent_directory_process_commands() {
    for program in ["sh", "../adapter"] {
        let command = AdapterProcessCommand {
            program: program.to_string(),
            args: Vec::new(),
        };
        assert!(ProcessCommand::from_manifest(Path::new("."), &command).is_err());
    }
}

#[tokio::test]
async fn external_process_timeout_is_reported() {
    let error = process_adapter::run_with_limits::<_, Value>(
        "checker",
        "external.checker.sleep",
        &sleep_command(),
        &serde_json::json!({}),
        ProcessLimits {
            timeout: Duration::from_millis(50),
            max_stdin_bytes: 1024,
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
        },
        None,
    )
    .await
    .expect_err("sleeping process should time out");
    assert!(error.to_string().contains("timed out"));
}

#[cfg(unix)]
#[tokio::test]
async fn external_process_timeout_terminates_unix_process_group() {
    let root = temp_root("runtime-process-group");
    fs::create_dir_all(&root).unwrap();
    let child_pid = root.join("child.pid");
    let command = ProcessCommand {
        program: sh_path(),
        args: vec![
            "-c".to_string(),
            format!("sleep 30 & echo $! > '{}'; wait", child_pid.display()),
        ],
        working_dir: test_working_dir(),
        expected_content_hash: None,
        expected_content_size_bytes: None,
        clear_environment: false,
    };

    let error = process_adapter::run_with_limits::<_, Value>(
        "checker",
        "external.checker.process_group",
        &command,
        &serde_json::json!({}),
        ProcessLimits {
            timeout: Duration::from_millis(200),
            max_stdin_bytes: 1024,
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
        },
        None,
    )
    .await
    .expect_err("process group should time out");
    assert!(matches!(error, CoreError::DeadlineExceeded(_)));

    let pid = fs::read_to_string(&child_pid)
        .expect("background child should record its pid")
        .trim()
        .to_string();
    let mut stopped = false;
    for _ in 0..20 {
        let status = std::process::Command::new("kill")
            .args(["-0", pid.as_str()])
            .output()
            .expect("kill command should be available on Unix");
        if !status.success() {
            stopped = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(stopped, "background process {pid} remained alive");
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cancellation_and_output_limits_are_enforced() {
    let cancellation = CancellationToken::new();
    let cancellation_for_task = cancellation.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancellation_for_task.cancel();
    });
    let error = process_adapter::run_with_limits::<_, Value>(
        "checker",
        "external.checker.sleep",
        &sleep_command(),
        &serde_json::json!({}),
        ProcessLimits {
            timeout: Duration::from_secs(5),
            max_stdin_bytes: 1024,
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
        },
        Some(&cancellation),
    )
    .await
    .expect_err("cancellation should stop the external process");
    assert!(matches!(error, CoreError::Cancelled(_)));

    let error = process_adapter::run_with_limits::<_, Value>(
        "checker",
        "external.checker.big_stdout",
        &stdout_bytes_command(2048),
        &serde_json::json!({}),
        ProcessLimits {
            timeout: Duration::from_secs(5),
            max_stdin_bytes: 1024,
            max_stdout_bytes: 32,
            max_stderr_bytes: 1024,
        },
        None,
    )
    .await
    .expect_err("oversized stdout should fail");
    assert!(error.to_string().contains("stdout exceeded"));
}

#[tokio::test]
async fn nonzero_exit_reports_bounded_stderr() {
    let error = process_adapter::run_with_limits::<_, Value>(
        "checker",
        "external.checker.fail",
        &failing_command(),
        &serde_json::json!({}),
        ProcessLimits {
            timeout: Duration::from_secs(5),
            max_stdin_bytes: 1024,
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
        },
        None,
    )
    .await
    .expect_err("non-zero process should fail");
    let message = error.to_string();
    assert!(message.contains("exited with"));
    assert!(message.contains("intentional failure"));
}

#[tokio::test]
async fn tokio_process_runner_implements_public_core_port() {
    let command = stdout_bytes_command(3);
    let output = ProcessRunner::run(
        &TokioProcessRunner,
        ProcessRequest {
            label: "test raw process".to_string(),
            program: command.program,
            args: command.args,
            working_dir: command.working_dir,
            clear_environment: false,
            stdin: Vec::new(),
            limits: CoreProcessLimits {
                timeout_ms: 10_000,
                max_stdin_bytes: 64,
                max_stdout_bytes: 64,
                max_stderr_bytes: 64,
            },
        },
    )
    .await
    .expect("raw process should run");
    assert!(output.success);
    assert_eq!(output.stdout.len(), 3);
    assert!(!output.stdout_truncated);
}

#[cfg(unix)]
#[tokio::test]
async fn clean_environment_process_profile_removes_inherited_environment() {
    let output = ProcessRunner::run(
        &TokioProcessRunner,
        ProcessRequest {
            label: "clean environment test".to_string(),
            program: PathBuf::from("/usr/bin/env"),
            args: Vec::new(),
            working_dir: test_working_dir(),
            clear_environment: true,
            stdin: Vec::new(),
            limits: CoreProcessLimits {
                timeout_ms: 10_000,
                max_stdin_bytes: 64,
                max_stdout_bytes: 1024,
                max_stderr_bytes: 1024,
            },
        },
    )
    .await
    .expect("clean-environment process should run");
    assert!(output.success);
    assert!(output.stdout.is_empty());
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
