use athanor_app::ProjectConfig;

const ROOT_CONFIG: &str = include_str!("../../../athanor.toml");
const CONFIG_SOURCE: &str = include_str!("../src/config.rs");
const INIT_SOURCE: &str = include_str!("../src/init.rs");
const CHECK_SOURCE: &str = include_str!("../src/docs/check.rs");
const POLICY_GUIDE: &str = include_str!("../../../docs/development/docs-completeness-policy.md");

const REQUIRED_FIELDS: &[&str] = &["id", "kind", "language", "source_language", "status"];
const ALLOWED_STATUSES: &[&str] = &["active", "implemented", "planned", "draft", "verified"];

#[test]
fn repository_policy_parses_as_current_project_config() {
    let config = toml::from_str::<ProjectConfig>(ROOT_CONFIG)
        .expect("root athanor.toml must parse as the current ProjectConfig");
    let required = config
        .docs
        .completeness
        .required_fields
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let allowed = config
        .docs
        .completeness
        .allowed_statuses
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(required.as_slice(), REQUIRED_FIELDS);
    assert_eq!(allowed.as_slice(), ALLOWED_STATUSES);
    assert!(!config.docs.completeness.require_current_snapshot);
}

#[test]
fn defaults_init_template_and_repository_policy_share_one_lifecycle() {
    for field in REQUIRED_FIELDS {
        assert!(CONFIG_SOURCE.contains(&format!("\"{field}\".to_string()")));
        assert!(ROOT_CONFIG.contains(field));
        assert!(INIT_SOURCE.contains(field));
    }
    for status in ALLOWED_STATUSES {
        assert!(CONFIG_SOURCE.contains(status));
        assert!(ROOT_CONFIG.contains(status));
        assert!(INIT_SOURCE.contains(status));
    }

    for source in [CONFIG_SOURCE, ROOT_CONFIG, INIT_SOURCE] {
        assert!(!source.contains(
            "required_fields = [\"id\", \"kind\", \"language\", \"source_language\", \"last_verified_snapshot\", \"status\"]"
        ));
    }
    assert!(INIT_SOURCE.contains("generated_config_parses_as_the_current_contract"));
    assert!(
        CONFIG_SOURCE.contains("default_docs_policy_separates_completeness_from_snapshot_drift")
    );
}

#[test]
fn completeness_and_snapshot_drift_remain_separate_contracts() {
    assert!(CHECK_SOURCE.contains(".required_fields"));
    assert!(CHECK_SOURCE.contains(".allowed_statuses"));
    assert!(CHECK_SOURCE.contains("if policy.require_current_snapshot"));
    assert!(CHECK_SOURCE.contains("build_docs_drift_report"));
    assert!(CHECK_SOURCE.contains("missing_verification_snapshot"));

    for statement in [
        "`last_verified_snapshot` is not required by the default completeness policy.",
        "Verification age belongs",
        "allowed_statuses` is a lifecycle vocabulary, not execution evidence",
        "Snapshot freshness remains separately observable through",
    ] {
        assert!(
            POLICY_GUIDE.contains(statement),
            "policy guide omits {statement}"
        );
    }
}

#[test]
fn lifecycle_policy_owners_remain_bounded() {
    for (name, source, max_lines) in [
        ("root config", ROOT_CONFIG, 20),
        ("config owner", CONFIG_SOURCE, 430),
        ("init owner", INIT_SOURCE, 150),
        ("docs check owner", CHECK_SOURCE, 260),
        ("policy guide", POLICY_GUIDE, 170),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
