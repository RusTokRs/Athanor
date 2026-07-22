// Public SurrealDB store boundary with stable retry classification.

use std::fs::{self, File, OpenOptions};
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fs2::FileExt;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, KnowledgeStore, OperationContext, OperationContextCancellation, RelationQuery,
    SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};

#[path = "transactional.rs"]
mod backend;

const BUSY_RETRY_DELAYS_MS: [u64; 4] = [10, 25, 50, 100];
const CANCELLATION_POLL_MS: u64 = 5;

/// SurrealDB store facade that translates confirmed transient backend failures into `CoreError::Busy`.
#[derive(Debug, Clone)]
pub struct SurrealKnowledgeStore {
    inner: backend::SurrealKnowledgeStore,
    _persistent_lease: Option<Arc<File>>,
}

impl SurrealKnowledgeStore {
    pub async fn connect(uri: &str) -> CoreResult<Self> {
        let persistent_lease = acquire_persistent_lease(uri)?;
        let inner = retry_busy(|| async {
            classify_backend_result(backend::SurrealKnowledgeStore::connect(uri).await)
        })
        .await?;
        Ok(Self {
            inner,
            _persistent_lease: persistent_lease,
        })
    }
}

fn acquire_persistent_lease(uri: &str) -> CoreResult<Option<Arc<File>>> {
    let Some(raw_path) = uri.strip_prefix("surrealkv://") else {
        return Ok(None);
    };
    if raw_path.trim().is_empty() {
        return Err(CoreError::InvalidInput(
            "persistent SurrealKV URI requires a filesystem path".to_string(),
        ));
    }

    let configured_path = PathBuf::from(raw_path);
    let absolute_path = if configured_path.is_absolute() {
        configured_path
    } else {
        std::env::current_dir()
            .map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to resolve persistent SurrealKV path: {error}"
                ))
            })?
            .join(configured_path)
    };
    let file_name = absolute_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            CoreError::InvalidInput(format!(
                "persistent SurrealKV path has no lockable directory name: {}",
                absolute_path.display()
            ))
        })?;
    let parent = absolute_path.parent().ok_or_else(|| {
        CoreError::InvalidInput(format!(
            "persistent SurrealKV path has no parent: {}",
            absolute_path.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to create persistent SurrealKV parent {}: {error}",
            parent.display()
        ))
    })?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to canonicalize persistent SurrealKV parent {}: {error}",
            parent.display()
        ))
    })?;
    let canonical_store = canonical_parent.join(file_name);
    let lock_path = canonical_parent.join(format!(".{file_name}.athanor.lock"));
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| {
            CoreError::Adapter(format!(
                "failed to open persistent SurrealKV lease {}: {error}",
                lock_path.display()
            ))
        })?;
    FileExt::try_lock_exclusive(&file).map_err(|error| {
        if is_persistent_lease_contention(&error) {
            CoreError::Busy(format!(
                "persistent SurrealKV store {} is already locked by another process",
                canonical_store.display()
            ))
        } else {
            CoreError::Adapter(format!(
                "failed to acquire persistent SurrealKV lease {}: {error}",
                lock_path.display()
            ))
        }
    })?;
    Ok(Some(Arc::new(file)))
}

fn is_persistent_lease_contention(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || (cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33)))
}

fn classify_backend_result<T>(result: CoreResult<T>) -> CoreResult<T> {
    result.map_err(classify_backend_error)
}

fn classify_backend_error(error: CoreError) -> CoreError {
    match error {
        CoreError::Adapter(message) if is_retryable_surreal_message(&message) => {
            CoreError::Busy(message)
        }
        other => other,
    }
}

fn is_retryable_surreal_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("already locked by another process")
        || message.contains("transaction write conflict")
        || message.contains("transaction retry required")
        || message.contains("transaction conflict:")
        || message.contains("read or write conflict")
}

async fn retry_busy<T, F, Fut>(mut operation: F) -> CoreResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = CoreResult<T>>,
{
    let mut retry_index = 0usize;

    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(CoreError::Busy(_)) if retry_index < BUSY_RETRY_DELAYS_MS.len() => {
                tokio::time::sleep(Duration::from_millis(BUSY_RETRY_DELAYS_MS[retry_index])).await;
                retry_index += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn sleep_with_context(context: &OperationContext, delay: Duration) -> CoreResult<()> {
    let poll = Duration::from_millis(CANCELLATION_POLL_MS);
    let mut remaining_delay = delay;

    while !remaining_delay.is_zero() {
        context.check_active()?;
        let step = remaining_delay.min(poll);
        tokio::time::sleep(step).await;
        remaining_delay = remaining_delay.saturating_sub(step);
    }

    context.check_active()
}

async fn retry_busy_with_context<T, F, Fut>(
    context: &OperationContext,
    mut operation: F,
) -> CoreResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = CoreResult<T>>,
{
    let mut retry_index = 0usize;

    loop {
        context.check_active()?;
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error @ CoreError::Busy(_)) if retry_index < BUSY_RETRY_DELAYS_MS.len() => {
                context.check_active()?;
                let delay = Duration::from_millis(BUSY_RETRY_DELAYS_MS[retry_index]);
                if context.remaining().is_some_and(|remaining| remaining <= delay) {
                    return Err(error);
                }
                sleep_with_context(context, delay).await?;
                retry_index += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

#[async_trait]
impl KnowledgeStore for SurrealKnowledgeStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        classify_backend_result(self.inner.begin_snapshot(repo, base).await)
    }

    async fn begin_snapshot_with_context(
        &self,
        repo: RepoId,
        base: SnapshotBase,
        context: &OperationContext,
    ) -> CoreResult<SnapshotId> {
        retry_busy_with_context(context, || {
            self.begin_snapshot(repo.clone(), base.clone())
        })
        .await
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        classify_backend_result(self.inner.put_entities(snapshot, entities).await)
    }

    async fn put_entities_with_context(
        &self,
        snapshot: SnapshotId,
        entities: Vec<Entity>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        retry_busy_with_context(context, || {
            self.put_entities(snapshot.clone(), entities.clone())
        })
        .await
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        classify_backend_result(self.inner.put_facts(snapshot, facts).await)
    }

    async fn put_facts_with_context(
        &self,
        snapshot: SnapshotId,
        facts: Vec<Fact>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        retry_busy_with_context(context, || {
            self.put_facts(snapshot.clone(), facts.clone())
        })
        .await
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        classify_backend_result(self.inner.put_relations(snapshot, relations).await)
    }

    async fn put_relations_with_context(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        retry_busy_with_context(context, || {
            self.put_relations(snapshot.clone(), relations.clone())
        })
        .await
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        classify_backend_result(self.inner.put_diagnostics(snapshot, diagnostics).await)
    }

    async fn put_diagnostics_with_context(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        retry_busy_with_context(context, || {
            self.put_diagnostics(snapshot.clone(), diagnostics.clone())
        })
        .await
    }

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        classify_backend_result(self.inner.put_snapshot(snapshot, batch).await)
    }

    async fn put_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        retry_busy_with_context(context, || {
            self.put_snapshot(snapshot.clone(), batch.clone())
        })
        .await
    }

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        classify_backend_result(self.inner.prepare_snapshot(snapshot).await)
    }

    async fn prepare_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        retry_busy_with_context(context, || self.prepare_snapshot(snapshot.clone())).await
    }

    async fn query_entities(
        &self,
        snapshot: SnapshotSelector,
        query: EntityQuery,
    ) -> CoreResult<Vec<Entity>> {
        classify_backend_result(self.inner.query_entities(snapshot, query).await)
    }

    async fn query_relations(
        &self,
        snapshot: SnapshotSelector,
        query: RelationQuery,
    ) -> CoreResult<Vec<Relation>> {
        classify_backend_result(self.inner.query_relations(snapshot, query).await)
    }

    async fn query_diagnostics(
        &self,
        snapshot: SnapshotSelector,
        query: DiagnosticQuery,
    ) -> CoreResult<Vec<Diagnostic>> {
        classify_backend_result(self.inner.query_diagnostics(snapshot, query).await)
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        classify_backend_result(self.inner.commit_snapshot(snapshot).await)
    }

    async fn commit_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        retry_busy_with_context(context, || self.commit_snapshot(snapshot.clone())).await
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        classify_backend_result(self.inner.abort_snapshot(snapshot).await)
    }

    async fn abort_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        retry_busy_with_context(context, || self.abort_snapshot(snapshot.clone())).await
    }
}

#[async_trait]
impl EntityResolver for SurrealKnowledgeStore {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        classify_backend_result(self.inner.resolve_stable_key(snapshot, stable_key).await)
    }
}

#[async_trait]
impl CanonicalSnapshotStore for SurrealKnowledgeStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        classify_backend_result(self.inner.load_snapshot(snapshot).await)
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        classify_backend_result(self.inner.load_latest_snapshot().await)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use athanor_core::CoreErrorCode;

    use super::*;

    fn deadline_after(milliseconds: u64) -> OperationContext {
        deadline_after_named("surreal-retry-test", milliseconds)
    }

    fn deadline_after_named(operation: &str, milliseconds: u64) -> OperationContext {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time after Unix epoch")
            .as_millis() as u64;
        OperationContext::new(operation).with_deadline_unix_ms(now + milliseconds)
    }

    #[test]
    fn classifies_persistent_lock_contention_as_retryable_busy() {
        let error = classify_backend_error(CoreError::Adapter(
            "failed to connect to surrealdb: Database at /tmp/test/LOCK is already locked by another process"
                .to_string(),
        ));

        assert_eq!(error.code(), CoreErrorCode::Busy);
        assert!(error.is_retryable());
    }

    #[test]
    fn classifies_transaction_conflicts_as_retryable_busy() {
        for message in [
            "Transaction write conflict",
            "Transaction retry required: snapshot is older than the commit oracle's GC window",
            "Transaction conflict: concurrent update",
            "Failed to commit transaction due to a read or write conflict. This transaction can be retried",
        ] {
            let error = classify_backend_error(CoreError::Adapter(message.to_string()));
            assert_eq!(error.code(), CoreErrorCode::Busy);
            assert!(error.is_retryable());
        }
    }

    #[test]
    fn leaves_data_and_statement_failures_non_retryable() {
        let error = classify_backend_error(CoreError::Adapter(
            "snapshot batch transaction rolled back: duplicate record id".to_string(),
        ));

        assert_eq!(error.code(), CoreErrorCode::AdapterExecution);
        assert!(!error.is_retryable());
    }

    #[tokio::test]
    async fn retries_busy_connect_initialization_with_bounded_backoff() {
        let attempts = AtomicUsize::new(0);

        let value = retry_busy(|| {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            async move {
                if attempt < 2 {
                    Err(CoreError::Busy("counter initialization conflict".to_string()))
                } else {
                    Ok("connected")
                }
            }
        })
        .await
        .expect("retryable connection initialization should eventually succeed");

        assert_eq!(value, "connected");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retries_busy_errors_with_bounded_backoff() {
        let attempts = AtomicUsize::new(0);
        let context = deadline_after(2_000);

        let value = retry_busy_with_context(&context, || {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            async move {
                if attempt < 2 {
                    Err(CoreError::Busy("transient conflict".to_string()))
                } else {
                    Ok("committed")
                }
            }
        })
        .await
        .expect("retryable operation should eventually succeed");

        assert_eq!(value, "committed");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn does_not_retry_non_retryable_errors() {
        let attempts = AtomicUsize::new(0);
        let context = deadline_after(2_000);

        let error = retry_busy_with_context(&context, || {
            attempts.fetch_add(1, Ordering::SeqCst);
            async { Err::<(), _>(CoreError::Adapter("invalid statement".to_string())) }
        })
        .await
        .expect_err("non-retryable operation must fail immediately");

        assert_eq!(error.code(), CoreErrorCode::AdapterExecution);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancellation_stops_retry_before_backoff() {
        let attempts = AtomicUsize::new(0);
        let context = deadline_after_named("surreal-retry-cancelled", 2_000);
        let cancellation = context
            .cancellation_handle()
            .expect("create cancellation handle");

        let error = retry_busy_with_context(&context, || {
            attempts.fetch_add(1, Ordering::SeqCst);
            cancellation.cancel();
            async { Err::<(), _>(CoreError::Busy("transient conflict".to_string())) }
        })
        .await
        .expect_err("cancelled retry must stop before sleeping");

        assert_eq!(error.code(), CoreErrorCode::Cancelled);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
