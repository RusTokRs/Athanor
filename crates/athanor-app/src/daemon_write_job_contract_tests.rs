use std::path::PathBuf;

use crate::{
    GENERATION_SCHEMA_V1, HTML_REPORT_SCHEMA_V1, INDEX_METRICS_SCHEMA,
    INDEX_REPORT_METRICS_SCHEMA, INDEX_REPORT_SCHEMA, WIKI_REPORT_SCHEMA_V1, GenerationMetrics,
    GenerationReport, GenerationStatus, HtmlReport, IndexPipelineMetrics, IndexReport,
    IndexReportMetrics, WikiReport,
};

#[test]
fn daemon_index_result_matches_public_index_report_shape() {
    let report = IndexReport {
        root: PathBuf::from("project"),
        snapshot: "snap-index".to_string(),
        files_indexed: 3,
        output_dir: PathBuf::from("jsonl"),
        changed_files: 2,
        unchanged_files: 1,
        removed_files: 0,
        validation_report: PathBuf::from("validation-report.json"),
        validation_result: None,
        validate_only: false,
        metrics: IndexReportMetrics {
            schema: INDEX_REPORT_METRICS_SCHEMA,
            total_ms: 10,
            pipeline: IndexPipelineMetrics {
                schema: INDEX_METRICS_SCHEMA,
                ..IndexPipelineMetrics::default()
            },
            read_model_write_ms: 2,
            validation_result_write_ms: 0,
            index_state_write_ms: 1,
        },
    };

    let direct = serde_json::to_value(&report).expect("serialize direct IndexReport");
    let daemon = crate::daemon_write_jobs::index_job_result(&report)
        .expect("serialize daemon IndexReport result");

    assert_eq!(daemon, direct);
    assert_eq!(daemon["schema"], INDEX_REPORT_SCHEMA);
    assert_eq!(daemon["validation_report"], "validation-report.json");
    assert_eq!(daemon["validate_only"], false);
    assert!(daemon.get("metrics").is_some());
}

#[test]
fn daemon_generation_result_matches_public_generation_report_shape() {
    let report = GenerationReport {
        schema: GENERATION_SCHEMA_V1,
        status: GenerationStatus::Published,
        root: PathBuf::from("project"),
        generation: "00000007".to_string(),
        generation_dir: PathBuf::from(".athanor/generated/generations/00000007"),
        current_pointer: PathBuf::from(".athanor/generated/current.json"),
        snapshot: "snap-generation".to_string(),
        entities: 12,
        facts: 18,
        relations: 7,
        diagnostics: 2,
        metrics: GenerationMetrics {
            schema: "athanor.generation_metrics.v1",
            total_ms: 50,
            snapshot_load_ms: 5,
            jsonl_ms: 10,
            wiki_ms: 11,
            html_ms: 12,
            publish_ms: 12,
        },
    };

    let direct = serde_json::to_value(&report).expect("serialize direct GenerationReport");
    let daemon = crate::daemon_write_jobs::generation_job_result(&report)
        .expect("serialize daemon GenerationReport result");

    assert_eq!(daemon, direct);
    assert_eq!(daemon["schema"], GENERATION_SCHEMA_V1);
    assert_eq!(daemon["status"], "published");
    assert_eq!(daemon["root"], "project");
    assert!(daemon.get("metrics").is_some());
}

#[test]
fn daemon_html_result_matches_public_html_report_shape() {
    let report = HtmlReport {
        schema: HTML_REPORT_SCHEMA_V1,
        root: PathBuf::from("project"),
        output_dir: PathBuf::from("project/.athanor/generated/current/html"),
        snapshot: "snap-html".to_string(),
        entities: 12,
        facts: 18,
        relations: 7,
        open_diagnostics: 2,
    };

    let direct = serde_json::to_value(&report).expect("serialize direct HtmlReport");
    let daemon = crate::daemon_write_jobs::html_report_job_result(&report)
        .expect("serialize daemon HtmlReport result");

    assert_eq!(daemon, direct);
    assert_eq!(daemon["schema"], HTML_REPORT_SCHEMA_V1);
    assert_eq!(daemon["root"], "project");
}

#[test]
fn daemon_wiki_result_matches_public_wiki_report_shape() {
    let report = WikiReport {
        schema: WIKI_REPORT_SCHEMA_V1,
        root: PathBuf::from("project"),
        output_dir: PathBuf::from("project/.athanor/generated/current/wiki"),
        snapshot: "snap-wiki".to_string(),
        entities: 12,
        facts: 18,
        relations: 7,
        open_diagnostics: 2,
    };

    let direct = serde_json::to_value(&report).expect("serialize direct WikiReport");
    let daemon = crate::daemon_write_jobs::wiki_job_result(&report)
        .expect("serialize daemon WikiReport result");

    assert_eq!(daemon, direct);
    assert_eq!(daemon["schema"], WIKI_REPORT_SCHEMA_V1);
    assert_eq!(daemon["root"], "project");
}
