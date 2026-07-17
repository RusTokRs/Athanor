use std::path::PathBuf;

use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, LanguageCode, Ownership, Relation, RelationId, RelationKind,
    RelationStatus, Severity, SnapshotId, SourceLocation, StableKey,
};
use serde_json::{Value, json};

pub(super) fn page(path: &str, payload: Value) -> Entity {
    Entity {
        id: EntityId(format!("ent_{path}")),
        stable_key: StableKey(format!("doc://{path}")),
        kind: EntityKind::DocumentationPage,
        name: path.to_string(),
        title: Some("Title".to_string()),
        source: Some(source(path, 1)),
        language: Some(LanguageCode("en".to_string())),
        aliases: Vec::new(),
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        payload: merge_layer(payload, "editable"),
    }
}

pub(super) fn api_endpoint(path: &str) -> Entity {
    Entity {
        id: EntityId(format!("ent_endpoint_{}", path.replace('/', "_"))),
        stable_key: StableKey(format!("api://POST:{path}")),
        kind: EntityKind::ApiEndpoint,
        name: format!("POST {path}"),
        title: Some("login".to_string()),
        source: Some(source("openapi.yaml", 10)),
        language: Some(LanguageCode("openapi".to_string())),
        aliases: Vec::new(),
        ownership: vec![Ownership {
            source_file: "openapi.yaml".to_string(),
        }],
        payload: json!({
            "method": "POST",
            "path": path,
            "operation_id": "login",
            "summary": "Login endpoint.",
            "responses": ["200"]
        }),
    }
}

pub(super) fn handler() -> Entity {
    basic_entity(
        "ent_handler",
        "symbol://rust:auth::login",
        EntityKind::Function,
        "login",
        "src/auth.rs",
    )
}

pub(super) fn api_schema(name: &str) -> Entity {
    basic_entity(
        &format!("ent_schema_{name}"),
        &format!("api-schema://openapi.yaml#{name}"),
        EntityKind::ApiSchema,
        name,
        "openapi.yaml",
    )
}

pub(super) fn api_example(endpoint: &Entity) -> Entity {
    let mut entity = basic_entity(
        "ent_example",
        "api-example://openapi.yaml#success",
        EntityKind::ApiExample,
        "success",
        "openapi.yaml",
    );
    entity.payload = json!({
        "endpoint": endpoint.stable_key.0.clone(),
        "direction": "response",
        "status_code": "200",
        "media_type": "application/json",
        "example_name": "success"
    });
    entity
}

pub(super) fn env_var() -> Entity {
    basic_entity(
        "ent_env",
        "env://DATABASE_URL",
        EntityKind::EnvVar,
        "DATABASE_URL",
        "src/config.rs",
    )
}

pub(super) fn script_command() -> Entity {
    basic_entity(
        "ent_script",
        "script-command://Makefile#target:deploy",
        EntityKind::ScriptCommand,
        "deploy",
        "Makefile",
    )
}

pub(super) fn missing_api_diagnostic(endpoint: &Entity) -> Diagnostic {
    diagnostic(
        "diag_missing_api",
        DiagnosticKind::ApiEndpointImplementedButNotDocumented,
        vec![endpoint.id.clone()],
        json!({"endpoint": endpoint.stable_key.0.clone()}),
    )
}

pub(super) fn missing_env_diagnostic(env: &Entity) -> Diagnostic {
    diagnostic(
        "diag_missing_env",
        DiagnosticKind::MissingEnvVar,
        vec![env.id.clone()],
        json!({"env_var": env.stable_key.0.clone()}),
    )
}

pub(super) fn missing_script_diagnostic(script: &Entity) -> Diagnostic {
    diagnostic(
        "diag_missing_script",
        DiagnosticKind::MissingDocumentation,
        vec![script.id.clone()],
        json!({"script_command": script.stable_key.0.clone(), "scope": "scripts"}),
    )
}

pub(super) fn relation(
    id: &str,
    kind: RelationKind,
    from: &Entity,
    to: &Entity,
    payload: Value,
) -> Relation {
    Relation {
        id: RelationId(id.to_string()),
        kind,
        from: from.id.clone(),
        to: to.id.clone(),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence: Vec::new(),
        ownership: Vec::new(),
        snapshot: SnapshotId("snap_current".to_string()),
        payload,
    }
}

pub(super) fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-docs-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn diagnostic(
    id: &str,
    kind: DiagnosticKind,
    entities: Vec<EntityId>,
    payload: Value,
) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId(id.to_string()),
        kind,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: "Documentation is missing".to_string(),
        message: "Documentation is missing.".to_string(),
        entities,
        evidence: vec![Evidence {
            source_file: Some("openapi.yaml".to_string()),
            line_start: Some(10),
            line_end: Some(20),
            extractor: Some("test".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Missing,
        }],
        ownership: Vec::new(),
        snapshot: SnapshotId("snap_current".to_string()),
        suggested_fix: Some("Add documentation.".to_string()),
        payload,
    }
}

fn basic_entity(
    id: &str,
    key: &str,
    kind: EntityKind,
    name: &str,
    path: &str,
) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(key.to_string()),
        kind,
        name: name.to_string(),
        title: None,
        source: Some(source(path, 42)),
        language: None,
        aliases: Vec::new(),
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        payload: json!({}),
    }
}

fn source(path: &str, line: u32) -> SourceLocation {
    SourceLocation {
        path: path.to_string(),
        line_start: Some(line),
        line_end: Some(line),
    }
}

fn merge_layer(payload: Value, layer: &str) -> Value {
    let mut object = payload.as_object().cloned().unwrap_or_default();
    object.insert("documentation_layer".to_string(), json!(layer));
    Value::Object(object)
}
