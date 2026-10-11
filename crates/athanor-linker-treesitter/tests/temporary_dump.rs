//! Temporary diagnostic dump used while the corpus expectations were derived. Deleted before the
//! slice is finalized. Each test stays under ten lines of output so the CI failure digest keeps it.

use athanor_core::{ExtractInput, Extractor, SourceFile};
use athanor_domain::{Entity, RelationKind, RepoId, SnapshotId};
use athanor_extractor_treesitter::TreeSitterExtractor;
use athanor_linker_treesitter::{resolution_coverage, resolve_entities};

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

async fn corpus() -> Vec<Entity> {
    let mut entities = Vec::new();
    for (path, content) in CORPUS {
        let output = TreeSitterExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_parity".to_string()),
                snapshot: SnapshotId("snap_parity".to_string()),
                source: SourceFile {
                    path: path.to_string(),
                    language_hint: None,
                    content_hash: None,
                    content: Some(content.to_string()),
                },
            })
            .await
            .expect("extraction");
        entities.extend(output.entities);
    }
    entities
}

#[tokio::test]
async fn dump_recorded_call_sites() {
    let entities = corpus().await;
    let mut lines = Vec::new();
    for entity in &entities {
        let Some(calls) = entity.payload.get("calls").and_then(|value| value.as_array()) else {
            continue;
        };
        for call in calls {
            lines.push(format!(
                "{:?} <= {}",
                entity.stable_key.0,
                call.get("callee").cloned().unwrap_or_default()
            ));
        }
    }
    lines.sort();
    panic!("RECORDED:\n{}", lines.join("\n"));
}

#[tokio::test]
async fn dump_resolved_references() {
    let entities = corpus().await;
    let mut lines = resolve_entities(&entities, &SnapshotId("snap_parity".to_string()))
        .iter()
        .filter(|relation| relation.kind != RelationKind::Imports)
        .map(|relation| {
            format!(
                "{:?} {:?} => {:?} [{}]",
                relation.kind,
                relation.payload["from"].as_str().unwrap_or_default(),
                relation.payload["to"].as_str().unwrap_or_default(),
                relation.payload["resolution"]
                    .as_str()
                    .or(relation.payload["callee"].as_str())
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    lines.sort();
    panic!("RESOLVED:\n{}", lines.join("\n"));
}

#[tokio::test]
async fn dump_coverage() {
    let entities = corpus().await;
    let lines = resolution_coverage(&entities)
        .iter()
        .map(|coverage| format!("COVERAGE {coverage:?}"))
        .collect::<Vec<_>>();
    panic!("COVERAGE:\n{}", lines.join("\n"));
}
