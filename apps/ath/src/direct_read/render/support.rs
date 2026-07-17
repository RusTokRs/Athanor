use anyhow::Result;
use athanor_app::NamedCount;

pub(super) fn print_named_counts(title: &str, counts: &[NamedCount]) {
    println!("{title}:");
    if counts.is_empty() {
        println!("  (none)");
    } else {
        for item in counts {
            println!("  - {}: {}", item.name, item.count);
        }
    }
}

pub(super) fn serialized_name(value: &impl serde::Serialize) -> Result<String> {
    Ok(serde_json::to_value(value)?
        .as_str()
        .map_or_else(|| "unknown".to_string(), str::to_string))
}
