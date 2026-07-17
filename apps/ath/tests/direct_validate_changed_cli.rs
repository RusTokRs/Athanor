use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

#[test]
fn validate_changed_help_exposes_focused_contract() {
    let output = run(&["validate-changed", "--help"]);
    assert!(
        output.status.success(),
        "validate-changed help failed: {}",
        stderr(&output)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for option in ["--path", "--file", "--json"] {
        assert!(
            stdout.contains(option),
            "validate-changed help omitted {option}: {stdout}"
        );
    }
}

#[test]
fn malformed_validate_changed_args_fail_before_project_access() {
    let output = run(&[
        "validate-changed",
        "--path",
        "/definitely/missing/athanor-project",
        "--file",
    ]);
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("--file"), "unexpected error: {error}");
    assert!(
        !error.contains("failed to canonicalize"),
        "parser should fail before project access: {error}"
    );
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
