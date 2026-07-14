use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Result, bail};
use athanor_core::{CancellationHandle, OperationContext, OperationContextCancellation};

#[derive(Debug, Default)]
struct CancellationState {
    cancelled: AtomicBool,
    operation: Mutex<Option<CancellationHandle>>,
}

/// Application cancellation token shared by daemon registries and running job clones.
///
/// A token can be bound to one transport-neutral `OperationContext`. Once bound, cancelling any
/// clone also cancels the core handle observed by context-aware stores and adapters.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    state: Arc<CancellationState>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.state.cancelled.store(true, Ordering::Release);
        if let Some(operation) = self
            .state
            .operation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            operation.cancel();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    /// Binds every clone of this token to one core operation cancellation handle.
    ///
    /// Rebinding to a different operation id is rejected because one daemon job must not control
    /// unrelated work. Cancellation requested before binding is propagated immediately.
    pub fn bind_operation(&self, operation: &OperationContext) -> Result<()> {
        let handle = operation
            .cancellation_handle()
            .map_err(anyhow::Error::new)?;
        let mut bound = self
            .state
            .operation
            .lock()
            .map_err(|_| anyhow::anyhow!("application cancellation binding lock is poisoned"))?;

        if let Some(existing) = bound.as_ref()
            && existing.operation_id() != handle.operation_id()
        {
            bail!(
                "cancellation token is already bound to operation `{}`, not `{}`",
                existing.operation_id(),
                handle.operation_id()
            );
        }

        if self.is_cancelled() {
            handle.cancel();
        }
        *bound = Some(handle);
        Ok(())
    }

    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            bail!("operation cancelled");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_shared_across_clones() {
        let token = CancellationToken::new();
        let clone = token.clone();

        clone.cancel();

        assert!(token.is_cancelled());
        assert_eq!(
            token.check().unwrap_err().to_string(),
            "operation cancelled"
        );
    }

    #[test]
    fn cancellation_propagates_to_bound_operation_context() {
        let token = CancellationToken::new();
        let registry_clone = token.clone();
        let operation = OperationContext::new("daemon.index.request-42");
        token.bind_operation(&operation).unwrap();

        registry_clone.cancel();

        assert!(operation.is_cancelled());
        assert!(matches!(
            operation.check_active(),
            Err(athanor_core::CoreError::Cancelled(_))
        ));
    }

    #[test]
    fn cancellation_requested_before_binding_is_propagated() {
        let token = CancellationToken::new();
        token.cancel();
        let operation = OperationContext::new("daemon.generate.request-42");

        token.bind_operation(&operation).unwrap();

        assert!(operation.is_cancelled());
    }

    #[test]
    fn one_token_cannot_control_two_operations() {
        let token = CancellationToken::new();
        token
            .bind_operation(&OperationContext::new("daemon.index.request-1"))
            .unwrap();

        let error = token
            .bind_operation(&OperationContext::new("daemon.index.request-2"))
            .expect_err("rebinding must fail");
        assert!(error.to_string().contains("already bound"));
    }
}
