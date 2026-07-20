use std::fs;

use athanor_domain::RelationKind;
use serde_json::json;

use crate::config::DocsConfig;

use super::super::proposal::build_docs_patch_proposal;
use super::fixtures::{
    api_endpoint, api_example, api_schema, env_var, handler, missing_api_diagnostic,
    missing_env_diagnostic, missing_script_diagnostic, page, relation, script_command, temp_root,
};

#[test]
fn proposal_repairs_policy_and_snapshot_drift() {
    let page = page(
        "docs/auth.md",
        json!({
            "documentation_kind": "project_overview",
            "frontmatter_fields": ["id", "language", "status"],
            "status": "draft",
            "last_verified_snapshot": "snap_previous"
        }),
    );
    let proposal = build_docs_patch_proposal(
        "snap_current".to_string(),
        &[page],
        &[],
        &[],
        &DocsConfig::default(),
        None,
    );
    let operation = &proposal.operations[0];
    assert!(
        operation
            .changes
            .iter()
            .any(|change| change.field == "kind")
    );
    assert!(operation.changes.iter().any(|change| {
        change.field == "last_verified_snapshot" && change.new_value == json!("snap_current")
    }));
}

#[test]
fn missing_api_page_contains_graph_context() {
    let endpoint = api_endpoint("/login");
    let handler = handler();
    let request = api_schema("LoginRequest");
    let response = api_schema("LoginResponse");
    let example = api_example(&endpoint);
    let diagnostic = missing_api_diagnostic(&endpoint);
    let relations = vec![
        relation(
            "impl",
            RelationKind::ImplementedBy,
            &endpoint,
            &handler,
            json!({}),
        ),
        relation(
            "request",
            RelationKind::SchemaForRequest,
            &endpoint,
            &request,
            json!({"media_type": "application/json"}),
        ),
        relation(
            "response",
            RelationKind::SchemaForResponse,
            &endpoint,
            &response,
            json!({"status_code": "200", "media_type": "application/json"}),
        ),
        relation(
            "example",
            RelationKind::ExampleFor,
            &example,
            &endpoint,
            json!({}),
        ),
    ];
    let proposal = build_docs_patch_proposal(
        "snap_current".to_string(),
        &[endpoint, handler, request, response, example],
        &relations,
        &[diagnostic],
        &DocsConfig::default(),
        None,
    );
    let operation = &proposal.operations[0];
    assert_eq!(operation.path, "docs/api/post-login.md");
    assert!(operation.create);
    let content = operation.content.as_deref().unwrap();
    assert!(content.contains("## Implementation"));
    assert!(content.contains("LoginRequest"));
    assert!(content.contains("LoginResponse"));
    assert!(content.contains("success"));
}

#[test]
fn existing_api_page_gets_managed_and_narrative_sections() {
    let root = temp_root("existing-api");
    let path = root.join("docs/api/login.md");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "---\nid: doc://docs/api/login.md\nkind: api_documentation\nlanguage: en\nsource_language: en\nentities:\n  - api://POST:/login\nlast_verified_snapshot: snap_current\nstatus: verified\n---\n\n# Login\n\nClients still call GET /login.\n",
    )
    .unwrap();
    let endpoint = api_endpoint("/login");
    let page = page(
        "docs/api/login.md",
        json!({
            "documentation_kind": "api_documentation",
            "frontmatter_fields": ["id", "kind", "language", "source_language", "entities", "last_verified_snapshot", "status"],
            "entities": ["api://POST:/login"],
            "last_verified_snapshot": "snap_current",
            "status": "verified"
        }),
    );
    let proposal = build_docs_patch_proposal(
        "snap_current".to_string(),
        &[endpoint, page],
        &[],
        &[],
        &DocsConfig::default(),
        Some(&root),
    );
    let content = proposal.operations[0].content.as_deref().unwrap();
    assert!(content.contains("athanor:api-doc:start"));
    assert!(content.contains("athanor:api-narrative-review:start"));
    assert!(content.contains("Replace `GET /login` with `POST /login`"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_operational_docs_create_bounded_pages() {
    let env = env_var();
    let script = script_command();
    let proposal = build_docs_patch_proposal(
        "snap_current".to_string(),
        &[env.clone(), script.clone()],
        &[],
        &[
            missing_env_diagnostic(&env),
            missing_script_diagnostic(&script),
        ],
        &DocsConfig::default(),
        None,
    );
    assert_eq!(proposal.operations.len(), 2);
    assert!(proposal.operations.iter().any(|operation| {
        operation.path == "docs/operations/env-database-url.md" && operation.create
    }));
    assert!(proposal.operations.iter().any(|operation| {
        operation
            .path
            .contains("script-script-command-makefile-target-deploy")
            && operation.create
    }));
}
