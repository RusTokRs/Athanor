use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use athanor_core::{CoreResult, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityKind, Evidence, EvidenceStatus, Ownership, Relation, RelationId, RelationKind,
    RelationStatus, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::json;

const EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];

#[derive(Debug, Default, Clone, Copy)]
pub struct JsTsImportLinker;

#[async_trait]
impl Linker for JsTsImportLinker {
    fn name(&self) -> &str {
        "js-ts-imports"
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
        let modules = input
            .entities
            .iter()
            .filter(|entity| is_js_ts_module(entity))
            .filter_map(|entity| {
                entity
                    .source
                    .as_ref()
                    .map(|source| (normalize_path(&source.path), entity))
            })
            .collect::<HashMap<_, _>>();
        let affected_ids = input
            .affected
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect::<HashSet<_>>();
        let mut relations = Vec::new();

        for source in modules.values() {
            let Some(imports) = source
                .payload
                .get("imports")
                .and_then(|value| value.as_array())
            else {
                continue;
            };
            for import in imports {
                let Some(specifier) = import.get("source").and_then(|value| value.as_str()) else {
                    continue;
                };
                let Some(target) = resolve_local_import(source, specifier, &modules) else {
                    continue;
                };
                if !affected_ids.contains(&source.id) && !affected_ids.contains(&target.id) {
                    continue;
                }
                relations.push(import_relation(
                    &input.snapshot,
                    source,
                    target,
                    specifier,
                    import.get("line_start").and_then(|value| value.as_u64()),
                    import.get("line_end").and_then(|value| value.as_u64()),
                ));
            }
        }

        relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        relations.dedup_by(|left, right| left.id == right.id);
        Ok(relations)
    }
}

fn is_js_ts_module(entity: &Entity) -> bool {
    entity.kind == EntityKind::Module && entity.stable_key.0.starts_with("module://js-ts:")
}

fn resolve_local_import<'a>(
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

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn import_relation(
    snapshot: &SnapshotId,
    source: &Entity,
    target: &Entity,
    specifier: &str,
    line_start: Option<u64>,
    line_end: Option<u64>,
) -> Relation {
    let id_material = format!(
        "js_ts_import\0{}\0{}",
        source.stable_key.0, target.stable_key.0
    );
    Relation {
        id: RelationId(format!(
            "rel_js_ts_import_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: RelationKind::Imports,
        from: source.id.clone(),
        to: target.id.clone(),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence: import_evidence(source, target, line_start, line_end),
        ownership: combined_ownership(source, target),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": source.stable_key.0,
            "to": target.stable_key.0,
            "specifier": specifier,
            "reason": "relative_module_import",
        }),
    }
}

fn import_evidence(
    source: &Entity,
    target: &Entity,
    line_start: Option<u64>,
    line_end: Option<u64>,
) -> Vec<Evidence> {
    let mut evidence = Vec::new();
    if let Some(location) = &source.source {
        evidence.push(Evidence {
            source_file: Some(location.path.clone()),
            line_start: line_start.and_then(|line| u32::try_from(line).ok()),
            line_end: line_end.and_then(|line| u32::try_from(line).ok()),
            extractor: Some("js-ts-imports".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Verified,
        });
    }
    if let Some(location) = &target.source {
        evidence.push(Evidence {
            source_file: Some(location.path.clone()),
            line_start: location.line_start,
            line_end: location.line_end,
            extractor: Some("js-ts-imports".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Verified,
        });
    }
    evidence
}

fn combined_ownership(source: &Entity, target: &Entity) -> Vec<Ownership> {
    let mut ownership = source.ownership.clone();
    for owner in &target.ownership {
        if !ownership
            .iter()
            .any(|existing| existing.source_file == owner.source_file)
        {
            ownership.push(owner.clone());
        }
    }
    ownership
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use athanor_core::AffectedSubset;
    use athanor_domain::{EntityId, LanguageCode, SourceLocation, StableKey};

    use super::*;

    #[tokio::test]
    async fn links_relative_extensionless_and_directory_imports() {
        let app = module(
            "ent_app",
            "src/app.tsx",
            json!({
                "imports": [
                    {"source": "./shared/api", "line_start": 2, "line_end": 2},
                    {"source": "./widgets", "line_start": 3, "line_end": 3},
                    {"source": "react", "line_start": 4, "line_end": 4}
                ]
            }),
        );
        let api = module("ent_api", "src/shared/api.ts", json!({"imports": []}));
        let widgets = module(
            "ent_widgets",
            "src/widgets/index.ts",
            json!({"imports": []}),
        );
        let entities = vec![app.clone(), api.clone(), widgets.clone()];

        let relations = JsTsImportLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 2);
        assert!(relations.iter().any(|relation| relation.to == api.id));
        assert!(relations.iter().any(|relation| relation.to == widgets.id));
        assert!(relations.iter().all(|relation| {
            relation.kind == RelationKind::Imports
                && relation.status == RelationStatus::Verified
                && relation.evidence.len() == 2
                && relation.ownership.len() == 2
        }));
    }

    #[tokio::test]
    async fn emits_relation_when_only_target_is_affected() {
        let app = module(
            "ent_app",
            "src/app.ts",
            json!({"imports": [{"source": "./api"}]}),
        );
        let api = module("ent_api", "src/api.ts", json!({"imports": []}));

        let relations = JsTsImportLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(vec![app.clone(), api.clone()]),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(vec![api.clone()], Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].from, app.id);
        assert_eq!(relations[0].to, api.id);
    }

    fn module(id: &str, path: &str, payload: serde_json::Value) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
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
            payload,
        }
    }
}
