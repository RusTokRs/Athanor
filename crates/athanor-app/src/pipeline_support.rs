use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use athanor_core::{CoreError, CoreResult, OperationContext, OperationContextCancellation};
use athanor_domain::SnapshotId;
use serde_json::Value;

use crate::CancellationToken;

pub(crate) fn unwrap_or_clone_arc_vec<T: Clone>(items: Arc<Vec<T>>) -> Vec<T> {
    Arc::try_unwrap(items).unwrap_or_else(|items| (*items).clone())
}

pub(crate) fn check_cancelled(cancellation: &Option<CancellationToken>) -> Result<()> {
    if let Some(cancellation) = cancellation {
        cancellation.check()?;
    }
    Ok(())
}

/// Executes one pre-commit adapter or Store boundary under shared cancellation and deadline rules.
///
/// Durable commit boundaries must not use this helper: they deliberately retain success once the
/// backend commit has happened, even when cancellation races with the return path.
pub(crate) async fn within_operation_deadline<T>(
    operation: &OperationContext,
    adapter: &str,
    future: impl Future<Output = CoreResult<T>>,
) -> CoreResult<T> {
    operation.check_active()?;
    let result = match operation.remaining() {
        Some(remaining) => tokio::time::timeout(remaining, future).await.map_err(|_| {
            let operation_id = operation.operation_id.as_deref().unwrap_or("operation");
            CoreError::DeadlineExceeded(format!(
                "{operation_id} exceeded its deadline while running {adapter}"
            ))
        })?,
        None => future.await,
    }?;
    operation.check_active()?;
    Ok(result)
}

pub(crate) fn replace_payload_snapshot(value: &mut Value, snapshot: &SnapshotId) {
    if let Value::Object(object) = value
        && object.contains_key("snapshot")
    {
        object.insert("snapshot".to_string(), Value::String(snapshot.0.clone()));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[tokio::test]
    async fn pre_cancelled_operation_does_not_poll_boundary_future() {
        let operation = OperationContext::new("pipeline-pre-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        cancellation.cancel();
        let polled = Arc::new(AtomicBool::new(false));
        let polled_by_future = Arc::clone(&polled);

        let error = within_operation_deadline(&operation, "test.boundary", async move {
            polled_by_future.store(true, Ordering::Release);
            Ok(())
        })
        .await
        .expect_err("pre-cancelled operation must reject boundary work");

        assert!(matches!(error, CoreError::Cancelled(_)));
        assert!(!polled.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn cancellation_during_pre_commit_boundary_rejects_success() {
        let operation = OperationContext::new("pipeline-mid-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        let cancellation_by_future = cancellation.clone();

        let error = within_operation_deadline(&operation, "test.boundary", async move {
            cancellation_by_future.cancel();
            Ok(42_u8)
        })
        .await
        .expect_err("cancellation before durable commit must replace boundary success");

        assert!(cancellation.is_cancelled());
        assert!(matches!(error, CoreError::Cancelled(_)));
    }
}
