use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use athanor_core::{CoreResult, InvalidationPolicy, InvalidationScope, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityKind, Evidence, EvidenceStatus, Ownership, Relation, RelationId, RelationKind,
    RelationStatus, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::{Value, json};

mod calls;
mod config;
mod implements;
mod paths;

pub use calls::JsTsCallLinker;
pub use implements::JsTsImplementsLinker;

use config::{build_module_resolution_config, resolve_module_specifier};
use paths::normalize_path;

#[derive(Debug, Default, Clone, Copy)]
pub struct JsTsImportLinker;

#[async_trait]
impl Linker for JsTsImportLinker {
    fn name(&self) -> &str {
        "js-ts-imports"
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy {
            on_change: InvalidationScope::DependencyClosure,
            on_add: InvalidationScope::DependencyClosure,
            on_remove: InvalidationScope::DependencyClosure,
        }
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
        let config = build_module_resolution_config(input.entities.as_slice());
        let mut relations = Vec::new();

        for source in modules.values() {
            let imports = source.payload.get("imports").and_then(Value::as_array);
            let exports = source.payload.get("exports").and_then(Value::as_array);
            for (references, kind) in [(imports, "import"), (exports, "re_export")] {
                let Some(references) = references else {
                    continue;
                };
                for reference in references {
                    let Some(specifier) = reference.get("source").and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    let Some((target, origin)) =
                        resolve_module_specifier(source, specifier, &modules, &config)
                    else {
                        continue;
                    };
                    if !affected_ids.contains(&source.id) && !affected_ids.contains(&target.id) {
                        continue;
                    }
                    let reason = format!("{}_module_{kind}", origin.as_str());
                    relations.push(import_relation(
                        &input.snapshot,
                        source,
                        target,
                        specifier,
                        &reason,
                        reference.get("line_start").and_then(|value| value.as_u64()),
                        reference.get("line_end").and_then(|value| value.as_u64()),
                    ));
                }
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

fn import_relation(
    snapshot: &SnapshotId,
    source: &Entity,
    target: &Entity,
    specifier: &str,
    reason: &str,
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
            "reason": reason,
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
    async fn emits_import_relation_for_source_backed_reexports() {
        let barrel = module(
            "ent_barrel",
            "src/barrel.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./leaf",
                    "bindings": [{"imported": "login", "exported": "signIn"}],
                    "line_start": 2,
                    "line_end": 2
                }]
            }),
        );
        let leaf = module(
            "ent_leaf",
            "src/leaf.ts",
            json!({"imports": [], "exports": []}),
        );
        let entities = vec![barrel.clone(), leaf.clone()];

        let relations = JsTsImportLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].kind, RelationKind::Imports);
        assert_eq!(relations[0].from, barrel.id);
        assert_eq!(relations[0].to, leaf.id);
        assert_eq!(relations[0].payload["reason"], "relative_module_re_export");
        assert_eq!(relations[0].evidence[0].line_start, Some(2));
    }

    #[tokio::test]
    async fn emits_import_relation_for_wildcard_reexports() {
        let barrel = module(
            "ent_barrel",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./auth",
                    "bindings": [],
                    "star": true,
                    "line_start": 3,
                    "line_end": 3
                }]
            }),
        );
        let auth = module("ent_auth", "src/auth.ts", json!({"imports": []}));
        let entities = vec![barrel.clone(), auth.clone()];

        let relations = JsTsImportLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].kind, RelationKind::Imports);
        assert_eq!(relations[0].from, barrel.id);
        assert_eq!(relations[0].to, auth.id);
        assert_eq!(relations[0].payload["reason"], "relative_module_re_export");
        assert_eq!(relations[0].evidence[0].line_start, Some(3));
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

    #[tokio::test]
    async fn links_path_aliased_and_workspace_package_imports() {
        let tsconfig = typescript_config(
            "ent_tsconfig",
            "tsconfig.json",
            json!({
                "config_kind": "js_ts_module_resolution",
                "config_file": "tsconfig.json",
                "base_dir": "src",
                "extends": null,
                "paths": [{"pattern": "@/*", "targets": ["app/*"], "line": 6}],
                "omitted_paths": 0,
                "omitted_targets": 0
            }),
        );
        let root_package = package(
            "ent_root_package",
            "package.json",
            "root",
            json!({"workspaces": ["packages/*"]}),
        );
        let ui_package = package(
            "ent_ui_package",
            "packages/ui/package.json",
            "@scope/ui",
            json!({"main": "src/index.ts"}),
        );
        let app = module(
            "ent_app",
            "src/app.ts",
            json!({
                "imports": [
                    {"source": "@/widget", "line_start": 1, "line_end": 1},
                    {"source": "@scope/ui", "line_start": 2, "line_end": 2},
                    {"source": "react", "line_start": 3, "line_end": 3}
                ]
            }),
        );
        let widget = module("ent_widget", "src/app/widget.ts", json!({"imports": []}));
        let ui_entry = module(
            "ent_ui_entry",
            "packages/ui/src/index.ts",
            json!({"imports": []}),
        );
        let entities = vec![
            tsconfig,
            root_package,
            ui_package,
            app.clone(),
            widget.clone(),
            ui_entry.clone(),
        ];

        let relations = JsTsImportLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 2, "the bare specifier stays unresolved");
        let alias = relations
            .iter()
            .find(|relation| relation.to == widget.id)
            .expect("aliased import");
        assert_eq!(alias.payload["specifier"], json!("@/widget"));
        assert_eq!(alias.payload["reason"], json!("path_alias_module_import"));
        let workspace = relations
            .iter()
            .find(|relation| relation.to == ui_entry.id)
            .expect("workspace package import");
        assert_eq!(workspace.payload["specifier"], json!("@scope/ui"));
        assert_eq!(
            workspace.payload["reason"],
            json!("workspace_package_module_import")
        );
    }

    #[tokio::test]
    async fn links_aliased_reexports_with_a_reexport_reason() {
        let tsconfig = typescript_config(
            "ent_tsconfig",
            "tsconfig.json",
            json!({
                "config_kind": "js_ts_module_resolution",
                "base_dir": "src",
                "paths": [{"pattern": "@/*", "targets": ["app/*"], "line": 6}]
            }),
        );
        let barrel = module(
            "ent_barrel",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [{"source": "@/widget", "line_start": 1, "line_end": 1}]
            }),
        );
        let widget = module("ent_widget", "src/app/widget.ts", json!({"imports": []}));
        let entities = vec![tsconfig, barrel.clone(), widget.clone()];

        let relations = JsTsImportLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].to, widget.id);
        assert_eq!(
            relations[0].payload["reason"],
            json!("path_alias_module_re_export")
        );
    }

    fn typescript_config(id: &str, path: &str, payload: serde_json::Value) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("config://{path}#js-ts-module-resolution")),
            kind: EntityKind::Feature,
            name: path.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload,
        }
    }

    fn package(id: &str, path: &str, name: &str, payload: serde_json::Value) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("package://npm:{name}")),
            kind: EntityKind::Package,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload,
        }
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
