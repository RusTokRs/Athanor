use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

#[test]
fn read_help_exposes_optional_deadline() {
    for command in [
        "context",
        "explain",
        "overview",
        "impact",
        "change-map",
        "search",
        "explore",
    ] {
        let output = run(&[command, "--help"]);
        assert!(
            output.status.success(),
            "{command} help failed: {}",
            stderr(&output)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("--deadline-unix-ms"),
            "{command} help omitted deadline option"
        );
    }
}

#[test]
fn explore_requires_a_query_or_a_target() {
    let output = run(&["explore", "--path", "."]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("explore requires a non-empty query or a target"),
        "explore must reject an empty request before touching the store: {}",
        stderr(&output)
    );
}

#[test]
fn malformed_deadline_is_rejected_before_project_access() {
    let output = run(&["search", "login", "--deadline-unix-ms", "tomorrow"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("invalid value 'tomorrow'"));
}

#[test]
fn malformed_environment_deadline_fails_closed() {
    let output = Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(["search", "login"])
        .env("ATHANOR_DEADLINE_UNIX_MS", "tomorrow")
        .output()
        .expect("run ath CLI");
    assert!(!output.status.success());
    assert!(stderr(&output).contains("ATHANOR_DEADLINE_UNIX_MS must be an unsigned integer"));
}

#[test]
fn non_read_help_remains_on_legacy_dispatcher() {
    let output = run(&["init", "--help"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("--deadline-unix-ms"));
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
