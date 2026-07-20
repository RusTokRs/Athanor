use serde_json::Value;

use super::super::DocsFrontmatterChange;
use super::super::frontmatter::{apply_frontmatter_changes, safe_project_path};

#[test]
fn frontmatter_changes_preserve_markdown_body() {
    let content = "---\nid: doc://docs/auth.md\nstatus: draft\n---\n# Auth\n";
    let updated = apply_frontmatter_changes(
        content,
        &[DocsFrontmatterChange {
            field: "status".to_string(),
            old_value: Some(Value::String("draft".to_string())),
            new_value: Value::String("verified".to_string()),
            reason: "status policy".to_string(),
        }],
    )
    .unwrap();
    assert!(updated.contains("status: verified\n"));
    assert!(updated.ends_with("# Auth\n"));
}

#[test]
fn project_paths_reject_parent_escape() {
    let root = std::path::Path::new("/tmp/project");
    assert!(safe_project_path(root, "docs/api.md").is_ok());
    assert!(safe_project_path(root, "../outside.md").is_err());
}
