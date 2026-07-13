//! Transport-neutral operation metadata for daemon jobs.

use athanor_core::OperationContext;

pub(crate) fn context(
    command: &str,
    request_id: &str,
    deadline_unix_ms: Option<u64>,
) -> OperationContext {
    let context = OperationContext::new(format!("daemon.{command}.{request_id}"));
    match deadline_unix_ms {
        Some(deadline) => context.with_deadline_unix_ms(deadline),
        None => context,
    }
}

#[cfg(test)]
mod tests {
    use super::context;

    #[test]
    fn preserves_command_and_request_identity_with_optional_deadline() {
        let context = context("html_report", "request-42", Some(u64::MAX));

        assert_eq!(
            context.operation_id.as_deref(),
            Some("daemon.html_report.request-42")
        );
        assert_eq!(context.deadline_unix_ms, Some(u64::MAX));
    }
}
