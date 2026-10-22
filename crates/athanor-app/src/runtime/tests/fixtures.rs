use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool};

use super::super::AdapterProcessCommand;
use super::super::process_adapter_support::{ProcessCommand, ProcessLimits};

#[cfg(windows)]
pub(super) fn empty_output_command() -> AdapterProcessCommand {
    powershell_json_command("{\"entities\":[],\"facts\":[]}")
}

#[cfg(not(windows))]
pub(super) fn empty_output_command() -> AdapterProcessCommand {
    sh_json_command("{\"entities\":[],\"facts\":[]}")
}

#[cfg(windows)]
pub(super) fn source_output_command() -> AdapterProcessCommand {
    powershell_json_command(
        "[{\"path\":\"virtual/readme.md\",\"language_hint\":\"markdown\",\"content_hash\":\"test:1\",\"content\":\"# Virtual\"}]",
    )
}

#[cfg(not(windows))]
pub(super) fn source_output_command() -> AdapterProcessCommand {
    sh_json_command(
        "[{\"path\":\"virtual/readme.md\",\"language_hint\":\"markdown\",\"content_hash\":\"test:1\",\"content\":\"# Virtual\"}]",
    )
}

#[cfg(windows)]
pub(super) fn empty_array_command() -> AdapterProcessCommand {
    powershell_json_command("[]")
}

#[cfg(not(windows))]
pub(super) fn empty_array_command() -> AdapterProcessCommand {
    sh_json_command("[]")
}

#[cfg(windows)]
fn powershell_json_command(json: &str) -> AdapterProcessCommand {
    AdapterProcessCommand {
        program: powershell_path().display().to_string(),
        args: vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            format!("'{}'", json.replace('\'', "''")),
        ],
    }
}

#[cfg(not(windows))]
fn sh_json_command(json: &str) -> AdapterProcessCommand {
    AdapterProcessCommand {
        program: sh_path().display().to_string(),
        args: vec![
            "-c".to_string(),
            format!(
                "cat >/dev/null; printf '%s' '{}'",
                json.replace('\'', "'\\''")
            ),
        ],
    }
}

#[cfg(windows)]
pub(super) fn empty_output_program() -> PathBuf {
    powershell_path()
}

#[cfg(not(windows))]
pub(super) fn empty_output_program() -> PathBuf {
    sh_path()
}

fn unverified_fields() -> (
    Vec<super::super::TrustedAdapterExecutable>,
    Arc<AtomicBool>,
    ProcessLimits,
) {
    (
        Vec::new(),
        Arc::new(AtomicBool::new(false)),
        ProcessLimits::default(),
    )
}

#[cfg(windows)]
pub(super) fn sleep_command() -> ProcessCommand {
    let (expected, fully_verified, limits) = unverified_fields();
    ProcessCommand {
        program: powershell_path(),
        args: vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "Start-Sleep -Seconds 5".to_string(),
        ],
        working_dir: test_working_dir(),
        expected,
        fully_verified,
        limits,
        clear_environment: false,
    }
}

#[cfg(not(windows))]
pub(super) fn sleep_command() -> ProcessCommand {
    let (expected, fully_verified, limits) = unverified_fields();
    ProcessCommand {
        program: sh_path(),
        args: vec!["-c".to_string(), "cat >/dev/null; sleep 5".to_string()],
        working_dir: test_working_dir(),
        expected,
        fully_verified,
        limits,
        clear_environment: false,
    }
}

#[cfg(windows)]
pub(super) fn stdout_bytes_command(bytes: usize) -> ProcessCommand {
    let (expected, fully_verified, limits) = unverified_fields();
    ProcessCommand {
        program: powershell_path(),
        args: vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            format!(
                "$stdout = [Console]::OpenStandardOutput(); $bytes = [byte[]]::new({bytes}); $stdout.Write($bytes, 0, $bytes.Length); $stdout.Flush()"
            ),
        ],
        working_dir: test_working_dir(),
        expected,
        fully_verified,
        limits,
        clear_environment: false,
    }
}

#[cfg(not(windows))]
pub(super) fn stdout_bytes_command(bytes: usize) -> ProcessCommand {
    let (expected, fully_verified, limits) = unverified_fields();
    ProcessCommand {
        program: sh_path(),
        args: vec![
            "-c".to_string(),
            format!("cat >/dev/null; yes x | tr -d '\\n' | head -c {bytes}"),
        ],
        working_dir: test_working_dir(),
        expected,
        fully_verified,
        limits,
        clear_environment: false,
    }
}

#[cfg(windows)]
pub(super) fn failing_command() -> ProcessCommand {
    let (expected, fully_verified, limits) = unverified_fields();
    ProcessCommand {
        program: powershell_path(),
        args: vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "$stderr = [Console]::OpenStandardError(); $bytes = [System.Text.Encoding]::ASCII.GetBytes('intentional failure'); $stderr.Write($bytes, 0, $bytes.Length); $stderr.Flush(); exit 7".to_string(),
        ],
        working_dir: test_working_dir(),
        expected,
        fully_verified,
        limits,
        clear_environment: false,
    }
}

#[cfg(not(windows))]
pub(super) fn failing_command() -> ProcessCommand {
    let (expected, fully_verified, limits) = unverified_fields();
    ProcessCommand {
        program: sh_path(),
        args: vec![
            "-c".to_string(),
            "cat >/dev/null; printf '%s' 'intentional failure' >&2; exit 7".to_string(),
        ],
        working_dir: test_working_dir(),
        expected,
        fully_verified,
        limits,
        clear_environment: false,
    }
}

#[cfg(windows)]
pub(super) fn powershell_path() -> PathBuf {
    let path = PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
    assert!(
        path.is_file(),
        "powershell.exe not found at {}",
        path.display()
    );
    path
}

#[cfg(not(windows))]
pub(super) fn sh_path() -> PathBuf {
    for candidate in ["/bin/sh", "/usr/bin/sh"] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return path;
        }
    }
    panic!("sh executable not found");
}

pub(super) fn test_working_dir() -> PathBuf {
    std::env::current_dir().expect("test process has a current directory")
}

pub(super) fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
