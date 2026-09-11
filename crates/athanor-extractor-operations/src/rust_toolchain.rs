use athanor_core::ExtractInput;
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use serde_json::json;

#[derive(Debug, Clone, Default)]
struct RustToolchainConfig {
    channel: Option<String>,
    profile: Option<String>,
    components: Vec<String>,
    targets: Vec<String>,
}

pub(super) fn is_rust_toolchain_path(path: &str) -> bool {
    path.replace('\\', "/").eq_ignore_ascii_case("rust-toolchain.toml")
}

pub(super) fn extract_rust_toolchain(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let Ok(root) = toml::from_str::<toml::Value>(content) else {
        return;
    };
    let Some(toolchain) = root.get("toolchain").and_then(toml::Value::as_table) else {
        return;
    };

    let config = RustToolchainConfig {
        channel: toolchain
            .get("channel")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        profile: toolchain
            .get("profile")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        components: string_list(toolchain.get("components")),
        targets: string_list(toolchain.get("targets")),
    };

    if config.channel.is_none()
        && config.profile.is_none()
        && config.components.is_empty()
        && config.targets.is_empty()
    {
        return;
    }

    let ownership = ownership_for_file(&input.source.path);
    let source_line = toolchain_key_line(content).unwrap_or(1);
    let stable_key = StableKey(format!("config://{}#rust-toolchain", input.source.path));
    let entity_id = EntityId(format!(
        "ent_feature_{:016x}",
        stable_hash(stable_key.0.as_bytes())
    ));

    entities.push(Entity {
        id: entity_id.clone(),
        stable_key: stable_key.clone(),
        kind: EntityKind::Feature,
        name: "rust-toolchain".to_string(),
        title: Some("Rust toolchain configuration".to_string()),
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(source_line),
            line_end: Some(source_line),
        }),
        language: Some(LanguageCode("toml".to_string())),
        aliases: Vec::new(),
        ownership: ownership.clone(),
        payload: json!({
            "feature_kind": "rust_toolchain",
            "channel": config.channel,
            "profile": config.profile,
            "components": config.components,
            "targets": config.targets,
        }),
    });

    facts.push(Fact {
        id: FactId(format!(
            "fact_rust_toolchain_defined_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        )),
        kind: FactKind::SymbolDefined,
        subject: entity_id,
        object: Some(file_id.clone()),
        value: json!({
            "stable_key": stable_key.0,
            "path": input.source.path,
            "source_kind": "rust_toolchain",
            "channel": config.channel,
            "profile": config.profile,
            "component_count": config.components.len(),
            "target_count": config.targets.len(),
        }),
        evidence: vec![evidence_for_file(
            &input.source.path,
            extractor,
            Some(source_line),
            Some(source_line),
        )],
        ownership,
        snapshot: input.snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    });
}

fn string_list(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn toolchain_key_line(content: &str) -> Option<u32> {
    content.lines().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim_start();
        (trimmed == "[toolchain]").then_some(index as u32 + 1)
    })
}
