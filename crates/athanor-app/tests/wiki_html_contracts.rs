use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    HTML_REPORT_SCHEMA_V1, HtmlReport, VersionedJsonContract, WIKI_REPORT_SCHEMA_V1, WikiReport,
};
use serde_json::Value;

#[test]
fn wiki_and_html_reports_match_golden_fixture() {
    let wiki = WikiReport {
        schema: WIKI_REPORT_SCHEMA_V1,
        root: PathBuf::from("project"),
        output_dir: PathBuf::from("project/.athanor/generated/current/wiki"),
        snapshot: "snap-wiki".to_string(),
        entities: 12,
        facts: 18,
        relations: 7,
        open_diagnostics: 2,
    };
    let html = HtmlReport {
        schema: HTML_REPORT_SCHEMA_V1,
        root: PathBuf::from("project"),
        output_dir: PathBuf::from("project/.athanor/generated/current/html"),
        snapshot: "snap-html".to_string(),
        entities: 12,
        facts: 18,
        relations: 7,
        open_diagnostics: 2,
    };

    wiki.validate_contract().expect("valid Wiki report contract");
    html.validate_contract().expect("valid HTML report contract");

    let fixture = read_fixture("wiki_html_contracts.v1.json");
    assert_eq!(serde_json::to_value(wiki).unwrap(), fixture["wiki"]);
    assert_eq!(serde_json::to_value(html).unwrap(), fixture["html"]);
}

fn read_fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    serde_json::from_str(
        &fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}
