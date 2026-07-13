use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use athanor_core::{CoreError, CoreErrorCode};
use athanor_store_surrealdb::SurrealKnowledgeStore;

#[tokio::test]
async fn persistent_surrealkv_rejects_second_connection_as_retryable_busy() {
    let root = unique_store_path();
    std::fs::create_dir_all(&root).expect("create persistent SurrealKV test directory");
    let uri = surrealkv_uri(&root);

    let first = SurrealKnowledgeStore::connect(&uri)
        .await
        .expect("open first persistent SurrealKV connection");
    let error = SurrealKnowledgeStore::connect(&uri)
        .await
        .expect_err("second persistent connection must fail while the directory is locked");

    assert_eq!(error.code(), CoreErrorCode::Busy);
    assert!(error.is_retryable());
    assert!(matches!(&error, CoreError::Busy(_)));
    assert!(
        error
            .to_string()
            .to_ascii_lowercase()
            .contains("already locked by another process"),
        "unexpected lock-contention error: {error}"
    );

    drop(first);
    let _ = std::fs::remove_dir_all(root);
}

fn unique_store_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "athanor-surrealkv-lock-{}-{timestamp}",
        std::process::id()
    ))
}

fn surrealkv_uri(path: &Path) -> String {
    format!("surrealkv://{}", path.to_string_lossy())
}
