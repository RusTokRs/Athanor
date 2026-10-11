//! JSON argument accessors shared by the MCP tool dispatch arms.

use anyhow::{Context, Result};
use athanor_app::SearchMode;
use serde_json::Value;

pub(super) fn string_arg(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .with_context(|| format!("missing {name}"))
}

pub(super) fn optional_string_arg(args: &Value, name: &str) -> Option<String> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

pub(super) fn search_mode_arg(args: &Value) -> SearchMode {
    match args
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("lexical")
    {
        "semantic" => SearchMode::Semantic,
        "hybrid" => SearchMode::Hybrid,
        _ => SearchMode::Lexical,
    }
}

pub(super) fn bool_arg(args: &Value, name: &str, default: bool) -> bool {
    args.get(name).and_then(Value::as_bool).unwrap_or(default)
}

pub(super) fn optional_usize_arg(args: &Value, name: &str) -> Option<usize> {
    args.get(name)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

pub(super) fn usize_arg(args: &Value, name: &str, default: usize) -> usize {
    optional_usize_arg(args, name).unwrap_or(default)
}
