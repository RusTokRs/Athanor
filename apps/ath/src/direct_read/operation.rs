use std::future::Future;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use athanor_core::{CancellationHandle, OperationContext, OperationContextCancellation};

pub(super) fn operation(
    name: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<(OperationContext, CancellationHandle)> {
    let deadline_unix_ms = match deadline_unix_ms {
        Some(deadline_unix_ms) => Some(deadline_unix_ms),
        None => match std::env::var("ATHANOR_DEADLINE_UNIX_MS") {
            Ok(value) => Some(
                value
                    .parse::<u64>()
                    .context("ATHANOR_DEADLINE_UNIX_MS must be an unsigned integer")?,
            ),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                bail!("ATHANOR_DEADLINE_UNIX_MS must contain valid Unicode")
            }
        },
    };
    let mut operation = OperationContext::new(format!("cli:{name}:{}", std::process::id()));
    if let Some(deadline_unix_ms) = deadline_unix_ms {
        operation = operation.with_deadline_unix_ms(deadline_unix_ms);
    }
    operation.check_active().map_err(anyhow::Error::new)?;
    let cancellation = operation
        .cancellation_handle()
        .map_err(anyhow::Error::new)?;
    Ok((operation, cancellation))
}

pub(super) async fn await_cli_operation<T>(
    operation: &OperationContext,
    cancellation: CancellationHandle,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    tokio::pin!(future);
    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
    let poll_interval = Duration::from_millis(25);
    loop {
        operation.check_active().map_err(anyhow::Error::new)?;
        let wait = operation
            .remaining()
            .map(|remaining| remaining.min(poll_interval))
            .unwrap_or(poll_interval);
        tokio::select! {
            biased;
            signal = &mut ctrl_c => {
                signal.context("failed to listen for CLI cancellation")?;
                cancellation.cancel();
            }
            result = &mut future => {
                let result = result?;
                operation.check_active().map_err(anyhow::Error::new)?;
                return Ok(result);
            }
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::pending;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use athanor_core::CoreError;

    use super::*;

    #[tokio::test]
    async fn expired_deadline_rejects_work_before_polling_future() {
        let operation = OperationContext::new("cli:test-expired").with_deadline_unix_ms(0);
        let cancellation = operation.cancellation_handle().unwrap();
        let started = Arc::new(AtomicBool::new(false));
        let started_in_future = Arc::clone(&started);

        let error = await_cli_operation(&operation, cancellation, async move {
            started_in_future.store(true, Ordering::Release);
            Ok::<_, anyhow::Error>(())
        })
        .await
        .expect_err("expired deadline must fail");

        assert!(!started.load(Ordering::Acquire));
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::DeadlineExceeded(_))
        )));
    }

    #[tokio::test]
    async fn cancellation_rejects_late_success() {
        let operation = OperationContext::new("cli:test-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        let cancel_from_future = cancellation.clone();

        let error = await_cli_operation(&operation, cancellation, async move {
            cancel_from_future.cancel();
            Ok::<_, anyhow::Error>(42_u8)
        })
        .await
        .expect_err("cancelled operation must not return success");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    #[tokio::test]
    async fn pending_future_is_bounded_by_operation_deadline() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let operation =
            OperationContext::new("cli:test-timeout").with_deadline_unix_ms(now.saturating_add(50));
        let cancellation = operation.cancellation_handle().unwrap();

        let error = await_cli_operation(&operation, cancellation, pending::<Result<()>>())
            .await
            .expect_err("deadline must terminate pending read");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::DeadlineExceeded(_))
        )));
    }
}
