//! File-qualified Rust symbol identities for file-local extraction.
//!
//! A Rust extractor cannot know the Cargo target or module source root from
//! one SourceFile alone. A file-qualified key prevents canonical coalescing;
//! target/module reachability must be proven separately from snapshot evidence.

use athanor_domain::{Entity, EntityKind};

pub const RUST_FILE_IDENTITY_VERSION: u64 = 1;

/// The byte length makes the file/qualified-name boundary unambiguous even
/// when a valid source path itself contains `:`, `#`, or Rust separators.
pub fn rust_file_symbol_key(source_file: &str, qualified_name: &str) -> String {
    format!(
        "symbol://rust:file:{}:{source_file}:{qualified_name}",
        source_file.len()
    )
}

/// Accept only a complete file-backed identity, never a legacy global Rust key
/// or a forged path marker. This is necessary but not sufficient for Cargo
/// package/target/module ownership.
pub fn rust_file_symbol_identity(entity: &Entity) -> Option<(&str, &str)> {
    if entity.language.as_ref()?.0 != "rust"
        || !matches!(
            &entity.kind,
            EntityKind::Symbol | EntityKind::Function | EntityKind::TestCase | EntityKind::Module
        )
        || entity
            .payload
            .get("symbol_kind")?
            .as_str()
            .is_none_or(str::is_empty)
        || entity.payload.get("rust_identity_version")?.as_u64()? != RUST_FILE_IDENTITY_VERSION
    {
        return None;
    }
    let path = entity.source.as_ref()?.path.as_str();
    let qualified_name = entity.payload.get("qualified_name")?.as_str()?;
    if entity.payload.get("source_file")?.as_str()? != path
        || entity.ownership.len() != 1
        || entity.ownership[0].source_file != path
        || entity.stable_key.0 != rust_file_symbol_key(path, qualified_name)
    {
        return None;
    }
    Some((path, qualified_name))
}

#[cfg(test)]
mod tests {
    use athanor_domain::{
        Entity, EntityId, EntityKind, LanguageCode, Ownership, SourceLocation, StableKey,
    };
    use serde_json::json;

    use super::*;

    fn symbol(path: &str, qualified_name: &str) -> Entity {
        Entity {
            id: EntityId("symbol".to_string()),
            stable_key: StableKey(rust_file_symbol_key(path, qualified_name)),
            kind: EntityKind::Function,
            name: "name".to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("rust".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload: json!({
                "symbol_kind": "function",
                "rust_identity_version": 1,
                "source_file": path,
                "qualified_name": qualified_name,
            }),
        }
    }

    #[test]
    fn file_key_is_unambiguous_and_validated_against_provenance() {
        let path = "crates/a#b:src/lib.rs";
        let entity = symbol(path, "crate::run");
        assert_eq!(
            rust_file_symbol_identity(&entity),
            Some((path, "crate::run"))
        );
        assert_ne!(
            entity.stable_key.0,
            rust_file_symbol_key("crates/a", "b:src/lib.rs:crate::run")
        );
        let mut forged = entity;
        forged.ownership[0].source_file = "crates/other/src/lib.rs".to_string();
        assert!(rust_file_symbol_identity(&forged).is_none());
        let mut forged_kind = symbol(path, "crate::run");
        forged_kind.kind = EntityKind::File;
        assert!(rust_file_symbol_identity(&forged_kind).is_none());
    }
}
