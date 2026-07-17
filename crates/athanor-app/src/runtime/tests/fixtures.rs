use std::path::PathBuf;

use super::super::{AdapterProcessCommand, process_adapter_support::ProcessCommand};

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
            format!("$input | Out-Null; '{}'", json.replace('\'', "''")),
        ],
    }
}

#[cfg(not(windows))]
fn sh_json_command(json: &str) -> AdapterProcessCommand {
    AdapterProcessCommand {
        program: sh_path().display().to_string(),
        args: vec![
            "-c".to_string(),
            format!("cat >/dev/null; printf '%s' '{}'", json.replace('\'', "'\\''")),
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

#[cfg(windows)]
pub(super) fn sleep_command() -> ProcessCommand {
    ProcessCommand {
        program: powershell_path(),
        args: vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "$input | Out-Null; Start-Sleep -Seconds 5".to_string(),
        ],
        working_dir: test_working_dir(),
        expected_content_hash: None,
        expected_content_size_bytes: None,
        clear_environment: false,
    }
}

#[cfg(not(windows))]
pub(super) fn sleep_command() -> ProcessCommand {
    ProcessCommand {
        program: sh_path(),
        args: vec!["-c".to_string(), "cat >/dev/null; sleep 5".to_string()],
        working_dir: test_working_dir(),
        expected_content_hash: None,
        expected_content_size_bytes: None,
        clear_environment: false,
    }
}

#[cfg(windows)]
pub(super) fn stdout_bytes_command(bytes: usize) -> ProcessCommand {
    ProcessCommand {
        program: powershell_path(),
        args: vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            format!("$input | Out-Null; [Console]::Out.Write(('x' * {bytes}))"),
        ],
        working_dir: test_working_dir(),
        expected_content_hash: None,
        expected_content_size_bytes: None,
        clear_environment: false,
    }
}

#[cfg(not(windows))]
pub(super) fn stdout_bytes_command(bytes: usize) -> ProcessCommand {
    ProcessCommand {
        program: sh_path(),
        args: vec![
            "-c".to_string(),
            format!("cat >/dev/null; yes x | tr -d '\\n' | head -c {bytes}"),
        ],
        working_dir: test_working_dir(),
        expected_content_hash: None,
        expected_content_size_bytes: None,
        clear_environment: false,
    }
}

#[cfg(windows)]
pub(super) fn failing_command() -> ProcessCommand {
    ProcessCommand {
        program: powershell_path(),
        args: vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "$input | Out-Null; [Console]::Error.Write('intentional failure'); exit 7"
                .to_string(),
        ],
        working_dir: test_working_dir(),
        expected_content_hash: None,
        expected_content_size_bytes: None,
        clear_environment: false,
    }
}

#[cfg(not(windows))]
pub(super) fn failing_command() -> ProcessCommand {
    ProcessCommand {
        program: sh_path(),
        args: vec![
            "-c".to_string(),
            "cat >/dev/null; printf '%s' 'intentional failure' >&2; exit 7".to_string(),
        ],
        working_dir: test_working_dir(),
        expected_content_hash: None,
        expected_content_size_bytes: None,
        clear_environment: false,
    }
}

#[cfg(windows)]
pub(super) fn powershell_path() -> PathBuf {
    let path = PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
    assert!(path.is_file(), "powershell.exe not found at {}", path.display());
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
