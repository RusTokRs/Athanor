//! Temporary diagnostic dump used while the corpus expectations were derived. Deleted before the
//! slice is finalized.

use athanor_core::{ExtractInput, Extractor, SourceFile};
use athanor_domain::{Entity, RepoId, SnapshotId};
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

#[tokio::test]
async fn dump_corpus_evidence() {
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

    let mut lines = Vec::new();
    let mut keys = entities
        .iter()
        .map(|entity| entity.stable_key.0.clone())
        .collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        let entity: &Entity = entities
            .iter()
            .find(|entity| entity.stable_key.0 == key)
            .expect("entity");
        let calls = entity
            .payload
            .get("calls")
            .map(ToString::to_string)
            .unwrap_or_default();
        let imports = entity
            .payload
            .get("imports")
            .map(ToString::to_string)
            .unwrap_or_default();
        lines.push(format!(
            "E {key} name={:?} qualified={:?} calls={calls} imports={imports}",
            entity.name,
            entity.payload.get("qualified_name")
        ));
    }
    for coverage in resolution_coverage(&entities) {
        lines.push(format!("C {coverage:?}"));
    }
    for relation in resolve_entities(&entities, &SnapshotId("snap_parity".to_string())) {
        lines.push(format!(
            "R {:?} payload={}",
            relation.kind, relation.payload
        ));
    }
    panic!("DUMP\n{}", lines.join("\n"));
}
