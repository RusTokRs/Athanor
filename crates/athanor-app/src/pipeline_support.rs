use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use athanor_core::{CoreError, CoreResult, OperationContext};
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

/// Executes one adapter boundary under the shared operation deadline.
pub(crate) async fn within_operation_deadline<T>(
    operation: &OperationContext,
    adapter: &str,
    future: impl Future<Output = CoreResult<T>>,
) -> CoreResult<T> {
    operation.check_deadline()?;
    match operation.remaining() {
        Some(remaining) => tokio::time::timeout(remaining, future).await.map_err(|_| {
            let operation_id = operation.operation_id.as_deref().unwrap_or("operation");
            CoreError::DeadlineExceeded(format!(
                "{operation_id} exceeded its deadline while running {adapter}"
            ))
        })?,
        None => future.await,
    }
}

pub(crate) fn replace_payload_snapshot(value: &mut Value, snapshot: &SnapshotId) {
    if let Value::Object(object) = value
        && object.contains_key("snapshot")
    {
        object.insert("snapshot".to_string(), Value::String(snapshot.0.clone()));
    }
}
