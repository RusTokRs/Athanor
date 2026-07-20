const RETENTION: &str = include_str!("../src/repair_retention.rs");
const CLEANUP_RECOVERY: &str = include_str!("../src/repair_cleanup_recovery.rs");

#[test]
fn retention_rolls_back_the_first_tombstone_when_second_staging_fails() {
    assert!(
        RETENTION.contains("if let Err(error) = fs::rename(&row.index_state, &state_tombstone)")
    );
    assert!(RETENTION.contains("let _ = fs::rename(&read_tombstone, &row.read_model);"));
    assert!(RETENTION.contains("failed to stage index state generation"));
}

#[test]
fn retention_cleanup_remains_strict_and_recovery_visible() {
    assert!(RETENTION.contains("fs::remove_dir_all(&read_tombstone).with_context"));
    assert!(RETENTION.contains("fs::remove_file(&state_tombstone).with_context"));
    assert!(RETENTION.contains("dry_run_token_is_required_for_the_exact_plan"));
    assert!(RETENTION.contains("corruption_matrix_fails_closed"));
}

#[test]
fn cleanup_recovery_covers_complete_and_partial_tombstone_states() {
    for marker in [
        "recovers_both_staged_tombstones_idempotently",
        "rolls_back_read_tombstone_before_state_staging",
        "recovers_state_tombstone_after_read_removal_fault",
        "fs::rename(read_tombstone, &live_read)",
        "fs::remove_dir_all(read_tombstone).with_context",
        "fs::remove_file(state_tombstone).with_context",
    ] {
        assert!(
            CLEANUP_RECOVERY.contains(marker),
            "cleanup recovery matrix is missing `{marker}`"
        );
    }
}

#[test]
fn cleanup_recovery_fails_closed_on_live_artifact_conflicts() {
    assert!(CLEANUP_RECOVERY.contains("refusing cleanup recovery because live {kind}"));
    assert!(CLEANUP_RECOVERY.contains("index cleanup recovery left staged tombstones behind"));
    assert!(CLEANUP_RECOVERY.contains("empty index cleanup tombstone row"));
}
