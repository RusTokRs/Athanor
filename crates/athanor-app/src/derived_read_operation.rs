use std::future::Future;

use anyhow::Result;
#[cfg(not(test))]
use anyhow::bail;
use athanor_core::{OperationContext, OperationContextCancellation};

use crate::context_operation::context_project_with_operation_context_impl;
use crate::{
    ChangeMapOptions, ChangeMapReport, ContextOptions, ContextReport, RuntimeComposition,
    change_map_project, change_map_project_with_composition,
};

/// Temporary compatibility edge for callers not yet migrated to explicit composition.
///
/// Production callers fail explicitly. Unit tests use a fresh local composition until daemon and
/// legacy operation callers are migrated.
pub async fn context_project_with_operation_context(
    options: ContextOptions,
    operation: &OperationContext,
) -> Result<ContextReport> {
    #[cfg(test)]
    {
        let composition = crate::test_runtime::composition();
        return context_project_with_composition_and_operation_context(
            options,
            &composition,
            operation,
        )
        .await;
    }

    #[cfg(not(test))]
    {
        let _ = (options, operation);
        bail!("explicit RuntimeComposition is required for operation-aware context generation")
    }
}

/// Builds a diff-aware context report with explicit runtime dependencies and operation metadata.
pub async fn context_project_with_composition_and_operation_context(
    options: ContextOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<ContextReport> {
    context_project_with_operation_context_impl(options, composition, operation)
        .await
        .map(ContextReport::from)
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
        let cancellation_in_future = cancellation.clone();

        let error = run_with_operation_context(&operation, async move {
            cancellation_in_future.cancel();
            Ok::<_, anyhow::Error>(42_u8)
        })
        .await
        .expect_err("postflight cancellation must reject legacy success");

        assert!(cancellation.is_cancelled());
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
