//! Real JSONL publication fixture shared by daemon read lifecycle regressions.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use athanor_core::SearchIndex;

use crate::daemon::{
    CachedSearchIndex, DAEMON_ENDPOINT_SCHEMA, DAEMON_PROTOCOL_VERSION, DaemonEndpoint,
    DaemonLifecycleState, DaemonState, DaemonTransport,
};
use crate::daemon_runtime::BoundedCache;

pub(crate) struct DaemonFixture {
    pub(crate) state: Arc<DaemonState>,
    pub(crate) initial_snapshot: String,
    request_sequence: AtomicU64,
    // Drop state/search handles before removing files, including on Windows.
    _directory: TestDirectory,
}

struct TestDirectory(PathBuf);

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl DaemonFixture {
    pub(crate) async fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "athanor-daemon-freshness-{label}-{}-{nonce}",
            std::process::id()
        ));
        let directory = TestDirectory(root.clone());
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"freshness-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        let initial_snapshot = publish_at(&root, "freshness_alpha").await;
        let state = Arc::new(DaemonState {
            composition: crate::test_runtime::composition(),
            endpoint: DaemonEndpoint {
                schema: DAEMON_ENDPOINT_SCHEMA.to_string(),
                protocol_version: DAEMON_PROTOCOL_VERSION,
                athanor_version: "test".to_string(),
                runtime_id: "runtime-test".to_string(),
                token_path: root.join("token"),
                project_id: "project".to_string(),
                registry_path: root.join("registry"),
                root,
                address: "127.0.0.1:1".parse().unwrap(),
                transport: DaemonTransport::Tcp,
                local_socket_path: None,
                windows_pipe_name: None,
                pid: 1,
                started_at_unix_ms: 0,
                max_concurrent_requests: 8,
                max_job_history: 32,
                watch: false,
                watch_poll: false,
                debounce_ms: 250,
                max_request_bytes: 1024 * 1024,
                max_response_bytes: 1024 * 1024,
            },
            auth_token: "token".to_string(),
            insecure_allow_v1: false,
            lifecycle: Mutex::new(DaemonLifecycleState::Running),
            last_successful_index: Mutex::new(None),
            jobs: Mutex::new(Vec::new()),
            next_job_sequence: Mutex::new(1),
            max_job_history: 32,
            last_observed_snapshot: Mutex::new(None),
            search_index_cache: Mutex::new(None),
            overview_cache: Mutex::new(BoundedCache::new(8)),
            context_cache: Mutex::new(BoundedCache::new(8)),
            cancellation_tokens: Mutex::new(HashMap::new()),
        });
        Self {
            state,
            initial_snapshot,
            request_sequence: AtomicU64::new(1),
            _directory: directory,
        }
    }

    pub(crate) async fn publish(&self, symbol: &str) -> String {
        // This is an external writer: deliberately do not invalidate daemon caches.
        publish_at(&self.state.endpoint.root, symbol).await
    }

    pub(crate) fn set_search_index(&self, index: Arc<dyn SearchIndex>) {
        *self.state.search_index_cache.lock().unwrap() = Some(CachedSearchIndex {
            snapshot_id: self.initial_snapshot.clone(),
            index,
        });
    }

    pub(crate) fn request_id(&self) -> String {
        // Cancellation identities are process-wide; fixtures run in parallel and must not alias.
        let prefix = self
            .state
            .endpoint
            .root
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let sequence = self.request_sequence.fetch_add(1, Ordering::Relaxed);
        let request_id = format!("{prefix}-{sequence}");
        assert!(request_id.len() <= 128);
        request_id
    }

    pub(crate) fn latest_pointer(&self) -> PathBuf {
        self.state
            .endpoint
            .root
            .join(crate::ProjectConfig::default().storage.path)
            .join("latest.json")
    }
}

async fn publish_at(root: &Path, symbol: &str) -> String {
    fs::write(
        root.join("src/lib.rs"),
        format!("pub fn {symbol}() -> u8 {{ 1 }}\n"),
    )
    .unwrap();
    crate::index_project_with_composition(
        crate::IndexOptions {
            root: root.to_path_buf(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        },
        &crate::test_runtime::composition(),
    )
    .await
    .unwrap()
    .snapshot
}
