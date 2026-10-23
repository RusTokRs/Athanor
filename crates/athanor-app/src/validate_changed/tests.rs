use std::fs;

use super::*;

#[test]
fn parses_git_status_z_changed_removed_and_renamed_paths() {
    let report = parse_git_status_z(
        b" M src/lib.rs\0D  docs/old.md\0?? apps/admin/app.tsx\0R  docs/new.md\0docs/old-name.md\0C  docs/copied.md\0docs/source.md\0",
        b"",
    )
    .unwrap();

    assert!(report.changed.contains("src/lib.rs"));
    assert!(report.changed.contains("apps/admin/app.tsx"));
    assert!(report.changed.contains("docs/new.md"));
    assert!(report.changed.contains("docs/copied.md"));
    assert!(report.removed.contains("docs/old.md"));
    assert!(report.removed.contains("docs/old-name.md"));
    assert!(!report.changed.contains("docs/old-name.md"));
    assert!(!report.changed.contains("docs/source.md"));
    assert!(!report.removed.contains("docs/source.md"));
}

#[test]
fn strips_the_nested_project_prefix_and_preserves_unix_backslashes() {
    let report = parse_git_status_z(
        b" M project/src/lib.rs\0?? project/src/back\\slash.rs\0?? outside/other.rs\0",
        b"project/",
    )
    .unwrap();

    assert!(report.changed.contains("src/lib.rs"));
    assert!(report.changed.contains("src/back\\slash.rs"));
    assert!(!report.changed.contains("project/src/lib.rs"));
    assert!(!report.changed.contains("outside/other.rs"));
}

#[test]
fn rejects_non_utf8_project_paths_instead_of_lossy_reclassification() {
    let error = parse_git_status_z(b"?? src/bad-\xff.rs\0", b"")
        .expect_err("non-UTF-8 project paths must fail closed");

    assert!(error.to_string().contains("non-UTF-8"));
}

#[test]
fn real_git_status_is_scoped_to_a_nested_project_root() {
    let root = temp_root("nested-git");
    fs::create_dir_all(root.join("project/src")).unwrap();
    fs::create_dir_all(root.join("outside")).unwrap();
    let root = normalize_canonical_path(root.canonicalize().unwrap());
    let project = root.join("project");
    git(&root, &["init", "--quiet"]);
    fs::write(project.join("src/lib.rs"), "fn inside() {}\n").unwrap();
    fs::write(root.join("outside/other.rs"), "fn outside() {}\n").unwrap();

    let report = changed_paths_from_git(&project)
        .unwrap()
        .expect("Git repository should be detected");

    assert!(report.changed.contains("src/lib.rs"));
    assert!(!report.changed.contains("project/src/lib.rs"));
    assert!(!report.changed.contains("outside/other.rs"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn index_state_fallback_detects_new_files_without_git() {
    let root = temp_root("non-git-new-file");
    fs::create_dir_all(&root).unwrap();
    let root = normalize_canonical_path(root.canonicalize().unwrap());
    fs::write(root.join("new.rs"), "fn new() {}\n").unwrap();

    let report = changed_paths_from_index_state(&root, &crate::IndexState::empty()).unwrap();

    assert!(report.changed.contains("new.rs"));
    assert!(report.removed.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn git_paths_cannot_read_through_an_escaping_symlink() {
    let root = temp_root("escaping-symlink");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    git(&root, &["init", "--quiet"]);
    let outside = root.with_extension("outside.rs");
    fs::write(&outside, "fn outside() {}\n").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked.rs")).unwrap();

    let error = changed_paths_from_git(&root)
        .expect_err("an escaping symlink must fail before source reading");

    assert!(error.to_string().contains("is outside"));
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(outside).unwrap();
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("run Git");
    assert!(output.status.success(), "Git failed: {output:?}");
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-validate-changed-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
