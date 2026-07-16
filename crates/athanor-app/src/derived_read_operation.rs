use std::future::Future;

use anyhow::Result;
use athanor_core::{OperationContext, OperationContextCancellation};
use athanor_domain::ContextPack;

use crate::{
    ChangeMapOptions, ChangeMapReport, ContextOptions, RuntimeComposition, change_map_project,
    change_map_project_with_composition, context_project, context_project_with_composition,
};

/// Builds a diff-aware context pack under the shared cancellation/deadline contract.
pub async fn context_project_with_operation_context(
    options: ContextOptions,
    operation: &OperationContext,
) -> Result<ContextPack> {
    run_with_operation_context(operation, context_project(options)).await
}

/// Builds a diff-aware context pack with explicit runtime dependencies and operation metadata.
pub async fn context_project_with_composition_and_operation_context(
    options: ContextOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<ContextPack> {
    run_with_operation_context(
        operation,
        context_project_with_composition(options, composition),
    )
    .await
}

/// Builds a change map under the shared cancellation/deadline contract.
pub async fn change_map_project_with_operation_context(
    options: ChangeMapOptions,
    operation: &OperationContext,
) -> Result<ChangeMapReport> {
    run_with_operation_context(operation, change_map_project(options)).await
}

/// Builds a change map with explicit runtime dependencies and operation metadata.
pub async fn change_map_project_with_composition_and_operation_context(
    options: ChangeMapOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<ChangeMapReport> {
    run_with_operation_context(
        operation,
        change_map_project_with_composition(options, composition),
    )
    .await
}

async fn run_with_operation_context<T>(
    operation: &OperationContext,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    operation.check_active().map_err(anyhow::Error::new)?;
    let result = future.await?;
    operation.check_active().map_err(anyhow::Error::new)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use athanor_core::CoreError;

    use super::*;

    #[tokio::test]
    async fn pre_cancelled_derived_read_does_not_start_work() {
        let operation = OperationContext::new("derived-read-pre-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        cancellation.cancel();
        let started = Arc::new(AtomicBool::new(false));
        let started_in_future = Arc::clone(&started);

        let error = run_with_operation_context(&operation, async move {
            started_in_future.store(true, Ordering::Release);
            Ok::<_, anyhow::Error>(())
        })
        .await
        .expect_err("cancelled operation must fail before derived work");

        assert!(!started.load(Ordering::Acquire));
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    #[tokio::test]
    async fn cancellation_after_legacy_work_rejects_false_success() {
        let operation = OperationContext::new("derived-read-post-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();

        let error = run_with_operation_context(&operation, async move {
            cancellation.cancel();
            Ok::<_, anyhow::Error>(42_u8)
        })
        .await
        .expect_err("postflight cancellation must reject legacy success");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    #[tokio::test]
    async fn expired_deadline_does_not_start_derived_work() {
        let operation = OperationContext::new("derived-read-expired").with_deadline_unix_ms(0);
        let started = Arc::new(AtomicBool::new(false));
        let started_in_future = Arc::clone(&started);

        let error = run_with_operation_context(&operation, async move {
            started_in_future.store(true, Ordering::Release);
            Ok::<_, anyhow::Error>(())
        })
        .await
        .expect_err("expired operation must fail before derived work");

        assert!(!started.load(Ordering::Acquire));
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::DeadlineExceeded(_))
        )));
    }
}
