const LIB: &str = include_str!("../src/lib.rs");
const STORE: &str = include_str!("../src/store.rs");
const LIFECYCLE: &str = include_str!("../src/lifecycle.rs");
const CANONICAL: &str = include_str!("../src/canonical.rs");
const POINTER: &str = include_str!("../src/pointer_publication.rs");

#[test]
fn jsonl_store_has_explicit_modules_without_compatibility_includes() {
    assert!(!LIB.contains("include!("));
    assert!(!STORE.contains("include!("));
    assert!(!LIFECYCLE.contains("include!("));
    assert!(!CANONICAL.contains("include!("));
    assert!(LIB.contains("mod pointer_publication;"));
    assert!(LIB.contains("mod snapshot_io;"));
    assert!(LIB.contains("mod indexes;"));
}

#[test]
fn all_pointer_writers_share_commit_then_best_effort_cleanup() {
    assert!(POINTER.contains("cleanup_backup_after_commit"));
    assert!(POINTER.contains("was published but backup cleanup failed"));
    assert!(POINTER.contains("post_commit_cleanup_failure_keeps_new_pointer_published"));
    assert!(!POINTER.contains("failed to remove previous latest pointer"));
}
