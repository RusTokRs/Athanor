use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    BenchmarkReport, BenchmarkSize, CHANGED_VALIDATION_SCHEMA_V1, ChangedValidationReport,
    INDEX_BENCHMARK_SCHEMA_V1, INDEX_METRICS_SCHEMA, INDEX_REPORT_METRICS_SCHEMA,
    INDEX_REPORT_SCHEMA_V1, IndexPipelineMetrics, IndexReport, IndexReportMetrics,
    VersionedJsonContract,
};
use serde_json::Value;

#[test]
fn application_output_contracts_match_golden_fixture() {
    let benchmark = BenchmarkReport {
        schema: INDEX_BENCHMARK_SCHEMA_V1,
        size: BenchmarkSize::Small,
        fixture_root: PathBuf::from("fixture"),
        kept_fixture: true,
        files_written: 3,
        total_ms: 50,
        index: IndexReport {
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
                total_ms: 40,
                pipeline: pipeline_metrics(),
                read_model_write_ms: 5,
                validation_result_write_ms: 0,
                index_state_write_ms: 5,
            },
        },
    };
    let changed_validation = ChangedValidationReport {
        schema: CHANGED_VALIDATION_SCHEMA_V1,
        root: PathBuf::from("project"),
        snapshot: "snap-validation".to_string(),
        files_checked: 2,
        changed_files: 2,
        removed_files: 1,
        diagnostics: Vec::new(),
        metrics: pipeline_metrics(),
    };

    benchmark
        .index
        .validate_contract()
        .expect("valid index report contract");
    benchmark
        .validate_contract()
        .expect("valid benchmark contract");
    changed_validation
        .validate_contract()
        .expect("valid changed validation contract");

    let fixture = read_fixture("application_output_contracts.v1.json");
    assert_eq!(benchmark.index.schema(), INDEX_REPORT_SCHEMA_V1);
    assert_eq!(
        serde_json::to_value(benchmark).unwrap(),
        fixture["benchmark"]
    );
    assert_eq!(
        serde_json::to_value(changed_validation).unwrap(),
        fixture["changed_validation"]
    );
}

fn pipeline_metrics() -> IndexPipelineMetrics {
    let mut extraction_concurrency_by_adapter = BTreeMap::new();
    extraction_concurrency_by_adapter.insert("rust".to_string(), 1);

    IndexPipelineMetrics {
        schema: INDEX_METRICS_SCHEMA,
        total_ms: 30,
        source_discovery_ms: 1,
        affected_classification_ms: 2,
        snapshot_begin_ms: 3,
        extraction_ms: 4,
        merge_ms: 5,
        linking_ms: 6,
        checking_ms: 7,
        canonicalize_ms: 8,
        storage_ms: 9,
        files_discovered: 3,
        files_to_extract: 2,
        extraction_concurrency: 2,
        max_extraction_bytes_in_flight: 1024,
        max_snapshot_batch_objects: 100,
        max_snapshot_batch_bytes: 2048,
        extraction_concurrency_by_adapter,
        changed_files: 2,
        unchanged_files: 1,
        removed_files: 0,
        invalidation_scope: Some("incremental".to_string()),
        invalidation_global_adapters: Vec::new(),
        entities: 4,
        facts: 5,
        relations: 6,
        diagnostics: 1,
        validation_issues: 0,
        adapters: Vec::new(),
    }
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
