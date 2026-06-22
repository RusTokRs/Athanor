use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::{EntityKind, RelationKind};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde::Serialize;

use crate::project_path::normalize_canonical_path;

pub const API_REGISTRY_SCHEMA: &str = "athanor.api_registry.v1";

#[derive(Debug, Clone)]
pub struct ApiRegistryOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiRegistryEndpoint {
    pub stable_key: String,
    pub method: String,
    pub path: String,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    pub handler: Option<String>,
    pub documentation: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiRegistryReport {
    pub schema: String,
    pub snapshot: String,
    pub endpoints: Vec<ApiRegistryEndpoint>,
}

pub async fn query_api_registry(options: ApiRegistryOptions) -> Result<ApiRegistryReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
    let canonical = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    let snapshot_id = canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());

    let entities_by_id = canonical
        .entities
        .iter()
        .map(|entity| (&entity.id, entity))
        .collect::<HashMap<_, _>>();

    let mut endpoints = Vec::new();

    for entity in &canonical.entities {
        if entity.kind != EntityKind::ApiEndpoint {
            continue;
        }

        let method = entity.payload["method"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let path = entity.payload["path"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_default();
        let operation_id = entity.payload["operation_id"].as_str().map(str::to_string);
        let summary = entity.payload["summary"].as_str().map(str::to_string);

        // Find linked handler (RelationKind::ImplementedBy)
        let mut handler = None;
        for relation in &canonical.relations {
            if relation.kind == RelationKind::ImplementedBy
                && relation.from == entity.id
                && let Some(target) = entities_by_id.get(&relation.to)
            {
                handler = Some(target.stable_key.0.clone());
                break;
            }
        }

        // Find linked documentation
        let mut documentation = BTreeSet::new();
        for relation in &canonical.relations {
            if relation.to == entity.id
                && (relation.kind == RelationKind::Documents
                    || relation.kind == RelationKind::DocumentsApi
                    || relation.kind == RelationKind::DocumentsOperation)
                && let Some(doc_entity) = entities_by_id.get(&relation.from)
            {
                documentation.insert(doc_entity.stable_key.0.clone());
            }
        }

        endpoints.push(ApiRegistryEndpoint {
            stable_key: entity.stable_key.0.clone(),
            method,
            path,
            operation_id,
            summary,
            handler,
            documentation: documentation.into_iter().collect(),
        });
    }

    endpoints.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));

    Ok(ApiRegistryReport {
        schema: API_REGISTRY_SCHEMA.to_string(),
        snapshot: snapshot_id,
        endpoints,
    })
}

#[cfg(test)]
mod tests {
    use athanor_core::KnowledgeStore;
    use athanor_domain::{
        Entity, EntityId, LanguageCode, Relation, RelationId, RelationStatus, SnapshotBase,
        StableKey,
    };
    use serde_json::json;
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn queries_api_registry_with_relations() {
        let root = std::env::temp_dir().join(format!(
            "athanor-api-registry-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();

        let store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
        let snapshot = store
            .begin_snapshot(
                athanor_domain::RepoId("repo_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();

        let endpoint_entity = Entity {
            id: EntityId("ent_endpoint".to_string()),
            stable_key: StableKey("api://POST:/login".to_string()),
            kind: EntityKind::ApiEndpoint,
            name: "login".to_string(),
            title: Some("User Login".to_string()),
            source: None,
            language: Some(LanguageCode("openapi".to_string())),
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({
                "method": "POST",
                "path": "/login",
                "operation_id": "login",
                "summary": "User Login",
            }),
        };

        let handler_entity = Entity {
            id: EntityId("ent_handler".to_string()),
            stable_key: StableKey("symbol://rust:auth::login".to_string()),
            kind: EntityKind::Function,
            name: "login".to_string(),
            title: None,
            source: None,
            language: Some(LanguageCode("rust".to_string())),
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        };

        let doc_entity = Entity {
            id: EntityId("ent_doc".to_string()),
            stable_key: StableKey("doc://docs/auth.md#login".to_string()),
            kind: EntityKind::DocumentationSection,
            name: "Login Details".to_string(),
            title: None,
            source: None,
            language: Some(LanguageCode("markdown".to_string())),
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        };

        store
            .put_entities(
                snapshot.clone(),
                vec![
                    endpoint_entity.clone(),
                    handler_entity.clone(),
                    doc_entity.clone(),
                ],
            )
            .await
            .unwrap();

        let impl_relation = Relation {
            id: RelationId("rel_impl".to_string()),
            kind: RelationKind::ImplementedBy,
            from: endpoint_entity.id.clone(),
            to: handler_entity.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: snapshot.clone(),
            payload: json!({}),
        };

        let doc_relation = Relation {
            id: RelationId("rel_doc".to_string()),
            kind: RelationKind::DocumentsOperation,
            from: doc_entity.id.clone(),
            to: endpoint_entity.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: snapshot.clone(),
            payload: json!({}),
        };

        store
            .put_relations(snapshot.clone(), vec![impl_relation, doc_relation])
            .await
            .unwrap();

        store.commit_snapshot(snapshot.clone()).await.unwrap();

        let report = query_api_registry(ApiRegistryOptions { root: root.clone() })
            .await
            .unwrap();

        assert_eq!(report.snapshot, snapshot.0);
        assert_eq!(report.endpoints.len(), 1);
        let ep = &report.endpoints[0];
        assert_eq!(ep.stable_key, "api://POST:/login");
        assert_eq!(ep.method, "POST");
        assert_eq!(ep.path, "/login");
        assert_eq!(ep.operation_id.as_deref(), Some("login"));
        assert_eq!(ep.summary.as_deref(), Some("User Login"));
        assert_eq!(ep.handler.as_deref(), Some("symbol://rust:auth::login"));
        assert_eq!(
            ep.documentation,
            vec!["doc://docs/auth.md#login".to_string()]
        );

        fs::remove_dir_all(root).unwrap();
    }
}
