//! Source fixtures for the read-only Cargo owner/target boundary. Intentionally
//! not execution-verified until the user runs the test suite.

use athanor_core::{CanonicalSnapshot, ExtractInput, Extractor, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, RepoId, SnapshotId,
    SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file};
use athanor_extractor_rust::RustExtractor;
use serde_json::{Value, json};

use super::*;

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_docs".to_string())),
        ..CanonicalSnapshot::default()
    }
}

fn add_file(snapshot: &mut CanonicalSnapshot, path: &str) {
    if snapshot
        .entities
        .iter()
        .any(|entity| entity.kind == EntityKind::File && entity.name == path)
    {
        return;
    }
    snapshot.entities.push(file_entity(
        &SourceFile {
            path: path.to_string(),
            language_hint: None,
            content_hash: None,
            content: None,
        },
        "snap_docs",
    ));
}

fn evidence() -> Value {
    json!({
        "version": 1,
        "autolib": null,
        "autobins": null,
        "autoexamples": null,
        "autotests": null,
        "autobenches": null,
        "workspace": null,
        "edition_inherited": false,
        "lib": null,
        "bins": [],
        "examples": [],
        "tests": [],
        "benches": []
    })
}

fn add_owner(
    snapshot: &mut CanonicalSnapshot,
    manifest: &str,
    name: &str,
    kind: &str,
    value: Value,
) {
    add_file(snapshot, manifest);
    let file_id = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::File && entity.name == manifest)
        .expect("manifest File entity")
        .id
        .clone();
    let stable_key = if kind == "cargo_workspace" {
        format!("cargo-workspace://{manifest}")
    } else {
        format!("package://{manifest}#{name}")
    };
    let fingerprint = stable_hash(stable_key.as_bytes());
    let id = EntityId(format!("ent_package_{fingerprint:016x}"));
    let payload = if kind == "cargo_workspace" {
        let members = value.get("members").cloned().unwrap_or_else(|| json!([]));
        json!({"package_kind": kind, "members": members, "membership_evidence": value})
    } else {
        json!({"package_kind": kind, "name": name, "edition": "2024", "target_evidence": value})
    };
    snapshot.entities.push(Entity {
        id: id.clone(),
        stable_key: StableKey(stable_key.clone()),

        kind: EntityKind::Package,
        name: name.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: manifest.to_string(),
            line_start: Some(1),
            line_end: Some(1),
        }),
        language: Some(LanguageCode("cargo".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file(manifest),
        payload,
    });
    snapshot.facts.push(Fact {
        id: FactId(format!("fact_package_defined_{fingerprint:016x}")),
        kind: FactKind::SymbolDefined,
        subject: id,
        object: Some(file_id),
        value: json!({
            "stable_key": stable_key,
            "path": manifest,
            "source_kind": "cargo_manifest",
            "package_kind": kind,
        }),
        evidence: vec![evidence_for_file(manifest, "operations", Some(1), Some(1))],
        ownership: ownership_for_file(manifest),
        snapshot: SnapshotId("snap_docs".to_string()),
        extractor: "operations".to_string(),
        confidence: 1.0,
    });
}

fn plan(snapshot: &CanonicalSnapshot) -> DocumentationRepositoryPlan {
    build_documentation_repository_plan(
        &DocumentationRepositoryPlanRequest::new("snap_docs", 100),
        snapshot,
    )
    .expect("build read-only plan")
}

async fn add_rust(snapshot: &mut CanonicalSnapshot, path: &str, source: &str) {
    add_file(snapshot, path);
    let output = RustExtractor
        .extract(ExtractInput {
            repo: RepoId("repo_docs".to_string()),
            snapshot: SnapshotId("snap_docs".to_string()),
            source: SourceFile {
                path: path.to_string(),
                language_hint: Some("rust".to_string()),
                content_hash: None,
                content: Some(source.to_string()),
            },
        })
        .await
        .expect("parse Rust fixture");
    snapshot.entities.extend(output.entities);
    snapshot.facts.extend(output.facts);
}

#[test]
fn exact_workspace_members_have_independent_target_roots_and_paths() {
    let mut model = snapshot();
    add_owner(
        &mut model,
        "Cargo.toml",
        "workspace",
        "cargo_workspace",
        json!({
            "version": 1, "members": ["crates/alpha", "crates/beta"], "exclude": []
        }),
    );
    for (root, name) in [("crates/alpha", "alpha"), ("crates/beta", "beta")] {
        add_owner(
            &mut model,
            &format!("{root}/Cargo.toml"),
            name,
            "cargo_package",
            evidence(),
        );
        add_file(&mut model, &format!("{root}/src/lib.rs"));
        add_file(&mut model, &format!("{root}/src/main.rs"));
    }
    let report = plan(&model);
    assert!(!report.writes_allowed);
    assert_eq!(report.totals.packages, 2);
    assert_eq!(report.totals.targets, 4);
    assert!(
        report
            .packages
            .iter()
            .all(|package| package.membership == "member")
    );
    assert_eq!(
        report
            .targets
            .iter()
            .filter(|target| target.kind == "lib")
            .count(),
        2
    );
    assert!(report.targets.iter().all(|target| {
        target
            .output_candidate
            .as_deref()
            .is_some_and(|path| path.starts_with("crates/"))
    }));
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.symbol_omission.as_deref()
                == Some("rust_target_module_scope_unverified"))
    );
}

#[test]
fn contradictory_workspace_member_claims_cannot_assign_package_owners() {
    let mut model = snapshot();
    add_owner(
        &mut model,
        "Cargo.toml",
        "workspace",
        "cargo_workspace",
        json!({"version": 1, "members": ["crates/app"], "exclude": []}),
    );
    add_owner(
        &mut model,
        "crates/app/Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut model, "crates/app/src/lib.rs");
    assert_eq!(plan(&model).targets.len(), 1);

    let workspace = model
        .entities
        .iter_mut()
        .find(|entity| entity.payload["package_kind"] == "cargo_workspace")
        .unwrap();
    workspace.payload["members"] = json!([]);
    let report = plan(&model);
    assert!(report.targets.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "workspace_membership_mismatch")
    );
}

#[test]
fn declared_custom_root_and_disabled_auto_targets_are_not_guessed() {
    let mut model = snapshot();
    let mut config = evidence();
    config["autolib"] = json!(false);
    config["autobins"] = json!(false);
    config["bins"] = json!([{"name": "worker", "path": "tools/worker.rs"}]);
    add_owner(&mut model, "Cargo.toml", "worker", "cargo_package", config);
    add_file(&mut model, "src/lib.rs");
    add_file(&mut model, "src/main.rs");
    add_file(&mut model, "tools/worker.rs");
    let report = plan(&model);
    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].root, "tools/worker.rs");
    assert_eq!(report.targets[0].origin, "declared");
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| file.path == "src/main.rs" && file.target.is_none())
    );
}

#[test]
fn legacy_cargo_auto_discovery_and_named_default_paths_are_not_guessed() {
    let mut legacy = snapshot();
    let mut config = evidence();
    config["lib"] = json!({"name": null, "path": "src/lib.rs"});
    add_owner(&mut legacy, "Cargo.toml", "legacy", "cargo_package", config);
    legacy
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::Package)
        .unwrap()
        .payload["edition"] = json!("2015");
    add_file(&mut legacy, "src/lib.rs");
    add_file(&mut legacy, "src/main.rs");
    add_file(&mut legacy, "examples/fixture.rs");
    let report = plan(&legacy);
    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].kind, "lib");
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "legacy_auto_discovery_unverified")
    );

    let mut modern = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "cli", "path": null}]);
    add_owner(
        &mut modern,
        "Cargo.toml",
        "service",
        "cargo_package",
        config,
    );
    add_file(&mut modern, "src/bin/cli.rs");
    let report = plan(&modern);
    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].root, "src/bin/cli.rs");
    assert_eq!(report.targets[0].origin, "declared");
}

#[test]
fn inherited_edition_requires_verified_workspace_evidence_for_modern_auto_rules() {
    let mut model = snapshot();
    add_owner(
        &mut model,
        "Cargo.toml",
        "workspace",
        "cargo_workspace",
        json!({
            "version": 1, "members": ["crates/app"], "exclude": [], "edition": "2024"
        }),
    );
    let mut config = evidence();
    config["edition_inherited"] = json!(true);
    config["lib"] = json!({"name": null, "path": "src/lib.rs"});
    add_owner(
        &mut model,
        "crates/app/Cargo.toml",
        "app",
        "cargo_package",
        config,
    );
    model
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::Package && entity.name == "app")
        .unwrap()
        .payload["edition"] = Value::Null;
    add_file(&mut model, "crates/app/src/lib.rs");
    add_file(&mut model, "crates/app/src/main.rs");
    let report = plan(&model);
    assert_eq!(report.packages[0].membership, "member");
    assert_eq!(report.targets.len(), 2);
    assert!(report.targets.iter().any(|target| target.kind == "bin"));

    let mut contradictory = model.clone();
    contradictory
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::Package && entity.name == "app")
        .unwrap()
        .payload["edition"] = json!("2024");
    let report = plan(&contradictory);
    assert_eq!(report.packages[0].membership, "unresolved");
    assert!(report.targets.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "cargo_edition_evidence_mismatch")
    );

    let mut missing = model;
    missing
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::Package && entity.name == "workspace")
        .unwrap()
        .payload["membership_evidence"]["edition"] = Value::Null;
    let report = plan(&missing);
    assert_eq!(report.packages[0].membership, "unresolved");
    assert!(report.targets.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "unresolved_inherited_edition")
    );
}

#[test]
fn common_workspace_glob_is_exact_and_unsupported_globs_fail_closed() {
    let mut model = snapshot();
    add_owner(
        &mut model,
        "Cargo.toml",
        "workspace",
        "cargo_workspace",
        json!({
            "version": 1, "members": ["crates/*"], "exclude": []
        }),
    );
    add_owner(
        &mut model,
        "crates/lib/Cargo.toml",
        "lib",
        "cargo_package",
        evidence(),
    );
    add_file(&mut model, "crates/lib/src/lib.rs");
    let report = plan(&model);
    assert_eq!(report.packages[0].membership, "member");
    assert_eq!(report.targets.len(), 1);
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.kind == "unsupported_workspace_member")
    );

    let mut unsupported = snapshot();
    add_owner(
        &mut unsupported,
        "Cargo.toml",
        "workspace",
        "cargo_workspace",
        json!({
            "version": 1, "members": ["crates/**/lib"], "exclude": []
        }),
    );
    add_owner(
        &mut unsupported,
        "crates/team/lib/Cargo.toml",
        "lib",
        "cargo_package",
        evidence(),
    );
    add_file(&mut unsupported, "crates/team/lib/src/lib.rs");
    let report = plan(&unsupported);
    assert_eq!(report.packages[0].membership, "unresolved");
    assert!(report.targets.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "unsupported_workspace_member")
    );

    let mut historical = snapshot();
    add_owner(
        &mut historical,
        "Cargo.toml",
        "old",
        "cargo_package",
        Value::Null,
    );
    add_file(&mut historical, "src/lib.rs");
    let report = plan(&historical);
    assert!(report.targets.is_empty());
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "cargo_target_evidence_unavailable")
    );
}

#[test]
fn human_readmes_and_unowned_suffix_remain_sources_not_output_ownership() {
    let mut model = snapshot();
    add_owner(
        &mut model,
        "crates/ui/Cargo.toml",
        "ui",
        "cargo_package",
        evidence(),
    );
    add_file(&mut model, "crates/ui/src/lib.rs");
    add_file(&mut model, "crates/ui/README.md");
    add_file(&mut model, "crates/ui/README.rst");
    add_file(&mut model, "crates/ui/README.rs");
    add_file(&mut model, "crates/ui/docs/index.athanor.md");
    let report = plan(&model);
    assert!(report.packages[0].index_candidate.is_none());
    assert!(report.sources.iter().any(|source| {
        source.path == "crates/ui/README.md"
            && source.scope == "package"
            && source.kind == "markdown"
    }));
    for path in ["crates/ui/README.rst", "crates/ui/README.rs"] {
        assert!(report.sources.iter().any(|source| {
            source.path == path && source.scope == "package" && source.kind == "readme"
        }));
    }
    assert!(report.sources.iter().any(
        |source| source.path == "crates/ui/docs/index.athanor.md" && source.kind == "markdown"
    ));
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "occupied_output")
    );
}

#[test]
fn portable_directory_alias_and_duplicate_target_name_fail_closed() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "app", "path": "tools/custom.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "src/main.rs");
    add_file(&mut model, "tools/custom.rs");
    add_file(&mut model, "Docs/README.md");
    let report = plan(&model);
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "output_parent_case_collision")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "ambiguous_cargo_target")
    );
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.output_candidate.is_none()
                && target.output_blocker.as_deref() == Some("ambiguous_target_root"))
    );
}

#[test]
fn root_package_without_workspace_does_not_treat_itself_as_an_ancestor() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_file(&mut model, "src/lib.rs");
    let report = plan(&model);
    assert_eq!(report.packages.len(), 1);
    assert_eq!(report.packages[0].membership, "standalone");
    assert!(report.packages[0].index_candidate.is_some());
    assert_eq!(report.targets.len(), 1);
    assert!(report.targets[0].output_candidate.is_some());
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.kind == "unverified_enclosing_manifest")
    );
}

#[test]
fn repeated_or_case_aliased_explicit_targets_on_one_root_have_no_unique_owner() {
    for names in [["runner", "runner"], ["runner", "Runner"]] {
        let mut model = snapshot();
        let mut config = evidence();
        config["autobins"] = json!(false);
        config["bins"] = json!([
            {"name": names[0], "path": "tools/runner.rs"},
            {"name": names[1], "path": "tools/runner.rs"}
        ]);
        add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
        add_file(&mut model, "tools/runner.rs");
        let report = plan(&model);
        assert_eq!(report.targets.len(), 1);
        assert!(report.targets[0].output_candidate.is_none());
        assert_eq!(
            report.targets[0].output_blocker.as_deref(),
            Some("ambiguous_target_identity")
        );
        assert!(
            report.modules.is_empty(),
            "a duplicate cannot own Rust modules"
        );
        assert!(
            report.issues.iter().any(|issue| {
                issue.kind == "ambiguous_cargo_target" && issue.path == "Cargo.toml"
            })
        );
    }
}

#[test]
fn invalid_second_explicit_target_does_not_disappear_from_duplicate_check() {
    let mut model = snapshot();
    let mut config = evidence();
    config["autobins"] = json!(false);
    config["bins"] = json!([
        {"name": "runner", "path": "tools/runner.rs"},
        {"name": "Runner", "path": "tools/missing.rs"}
    ]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "tools/runner.rs");
    let report = plan(&model);
    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].root, "tools/runner.rs");
    assert!(report.targets[0].output_candidate.is_none());
    assert_eq!(
        report.targets[0].output_blocker.as_deref(),
        Some("ambiguous_target_identity")
    );
    assert!(report.modules.is_empty());
    assert!(report.issues.iter().any(|issue| {
        issue.kind == "missing_cargo_target_root" && issue.path == "tools/missing.rs"
    }));
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "ambiguous_cargo_target")
    );
}

#[test]
fn explicit_target_can_confirm_one_matching_cargo_default() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "runner", "path": "src/bin/runner.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "src/bin/runner.rs");
    let report = plan(&model);
    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].origin, "declared");
    assert_eq!(report.targets[0].root, "src/bin/runner.rs");
    assert!(report.targets[0].output_candidate.is_some());
    assert!(report.targets[0].output_blocker.is_none());
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.kind == "ambiguous_cargo_target")
    );
}

#[test]
fn unverified_alternative_cargo_default_cannot_confirm_a_named_target_root() {
    let mut baseline = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "runner", "path": null}]);
    add_owner(&mut baseline, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut baseline, "src/bin/runner.rs");
    assert!(plan(&baseline).targets[0].output_candidate.is_some());

    let mut model = baseline;
    add_file(&mut model, "src/bin/runner/main.rs");
    model
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::File && entity.name == "src/bin/runner/main.rs")
        .unwrap()
        .id = EntityId("forged_default_root".to_string());
    // The explicit target cannot infer a unique root. Even with auto-discovery
    // enabled, the surviving File cannot prove a unique Cargo target identity.
    let report = plan(&model);
    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].origin, "cargo_default");
    assert!(report.targets[0].output_candidate.is_none());
    assert_eq!(
        report.targets[0].output_blocker.as_deref(),
        Some("ambiguous_target_root")
    );
    assert!(report.modules.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "ambiguous_declared_target_root")
    );

    model
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::Package)
        .unwrap()
        .payload["target_evidence"]["autobins"] = json!(false);
    let report = plan(&model);
    assert!(report.targets.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "ambiguous_declared_target_root")
    );
}

#[test]
fn rejected_package_claim_without_file_also_fences_its_nested_tree() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "nested", "path": "nested/src/main.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_owner(
        &mut model,
        "nested/Cargo.toml",
        "nested",
        "cargo_package",
        evidence(),
    );
    add_file(&mut model, "nested/src/main.rs");
    add_file(&mut model, "nested/README.md");
    model
        .entities
        .retain(|entity| !(entity.kind == EntityKind::File && entity.name == "nested/Cargo.toml"));
    let report = plan(&model);
    assert_eq!(report.packages.len(), 1);
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.root != "nested/src/main.rs")
    );
    assert!(
        report
            .sources
            .iter()
            .any(|source| source.path == "nested/README.md" && source.owner_manifest.is_none())
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_cargo_evidence")
    );
}

#[test]
fn case_aliased_package_claim_without_file_is_still_a_boundary() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "cased", "path": "cased/src/main.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_owner(
        &mut model,
        "cased/CARGO.TOML",
        "cased",
        "cargo_package",
        evidence(),
    );
    add_file(&mut model, "cased/src/main.rs");
    add_file(&mut model, "cased/README.md");
    model
        .entities
        .retain(|entity| !(entity.kind == EntityKind::File && entity.name == "cased/CARGO.TOML"));
    let report = plan(&model);
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.root != "cased/src/main.rs")
    );
    assert!(
        report
            .sources
            .iter()
            .any(|source| source.path == "cased/README.md" && source.owner_manifest.is_none())
    );
}

#[test]
fn windows_forbidden_filename_characters_cannot_become_package_paths() {
    for character in ['<', '>', '"', '|', '?', '*', ':', '\\'] {
        assert!(!portable_path(&format!("pkg{character}/Cargo.toml")));
    }
    let mut model = snapshot();
    add_owner(
        &mut model,
        "pkg?/Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut model, "pkg?/src/lib.rs");
    let report = plan(&model);
    assert!(report.packages.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_snapshot_path")
    );
}

#[test]
fn unicode_casefold_and_normalization_aliases_block_claims() {
    assert_eq!(portable_fold("src/Maße.rs"), portable_fold("SRC/MASSE.rs"));
    assert_eq!(
        portable_fold("src/café.rs"),
        portable_fold("src/cafe\u{301}.rs")
    );
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_file(&mut model, "src/lib.rs");
    add_file(&mut model, "Ｓｒｃ/human.rs");
    add_file(&mut model, "ｄｏｃｓ/README.md");
    add_file(&mut model, "src/Maße.rs");
    add_file(&mut model, "src/MASSE.rs");
    add_file(&mut model, "src/café.rs");
    add_file(&mut model, "src/cafe\u{301}.rs");
    let report = plan(&model);
    assert!(report.targets.is_empty());
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "ambiguous_cargo_target_root")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "output_parent_case_collision")
    );
    assert!(
        report
            .issues
            .iter()
            .filter(|issue| issue.kind == "portable_case_collision")
            .count()
            >= 2
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "portable_directory_case_collision")
    );
    assert!(
        report
            .rust_files
            .iter()
            .all(|file| file.owner_manifest.is_none() || file.path != "src/lib.rs")
    );
}

#[test]
fn compatibility_spelled_foreign_manifest_is_an_unresolved_project_boundary() {
    let foreign = "web/ＰＡＣＫＡＧＥ．ＪＳＯＮ";
    assert_eq!(source_kind(foreign), Some("ecosystem_manifest"));
    assert_eq!(
        source_kind("nested/Ｃａｒｇｏ．ｔｏｍｌ"),
        Some("cargo_manifest")
    );
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_file(&mut model, "src/lib.rs");
    add_file(&mut model, foreign);
    add_file(&mut model, "web/src/native.rs");
    let report = plan(&model);
    assert!(
        report
            .sources
            .iter()
            .any(|source| { source.path == foreign && source.kind == "ecosystem_manifest" })
    );
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| { file.path == "web/src/native.rs" && file.owner_manifest.is_none() })
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| { issue.kind == "unsupported_ecosystem_owner" && issue.path == foreign })
    );
}

#[test]
fn an_unverified_nested_manifest_blocks_parent_targets_and_source_ownership() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "nested", "path": "nested/src/main.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "nested/Cargo.toml");
    add_file(&mut model, "nested/src/main.rs");
    add_file(&mut model, "nested/README.md");
    let report = plan(&model);
    assert!(report.targets.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "overlapping_cargo_target")
    );
    assert!(
        report
            .sources
            .iter()
            .any(|source| source.path == "nested/README.md" && source.owner_manifest.is_none())
    );
}

#[tokio::test]
async fn rejected_file_identity_still_fences_nested_owners_and_output_paths() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "nested", "path": "nested/src/main.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "nested/Cargo.toml");
    add_file(&mut model, "nested/src/main.rs");
    add_file(&mut model, "nested/README.md");
    add_file(&mut model, "docs/Cargo.toml");
    add_rust(&mut model, "src/lib.rs", "mod private;").await;
    add_file(&mut model, "src/private/Cargo.toml");
    add_rust(
        &mut model,
        "src/private/mod.rs",
        "pub fn not_a_parent_symbol() {}",
    )
    .await;
    for path in [
        "nested/Cargo.toml",
        "docs/Cargo.toml",
        "src/private/Cargo.toml",
    ] {
        model
            .entities
            .iter_mut()
            .find(|entity| entity.kind == EntityKind::File && entity.name == path)
            .unwrap()
            .id = EntityId(format!("rejected:{path}"));
    }
    let report = plan(&model);
    assert!(
        report
            .issues
            .iter()
            .filter(|issue| issue.kind == "invalid_file_identity")
            .count()
            >= 3
    );
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.root != "nested/src/main.rs")
    );
    assert!(
        report
            .sources
            .iter()
            .any(|source| source.path == "nested/README.md" && source.owner_manifest.is_none())
    );
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "nested_output_package")
    );
    assert!(
        report
            .modules
            .iter()
            .all(|module| module.path != "src/private/mod.rs")
    );
    assert!(
        report
            .rust_symbols
            .iter()
            .all(|symbol| symbol.source_file != "src/private/mod.rs")
    );
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| file.path == "src/private/mod.rs" && file.owner_manifest.is_none())
    );
}

#[test]
fn malformed_file_source_and_ownership_paths_are_only_conservative_boundaries() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "nested", "path": "nested/src/main.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "src/lib.rs");
    add_file(&mut model, "nested/src/main.rs");
    add_file(&mut model, "nested/README.md");
    add_file(&mut model, "docs/human.md");
    let human = model
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::File && entity.name == "docs/human.md")
        .unwrap();
    human.source.as_mut().unwrap().path = "docs/index.athanor.md".to_string();
    human.ownership[0].source_file = "nested/Cargo.toml".to_string();
    let report = plan(&model);
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.root != "nested/src/main.rs")
    );
    assert!(
        report
            .sources
            .iter()
            .any(|source| { source.path == "nested/README.md" && source.owner_manifest.is_none() })
    );
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| { file.path == "nested/src/main.rs" && file.owner_manifest.is_none() })
    );
    for kind in [
        "invalid_file_identity",
        "occupied_output",
        "overlapping_cargo_target",
    ] {
        assert!(
            report.issues.iter().any(|issue| issue.kind == kind),
            "{kind}"
        );
    }
    assert!(
        report
            .sources
            .iter()
            .all(|source| source.path != "docs/human.md")
    );

    // Even when the File's name itself is nonportable, a separate portable
    // source claim can still contradict a proposed output path.
    let mut nonportable = snapshot();
    add_owner(
        &mut nonportable,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut nonportable, "src/lib.rs");
    add_file(&mut nonportable, "docs/bad?name");
    nonportable
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::File && entity.name == "docs/bad?name")
        .unwrap()
        .source
        .as_mut()
        .unwrap()
        .path = "docs/index.athanor.md".to_string();
    let report = plan(&nonportable);
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_snapshot_path")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "occupied_output")
    );

    let mut keyed = snapshot();
    add_owner(&mut keyed, "Cargo.toml", "app", "cargo_package", evidence());
    add_file(&mut keyed, "src/lib.rs");
    add_file(&mut keyed, "docs/human.md");
    keyed
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::File && entity.name == "docs/human.md")
        .unwrap()
        .stable_key = StableKey("file://docs/index.athanor.md".to_string());
    let report = plan(&keyed);
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_file_identity")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "occupied_output")
    );
}

#[test]
fn nested_docs_manifest_and_shared_workspace_root_do_not_claim_outputs() {
    for (manifest, lib) in [
        ("Cargo.toml", "src/lib.rs"),
        ("crates/combined/Cargo.toml", "crates/combined/src/lib.rs"),
    ] {
        let mut model = snapshot();
        add_owner(
            &mut model,
            manifest,
            "workspace",
            "cargo_workspace",
            json!({
                "version": 1, "members": [], "exclude": []
            }),
        );
        add_owner(
            &mut model,
            manifest,
            "cargo-workspace",
            "cargo_package",
            evidence(),
        );
        add_file(&mut model, lib);
        let report = plan(&model);
        assert_eq!(report.packages.len(), 1);
        assert_eq!(report.packages[0].membership, "member");
        assert!(report.packages[0].index_candidate.is_none());
        assert!(report.targets.is_empty());
        assert!(report.issues.iter().any(|issue| {
            issue.kind == "shared_workspace_documentation_root" && issue.path == manifest
        }));
    }

    let mut nested = snapshot();
    add_owner(
        &mut nested,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut nested, "src/lib.rs");
    add_file(&mut nested, "docs/Cargo.toml");
    let report = plan(&nested);
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.output_candidate.is_none())
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "nested_output_package")
    );
}

#[test]
fn unverified_copresent_package_claim_cannot_grant_exclusive_cargo_docs() {
    let mut baseline = snapshot();
    add_owner(
        &mut baseline,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut baseline, "src/lib.rs");
    assert!(plan(&baseline).packages[0].index_candidate.is_some());
    assert_eq!(plan(&baseline).targets.len(), 1);

    for flaw in ["language", "package_kind", "source"] {
        let mut model = baseline.clone();
        add_owner(
            &mut model,
            "Cargo.toml",
            "workspace",
            "cargo_workspace",
            json!({"version": 1, "members": [], "exclude": []}),
        );
        let workspace = model
            .entities
            .iter_mut()
            .find(|entity| {
                entity.kind == EntityKind::Package
                    && entity.payload["package_kind"] == "cargo_workspace"
            })
            .unwrap();
        match flaw {
            "language" => workspace.language = Some(LanguageCode("unknown".to_string())),
            "package_kind" => workspace.payload["package_kind"] = Value::Null,
            "source" => workspace.source.as_mut().unwrap().path = "unrelated.txt".to_string(),
            _ => unreachable!(),
        }
        let report = plan(&model);
        assert!(report.packages.is_empty(), "unverified {flaw} claim");
        assert!(report.targets.is_empty(), "unverified {flaw} claim");
        assert!(report.issues.iter().any(|issue| {
            issue.path == "Cargo.toml"
                && matches!(
                    issue.kind.as_str(),
                    "invalid_cargo_evidence" | "unverified_cargo_manifest_claim"
                )
        }));
        assert!(
            report
                .sources
                .iter()
                .any(|source| { source.path == "Cargo.toml" && source.owner_manifest.is_none() })
        );
    }
}

#[test]
fn fragmented_key_only_package_claim_blocks_an_exclusive_manifest_owner() {
    let mut baseline = snapshot();
    add_owner(
        &mut baseline,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut baseline, "src/lib.rs");
    assert!(plan(&baseline).packages[0].index_candidate.is_some());

    for key in [
        "cargo-workspace://Cargo.toml#stray",
        "package://Cargo.toml#ghost#extra",
    ] {
        let mut model = baseline.clone();
        let mut claim = model
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Package)
            .unwrap()
            .clone();
        claim.id = EntityId("unverified-key-only-package".to_string());
        claim.stable_key = StableKey(key.to_string());
        claim.source = None;
        claim.ownership.clear();
        claim.language = None;
        model.entities.push(claim);

        let report = plan(&model);
        assert!(report.packages.is_empty(), "unverified key {key}");
        assert!(report.targets.is_empty());
        assert!(report.issues.iter().any(|issue| {
            issue.kind == "unverified_cargo_manifest_claim" && issue.path == "Cargo.toml"
        }));
    }
}

#[test]
fn fragmented_key_claim_inside_a_hash_named_directory_fences_parent_targets() {
    let mut valid = snapshot();
    add_owner(
        &mut valid,
        "nested#route/Cargo.toml",
        "nested",
        "cargo_package",
        evidence(),
    );
    add_file(&mut valid, "nested#route/src/lib.rs");
    assert!(plan(&valid).packages[0].index_candidate.is_some());

    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "nested", "path": "nested#route/src/main.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "nested#route/src/main.rs");
    add_file(&mut model, "nested#route/README.md");
    assert_eq!(plan(&model).targets.len(), 1);

    let mut claim = model
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::Package)
        .unwrap()
        .clone();
    claim.id = EntityId("unverified-nested-package".to_string());
    claim.stable_key = StableKey("package://nested#route/Cargo.toml#ghost#extra".to_string());
    claim.source = None;
    claim.ownership.clear();
    claim.language = None;
    model.entities.push(claim);

    let report = plan(&model);
    assert_eq!(report.packages.len(), 1);
    assert!(report.targets.is_empty());
    assert!(report.issues.iter().any(|issue| {
        issue.kind == "unverified_cargo_manifest_claim" && issue.path == "nested#route/Cargo.toml"
    }));
    assert!(report.sources.iter().any(|source| {
        source.path == "nested#route/README.md" && source.owner_manifest.is_none()
    }));
}

#[test]
fn file_directory_conflicts_cannot_prove_manifests_or_cargo_target_roots() {
    let mut manifest = snapshot();
    add_owner(
        &mut manifest,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut manifest, "Cargo.toml/nested.rs");
    add_file(&mut manifest, "src/lib.rs");
    let report = plan(&manifest);
    assert!(report.packages.is_empty());
    assert!(report.issues.iter().any(|issue| {
        issue.kind == "snapshot_file_directory_conflict" && issue.path == "Cargo.toml"
    }));

    for contradictory_path in ["src", "src/lib.rs/nested.rs"] {
        let mut target = snapshot();
        add_owner(
            &mut target,
            "Cargo.toml",
            "app",
            "cargo_package",
            evidence(),
        );
        add_file(&mut target, "src/lib.rs");
        add_file(&mut target, contradictory_path);
        let report = plan(&target);
        assert_eq!(report.packages.len(), 1);
        assert!(report.targets.is_empty());
        assert!(report.issues.iter().any(|issue| {
            issue.kind == "snapshot_file_directory_conflict"
                && matches!(issue.path.as_str(), "src" | "src/lib.rs")
        }));
    }
}

#[test]
fn snapshot_directory_at_candidate_path_blocks_file_proposal() {
    let mut model = snapshot();
    add_owner(
        &mut model,
        "crates/app/Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut model, "crates/app/src/lib.rs");
    // Both would-be page paths are known directories, even though neither is
    // a File entity. A publisher could not create a file at either location.
    add_file(&mut model, "crates/app/docs/index.athanor.md/human.md");
    add_file(
        &mut model,
        "crates/app/docs/modules/lib-app.athanor.md/human.md",
    );
    let report = plan(&model);
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.output_candidate.is_none())
    );
    assert_eq!(
        report
            .issues
            .iter()
            .filter(|issue| issue.kind == "occupied_output_directory")
            .count(),
        2,
    );
}

#[test]
fn mixed_repository_does_not_claim_foreign_project_docs_or_rust_files() {
    let mut model = snapshot();
    add_owner(
        &mut model,
        "Cargo.toml",
        "service",
        "cargo_package",
        evidence(),
    );
    add_file(&mut model, "src/lib.rs");
    add_file(&mut model, "web/package.json");
    add_file(&mut model, "web/README.md");
    add_file(&mut model, "web/src/native.rs");
    let report = plan(&model);
    assert!(report.packages[0].index_candidate.is_some());
    assert!(
        report
            .sources
            .iter()
            .any(|source| source.path == "web/README.md" && source.owner_manifest.is_none())
    );
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| file.path == "web/src/native.rs" && file.owner_manifest.is_none())
    );

    let mut shared = snapshot();
    add_owner(
        &mut shared,
        "Cargo.toml",
        "polyglot",
        "cargo_package",
        evidence(),
    );
    add_file(&mut shared, "src/lib.rs");
    add_file(&mut shared, "package.json");
    let report = plan(&shared);
    assert!(report.packages[0].index_candidate.is_none());
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.output_candidate.is_none())
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "foreign_output_project")
    );
}

#[test]
fn foreign_package_claim_without_a_file_still_fences_an_enclosing_cargo_owner() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "web", "path": "web/src/main.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "src/lib.rs");
    add_file(&mut model, "web/src/main.rs");
    add_file(&mut model, "web/README.md");
    // Only a Package row claims the foreign manifest. It must not prove a
    // source File, but can still disprove exclusive Cargo ownership below it.
    model.entities.push(Entity {
        id: EntityId("unverified_npm_package".to_string()),
        stable_key: StableKey("package://npm:web".to_string()),
        kind: EntityKind::Package,
        name: "web".to_string(),
        title: None,
        source: Some(SourceLocation {
            path: "web/package.json".to_string(),
            line_start: Some(1),
            line_end: None,
        }),
        language: Some(LanguageCode("json".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file("web/package.json"),
        payload: json!({"package_manager": "npm"}),
    });
    let report = plan(&model);
    assert!(
        report
            .sources
            .iter()
            .all(|source| source.path != "web/package.json")
    );
    assert!(
        report
            .sources
            .iter()
            .any(|source| { source.path == "web/README.md" && source.owner_manifest.is_none() })
    );
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| { file.path == "web/src/main.rs" && file.owner_manifest.is_none() })
    );
    let target = report
        .targets
        .iter()
        .find(|target| target.name == "web")
        .unwrap();
    assert_eq!(
        target.output_blocker.as_deref(),
        Some("foreign_target_project")
    );
    assert!(target.output_candidate.is_none());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| { issue.kind == "foreign_cargo_target" && issue.path == target.root })
    );
}

#[test]
fn inconsistent_package_ownership_or_key_still_fences_foreign_manifest_claims() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([
        {"name": "web", "path": "web/src/main.rs"},
        {"name": "native", "path": "native/src/main.rs"}
    ]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "src/lib.rs");
    for project in ["web", "native"] {
        add_file(&mut model, &format!("{project}/src/main.rs"));
        add_file(&mut model, &format!("{project}/README.md"));
    }
    // Neither row has a manifest-shaped source File. The first claims a
    // foreign manifest via ownership, the second via a path-bearing key.
    model.entities.push(Entity {
        id: EntityId("unverified_web".to_string()),
        stable_key: StableKey("package://npm:web".to_string()),
        kind: EntityKind::Package,
        name: "web".to_string(),
        title: None,
        source: Some(SourceLocation {
            path: "web/metadata.txt".to_string(),
            line_start: None,
            line_end: None,
        }),
        language: Some(LanguageCode("json".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file("web/package.json"),
        payload: json!({}),
    });
    model.entities.push(Entity {
        id: EntityId("unverified_native".to_string()),
        stable_key: StableKey("package://native/package.json#native".to_string()),
        kind: EntityKind::Package,
        name: "native".to_string(),
        title: None,
        source: Some(SourceLocation {
            path: "native/metadata.txt".to_string(),
            line_start: None,
            line_end: None,
        }),
        language: Some(LanguageCode("json".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file("native/metadata.txt"),
        payload: json!({}),
    });
    let report = plan(&model);
    for project in ["web", "native"] {
        assert!(
            report
                .sources
                .iter()
                .all(|source| { source.path != format!("{project}/package.json") })
        );
        assert!(report.sources.iter().any(|source| {
            source.path == format!("{project}/README.md") && source.owner_manifest.is_none()
        }));
        assert!(report.rust_files.iter().any(|file| {
            file.path == format!("{project}/src/main.rs") && file.owner_manifest.is_none()
        }));
        let target = report
            .targets
            .iter()
            .find(|target| target.name == project)
            .unwrap();
        assert_eq!(
            target.output_blocker.as_deref(),
            Some("foreign_target_project")
        );
        assert_eq!(
            target.symbol_omission.as_deref(),
            Some("foreign_target_project")
        );
        assert!(target.output_candidate.is_none());
    }
}

#[test]
fn explicit_cargo_target_in_foreign_project_is_visible_but_not_owned() {
    let mut model = snapshot();
    let mut config = evidence();
    config["bins"] = json!([{"name": "web", "path": "web/src/main.rs"}]);
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", config);
    add_file(&mut model, "src/lib.rs");
    add_file(&mut model, "web/package.json");
    add_file(&mut model, "web/src/main.rs");
    let report = plan(&model);
    let web = report
        .targets
        .iter()
        .find(|target| target.name == "web")
        .unwrap();
    assert_eq!(web.root, "web/src/main.rs");
    assert_eq!(web.origin, "declared");
    assert_eq!(
        web.output_blocker.as_deref(),
        Some("foreign_target_project")
    );
    assert_eq!(
        web.symbol_omission.as_deref(),
        Some("foreign_target_project")
    );
    assert!(web.output_candidate.is_none());
    assert!(report.modules.iter().all(|module| module.path != web.root));
    assert!(report.rust_files.iter().any(|file| {
        file.path == web.root && file.owner_manifest.is_none() && file.module.is_none()
    }));
    assert!(
        report
            .issues
            .iter()
            .any(|issue| { issue.kind == "foreign_cargo_target" && issue.path == web.root })
    );
    assert!(
        report
            .targets
            .iter()
            .any(|target| { target.kind == "lib" && target.output_candidate.is_some() })
    );

    // A verified package nested under the foreign root reestablishes its own
    // Cargo boundary. Only the enclosing package's target is withheld.
    let mut nested = snapshot();
    add_owner(
        &mut nested,
        "Cargo.toml",
        "workspace",
        "cargo_workspace",
        json!({"version": 1, "members": ["web/native"], "exclude": []}),
    );
    add_file(&mut nested, "web/package.json");
    add_owner(
        &mut nested,
        "web/native/Cargo.toml",
        "native",
        "cargo_package",
        evidence(),
    );
    add_file(&mut nested, "web/native/src/lib.rs");
    let report = plan(&nested);
    assert_eq!(report.packages[0].membership, "member");
    assert!(report.packages[0].index_candidate.is_some());
    assert_eq!(report.targets.len(), 1);
    assert!(report.targets[0].output_candidate.is_some());
    assert!(report.targets[0].output_blocker.is_none());
}

#[test]
fn other_ecosystems_and_limits_are_reported_without_guessing_owners() {
    let mut model = snapshot();
    add_file(&mut model, "pkg/package.json");
    add_file(&mut model, "pkg/README.md");
    add_file(&mut model, "pkg/go.mod");
    let report = build_documentation_repository_plan(
        &DocumentationRepositoryPlanRequest::new("snap_docs", 1),
        &model,
    )
    .unwrap();
    assert!(report.packages.is_empty());
    assert_eq!(report.totals.sources, 3);
    assert_eq!(report.sources.len(), 1);
    assert_eq!(report.omitted.sources, 2);
    assert!(report.totals.issues >= 2);
    assert!(
        report
            .issues
            .iter()
            .all(|issue| issue.kind == "unsupported_ecosystem_owner")
    );
}

#[test]
fn file_claiming_another_snapshot_cannot_supply_a_cargo_target() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_file(&mut model, "src/lib.rs");
    model
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::File && entity.name == "src/lib.rs")
        .unwrap()
        .payload["snapshot"] = json!("snap_other");
    let report = plan(&model);
    assert!(report.targets.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_file_identity")
    );
}

#[test]
fn noncanonical_file_id_cannot_anchor_rust_symbols_or_a_target() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_file(&mut model, "src/lib.rs");
    model
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::File && entity.name == "src/lib.rs")
        .unwrap()
        .id = EntityId("forged_file_id".to_string());
    let report = plan(&model);
    assert!(report.targets.is_empty());
    assert!(report.rust_symbols.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_file_identity")
    );
}

#[test]
fn duplicated_file_id_invalidates_both_paths_not_just_the_forged_one() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_file(&mut model, "src/lib.rs");
    add_file(&mut model, "src/other.rs");
    let lib_id = model
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::File && entity.name == "src/lib.rs")
        .unwrap()
        .id
        .clone();
    model
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::File && entity.name == "src/other.rs")
        .unwrap()
        .id = lib_id;
    let report = plan(&model);
    assert!(report.targets.is_empty());
    assert!(
        report
            .issues
            .iter()
            .filter(|issue| issue.kind == "invalid_file_identity")
            .count()
            >= 2
    );
}

#[test]
fn duplicate_file_key_cannot_prove_manifest_even_when_the_first_file_is_valid() {
    let mut baseline = snapshot();
    add_owner(
        &mut baseline,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut baseline, "src/lib.rs");
    assert_eq!(plan(&baseline).targets.len(), 1);

    for kind in [EntityKind::File, EntityKind::DocumentationPage] {
        let mut model = baseline.clone();
        let mut forged = file_entity(
            &SourceFile {
                path: "docs/rogue.md".to_string(),
                language_hint: None,
                content_hash: None,
                content: None,
            },
            "snap_docs",
        );
        forged.kind = kind.clone();
        forged.stable_key = StableKey("file://Cargo.toml".to_string());
        model.entities.push(forged);
        let report = plan(&model);
        assert!(report.packages.is_empty(), "reused File key from {kind:?}");
        assert!(report.targets.is_empty());
        assert!(report.sources.iter().all(|row| row.path != "Cargo.toml"));
        assert!(
            report.issues.iter().any(|issue| {
                issue.kind == "invalid_file_identity" && issue.path == "Cargo.toml"
            })
        );
    }
}

#[test]
fn a_second_file_claiming_the_same_source_or_owner_path_invalidates_the_file_anchor() {
    let mut baseline = snapshot();
    add_owner(
        &mut baseline,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut baseline, "src/lib.rs");
    add_file(&mut baseline, "docs/README.md");
    assert_eq!(plan(&baseline).targets.len(), 1);

    for claim in ["source", "ownership"] {
        for occupied in ["Cargo.toml", "src/lib.rs"] {
            let mut model = baseline.clone();
            let unrelated = model
                .entities
                .iter_mut()
                .find(|entity| entity.kind == EntityKind::File && entity.name == "docs/README.md")
                .unwrap();
            if claim == "source" {
                unrelated.source.as_mut().unwrap().path = occupied.to_string();
            } else {
                unrelated.ownership[0].source_file = occupied.to_string();
            }
            let report = plan(&model);
            if occupied == "Cargo.toml" {
                assert!(
                    report.packages.is_empty(),
                    "overlapping {claim} manifest claim"
                );
            } else {
                assert_eq!(report.packages.len(), 1);
                assert!(
                    report.targets.is_empty(),
                    "overlapping {claim} target claim"
                );
            }
            assert!(
                report.issues.iter().any(|issue| {
                    issue.kind == "invalid_file_identity" && issue.path == occupied
                })
            );
        }
    }
}

#[test]
fn cargo_owner_needs_canonical_unique_ids_and_positive_single_fact_evidence() {
    let mut baseline = snapshot();
    add_owner(
        &mut baseline,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_file(&mut baseline, "src/lib.rs");
    assert_eq!(plan(&baseline).targets.len(), 1);

    let mut forged = baseline.clone();
    let forged_id = EntityId("forged_package_id".to_string());
    forged
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::Package)
        .unwrap()
        .id = forged_id.clone();
    forged
        .facts
        .iter_mut()
        .find(|fact| fact.extractor == "operations")
        .unwrap()
        .subject = forged_id;
    assert!(plan(&forged).packages.is_empty());

    let mut weak = baseline.clone();
    weak.facts
        .iter_mut()
        .find(|fact| fact.extractor == "operations")
        .unwrap()
        .confidence = 0.0;
    assert!(plan(&weak).packages.is_empty());

    let mut weak_span = baseline.clone();
    weak_span
        .facts
        .iter_mut()
        .find(|fact| fact.extractor == "operations")
        .unwrap()
        .evidence[0]
        .confidence = 0.0;
    assert!(plan(&weak_span).packages.is_empty());

    for (field, replacement) in [
        ("path", json!("other/Cargo.toml")),
        ("source_kind", json!("dockerfile")),
        ("package_kind", json!("cargo_workspace")),
    ] {
        let mut contradictory = baseline.clone();
        contradictory
            .facts
            .iter_mut()
            .find(|fact| fact.extractor == "operations")
            .unwrap()
            .value[field] = replacement;
        assert!(
            plan(&contradictory).packages.is_empty(),
            "mismatched {field}"
        );
    }

    let mut extra_fact = baseline;
    let duplicated = extra_fact
        .facts
        .iter()
        .find(|fact| fact.extractor == "operations")
        .unwrap()
        .clone();
    extra_fact.facts.push(duplicated);
    let report = plan(&extra_fact);
    assert!(report.packages.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_cargo_evidence")
    );
}

#[tokio::test]
async fn rust_modules_follow_only_matching_facts_within_each_cargo_package() {
    let mut model = snapshot();
    add_owner(
        &mut model,
        "Cargo.toml",
        "workspace",
        "cargo_workspace",
        json!({"version": 1, "members": ["crates/*"], "exclude": []}),
    );
    for (package, name) in [("crates/alpha", "alpha"), ("crates/beta", "beta")] {
        add_owner(
            &mut model,
            &format!("{package}/Cargo.toml"),
            name,
            "cargo_package",
            evidence(),
        );
        add_rust(&mut model, &format!("{package}/src/lib.rs"), "pub mod api;").await;
        add_rust(
            &mut model,
            &format!("{package}/src/api.rs"),
            "pub fn login() {}",
        )
        .await;
    }
    let report = plan(&model);
    assert!(!report.writes_allowed);
    assert_eq!(report.totals.modules, 4);
    assert_eq!(report.totals.rust_symbols, 2);
    for package in ["crates/alpha", "crates/beta"] {
        let manifest = format!("{package}/Cargo.toml");
        let child = report
            .modules
            .iter()
            .find(|module| module.path == format!("{package}/src/api.rs"))
            .unwrap();
        assert_eq!(child.manifest, manifest);
        assert_eq!(
            child.target,
            "lib:".to_string() + &package["crates/".len()..]
        );
        assert_eq!(child.module, "crate::api");
        assert_eq!(
            child.declared_in.as_deref(),
            Some(format!("{package}/src/lib.rs").as_str())
        );
        assert!(
            child
                .declaration_fact
                .as_deref()
                .is_some_and(|id| id.starts_with("fact_rust_module_decl_"))
        );
        assert_eq!(child.syntax, "verified");
        assert!(report.rust_symbols.iter().any(|symbol| {
            symbol.source_file == child.path
                && symbol.manifest == manifest
                && symbol.qualified_name == "crate::api::login"
                && symbol.source_qualified_name.ends_with("::api::login")
                && symbol.definition_fact.starts_with("fact_symbol_defined_")
        }));
        assert!(report.rust_files.iter().any(|file| {
            file.path == child.path
                && file.owner_manifest.as_deref() == Some(manifest.as_str())
                && file.target == Some(child.target.clone())
                && file.module.as_deref() == Some("crate::api")
                && file.omission.as_deref() == Some("rust_symbol_inventory_partial")
        }));
    }
}

#[tokio::test]
async fn inline_and_mod_rs_declarations_resolve_relative_to_their_source_file() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(&mut model, "src/lib.rs", "mod inner { mod leaf; } mod api;").await;
    add_rust(&mut model, "src/inner/leaf.rs", "pub fn read() {}").await;
    add_rust(&mut model, "src/api/mod.rs", "mod detail;").await;
    add_rust(&mut model, "src/api/detail.rs", "pub fn write() {}").await;
    let report = plan(&model);
    assert_eq!(report.totals.modules, 4);
    for (path, module) in [
        ("src/inner/leaf.rs", "crate::inner::leaf"),
        ("src/api/mod.rs", "crate::api"),
        ("src/api/detail.rs", "crate::api::detail"),
    ] {
        assert!(report.modules.iter().any(|row| row.path == path
            && row.module == module
            && row.syntax == "verified"
            && row.declaration_fact.is_some()));
    }
}

#[tokio::test]
async fn module_report_limit_counts_reachable_files_before_truncation() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(&mut model, "src/lib.rs", "mod left; mod right;").await;
    add_rust(&mut model, "src/left.rs", "pub fn left() {}").await;
    add_rust(&mut model, "src/right.rs", "pub fn right() {}").await;
    let report = build_documentation_repository_plan(
        &DocumentationRepositoryPlanRequest::new("snap_docs", 1),
        &model,
    )
    .unwrap();
    assert_eq!(report.totals.modules, 3);
    assert_eq!(report.modules.len(), 1);
    assert_eq!(report.omitted.modules, 2);
    assert_eq!(report.totals.rust_symbols, 2);
    assert_eq!(report.rust_symbols.len(), 1);
    assert_eq!(report.omitted.rust_symbols, 1);
    assert!(!report.writes_allowed);
}

#[tokio::test]
async fn raw_identifier_module_uses_the_unprefixed_file_name() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(&mut model, "src/lib.rs", "mod r#type;").await;
    add_rust(&mut model, "src/type.rs", "pub fn named() {}").await;
    let report = plan(&model);
    assert!(report.modules.iter().any(|row| {
        row.path == "src/type.rs" && row.module == "crate::r#type" && row.syntax == "verified"
    }));
    assert!(report.rust_symbols.iter().any(|row| {
        row.source_file == "src/type.rs"
            && row.qualified_name == "crate::r#type::named"
            && row.source_qualified_name == "crate::type::named"
    }));
}

#[tokio::test]
async fn conditional_custom_ambiguous_and_missing_module_files_do_not_get_an_owner() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut model,
        "src/lib.rs",
        r#"
#[cfg(feature = "x")] mod platform;
#[path = "alt.rs"] mod custom;
mod ambiguous;
mod missing;
mod visible;
"#,
    )
    .await;
    for path in [
        "src/platform.rs",
        "src/alt.rs",
        "src/ambiguous.rs",
        "src/ambiguous/mod.rs",
    ] {
        add_file(&mut model, path);
    }
    add_rust(&mut model, "src/visible.rs", "pub fn okay() {}").await;
    let report = plan(&model);
    assert_eq!(report.modules.len(), 2);
    assert!(
        report
            .modules
            .iter()
            .any(|row| row.path == "src/visible.rs")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "rust_conditional_module")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "rust_custom_module_path")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "ambiguous_rust_module_file")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "missing_rust_module_file")
    );
    assert!(report.rust_files.iter().any(|file| {
        file.path == "src/platform.rs"
            && file.target.is_none()
            && file.omission.as_deref() == Some("module_graph_unverified")
    }));
}

#[tokio::test]
async fn an_unverified_sole_module_file_is_not_a_reachable_module() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(&mut model, "src/lib.rs", "mod child;").await;
    add_file(&mut model, "src/child.rs");
    model
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::File && entity.name == "src/child.rs")
        .unwrap()
        .id = EntityId("forged_child_file".to_string());
    let report = plan(&model);
    assert_eq!(report.modules.len(), 1);
    assert_eq!(report.modules[0].path, "src/lib.rs");
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "unverified_rust_module_file")
    );
}

#[tokio::test]
async fn unverified_or_portably_aliased_alternative_blocks_module_ownership() {
    let mut baseline = snapshot();
    add_owner(
        &mut baseline,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_rust(&mut baseline, "src/lib.rs", "mod child;").await;
    add_rust(&mut baseline, "src/child.rs", "pub fn reachable() {}").await;
    assert!(
        plan(&baseline)
            .modules
            .iter()
            .any(|module| module.path == "src/child.rs")
    );

    for alternate in ["src/child/mod.rs", "src/child/Mod.rs"] {
        let mut model = baseline.clone();
        add_file(&mut model, alternate);
        if alternate == "src/child/mod.rs" {
            // The rejected File row still claims a competing Cargo path.
            model
                .entities
                .iter_mut()
                .find(|entity| entity.kind == EntityKind::File && entity.name == alternate)
                .unwrap()
                .id = EntityId("forged_alternative_file".to_string());
        }
        let report = plan(&model);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == "ambiguous_rust_module_file")
        );
        assert!(
            report
                .modules
                .iter()
                .all(|module| module.path != "src/child.rs")
        );
        assert!(
            report
                .rust_symbols
                .iter()
                .all(|symbol| symbol.source_file != "src/child.rs")
        );
        assert!(
            report
                .rust_files
                .iter()
                .any(|file| { file.path == "src/child.rs" && file.target.is_none() })
        );
    }
}

#[tokio::test]
async fn portable_directory_alias_blocks_rust_module_attribution() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(&mut model, "src/lib.rs", "mod api;").await;
    add_rust(&mut model, "src/api.rs", "pub fn login() {}").await;
    add_file(&mut model, "Src/human.rs");
    let report = plan(&model);
    assert!(report.modules.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "portable_directory_case_collision")
    );
    assert!(report.rust_files.iter().all(|file| file.target.is_none()));
}

#[tokio::test]
async fn module_graph_never_crosses_nested_manifest_or_foreign_project_boundary() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(&mut model, "src/lib.rs", "mod nested; mod private;").await;
    for path in [
        "src/nested/mod.rs",
        "src/nested/package.json",
        "src/private/mod.rs",
        "src/private/Cargo.toml",
    ] {
        add_file(&mut model, path);
    }
    add_rust(&mut model, "src/nested/mod.rs", "pub fn unowned() {}").await;
    add_rust(&mut model, "src/private/mod.rs", "pub fn unowned() {}").await;
    let report = plan(&model);
    assert_eq!(report.modules.len(), 1);
    assert!(report.rust_symbols.is_empty());
    assert!(
        report
            .issues
            .iter()
            .filter(|issue| issue.kind == "rust_module_crosses_owner")
            .count()
            >= 2
    );
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| { file.path == "src/private/mod.rs" && file.target.is_none() })
    );
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| { file.path == "src/nested/mod.rs" && file.owner_manifest.is_none() })
    );
}

#[tokio::test]
async fn crate_level_cfg_or_item_macro_blocks_unconditional_reachability() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut model,
        "src/lib.rs",
        "#![cfg(feature = \"x\")]\nmod hidden;",
    )
    .await;
    add_file(&mut model, "src/hidden.rs");
    let report = plan(&model);
    assert_eq!(report.modules.len(), 1);
    assert_eq!(report.modules[0].syntax, "opaque");
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| { file.path == "src/hidden.rs" && file.target.is_none() })
    );

    let mut macro_model = snapshot();
    add_owner(
        &mut macro_model,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_rust(
        &mut macro_model,
        "src/lib.rs",
        "include!(\"extra.rs\"); mod hidden;",
    )
    .await;
    add_file(&mut macro_model, "src/hidden.rs");
    let report = plan(&macro_model);
    assert_eq!(report.modules.len(), 1);
    assert_eq!(report.modules[0].syntax, "opaque");
}

#[tokio::test]
async fn shared_module_file_and_tampered_fact_fail_closed() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(&mut model, "src/lib.rs", "mod common;").await;
    add_rust(&mut model, "src/main.rs", "mod common;").await;
    add_rust(&mut model, "src/common.rs", "pub fn shared() {}").await;
    let report = plan(&model);
    assert_eq!(report.modules.len(), 2);
    assert!(report.rust_symbols.is_empty());
    assert!(
        report
            .rust_files
            .iter()
            .any(|file| file.path == "src/common.rs"
                && file.target.is_none()
                && file.omission.as_deref() == Some("ambiguous_module_owner"))
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "ambiguous_rust_module_owner")
    );

    let mut corrupt = model;
    corrupt
        .facts
        .iter_mut()
        .find(|fact| {
            fact.kind == FactKind::Other("rust_module_declaration".to_string())
                && fact.ownership[0].source_file == "src/lib.rs"
        })
        .unwrap()
        .value["ordinal"] = json!(99);
    let report = plan(&corrupt);
    assert!(
        report
            .modules
            .iter()
            .any(|row| row.path == "src/lib.rs" && row.syntax == "unavailable")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_rust_module_fact")
    );
}

#[tokio::test]
async fn module_fact_must_agree_with_module_presence_name_and_source_span() {
    let mut conditional = snapshot();
    add_owner(
        &mut conditional,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_rust(
        &mut conditional,
        "src/lib.rs",
        "#[cfg(feature = \"x\")] mod child;",
    )
    .await;
    add_rust(&mut conditional, "src/child.rs", "pub fn hidden() {}").await;
    assert_eq!(plan(&conditional).modules.len(), 1);
    conditional
        .facts
        .iter_mut()
        .find(|fact| fact.kind == FactKind::Other("rust_module_declaration".to_string()))
        .unwrap()
        .value["conditional"] = json!(false);
    let report = plan(&conditional);
    assert!(
        report
            .modules
            .iter()
            .all(|module| module.path != "src/child.rs")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_rust_module_fact")
    );

    let mut baseline = snapshot();
    add_owner(
        &mut baseline,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_rust(&mut baseline, "src/lib.rs", "mod child;").await;
    add_rust(&mut baseline, "src/child.rs", "pub fn present() {}").await;
    assert_eq!(plan(&baseline).modules.len(), 2);

    let mut misplaced_evidence = baseline.clone();
    let fact = misplaced_evidence
        .facts
        .iter_mut()
        .find(|fact| fact.kind == FactKind::Other("rust_module_declaration".to_string()))
        .unwrap();
    fact.evidence[0].line_start = Some(2);
    fact.evidence[0].line_end = Some(2);
    let report = plan(&misplaced_evidence);
    assert!(
        report
            .modules
            .iter()
            .all(|module| module.path != "src/child.rs")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_rust_module_fact")
    );

    let mut mismatched_root = baseline.clone();
    let root = mismatched_root
        .entities
        .iter_mut()
        .find(|entity| {
            entity.kind == EntityKind::Module
                && entity.payload.get("rust_module_scan").is_some()
                && entity
                    .source
                    .as_ref()
                    .is_some_and(|source| source.path == "src/lib.rs")
        })
        .unwrap();
    root.source.as_mut().unwrap().line_start = Some(2);
    root.source.as_mut().unwrap().line_end = Some(2);
    let report = plan(&mismatched_root);
    assert!(
        report
            .modules
            .iter()
            .any(|module| module.path == "src/lib.rs" && module.syntax == "unavailable")
    );
    assert!(
        report
            .modules
            .iter()
            .all(|module| module.path != "src/child.rs")
    );

    let mut mismatched_name = baseline;
    mismatched_name
        .entities
        .iter_mut()
        .find(|entity| entity.kind == EntityKind::Module && entity.name == "child")
        .unwrap()
        .name = "different".to_string();
    let report = plan(&mismatched_name);
    assert!(
        report
            .modules
            .iter()
            .all(|module| module.path != "src/child.rs")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_rust_module_fact")
    );
}

#[tokio::test]
async fn rust_symbols_follow_verified_inline_scope_but_not_cfg_attributes_or_impls() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut model,
        "src/lib.rs",
        r#"
pub fn ready() {}
#[test] pub fn only_harness() {}
mod inner {
    #[derive(Debug)] pub struct Record;
    pub fn run() {}
    #[cfg(test)] pub fn conditional() {}
    #[route("/")] pub fn transformed() {}
    impl Record { pub fn method(&self) {} }
    pub trait Reader {
        fn read(&self);
        #[cfg(test)] fn optional(&self);
    }
}
#[cfg(feature = "x")]
mod gated { pub fn nested() {} }
mod child;
"#,
    )
    .await;
    add_rust(&mut model, "src/child.rs", "pub fn fetch() {}").await;
    let report = plan(&model);
    assert_eq!(report.totals.rust_symbols, 7);
    assert_eq!(report.totals.modules, 2);
    for (source, name, kind) in [
        ("src/lib.rs", "crate::ready", "function"),
        ("src/lib.rs", "crate::inner::Record", "struct"),
        ("src/lib.rs", "crate::inner::Record::method", "method"),
        ("src/lib.rs", "crate::inner::run", "function"),
        ("src/lib.rs", "crate::inner::Reader", "trait"),
        ("src/lib.rs", "crate::inner::Reader::read", "trait_method"),
        ("src/child.rs", "crate::child::fetch", "function"),
    ] {
        assert!(
            report.rust_symbols.iter().any(|row| {
                row.source_file == source
                    && row.qualified_name == name
                    && row.symbol_kind == kind
                    && row.line_start > 0
                    && row.line_end >= row.line_start
            }),
            "missing verified {name}"
        );
    }
    assert!(report.rust_symbols.iter().all(|row| {
        ![
            "conditional",
            "transformed",
            "optional",
            "nested",
            "only_harness",
        ]
        .contains(&row.name.as_str())
    }));
    let record = report
        .rust_symbols
        .iter()
        .find(|row| row.name == "Record")
        .unwrap();
    let method = report
        .rust_symbols
        .iter()
        .find(|row| row.name == "method")
        .unwrap();
    assert_eq!(
        method.owner_symbol.as_deref(),
        Some(record.stable_key.as_str())
    );
    assert_eq!(
        method.owner_definition_fact.as_deref(),
        Some(record.definition_fact.as_str())
    );
    assert!(
        method
            .association_fact
            .as_deref()
            .is_some_and(|id| id.starts_with("fact_rust_inherent_method_"))
    );
    let reader = report
        .rust_symbols
        .iter()
        .find(|row| row.name == "Reader")
        .unwrap();
    let read = report
        .rust_symbols
        .iter()
        .find(|row| row.name == "read")
        .unwrap();
    assert_eq!(
        read.owner_symbol.as_deref(),
        Some(reader.stable_key.as_str())
    );
    assert_eq!(
        read.owner_definition_fact.as_deref(),
        Some(reader.definition_fact.as_str())
    );
    assert!(read.association_fact.is_none());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "unverified_rust_symbol_presence")
    );
    assert!(report.targets.iter().all(|target| {
        target.symbol_omission.as_deref() == Some("rust_symbol_inventory_partial")
    }));
    assert!(!report.writes_allowed);
}

#[tokio::test]
async fn trait_method_requires_an_unconditional_same_file_owner_definition() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut model,
        "src/lib.rs",
        "pub trait Reader { fn read(&self); }",
    )
    .await;
    assert_eq!(plan(&model).totals.rust_symbols, 2);

    let mut missing_owner_fact = model.clone();
    missing_owner_fact.facts.retain(|fact| {
        !(fact.kind == FactKind::SymbolDefined
            && fact.value["symbol_kind"] == "trait"
            && fact.ownership[0].source_file == "src/lib.rs")
    });
    let report = plan(&missing_owner_fact);
    assert!(report.rust_symbols.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "unverified_rust_associated_owner")
    );

    let mut gated = snapshot();
    add_owner(&mut gated, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut gated,
        "src/lib.rs",
        "#[cfg(test)] pub trait Reader { fn read(&self); }",
    )
    .await;
    assert!(plan(&gated).rust_symbols.is_empty());

    let mut gated_method = snapshot();
    add_owner(
        &mut gated_method,
        "Cargo.toml",
        "app",
        "cargo_package",
        evidence(),
    );
    add_rust(
        &mut gated_method,
        "src/lib.rs",
        "pub trait Reader { #[cfg(test)] fn read(&self); }",
    )
    .await;
    let report = plan(&gated_method);
    assert_eq!(report.totals.rust_symbols, 1);
    assert_eq!(report.rust_symbols[0].name, "Reader");
}

#[tokio::test]
async fn inherent_methods_require_a_local_type_and_exact_impl_fact() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut model,
        "src/lib.rs",
        r#"
pub struct Local;
impl Local { pub fn keep(&self) {} }
#[cfg(test)] impl Local { fn conditional(&self) {} }
impl crate::Local { fn qualified(&self) {} }
pub trait Worker { fn work(&self); }
impl Worker for Local { fn work(&self) {} }
"#,
    )
    .await;
    let report = plan(&model);
    assert_eq!(report.totals.rust_symbols, 4);
    assert!(report.rust_symbols.iter().any(|row| {
        row.qualified_name == "crate::Local::keep"
            && row
                .association_fact
                .as_deref()
                .is_some_and(|id| id.starts_with("fact_rust_inherent_method_"))
    }));
    assert!(report.rust_symbols.iter().all(|row| {
        row.name != "conditional"
            && row.name != "qualified"
            && row.source_qualified_name != "crate::Local::Worker::work"
    }));
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "unverified_rust_associated_owner")
    );

    let is_impl = |fact: &Fact| fact.kind == FactKind::Other("rust_inherent_method".to_string());
    let mut missing = model.clone();
    missing.facts.retain(|fact| !is_impl(fact));
    assert!(
        !plan(&missing)
            .rust_symbols
            .iter()
            .any(|row| row.name == "keep")
    );

    let mut wrong_snapshot = model.clone();
    wrong_snapshot
        .facts
        .iter_mut()
        .find(|fact| is_impl(fact))
        .unwrap()
        .snapshot = SnapshotId("snap_other".to_string());
    let report = plan(&wrong_snapshot);
    assert!(!report.rust_symbols.iter().any(|row| row.name == "keep"));
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_rust_inherent_method_fact")
    );

    let mut wrong_owner = model.clone();
    wrong_owner
        .facts
        .iter_mut()
        .find(|fact| is_impl(fact))
        .unwrap()
        .object = None;
    assert!(
        !plan(&wrong_owner)
            .rust_symbols
            .iter()
            .any(|row| row.name == "keep")
    );

    let mut wrong_span = model.clone();
    wrong_span
        .facts
        .iter_mut()
        .find(|fact| is_impl(fact))
        .unwrap()
        .evidence[1]
        .line_end = Some(1);
    assert!(
        !plan(&wrong_span)
            .rust_symbols
            .iter()
            .any(|row| row.name == "keep")
    );

    let mut duplicate = model.clone();
    let duplicate_fact = duplicate
        .facts
        .iter()
        .find(|fact| is_impl(fact))
        .unwrap()
        .clone();
    duplicate.facts.push(duplicate_fact);
    assert!(
        !plan(&duplicate)
            .rust_symbols
            .iter()
            .any(|row| row.name == "keep")
    );
}

#[tokio::test]
async fn generic_local_inherent_impl_uses_its_declared_same_file_type() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut model,
        "src/lib.rs",
        r#"
pub struct Bucket<T>(pub T);
impl<T> Bucket<T> { pub fn open(&self) {} }
"#,
    )
    .await;
    let report = plan(&model);
    assert_eq!(report.totals.rust_symbols, 2);
    assert!(report.rust_symbols.iter().any(|row| {
        row.qualified_name == "crate::Bucket::open" && row.association_fact.is_some()
    }));
}

#[tokio::test]
async fn symbol_definition_requires_one_exact_snapshot_fact_and_matching_lines() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(&mut model, "src/lib.rs", "pub fn safe() {}").await;
    assert_eq!(plan(&model).totals.rust_symbols, 1);
    let is_function = |fact: &Fact| {
        fact.kind == FactKind::SymbolDefined && fact.value["symbol_kind"] == "function"
    };

    let mut foreign_snapshot = model.clone();
    foreign_snapshot
        .facts
        .iter_mut()
        .find(|fact| is_function(fact))
        .unwrap()
        .snapshot = SnapshotId("snap_other".to_string());
    let report = plan(&foreign_snapshot);
    assert!(report.rust_symbols.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "invalid_rust_symbol_evidence")
    );

    let mut extra_fact = model.clone();
    let duplicate = extra_fact
        .facts
        .iter()
        .find(|fact| is_function(fact))
        .unwrap()
        .clone();
    extra_fact.facts.push(duplicate);
    assert!(plan(&extra_fact).rust_symbols.is_empty());

    let mut inconsistent_span = model.clone();
    inconsistent_span
        .entities
        .iter_mut()
        .find(|entity| entity.name == "safe")
        .unwrap()
        .source
        .as_mut()
        .unwrap()
        .line_start = Some(42);
    assert!(plan(&inconsistent_span).rust_symbols.is_empty());

    let mut changed_presence = model.clone();
    changed_presence
        .facts
        .iter_mut()
        .find(|fact| is_function(fact))
        .unwrap()
        .value["rust_presence_unverified"] = json!(true);
    assert!(plan(&changed_presence).rust_symbols.is_empty());

    let mut duplicate_entity = model.clone();
    let duplicate = duplicate_entity
        .entities
        .iter()
        .find(|entity| entity.name == "safe")
        .unwrap()
        .clone();
    duplicate_entity.entities.push(duplicate);
    let report = plan(&duplicate_entity);
    assert!(report.rust_symbols.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "ambiguous_rust_symbol_identity")
    );
}

#[tokio::test]
async fn unnamed_const_is_not_projected_as_a_named_package_symbol() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut model,
        "src/lib.rs",
        "const _: () = (); pub fn named() {}",
    )
    .await;
    let report = plan(&model);
    assert_eq!(report.totals.rust_symbols, 1);
    assert_eq!(report.rust_symbols[0].name, "named");
}

#[tokio::test]
async fn cfg_alternative_definitions_are_not_a_single_confirmed_symbol() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut model,
        "src/lib.rs",
        r#"
#[cfg(unix)] pub fn platform() {}
#[cfg(windows)] pub fn platform() {}
pub fn named() {}
"#,
    )
    .await;
    let report = plan(&model);
    assert_eq!(report.totals.rust_symbols, 1);
    assert_eq!(report.rust_symbols[0].name, "named");
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "unverified_rust_symbol_presence")
    );
}

#[tokio::test]
async fn duplicate_inline_or_external_declarations_do_not_prove_child_scopes() {
    let mut model = snapshot();
    add_owner(&mut model, "Cargo.toml", "app", "cargo_package", evidence());
    add_rust(
        &mut model,
        "src/lib.rs",
        r#"
mod doubled { pub fn inner() {} mod child; }
mod doubled { mod child; }
mod external;
mod external;
pub fn safe() {}
"#,
    )
    .await;
    add_rust(&mut model, "src/doubled/child.rs", "pub fn hidden() {}").await;
    add_rust(&mut model, "src/external.rs", "pub fn hidden() {}").await;
    let report = plan(&model);
    assert_eq!(report.totals.modules, 1);
    assert_eq!(report.totals.rust_symbols, 1);
    assert_eq!(report.rust_symbols[0].name, "safe");
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.kind == "unverified_rust_module_scope")
    );
}

#[test]
fn snapshot_escape_and_identity_mismatch_are_rejected_before_planning() {
    let model = snapshot();
    for value in ["../outside", "CON", "a\\b", "snap.with.dot", ""] {
        assert!(
            DocumentationRepositoryPlanRequest::new(value, 1)
                .validate()
                .is_err()
        );
    }
    assert!(
        build_documentation_repository_plan(
            &DocumentationRepositoryPlanRequest::new("snap_other", 1),
            &model
        )
        .is_err()
    );
}
