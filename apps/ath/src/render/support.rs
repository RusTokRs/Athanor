use anyhow::Result;

pub(super) fn serialized_name(value: &impl serde::Serialize) -> Result<String> {
    Ok(serde_json::to_value(value)?
        .as_str()
        .map_or_else(|| "unknown".to_string(), str::to_string))
}
