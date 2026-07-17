const LIB: &str = include_str!("../src/lib.rs");
const STORE: &str = include_str!("../src/store.rs");
const LIFECYCLE: &str = include_str!("../src/lifecycle.rs");
const CANONICAL: &str = include_str!("../src/canonical.rs");
const POINTER: &str = include_str!("../src/pointer_publication.rs");
const LATEST: &str = include_str!("../src/latest.rs");
const SNAPSHOT_IO: &str = include_str!("../src/snapshot_io.rs");
const INDEXES: &str = include_str!("../src/indexes.rs");
const COMMIT_MARKER: &str = include_str!("../src/commit_marker.rs");
const ATOMIC_PUBLICATION: &str = include_str!("../src/atomic_publication.rs");

#[test]
fn jsonl_store_has_explicit_modules_without_compatibility_includes() {
    for source in [LIB, STORE, LIFECYCLE, CANONICAL] {
        assert!(!source.contains("include!("));
    }
    assert!(LIB.contains("mod pointer_publication;"));
    assert!(LIB.contains("mod snapshot_io;"));
    assert!(LIB.contains("mod indexes;"));
}

#[test]
fn production_modules_remain_bounded() {
    for (name, source) in [
        ("store", STORE),
        ("lifecycle", LIFECYCLE),
        ("canonical", CANONICAL),
        ("pointer_publication", POINTER),
        ("latest", LATEST),
        ("snapshot_io", SNAPSHOT_IO),
        ("indexes", INDEXES),
        ("commit_marker", COMMIT_MARKER),
        ("atomic_publication", ATOMIC_PUBLICATION),
    ] {
        let lines = source.lines().count();
        assert!(lines <= 300, "{name} grew back to {lines} lines");
    }
}

#[test]
fn all_pointer_writers_share_commit_then_best_effort_cleanup() {
    assert!(POINTER.contains("cleanup_backup_after_commit"));
    assert!(POINTER.contains("was published but backup cleanup failed"));
    assert!(POINTER.contains("post_commit_cleanup_failure_keeps_new_pointer_published"));
    assert!(POINTER.contains("existing target is not a file"));
    assert!(!POINTER.contains("failed to remove previous latest pointer"));
}
