//! Shared JavaScript/TypeScript module path resolution.
//!
//! The import linker and the call linker resolve module specifiers the same way:
//! relative specifiers stay inside the repository, extensionless and directory
//! specifiers try the standard JavaScript/TypeScript file extensions, and bare
//! package specifiers stay unresolved because they leave the repository.

use std::collections::HashMap;

use athanor_domain::Entity;

pub(crate) const EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];

/// Repository-relative, forward-slash module path for one entity source.
pub(crate) fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

/// Resolves a relative module specifier from `source` to a known module entity.
pub(crate) fn resolve_local_import<'a>(
    source: &Entity,
    specifier: &str,
    modules: &HashMap<String, &'a Entity>,
) -> Option<&'a Entity> {
    if !specifier.starts_with('.') {
        return None;
    }
    let source_path = normalize_path(&source.source.as_ref()?.path);
    let parent = source_path
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
    let base = normalize_relative(parent, specifier)?;

    import_candidates(&base)
        .into_iter()
        .find_map(|candidate| modules.get(&candidate).copied())
}

fn import_candidates(base: &str) -> Vec<String> {
    let mut candidates = vec![base.to_string()];
    let file_name = base.rsplit('/').next().unwrap_or(base);
    if !file_name.contains('.') {
        candidates.extend(
            EXTENSIONS
                .iter()
                .map(|extension| format!("{base}.{extension}")),
        );
        candidates.extend(
            EXTENSIONS
                .iter()
                .map(|extension| format!("{base}/index.{extension}")),
        );
    }
    candidates
}

fn normalize_relative(parent: &str, specifier: &str) -> Option<String> {
    let mut parts = parent
        .split('/')
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    for part in specifier.replace('\\', "/").split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part.to_string()),
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use athanor_domain::{
        EntityId, EntityKind, LanguageCode, Ownership, SourceLocation, StableKey,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn resolves_extensionless_and_directory_specifiers_and_rejects_bare_packages() {
        let source = module("src/app.tsx");
        let shared = module("src/shared/api.ts");
        let widgets = module("src/widgets/index.ts");
        let mut modules = HashMap::new();
        modules.insert("src/shared/api.ts".to_string(), &shared);
        modules.insert("src/widgets/index.ts".to_string(), &widgets);

        let shared_resolved = resolve_local_import(&source, "./shared/api", &modules)
            .map(|entity| entity.name.clone());
        assert_eq!(shared_resolved, Some("src/shared/api.ts".to_string()));

        let widgets_resolved =
            resolve_local_import(&source, "./widgets", &modules).map(|entity| entity.name.clone());
        assert_eq!(widgets_resolved, Some("src/widgets/index.ts".to_string()));
        assert!(resolve_local_import(&source, "react", &modules).is_none());
        assert!(resolve_local_import(&source, "./missing", &modules).is_none());
    }

    #[test]
    fn normalizes_windows_separators_and_leading_dot_segments() {
        assert_eq!(normalize_path(".\\src\\app.ts"), "src/app.ts");
        assert_eq!(normalize_path("./src/app.ts"), "src/app.ts");
    }

    fn module(path: &str) -> Entity {
        Entity {
            id: EntityId(format!("ent_{path}")),
            stable_key: StableKey(format!("module://js-ts:{path}")),
            kind: EntityKind::Module,
            name: path.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("typescript".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload: json!({ "imports": [] }),
        }
    }
}
