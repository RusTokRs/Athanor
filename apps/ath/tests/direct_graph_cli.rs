use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

#[test]
fn standard_graph_help_exposes_deadline() {
    for command in ["export", "related", "path", "hubs", "pagerank", "cycles"] {
        let output = run(&["graph", command, "--help"]);
        assert!(
            output.status.success(),
            "graph {command} help failed: {}",
            stderr(&output)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("--deadline-unix-ms"),
            "graph {command} help omitted deadline option"
        );
    }
}

#[test]
fn malformed_graph_deadline_fails_before_project_access() {
    let output = run(&[
        "graph",
        "pagerank",
        "--deadline-unix-ms",
        "tomorrow",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("invalid value 'tomorrow'"));
}

#[test]
fn manual_rustok_graph_help_remains_on_legacy_dispatcher() {
    let output = run(&["graph", "ffa", "violations", "--help"]);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("--deadline-unix-ms"));
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
