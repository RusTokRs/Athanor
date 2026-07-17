const INVENTORY: &str =
    include_str!("../../../docs/development/publication-semantics-inventory.md");
const PROJECTOR_SUPPORT: &str =
    include_str!("../../athanor-projector-support/src/lib.rs");
const DAEMON_ENDPOINT: &str = include_str!("../src/daemon_endpoint.rs");
const PLUGIN_TRUST_REGISTRY: &str =
    include_str!("../src/runtime/plugin_trust_registry.rs");
const RUNTIME_PROJECTOR: &str =
    include_str!("../../athanor-runtime-defaults/src/projector_operation.rs");
const TANTIVY_SEARCH: &str = include_str!("../../athanor-search-tantivy/src/lib.rs");
const READ_MODEL: &str = include_str!("../src/read_model.rs");
const INDEX_STATE: &str = include_str!("../src/index_state.rs");
const PROJECT_REGISTRY: &str = include_str!("../src/project_registry.rs");
const PUBLICATION_JOURNAL: &str = include_str!("../src/index_publication_journal.rs");

#[test]
fn inventory_covers_every_confirmed_publication_owner() {
    for owner in [
        "athanor-projector-support",
        "athanor-app/daemon_endpoint.rs",
        "athanor-app/runtime/plugin_trust_registry.rs",
        "athanor-runtime-defaults/projector_operation.rs",
        "athanor-search-tantivy",
        "athanor-store-jsonl/atomic_publication.rs",
        "athanor-app/index_publication.rs",
        "athanor-app/read_model.rs",
        "athanor-app/index_state.rs",
        "athanor-app/project_registry.rs",
        "athanor-app/index_publication_journal.rs",
        "athanor-store-jsonl/lib.rs",
        "athanor-store-jsonl/store.rs",
        "athanor-app/repair_retention.rs",
        "athanor-app/repair_cleanup_recovery.rs",
        "apps/athd",
    ] {
        assert!(INVENTORY.contains(owner), "missing publication owner {owner}");
    }
    assert!(INVENTORY.contains("PUB-004"));
    assert!(INVENTORY.contains("PUB-005"));
}

#[test]
fn generic_replacement_commits_before_best_effort_cleanup() {
    assert!(PROJECTOR_SUPPORT.contains("cleanup_backup_after_publish"));
    assert!(PROJECTOR_SUPPORT.contains("output was published but backup cleanup failed"));
    assert!(PROJECTOR_SUPPORT.contains("cleanup_backup_after_publish(&backup, output_kind);\n    Ok(())"));
}

#[test]
fn generic_replacement_has_deterministic_post_commit_fault_coverage() {
    assert!(PROJECTOR_SUPPORT.contains("INJECT_BACKUP_CLEANUP_FAILURE"));
    assert!(PROJECTOR_SUPPORT.contains(
        "post_commit_cleanup_failure_keeps_new_directory_published"
    ));
    assert!(PROJECTOR_SUPPORT.contains(
        "output was published but backup cleanup failed (injected)"
    ));
}

#[test]
fn daemon_endpoint_uses_the_shared_atomic_replacement() {
    assert!(DAEMON_ENDPOINT.contains("replace_output_file"));
    assert!(!DAEMON_ENDPOINT.contains("fs::remove_file(path)"));
}

#[test]
fn app_publication_cleanup_is_non_fatal_after_commit() {
    for (source, warning, legacy_error) in [
        (
            READ_MODEL,
            "read model was published but backup cleanup failed",
            "failed to remove read model backup",
        ),
        (
            INDEX_STATE,
            "index state was published but backup cleanup failed",
            "failed to remove index state backup",
        ),
        (
            PROJECT_REGISTRY,
            "project registry was published but backup cleanup failed",
            "failed to remove registry backup",
        ),
        (
            PUBLICATION_JOURNAL,
            "publication journal was published but backup cleanup failed",
            "failed to remove publication journal backup",
        ),
    ] {
        assert!(source.contains("use tracing::warn;"));
        assert!(source.contains(warning), "missing warning path `{warning}`");
        assert!(
            !source.contains(legacy_error),
            "post-commit cleanup still propagates `{legacy_error}`"
        );
    }
}

#[test]
fn previously_safe_publishers_keep_non_fatal_cleanup() {
    assert!(PLUGIN_TRUST_REGISTRY.contains(
        "adapter trust registry was published but backup cleanup failed"
    ));
    assert!(RUNTIME_PROJECTOR.contains("let _ = remove_path_if_exists(&backup)"));
    assert!(TANTIVY_SEARCH.contains("let _ = std::fs::remove_dir_all(backup)"));
}
