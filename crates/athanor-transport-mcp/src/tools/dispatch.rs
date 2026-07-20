use std::future::Future;
use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_app::{
    ChangeMapOptions, ContextLimitOverrides, ContextOptions, DiagnosticCheckOptions,
    DiagnosticScope, ExplainOptions, ImpactOptions, IndexOptions, RuntimeComposition,
    RustokArchitectureContextOptions, SearchOptions,
};
use athanor_core::{CoreError, CoreErrorCode, OperationContext, OperationContextCancellation};
use serde_json::{Value, json};

pub(crate) fn call<'a>(
    root: &'a Path,
    name: &'a str,
    args: Value,
    composition: &'a RuntimeComposition,
    operation: &OperationContext,
) -> impl Future<Output = Result<String>> + Send + 'a {
    let operation = operation.clone();
    async move { call_inner(root, name, args, composition, &operation).await }
}

async fn call_inner(
    root: &Path,
    name: &str,
    args: Value,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<String> {
    operation.check_active().map_err(anyhow::Error::new)?;
    let result = match name {
        "index" => {
            let report = athanor_app::index_project_with_composition_and_operation_context(
                IndexOptions {
                    root: root.to_path_buf(),
                    validation_report: None,
                    validation_result: None,
                    validate_only: bool_arg(&args, "validate_only", false),
                },
                composition,
                operation.clone(),
            )
            .await?;
            // Index publication has a durable commit boundary. Once the application returns a report,
            // a transport-level cancellation that races after commit must not replace that success.
            return Ok(serde_json::to_string_pretty(&report)?);
        }
        "explain" => {
            let report = athanor_app::explain_project_with_composition(
                ExplainOptions {
                    root: root.to_path_buf(),
                    stable_key: string_arg(&args, "stable_key")?,
                },
                composition,
            )
            .await?;
            serde_json::to_string_pretty(&report)?
        }
        "search" => {
            let report = athanor_app::search_project_with_composition_and_operation_context(
                SearchOptions {
                    root: root.to_path_buf(),
                    query: string_arg(&args, "query")?,
                    limit: usize_arg(&args, "limit", 10),
                },
                composition,
                operation,
            )
            .await?;
            serde_json::to_string_pretty(&report)?
        }
        "context" => {
            let level = match args
                .get("level")
                .and_then(Value::as_str)
                .unwrap_or("normal")
            {
                "summary" => athanor_domain::ContextLevel::Summary,
                "deep" => athanor_domain::ContextLevel::Deep,
                "full" => athanor_domain::ContextLevel::Full,
                _ => athanor_domain::ContextLevel::Normal,
            };
            let report = athanor_app::context_project_with_composition_and_operation_context(
                ContextOptions {
                    root: root.to_path_buf(),
                    task: string_arg(&args, "task")?,
                    diff: false,
                    level,
                    limits: ContextLimitOverrides {
                        max_tokens: optional_usize_arg(&args, "max_tokens"),
                        max_files: optional_usize_arg(&args, "max_files"),
                        max_entities: optional_usize_arg(&args, "max_entities"),
                        max_diagnostics: optional_usize_arg(&args, "max_diagnostics"),
                        max_depth: optional_usize_arg(&args, "max_depth"),
                    },
                },
                composition,
                operation,
            )
            .await?;
            serde_json::to_string_pretty(&report)?
        }
        "impact" => {
            let report = athanor_app::impact_project_with_composition(
                ImpactOptions {
                    root: root.to_path_buf(),
                    target: optional_string_arg(&args, "target"),
                    diff: bool_arg(&args, "diff", false),
                    max_depth: usize_arg(&args, "max_depth", 10),
                },
                composition,
            )
            .await?;
            serde_json::to_string_pretty(&report)?
        }
        "change_map" => {
            let report = athanor_app::change_map_project_with_composition_and_operation_context(
                ChangeMapOptions {
                    root: root.to_path_buf(),
                    task: optional_string_arg(&args, "task"),
                    target: optional_string_arg(&args, "target"),
                    diff: bool_arg(&args, "diff", false),
                    max_entities: usize_arg(&args, "max_entities", 30),
                    max_files: usize_arg(&args, "max_files", 20),
                    max_diagnostics: usize_arg(&args, "max_diagnostics", 20),
                    max_depth: usize_arg(&args, "max_depth", 3),
                },
                composition,
                operation,
            )
            .await?;
            serde_json::to_string_pretty(&report)?
        }
        "rustok_architecture_context" => {
            let mut options = RustokArchitectureContextOptions::bounded(
                root.to_path_buf(),
                string_arg(&args, "intent")?,
                optional_string_arg(&args, "module"),
            );
            options.max_modules = usize_arg(&args, "max_modules", 6);
            options.max_contracts = usize_arg(&args, "max_contracts", 16);
            options.max_interactions = usize_arg(&args, "max_interactions", 16);
            options.max_evidence = usize_arg(&args, "max_evidence", 20);
            let report =
                athanor_app::rustok_architecture_context_with_composition_and_operation_context(
                    options,
                    composition,
                    operation,
                )
                .await?;
            serde_json::to_string_pretty(&report)?
        }
        "check" => {
            let scope = match args
                .get("scope")
                .and_then(Value::as_str)
                .context("missing scope")?
            {
                "docs" => DiagnosticScope::Docs,
                "env" => DiagnosticScope::Env,
                "scripts" => DiagnosticScope::Scripts,
                "deployment" => DiagnosticScope::Deployment,
                "runbooks" => DiagnosticScope::Runbooks,
                _ => DiagnosticScope::Api,
            };
            let report = athanor_app::check_project_with_composition(
                DiagnosticCheckOptions {
                    root: root.to_path_buf(),
                    scope,
                },
                composition,
            )
            .await?;
            serde_json::to_string_pretty(&report)?
        }
        other => bail!("unknown MCP tool `{other}`"),
    };
    operation.check_active().map_err(anyhow::Error::new)?;
    Ok(result)
}

pub(crate) fn rpc_error(error: &anyhow::Error) -> Value {
    let core_error = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<CoreError>());
    let (stable_code, retryable) = match core_error {
        Some(error) => (core_error_code_name(error.code()), error.is_retryable()),
        None if error.to_string().contains("deadline") => ("deadline_exceeded", true),
        None if error.to_string().contains("unknown MCP tool")
            || error.to_string().contains("must be")
            || error.to_string().contains("missing") =>
        {
            ("invalid_input", false)
        }
        None => ("internal", false),
    };
    let message = if stable_code == "internal" {
        "internal MCP tool error".to_string()
    } else {
        error.to_string()
    };
    json!({
        "code": -32603,
        "message": message,
        "data": {
            "code": stable_code,
            "retryable": retryable
        }
    })
}

fn string_arg(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .with_context(|| format!("missing {name}"))
}

fn optional_string_arg(args: &Value, name: &str) -> Option<String> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn bool_arg(args: &Value, name: &str, default: bool) -> bool {
    args.get(name).and_then(Value::as_bool).unwrap_or(default)
}

fn optional_usize_arg(args: &Value, name: &str) -> Option<usize> {
    args.get(name)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn usize_arg(args: &Value, name: &str, default: usize) -> usize {
    optional_usize_arg(args, name).unwrap_or(default)
}

fn core_error_code_name(code: CoreErrorCode) -> &'static str {
    match code {
        CoreErrorCode::NotFound => "not_found",
        CoreErrorCode::InvalidInput => "invalid_input",
        CoreErrorCode::AdapterProtocol => "adapter_protocol",
        CoreErrorCode::AdapterExecution => "adapter_execution",
        CoreErrorCode::SnapshotNotCommitted => "snapshot_not_committed",
        CoreErrorCode::Conflict => "conflict",
        CoreErrorCode::Busy => "busy",
        CoreErrorCode::Cancelled => "cancelled",
        CoreErrorCode::DeadlineExceeded => "deadline_exceeded",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_errors_are_redacted() {
        let error = anyhow::anyhow!("failed to read C:/private/token");
        let rpc = rpc_error(&error);
        assert_eq!(rpc["message"], "internal MCP tool error");
        assert_eq!(rpc["data"]["code"], "internal");
    }
}
