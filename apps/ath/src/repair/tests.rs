use std::path::PathBuf;

use super::model::{Command, HelpTopic, parse};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn parses_exact_retention_plan_and_apply_forms() {
    assert_eq!(
        parse(&args(&[
            "repair",
            "index-retention",
            "project",
            "--dry-run",
            "--keep",
            "2",
            "--json",
        ]))
        .unwrap(),
        Some(Command::IndexRetention {
            path: PathBuf::from("project"),
            dry_run: true,
            keep: 2,
            confirmation_token: None,
            json: true,
        })
    );
    assert_eq!(
        parse(&args(&[
            "repair",
            "index-retention",
            "--confirmation-token",
            "sha256:test",
        ]))
        .unwrap(),
        Some(Command::IndexRetention {
            path: PathBuf::from("."),
            dry_run: false,
            keep: 0,
            confirmation_token: Some("sha256:test".to_string()),
            json: false,
        })
    );
}

#[test]
fn parses_repair_inspection_and_transactional_commands() {
    assert_eq!(
        parse(&args(&["repair", "inspect", "project", "--json"])).unwrap(),
        Some(Command::Inspect {
            path: PathBuf::from("project"),
            json: true,
        })
    );
    assert_eq!(
        parse(&args(&["repair", "inspect", "--help"])).unwrap(),
        Some(Command::Help(HelpTopic::Inspect))
    );
    assert_eq!(
        parse(&args(&["repair", "recover-index", "project", "--dry-run"])).unwrap(),
        Some(Command::RecoverIndex {
            path: PathBuf::from("project"),
            dry_run: true,
            json: false,
        })
    );
    assert_eq!(
        parse(&args(&[
            "repair",
            "recover-index-cleanup",
            "project",
            "--json",
        ]))
        .unwrap(),
        Some(Command::RecoverIndexCleanup {
            path: PathBuf::from("project"),
            dry_run: false,
            json: true,
        })
    );
    assert_eq!(
        parse(&args(&[
            "repair",
            "repair-latest",
            "project",
            "--snapshot",
            "snap_jsonl_00000002",
            "--dry-run",
        ]))
        .unwrap(),
        Some(Command::RepairLatest {
            path: PathBuf::from("project"),
            dry_run: true,
            snapshot: Some("snap_jsonl_00000002".to_string()),
            json: false,
        })
    );
    assert_eq!(
        parse(&args(&["repair", "index-retention", "--help"])).unwrap(),
        Some(Command::Help(HelpTopic::IndexRetention))
    );
}

#[test]
fn rejects_ambiguous_or_invalid_repair_arguments() {
    assert!(
        parse(&args(&[
            "repair",
            "index-retention",
            "--dry-run",
            "--confirmation-token",
            "sha256:test",
        ]))
        .unwrap_err()
        .to_string()
        .contains("conflicts")
    );
    assert!(
        parse(&args(&["repair", "index-retention", "--keep", "many"]))
            .unwrap_err()
            .to_string()
            .contains("non-negative integer")
    );
    assert!(
        parse(&args(&["repair", "inspect", "one", "two"]))
            .unwrap_err()
            .to_string()
            .contains("at most one")
    );
}
