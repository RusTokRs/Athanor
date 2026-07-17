use std::collections::BTreeMap;
use std::path::Path;

use athanor_domain::{Diagnostic, DiagnosticKind, DiagnosticStatus, Entity, EntityKind, Relation};
use serde_json::Value;

use crate::config::DocsConfig;

use super::shared::{operation, push_change, read_project_file};
use super::super::api_docs::{
    api_doc_content, api_doc_path, documented_api_pages, explicit_api_entity_reference_change,
    is_api_documentation_page, stale_api_route_mentions, upsert_api_doc_managed_section,
    upsert_api_docs_coordination_section, upsert_api_narrative_review_section,
};
use super::super::check::page_path;
use super::super::DocsPatchOperation;

pub(super) fn add_missing(
    changes: &mut BTreeMap<String, DocsPatchOperation>,
    snapshot: &str,
    config: &DocsConfig,
    pages: &BTreeMap<String, &Entity>,
    entities: &[Entity],
    relations: &[Relation],
    diagnostics: &[Diagnostic],
) {
    for diagnostic in diagnostics.iter().filter(|diagnostic| {
        diagnostic.status == DiagnosticStatus::Open
            && diagnostic.kind == DiagnosticKind::ApiEndpointImplementedButNotDocumented
    }) {
        let Some(endpoint_key) = diagnostic.payload.get("endpoint").and_then(Value::as_str) else {
            continue;
        };
        let Some(endpoint) = entities.iter().find(|entity| {
            entity.kind == EntityKind::ApiEndpoint && entity.stable_key.0 == endpoint_key
        }) else {
            continue;
        };
        let path = api_doc_path(&config.editable_path, endpoint);
        if pages.contains_key(&path) || changes.contains_key(&path) {
            continue;
        }
        changes.insert(
            path.clone(),
            DocsPatchOperation {
                path: path.clone(),
                stable_key: format!("doc://{path}"),
                create: true,
                content: Some(api_doc_content(
                    snapshot,
                    endpoint,
                    diagnostic,
                    &path,
                    entities,
                    relations,
                )),
                changes: Vec::new(),
            },
        );
    }
}

pub(super) fn update_existing(
    changes: &mut BTreeMap<String, DocsPatchOperation>,
    root: &Path,
    pages: &[&Entity],
    pages_by_path: &BTreeMap<String, &Entity>,
    entities: &[Entity],
    relations: &[Relation],
) {
    let endpoints = entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::ApiEndpoint)
        .collect::<Vec<_>>();

    for endpoint in &endpoints {
        let documented = documented_api_pages(endpoint, pages_by_path, entities, relations);
        let split = documented.len() > 1;
        for page in &documented {
            let Some(path) = page_path(page) else {
                continue;
            };
            if changes.get(&path).is_some_and(|entry| entry.create) {
                continue;
            }
            if let Some(change) = explicit_api_entity_reference_change(page, endpoint) {
                push_change(changes, &path, page, change);
            }
            let Some(existing) = read_project_file(root, &path) else {
                continue;
            };
            let base = changes
                .get(&path)
                .and_then(|entry| entry.content.as_deref())
                .unwrap_or(existing.as_str());
            let mut updated = upsert_api_doc_managed_section(base, endpoint, entities, relations);
            if split {
                updated = upsert_api_docs_coordination_section(&updated, endpoint, &documented);
            }
            if updated != base {
                operation(changes, &path, page).content = Some(updated);
            }
        }
    }

    for page in pages.iter().copied().filter(|page| is_api_documentation_page(page)) {
        let Some(path) = page_path(page) else {
            continue;
        };
        if changes.get(&path).is_some_and(|entry| entry.create) {
            continue;
        }
        let page_endpoints = endpoints
            .iter()
            .copied()
            .filter(|endpoint| {
                documented_api_pages(endpoint, pages_by_path, entities, relations)
                    .iter()
                    .any(|documented| documented.id == page.id)
            })
            .collect::<Vec<_>>();
        if page_endpoints.is_empty() {
            continue;
        }
        let Some(existing) = read_project_file(root, &path) else {
            continue;
        };
        let base = changes
            .get(&path)
            .and_then(|entry| entry.content.as_deref())
            .unwrap_or(existing.as_str());
        let stale = stale_api_route_mentions(base, &page_endpoints);
        if stale.is_empty() {
            continue;
        }
        let updated = upsert_api_narrative_review_section(base, &page_endpoints, &stale);
        if updated != base {
            operation(changes, &path, page).content = Some(updated);
        }
    }
}
