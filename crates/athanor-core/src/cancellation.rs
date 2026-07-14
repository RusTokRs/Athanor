//! Transport-neutral cancellation attached to `OperationContext` without changing its wire shape.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::ports::{CoreError, CoreResult, OperationContext};

static CANCELLATIONS: OnceLock<Mutex<HashMap<String, Weak<AtomicBool>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, Weak<AtomicBool>>> {
    CANCELLATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Cloneable cancellation authority for one active operation identity.
///
/// The handle is process-local and is intentionally not serialized with `OperationContext`.
/// Transports keep the existing operation-id/deadline wire contract. One operation id may own only
/// one live cancellation authority at a time; callers that need another clone must clone the
/// returned handle instead of registering the id again.
#[derive(Debug, Clone)]
pub struct CancellationHandle {
    operation_id: String,
    state: Arc<AtomicBool>,
}

impl CancellationHandle {
    /// Registers the cancellation authority for a context.
    ///
    /// A second live registration for the same operation id is rejected. This fail-closed lease
    /// prevents independent contexts from being silently merged into one global cancellation state.
    pub fn for_context(context: &OperationContext) -> CoreResult<Self> {
        let operation_id = context
            .operation_id
            .clone()
            .filter(|operation_id| !operation_id.trim().is_empty())
            .ok_or_else(|| {
                CoreError::InvalidInput(
                    "cancellable operation context requires a non-empty operation_id".to_string(),
                )
            })?;

        let mut cancellations = registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cancellations
            .get(&operation_id)
            .and_then(Weak::upgrade)
            .is_some()
        {
            return Err(CoreError::Conflict(format!(
                "cancellation identity `{operation_id}` is already active"
            )));
        }
        cancellations.remove(&operation_id);

        let state = Arc::new(AtomicBool::new(false));
        cancellations.insert(operation_id.clone(), Arc::downgrade(&state));

        Ok(Self {
            operation_id,
            state,
        })
    }

    pub fn cancel(&self) {
        self.state.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::Acquire)
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
}

/// Cancellation helpers for the existing serializable `OperationContext` contract.
pub trait OperationContextCancellation {
    fn cancellation_handle(&self) -> CoreResult<CancellationHandle>;
    fn is_cancelled(&self) -> bool;
    fn check_cancelled(&self) -> CoreResult<()>;
    fn check_active(&self) -> CoreResult<()>;
}

impl OperationContextCancellation for OperationContext {
    fn cancellation_handle(&self) -> CoreResult<CancellationHandle> {
        CancellationHandle::for_context(self)
    }

    fn is_cancelled(&self) -> bool {
        let Some(operation_id) = self.operation_id.as_deref() else {
            return false;
        };
        let mut cancellations = registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match cancellations.get(operation_id).and_then(Weak::upgrade) {
            Some(state) => state.load(Ordering::Acquire),
            None => {
                cancellations.remove(operation_id);
                false
            }
        }
    }

    fn check_cancelled(&self) -> CoreResult<()> {
        if self.is_cancelled() {
            let operation = self.operation_id.as_deref().unwrap_or("operation");
            return Err(CoreError::Cancelled(format!(
                "{operation} was cancelled"
            )));
        }
        Ok(())
    }

    fn check_active(&self) -> CoreResult<()> {
        self.check_cancelled()?;
        self.check_deadline()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloned_handle_cancels_one_operation_context() {
        let context = OperationContext::new("core-cancellation-shared");
        let first = context.cancellation_handle().expect("create handle");
        let second = first.clone();

        assert_eq!(first.operation_id(), "core-cancellation-shared");
        assert!(!context.is_cancelled());
        second.cancel();

        assert!(first.is_cancelled());
        assert!(context.is_cancelled());
        let error = context
            .check_active()
            .expect_err("cancelled context must reject new work");
        assert!(matches!(error, CoreError::Cancelled(_)));
        assert!(!error.is_retryable());
    }

    #[test]
    fn duplicate_live_operation_identity_is_rejected() {
        let first_context = OperationContext::new("core-cancellation-exclusive");
        let _first = first_context
            .cancellation_handle()
            .expect("register first handle");
        let second_context = OperationContext::new("core-cancellation-exclusive");

        let error = second_context
            .cancellation_handle()
            .expect_err("duplicate active identity must be rejected");

        assert!(matches!(error, CoreError::Conflict(_)));
        assert!(error.to_string().contains("already active"));
    }

    #[test]
    fn operation_identity_can_be_reused_after_handle_drop() {
        let first_context = OperationContext::new("core-cancellation-reuse");
        let first = first_context
            .cancellation_handle()
            .expect("register first handle");
        drop(first);

        let second_context = OperationContext::new("core-cancellation-reuse");
        let second = second_context
            .cancellation_handle()
            .expect("reuse released operation identity");

        assert!(!second.is_cancelled());
        assert!(!second_context.is_cancelled());
    }

    #[test]
    fn cancellation_state_is_not_serialized() {
        let context = OperationContext::new("core-cancellation-wire").with_deadline_unix_ms(42);
        context
            .cancellation_handle()
            .expect("create cancellation handle")
            .cancel();

        let value = serde_json::to_value(&context).expect("serialize operation context");
        assert_eq!(value["operation_id"], "core-cancellation-wire");
        assert_eq!(value["deadline_unix_ms"], 42);
        assert_eq!(value.as_object().expect("context object").len(), 2);
    }

    #[test]
    fn cancellation_requires_operation_identity() {
        for context in [OperationContext::default(), OperationContext::new("   ")] {
            let error = context
                .cancellation_handle()
                .expect_err("anonymous context cannot own a cancellation handle");
            assert!(matches!(error, CoreError::InvalidInput(_)));
        }
    }
}
