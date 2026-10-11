use std::sync::Arc;

use athanor_core::{AffectedSubset, ExtractInput, Extractor, LinkInput, Linker, SourceFile};
use athanor_domain::{Entity, RelationKind, RepoId, SnapshotId};
use athanor_extractor_treesitter::TreeSitterExtractor;
use athanor_linker_treesitter::{
    LanguageCoverage, TreeSitterLinker, resolution_coverage, resolve_entities,
};

const CORPUS: &[(&str, &str)] = &[
    (
        "pkg/util.py",
        r#"__all__ = ["helper"]


def helper(value):
    return value.strip()
"#,
    ),
    (
        "pkg/api.py",
        r#"from pkg.util import helper


def handle(request):
    return helper(request)
"#,
    ),
    (
        "app/main.py",
        r#"import pkg.api


def run():
    return pkg.api.handle(None)
"#,
    ),
    (
        "tests/test_main.py",
        r#"from pkg.util import helper


def test_helper():
    return helper(" x ")
"#,
    ),
    (
        "pkg/util/util.go",
        r#"package util

func Helper() string {
	return "x"
}
"#,
    ),
    (
        "api/server.go",
        r#"package api

import "example.com/project/pkg/util"

type Server struct{}

func (s *Server) Listen() {
	s.handle()
	util.Helper()
}

func (s *Server) handle() {}
"#,
    ),
    (
        "api/router.go",
        r#"package api

func route() {
	routeHelper()
}

func routeHelper() {}
"#,
    ),
    (
        "api/server_test.go",
        r#"package api

import "testing"

func TestRoute(t *testing.T) {
	routeHelper()
}
"#,
    ),
];

const SNAPSHOT: &str = "snap_parity";

fn extract_input(path: &str, content: &str) -> ExtractInput {
    ExtractInput {
        repo: RepoId("repo_parity".to_string()),
        snapshot: SnapshotId(SNAPSHOT.to_string()),
        source: SourceFile {
            path: path.to_string(),
            language_hint: None,
            content_hash: None,
            content: Some(content.to_string()),
        },
    }
}

async fn corpus() -> Vec<Entity> {
    let mut entities = Vec::new();
    for (path, content) in CORPUS {
        let output = TreeSitterExtractor
            .extract(extract_input(path, content))
            .await
            .expect("extraction");
        entities.extend(output.entities);
    }
    entities
}

#[tokio::test]
async fn imports_resolve_across_files_and_skip_external_paths() {
    let entities = corpus().await;
    let relations = resolve_entities(&entities, &SnapshotId(SNAPSHOT.to_string()));
    let imports = relations
        .iter()
        .filter(|relation| relation.kind == RelationKind::Imports)
        .collect::<Vec<_>>();
    assert_eq!(imports.len(), 4);

    let api_to_util = imports
        .iter()
        .find(|relation| relation.payload["from"] == "module://python:pkg/api.py")
        .expect("python import relation");
    assert_eq!(api_to_util.payload["to"], "module://python:pkg/util.py");
    assert_eq!(api_to_util.payload["resolution"], "dotted_module_path");
    assert_eq!(api_to_util.status, athanor_domain::RelationStatus::Verified);

    let server_to_util = imports
        .iter()
        .find(|relation| relation.payload["from"] == "module://go:api/server.go")
        .expect("go import relation");
    assert_eq!(server_to_util.payload["to"], "module://go:pkg/util/util.go");
    assert_eq!(server_to_util.payload["resolution"], "package_suffix");
    assert_eq!(
        server_to_util.status,
        athanor_domain::RelationStatus::Inferred
    );
    assert_eq!(server_to_util.confidence, 0.8);

    assert!(
        !imports
            .iter()
            .any(|relation| relation.payload["specifier"] == "testing"),
        "a standard-library import stays unresolved"
    );
}

#[tokio::test]
async fn calls_resolve_through_units_and_import_bindings() {
    let entities = corpus().await;
    let relations = resolve_entities(&entities, &SnapshotId(SNAPSHOT.to_string()));
    let calls = relations
        .iter()
        .filter(|relation| relation.kind == RelationKind::Calls)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 7);

    let resolutions = calls
        .iter()
        .map(|relation| {
            (
                relation.payload["from"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                relation.payload["callee"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                relation.payload["resolution"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                relation.confidence,
            )
        })
        .collect::<Vec<_>>();

    assert!(resolutions.contains(&(
        "symbol://python:pkg/api.py#handle".to_string(),
        "helper".to_string(),
        "imported_exact_name".to_string(),
        0.85
    )));
    assert!(resolutions.contains(&(
        "symbol://python:app/main.py#run".to_string(),
        "pkg.api.handle".to_string(),
        "imported_exact_name".to_string(),
        0.85
    )));
    assert!(resolutions.contains(&(
        "symbol://go:api/router.go#route".to_string(),
        "routeHelper".to_string(),
        "same_unit_exact_name".to_string(),
        0.9
    )));
    assert!(resolutions.contains(&(
        "symbol://go:api/server.go#Listen".to_string(),
        "util.Helper".to_string(),
        "imported_exact_name".to_string(),
        0.85
    )));
    assert!(resolutions.contains(&(
        "symbol://go:api/server.go#Listen".to_string(),
        "s.handle".to_string(),
        "same_unit_member_name".to_string(),
        0.5
    )));
}

#[tokio::test]
async fn resolves_own_instance_members_and_discloses_ambiguous_names() {
    let entities = corpus().await;
    let calls = resolve_entities(&entities, &SnapshotId(SNAPSHOT.to_string()))
        .into_iter()
        .filter(|relation| relation.kind == RelationKind::Calls)
        .collect::<Vec<_>>();
    let unresolved = calls
        .iter()
        .filter(|relation| relation.payload["from"] == "symbol://python:pkg/util.py#helper")
        .collect::<Vec<_>>();
    assert!(
        unresolved.is_empty(),
        "`value.strip()` has no declaration to resolve to"
    );
}

#[tokio::test]
async fn test_callers_produce_tested_by_relations() {
    let entities = corpus().await;
    let tested_by = resolve_entities(&entities, &SnapshotId(SNAPSHOT.to_string()))
        .into_iter()
        .filter(|relation| relation.kind == RelationKind::TestedBy)
        .collect::<Vec<_>>();
    assert_eq!(tested_by.len(), 2);
    assert!(tested_by.iter().any(|relation| {
        relation.payload["from"] == "symbol://python:pkg/util.py#helper"
            && relation.payload["to"] == "symbol://python:tests/test_main.py#test_helper"
    }));
    assert!(tested_by.iter().any(|relation| {
        relation.payload["from"] == "symbol://go:api/router.go#routeHelper"
            && relation.payload["to"] == "symbol://go:api/server_test.go#TestRoute"
    }));
    assert!(tested_by.iter().all(|relation| relation.confidence == 0.6
        && relation.payload["reason"] == "treesitter_test_call"));
}

#[tokio::test]
async fn every_relation_is_evidence_backed_and_deterministic() {
    let entities = corpus().await;
    let first = resolve_entities(&entities, &SnapshotId(SNAPSHOT.to_string()));
    let second = resolve_entities(&entities, &SnapshotId(SNAPSHOT.to_string()));
    let ids = |relations: &[athanor_domain::Relation]| {
        relations
            .iter()
            .map(|relation| relation.id.0.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(first.len(), 13);
    assert_eq!(ids(&first), ids(&second));
    assert!(first.iter().all(|relation| {
        relation.id.0.starts_with("rel_treesitter_")
            && !relation.evidence.is_empty()
            && relation
                .evidence
                .iter()
                .all(|evidence| evidence.extractor.as_deref() == Some("treesitter-linker"))
    }));
}

#[tokio::test]
async fn link_keeps_only_relations_touching_the_affected_subset() {
    let entities = corpus().await;
    let handle = entities
        .iter()
        .find(|entity| entity.stable_key.0 == "symbol://python:pkg/api.py#handle")
        .expect("handle declaration")
        .clone();
    let input = LinkInput {
        snapshot: SnapshotId(SNAPSHOT.to_string()),
        entities: Arc::new(entities),
        facts: Arc::new(Vec::new()),
        affected: AffectedSubset::from_extracted(vec![handle], Vec::new()),
    };
    let relations = TreeSitterLinker.link(input).await.expect("link");
    assert!(
        relations
            .iter()
            .all(|relation| relation.kind == RelationKind::Calls)
    );
    assert_eq!(relations.len(), 2);
}

#[tokio::test]
async fn coverage_is_published_per_language() {
    let entities = corpus().await;
    let coverage = resolution_coverage(&entities);
    assert_eq!(coverage.len(), 2);

    let python = coverage
        .iter()
        .find(|entry| entry.language == "python")
        .expect("python coverage");
    assert_eq!(
        python,
        &LanguageCoverage {
            language: "python".to_string(),
            modules: 4,
            declarations: 4,
            imports_declared: 3,
            imports_resolved: 3,
            call_sites: 4,
            calls_resolved: 3,
            module_call_sites: 0,
        }
    );
    assert_eq!(python.calls_basis_points(), 7500);
    assert_eq!(python.imports_basis_points(), 10_000);

    let go = coverage
        .iter()
        .find(|entry| entry.language == "go")
        .expect("go coverage");
    assert_eq!(
        go,
        &LanguageCoverage {
            language: "go".to_string(),
            modules: 4,
            declarations: 7,
            imports_declared: 2,
            imports_resolved: 1,
            call_sites: 4,
            calls_resolved: 4,
            module_call_sites: 0,
        }
    );
    assert_eq!(go.calls_basis_points(), 10_000);
    assert_eq!(go.imports_basis_points(), 5000);
}
