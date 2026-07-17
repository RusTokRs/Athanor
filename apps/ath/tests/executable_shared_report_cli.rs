use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

#[test]
fn shared_generation_help_exposes_json_output() {
    for args in [
        vec!["wiki", "--help"],
        vec!["report", "html", "--help"],
        vec!["generate", "--help"],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{} help failed: {}",
            args.join(" "),
            stderr(&output)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("--json"),
            "{} help omitted --json",
            args.join(" ")
        );
    }
}

#[test]
fn executable_shared_reports_preserve_registered_shapes() {
    let root = temp_root("shared-reports");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn executable_shared_report_fixture() -> usize { 7 }\n",
    )
    .unwrap();
    let root_arg = root.to_str().expect("UTF-8 fixture path");

    let index = run_json(&["index", root_arg, "--json"]);
    assert_eq!(index["schema"], "athanor.index_report.v1");
    assert!(index["metrics"].is_object());
    let snapshot = index["snapshot"]
        .as_str()
        .expect("index snapshot")
        .to_string();

    let context = run_json(&[
        "context",
        "executable shared report fixture",
        "--path",
        root_arg,
        "--json",
    ]);
    assert_eq!(context["schema"], "athanor.context_pack.v1");
    assert_eq!(context["task"], "executable shared report fixture");
    assert!(context["summary"].is_string());
    assert!(context["files"].is_array());

    let generation = run_json(&["generate", root_arg, "--json"]);
    assert_eq!(generation["schema"], "athanor.generation.v1");
    assert_eq!(generation["snapshot"].as_str(), Some(snapshot.as_str()));
    assert!(generation["metrics"].is_object());

    let wiki = run_json(&[
        "wiki",
        root_arg,
        "--output",
        "parity-wiki",
        "--json",
    ]);
    assert_eq!(wiki["schema"], "athanor.wiki_report.v1");
    assert_eq!(wiki["snapshot"].as_str(), Some(snapshot.as_str()));
    assert!(wiki["output_dir"]
        .as_str()
        .expect("wiki output directory")
        .ends_with("parity-wiki"));

    let html = run_json(&[
        "report",
        "html",
        root_arg,
        "--output",
        "parity-html",
        "--json",
    ]);
    assert_eq!(html["schema"], "athanor.html_report.v1");
    assert_eq!(html["snapshot"].as_str(), Some(snapshot.as_str()));
    assert!(html["output_dir"]
        .as_str()
        .expect("HTML output directory")
        .ends_with("parity-html"));

    for report in [&wiki, &html] {
        assert!(report["entities"].is_number());
        assert!(report["facts"].is_number());
        assert!(report["relations"].is_number());
        assert!(report["open_diagnostics"].is_number());
    }

    fs::remove_dir_all(root).unwrap();
}

fn run_json(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{} failed: {}",
        args.join(" "),
        stderr(&output)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{} returned invalid JSON: {error}", args.join(" ")))
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-cli-{label}-{nonce}"))
}
