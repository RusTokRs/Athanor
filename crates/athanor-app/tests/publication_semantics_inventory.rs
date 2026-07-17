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
fn daemon_endpoint_uses_the_shared_atomic_replacement() {
    assert!(DAEMON_ENDPOINT.contains("replace_output_file"));
    assert!(!DAEMON_ENDPOINT.contains("fs::remove_file(path)"));
}

#[test]
fn previously_safe_publishers_keep_non_fatal_cleanup() {
    assert!(PLUGIN_TRUST_REGISTRY.contains(
        "adapter trust registry was published but backup cleanup failed"
    ));
    assert!(RUNTIME_PROJECTOR.contains("let _ = remove_path_if_exists(&backup)"));
    assert!(TANTIVY_SEARCH.contains("let _ = std::fs::remove_dir_all(backup)"));
}
