use std::fs;
use std::path::{Path, PathBuf};

use athanor_core::CoreError;
use athanor_projector_support::{
    NewDirectoryPublication, publish_staged_directory, write_output_file,
};

#[test]
fn failed_build_preserves_previous_directory_and_removes_staging() {
    let root = test_root("build-failure");
    let target = root.join("report");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("previous.txt"), "previous").unwrap();

    let error = publish_staged_directory(&target, "test report", |staging| {
        write_output_file(&staging.join("candidate.txt"), "candidate")?;
        Err(CoreError::Adapter("injected build failure".to_string()))
    })
    .expect_err("a failed build must reject the candidate");

    assert!(error.to_string().contains("injected build failure"));
    assert_eq!(
        fs::read_to_string(target.join("previous.txt")).unwrap(),
        "previous"
    );
    assert!(!target.join("candidate.txt").exists());
    assert_no_publication_siblings(&root, "report");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unpublished_immutable_candidate_is_removed_on_drop() {
    let root = test_root("drop-cleanup");
    let target = root.join("generations/one");
    let publication = NewDirectoryPublication::new(&target, "generation").unwrap();
    let staging = publication.staging_path().to_path_buf();
    write_output_file(&staging.join("manifest.json"), "candidate").unwrap();

    drop(publication);

    assert!(!target.exists());
    assert!(!staging.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn immutable_target_race_preserves_incumbent_and_cleans_candidate() {
    let root = test_root("target-race");
    let target = root.join("generations/one");
    let publication = NewDirectoryPublication::new(&target, "generation").unwrap();
    let staging = publication.staging_path().to_path_buf();
    write_output_file(&staging.join("manifest.json"), "candidate").unwrap();

    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("manifest.json"), "incumbent").unwrap();

    let error = publication
        .publish()
        .expect_err("a target created after staging must win the race");

    assert!(error.to_string().contains("appeared during publication"));
    assert_eq!(
        fs::read_to_string(target.join("manifest.json")).unwrap(),
        "incumbent"
    );
    assert!(!staging.exists());
    fs::remove_dir_all(root).unwrap();
}

fn assert_no_publication_siblings(root: &Path, target_name: &str) {
    let prefixes = [
        format!(".{target_name}.tmp-"),
        format!(".{target_name}.backup-"),
    ];
    let leftovers = fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| prefixes.iter().any(|prefix| name.starts_with(prefix)))
        .collect::<Vec<_>>();
    assert!(leftovers.is_empty(), "publication leftovers: {leftovers:?}");
}

fn test_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-publication-failure-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
