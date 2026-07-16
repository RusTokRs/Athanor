use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

#[test]
fn check_help_exposes_deadline() {
    let output = run(&["check", "--help"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--deadline-unix-ms"));
}

#[test]
fn malformed_check_deadline_fails_before_project_access() {
    let output = run(&[
        "check",
        "api",
        "--deadline-unix-ms",
        "tomorrow",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("invalid value 'tomorrow'"));
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
