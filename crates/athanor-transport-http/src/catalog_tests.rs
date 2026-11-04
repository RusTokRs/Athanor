use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "athanor-docs-serve-catalog-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create fixture project");
    root
}

fn write_generation(root: &Path, id: &str, body: &str) -> PathBuf {
    let dir = root
        .join(DOCUMENTATION_ROOT_RELATIVE)
        .join("generations")
        .join(id);
    fs::create_dir_all(dir.join("architecture")).expect("create generation directory");
    fs::write(dir.join("architecture/index.md"), body).expect("write docs body");
    let manifest = serde_json::json!({
        "schema": DocumentationGenerationManifest::SCHEMA,
        "request_schema": athanor_app::DocumentationGenerationRequest::SCHEMA,
        "generation": id,
        "snapshot": "snap-1",
        "profile": "architecture",
        "status": "complete",
        "effective_limits": {
            "max_entities": 1,
            "max_facts": 1,
            "max_relations": 1,
            "max_diagnostics": 1
        },
        "omitted": { "entities": 0, "facts": 0, "relations": 0, "diagnostics": 0 },
        "documents": [{
            "id": "architecture-overview",
            "path": "architecture/index.md",
            "media_type": "text/markdown",
            "sha256": sha256_hex(body.as_bytes())
        }]
    });
    fs::write(
        dir.join(MANIFEST_FILE),
        serde_json::to_string_pretty(&manifest).expect("manifest JSON"),
    )
    .expect("write manifest");
    dir
}

fn limits() -> CatalogLimits {
    CatalogLimits {
        max_generations: 8,
        max_document_bytes: 64 * 1024,
    }
}

#[test]
fn loads_a_validated_generation_and_verifies_checksums() {
    let root = fixture_root("valid");
    write_generation(&root, "00000001", "# Architecture\n");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert_eq!(catalog.generations().len(), 1);
    let served = catalog
        .read_document("00000001", "architecture/index.md", 4096)
        .expect("declared document");
    assert_eq!(served.bytes, b"# Architecture\n".to_vec());
    assert_eq!(served.media_type, "text/markdown; charset=utf-8");

    fs::write(
        root.join(DOCUMENTATION_ROOT_RELATIVE)
            .join("generations/00000001/architecture/index.md"),
        "# Tampered\n",
    )
    .expect("tamper with the published document");
    let error = catalog
        .read_document("00000001", "architecture/index.md", 4096)
        .expect_err("tampered document must be rejected");
    assert!(matches!(error, CatalogError::ChecksumMismatch(_)));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn refuses_undeclared_documents_and_unknown_generations() {
    let root = fixture_root("undeclared");
    let dir = write_generation(&root, "00000001", "# Architecture\n");
    fs::write(dir.join("secret.md"), "classified").expect("write undeclared file");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert!(matches!(
        catalog.read_document("00000001", "secret.md", 4096),
        Err(CatalogError::NotFound(_))
    ));
    assert!(matches!(
        catalog.read_document("00000001", "../secret.md", 4096),
        Err(CatalogError::NotFound(_))
    ));
    assert!(matches!(
        catalog.read_document("00999999", "architecture/index.md", 4096),
        Err(CatalogError::NotFound(_))
    ));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn reports_invalid_generations_without_failing_the_catalog() {
    let root = fixture_root("invalid");
    write_generation(&root, "00000001", "# Architecture\n");
    let broken = root
        .join(DOCUMENTATION_ROOT_RELATIVE)
        .join("generations/00000002");
    fs::create_dir_all(&broken).expect("create broken generation");
    fs::write(broken.join(MANIFEST_FILE), "{ not json").expect("write broken manifest");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert_eq!(catalog.generations().len(), 1);
    assert_eq!(catalog.invalid().len(), 1);
    assert_eq!(catalog.invalid()[0].generation, "00000002");
    let report = catalog.report();
    assert_eq!(report.schema, SERVE_CATALOG_SCHEMA_V1);
    assert_eq!(report.generations[0].profile, "architecture");
    assert_eq!(report.generations[0].status, "complete");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn binds_the_current_pointer_to_a_loadable_generation() {
    let root = fixture_root("current");
    write_generation(&root, "00000001", "# Architecture\n");
    fs::write(
        root.join(DOCUMENTATION_ROOT_RELATIVE).join(CURRENT_FILE),
        serde_json::json!({
            "schema": DOCUMENTATION_CURRENT_SCHEMA_V1,
            "generation": "00000001",
            "snapshot": "snap-1",
            "profile": "architecture",
            "path": "generations/00000001",
            "manifest": "generations/00000001/manifest.json"
        })
        .to_string(),
    )
    .expect("write current pointer");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert_eq!(catalog.current(), Some("00000001"));
    assert!(catalog.generations()[0].current);

    fs::write(
        root.join(DOCUMENTATION_ROOT_RELATIVE).join(CURRENT_FILE),
        serde_json::json!({
            "schema": DOCUMENTATION_CURRENT_SCHEMA_V1,
            "generation": "00000404",
            "snapshot": "snap-1",
            "profile": "architecture",
            "path": "generations/00000404",
            "manifest": "generations/00000404/manifest.json"
        })
        .to_string(),
    )
    .expect("write dangling current pointer");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert_eq!(catalog.current(), None);
    assert!(catalog.report().current_error.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn discloses_generations_above_the_catalog_bound() {
    let root = fixture_root("bounded");
    for index in 1..=3 {
        write_generation(&root, &format!("0000000{index}"), "# Architecture\n");
    }
    let catalog = Catalog::load(
        &root,
        CatalogLimits {
            max_generations: 2,
            max_document_bytes: 64 * 1024,
        },
    )
    .expect("catalog");
    assert_eq!(catalog.generations().len(), 2);
    assert_eq!(catalog.report().omitted_generations, 1);
    assert_eq!(catalog.generations()[0].id, "00000003");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn refuses_a_document_that_escapes_its_generation_directory() {
    let root = fixture_root("escape");
    let dir = write_generation(&root, "00000001", "# Architecture\n");
    let outside = root.join("outside.md");
    fs::write(&outside, "outside").expect("write outside file");
    let manifest_path = dir.join(MANIFEST_FILE);
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("manifest"))
            .expect("manifest JSON");
    manifest["documents"][0]["path"] = serde_json::json!("../outside.md");
    manifest["documents"][0]["sha256"] = serde_json::json!(sha256_hex(b"outside"));
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("manifest JSON"),
    )
    .expect("write manifest");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert!(catalog.generations().is_empty());
    assert_eq!(catalog.invalid().len(), 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn validation_summary_requires_a_declared_checksum_and_matching_identity() {
    let root = fixture_root("validation");
    let dir = write_generation(&root, "00000001", "# Architecture\n");
    let mut report = serde_json::json!({
        "schema": DocumentationValidationReport::SCHEMA,
        "draft_schema": DocumentationDraft::SCHEMA,
        "snapshot": "snap-1", "profile": "architecture", "status": "valid",
        "policy": { "provider_enabled": false, "network_enabled": false, "raw_file_access": false, "secrets_included": false },
        "diagnostics": [],
        "metrics": {
            "citation_coverage_basis_points": 10000, "citation_validity_basis_points": 10000,
            "diagram_validity_basis_points": 10000, "deterministic_repeatability": true,
            "unsupported_relations": 0, "prompt_tokens": null, "completion_tokens": null,
            "provider_cost_microunits": null, "human_review_score_basis_points": null
        }
    });
    let body = report.to_string();
    fs::write(dir.join(VALIDATION_REPORT_FILE), &body).expect("write validation sidecar");
    // Undeclared reports are never trusted as validation metadata.
    assert!(
        Catalog::load(&root, limits())
            .expect("catalog")
            .generations()[0]
            .validation
            .is_none()
    );
    let manifest_path = dir.join(MANIFEST_FILE);
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("manifest"))
            .expect("manifest JSON");
    manifest["documents"]
        .as_array_mut()
        .expect("documents")
        .push(serde_json::json!({
            "id": "architecture-validation-report", "path": VALIDATION_REPORT_FILE,
            "media_type": "application/json", "sha256": sha256_hex(body.as_bytes())
        }));
    fs::write(&manifest_path, manifest.to_string()).expect("declare report");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert_eq!(
        catalog.generations()[0]
            .validation
            .as_ref()
            .expect("validated sidecar")
            .status,
        "valid"
    );
    report["snapshot"] = serde_json::json!("different-snapshot");
    let body = report.to_string();
    fs::write(dir.join(VALIDATION_REPORT_FILE), &body).expect("tamper with report");
    assert!(
        Catalog::load(&root, limits())
            .expect("catalog")
            .generations()[0]
            .validation
            .is_none()
    );
    manifest["documents"][1]["sha256"] = serde_json::json!(sha256_hex(body.as_bytes()));
    fs::write(manifest_path, manifest.to_string()).expect("update declared checksum");
    // Even a matching checksum is not enough for the wrong snapshot.
    assert!(
        Catalog::load(&root, limits())
            .expect("catalog")
            .generations()[0]
            .validation
            .is_none()
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn current_pointer_identity_and_layout_cannot_mark_an_unrelated_profile() {
    let root = fixture_root("current-identity");
    write_generation(&root, "00000001", "# Architecture\n");
    let mut pointer = serde_json::json!({
        "schema": DOCUMENTATION_CURRENT_SCHEMA_V1,
        "generation": "00000001", "snapshot": "snap-1", "profile": "module",
        "path": "generations/00000001", "manifest": "generations/00000001/manifest.json"
    });
    for (field, value) in [
        ("profile", "module"),
        ("snapshot", "wrong-snapshot"),
        ("path", "../outside"),
        ("manifest", "../manifest.json"),
    ] {
        pointer["profile"] = serde_json::json!("architecture");
        pointer["snapshot"] = serde_json::json!("snap-1");
        pointer["path"] = serde_json::json!("generations/00000001");
        pointer["manifest"] = serde_json::json!("generations/00000001/manifest.json");
        pointer[field] = serde_json::json!(value);
        fs::write(
            root.join(DOCUMENTATION_ROOT_RELATIVE).join(CURRENT_FILE),
            pointer.to_string(),
        )
        .expect("write invalid pointer");
        let catalog = Catalog::load(&root, limits()).expect("catalog remains readable");
        assert!(catalog.current().is_none());
        assert!(catalog.report().current_error.is_some());
        assert!(!catalog.generations()[0].current);
    }
    fs::remove_dir_all(root).ok();
}

#[test]
fn document_reads_and_manifest_reads_have_independent_hard_bounds() {
    let root = fixture_root("file-bounds");
    let dir = write_generation(&root, "00000001", &"x".repeat(2048));
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert!(matches!(
        catalog.read_document("00000001", "architecture/index.md", 1024),
        Err(CatalogError::TooLarge(_))
    ));
    fs::write(
        dir.join(MANIFEST_FILE),
        " ".repeat(MAX_MANIFEST_BYTES as usize + 1),
    )
    .expect("write oversized metadata");
    let catalog = Catalog::load(&root, limits()).expect("bounded catalog");
    assert!(catalog.generations().is_empty());
    assert_eq!(catalog.invalid().len(), 1);
    assert!(
        catalog.invalid()[0]
            .reason
            .contains("65536 byte serve limit")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn numeric_generation_order_survives_a_padding_width_boundary_and_ignores_staging() {
    let root = fixture_root("numeric-order");
    write_generation(&root, "99999999", "# Earlier\n");
    write_generation(&root, "100000000", "# Later\n");
    fs::create_dir_all(
        root.join(DOCUMENTATION_ROOT_RELATIVE)
            .join("generations/.staging-new"),
    )
    .expect("staging directory");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert_eq!(catalog.generations()[0].id, "100000000");
    assert_eq!(catalog.generations().len(), 2);
    assert!(catalog.invalid().is_empty());
    fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn refuses_symlinked_documents_manifests_generations_and_publication_roots() {
    use std::os::unix::fs::symlink;

    let root = fixture_root("symlinks");
    let dir = write_generation(&root, "00000001", "# Architecture\n");
    let outside = root.join("outside.md");
    fs::write(&outside, "# Architecture\n").expect("write matching external content");
    fs::remove_file(dir.join("architecture/index.md")).expect("remove original body");
    symlink(&outside, dir.join("architecture/index.md")).expect("external document symlink");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert!(matches!(
        catalog.read_document("00000001", "architecture/index.md", 4096),
        Err(CatalogError::Unreadable(_))
    ));

    let external_manifest = root.join("outside-manifest.json");
    fs::copy(dir.join(MANIFEST_FILE), &external_manifest).expect("copy manifest");
    fs::remove_file(dir.join(MANIFEST_FILE)).expect("remove original manifest");
    symlink(&external_manifest, dir.join(MANIFEST_FILE)).expect("external manifest symlink");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert!(catalog.generations().is_empty());
    assert_eq!(catalog.invalid().len(), 1);
    fs::remove_file(dir.join(MANIFEST_FILE)).expect("remove manifest symlink");
    fs::copy(&external_manifest, dir.join(MANIFEST_FILE)).expect("restore manifest");
    symlink(&dir, dir.parent().expect("generations").join("00000002")).expect("generation symlink");
    let catalog = Catalog::load(&root, limits()).expect("catalog");
    assert_eq!(catalog.generations().len(), 1);
    assert_eq!(catalog.invalid().len(), 1);

    let outside_root = fixture_root("outside-publication");
    write_generation(&outside_root, "00000001", "# External publication\n");
    let publication = root.join(DOCUMENTATION_ROOT_RELATIVE);
    fs::remove_dir_all(&publication).expect("remove fixture publication");
    symlink(outside_root.join(DOCUMENTATION_ROOT_RELATIVE), &publication)
        .expect("external publication symlink");
    assert!(Catalog::load(&root, limits()).is_err());
    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(outside_root).ok();
}

#[test]
fn catalog_queries_preserve_operation_deadlines_without_initializing_state() {
    let root = fixture_root("expired-catalog");
    let operation = OperationContext::new("expired-documentation-catalog").with_deadline_unix_ms(1);
    let error = Catalog::load_for_operation(&root, limits(), &operation)
        .expect_err("expired operations must not scan the catalog");
    assert!(error.chain().any(|cause| matches!(
        cause.downcast_ref::<athanor_core::CoreError>(),
        Some(athanor_core::CoreError::DeadlineExceeded(_))
    )));
    assert!(!root.join(".athanor").exists());
    fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn dangling_publication_roots_are_not_mistaken_for_empty_history() {
    use std::os::unix::fs::symlink;

    let root = fixture_root("dangling-publication");
    let publication = root.join(DOCUMENTATION_ROOT_RELATIVE);
    fs::create_dir_all(publication.parent().expect("publication parent"))
        .expect("create publication parent");
    symlink(root.join("missing-documentation"), &publication)
        .expect("create dangling publication link");
    let error =
        Catalog::load(&root, limits()).expect_err("an existing dangling root must fail closed");
    assert!(error.to_string().contains("failed to resolve"));
    fs::remove_dir_all(root).ok();
}
