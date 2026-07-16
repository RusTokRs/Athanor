use std::future::Future;

use anyhow::{Context, Result, bail};
use athanor_core::{CancellationHandle, OperationContext, OperationContextCancellation};

pub(crate) fn operation(
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

/// Cancels on Ctrl-C but keeps polling the operation-aware future until it drains its worker.
pub(crate) async fn await_drained_operation<T>(
    cancellation: CancellationHandle,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    tokio::pin!(future);
    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
    let mut cancellation_requested = false;
    loop {
        tokio::select! {
            biased;
            signal = &mut ctrl_c, if !cancellation_requested => {
                signal.context("failed to listen for CLI cancellation")?;
                cancellation.cancel();
                cancellation_requested = true;
            }
            result = &mut future => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;

    use athanor_core::{CoreError, OperationContextCancellation};

    use super::*;

    #[tokio::test]
    async fn drained_operation_waits_for_worker_after_cancellation() {
        let operation = OperationContext::new("direct-operation-drain");
        let cancellation = operation.cancellation_handle().unwrap();
        let cancel_from_future = cancellation.clone();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_in_future = Arc::clone(&completed);

        let error = await_drained_operation(cancellation, async move {
            cancel_from_future.cancel();
            tokio::time::sleep(Duration::from_millis(10)).await;
            completed_in_future.store(true, Ordering::Release);
            operation.check_active().map_err(anyhow::Error::new)?;
            Ok::<_, anyhow::Error>(())
        })
        .await
        .expect_err("cancelled operation must fail after drain");

        assert!(completed.load(Ordering::Acquire));
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }
}
