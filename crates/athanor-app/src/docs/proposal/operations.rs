use std::collections::BTreeMap;

use athanor_domain::{Diagnostic, DiagnosticKind, DiagnosticStatus, Entity, EntityKind};
use serde_json::Value;

use crate::config::DocsConfig;

use super::super::super::operations::{
    env_doc_content, env_doc_path, operation_doc_content, operation_doc_diagnostic_shape,
    operation_doc_path,
};
use super::super::super::DocsPatchOperation;

pub(super) fn add_missing(
    changes: &mut BTreeMap<String, DocsPatchOperation>,
    snapshot: &str,
    config: &DocsConfig,
    pages: &BTreeMap<String, &Entity>,
    entities: &[Entity],
    diagnostics: &[Diagnostic],
) {
    for diagnostic in diagnostics.iter().filter(|diagnostic| {
        diagnostic.status == DiagnosticStatus::Open
            && diagnostic.kind == DiagnosticKind::MissingEnvVar
    }) {
        let Some(key) = diagnostic.payload.get("env_var").and_then(Value::as_str) else {
            continue;
        };
        let Some(entity) = entities.iter().find(|entity| {
            entity.kind == EntityKind::EnvVar && entity.stable_key.0 == key
        }) else {
            continue;
        };
        let path = env_doc_path(&config.editable_path, entity);
        if !pages.contains_key(&path) && !changes.contains_key(&path) {
            changes.insert(
                path.clone(),
                DocsPatchOperation {
                    path: path.clone(),
                    stable_key: format!("doc://{path}"),
                    create: true,
                    content: Some(env_doc_content(snapshot, entity, diagnostic, &path)),
                    changes: Vec::new(),
                },
            );
        }
    }

    for diagnostic in diagnostics.iter().filter(|diagnostic| {
        diagnostic.status == DiagnosticStatus::Open
            && matches!(
                diagnostic.kind,
                DiagnosticKind::MissingDocumentation | DiagnosticKind::StaleDocumentation
            )
            && operation_doc_diagnostic_shape(diagnostic).is_some()
    }) {
        let Some((kind, payload_key, prefix, title)) = operation_doc_diagnostic_shape(diagnostic)
        else {
            continue;
        };
        let Some(key) = diagnostic.payload.get(payload_key).and_then(Value::as_str) else {
            continue;
        };
        let Some(entity) = entities
            .iter()
            .find(|entity| entity.kind == kind && entity.stable_key.0 == key)
        else {
            continue;
        };
        let path = operation_doc_path(&config.editable_path, prefix, entity);
        if !pages.contains_key(&path) && !changes.contains_key(&path) {
            changes.insert(
                path.clone(),
                DocsPatchOperation {
                    path: path.clone(),
                    stable_key: format!("doc://{path}"),
                    create: true,
                    content: Some(operation_doc_content(
                        snapshot, entity, diagnostic, &path, title,
                    )),
                    changes: Vec::new(),
                },
            );
        }
    }
}
