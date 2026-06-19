use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;

use crate::IndexPipelineOutput;

pub const JSONL_MANIFEST_SCHEMA: &str = "athanor.jsonl_manifest.v1";

#[derive(Debug, Clone)]
pub struct JsonlReadModelWriter {
    output_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct JsonlReadModelReport {
    pub output_dir: PathBuf,
    pub snapshot: String,
    pub files_indexed: usize,
    pub changed_files: usize,
    pub unchanged_files: usize,
    pub removed_files: usize,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
}

impl JsonlReadModelWriter {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    pub fn write(&self, output: &IndexPipelineOutput) -> Result<JsonlReadModelReport> {
        write_jsonl(&self.output_dir.join("entities.jsonl"), &output.entities)?;
        write_jsonl(&self.output_dir.join("facts.jsonl"), &output.facts)?;
        write_jsonl(&self.output_dir.join("relations.jsonl"), &output.relations)?;
        write_jsonl(
            &self.output_dir.join("diagnostics.jsonl"),
            &output.diagnostics,
        )?;

        let report = JsonlReadModelReport {
            output_dir: self.output_dir.clone(),
            snapshot: output.snapshot.0.clone(),
            files_indexed: output.files.len(),
            changed_files: output.affected_files.changed.len(),
            unchanged_files: output.affected_files.unchanged.len(),
            removed_files: output.affected_files.removed.len(),
            entities: output.entities.len(),
            facts: output.facts.len(),
            relations: output.relations.len(),
            diagnostics: output.diagnostics.len(),
        };

        write_manifest(&self.output_dir.join("manifest.json"), &report)?;

        Ok(report)
    }
}

fn write_jsonl<T: Serialize>(path: &Path, items: &[T]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;

    for item in items {
        serde_json::to_writer(&mut file, item)
            .with_context(|| format!("failed to write JSON to {}", path.display()))?;
        file.write_all(b"\n")
            .with_context(|| format!("failed to write newline to {}", path.display()))?;
    }

    Ok(())
}

fn write_manifest(path: &Path, report: &JsonlReadModelReport) -> Result<()> {
    let manifest = json!({
        "schema": JSONL_MANIFEST_SCHEMA,
        "snapshot": report.snapshot,
        "files_indexed": report.files_indexed,
        "changed_files": report.changed_files,
        "unchanged_files": report.unchanged_files,
        "removed_files": report.removed_files,
        "entities": report.entities,
        "facts": report.facts,
        "relations": report.relations,
        "diagnostics": report.diagnostics,
    });

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(path, serde_json::to_string_pretty(&manifest)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use athanor_domain::SnapshotId;

    use super::*;
    use crate::IndexPipelineOutput;

    #[test]
    fn writes_empty_jsonl_read_model() {
        let output_dir = std::env::temp_dir().join(format!(
            "athanor-jsonl-read-model-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let output = IndexPipelineOutput {
            snapshot: SnapshotId("snap_test".to_string()),
            files: Vec::new(),
            entities: Vec::new(),
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
            affected_files: crate::AffectedFileSet::default(),
        };

        let report = JsonlReadModelWriter::new(&output_dir)
            .write(&output)
            .unwrap();

        assert_eq!(report.snapshot, "snap_test");
        assert_eq!(report.files_indexed, 0);
        assert!(output_dir.join("entities.jsonl").is_file());
        assert!(output_dir.join("facts.jsonl").is_file());
        assert!(output_dir.join("relations.jsonl").is_file());
        assert!(output_dir.join("diagnostics.jsonl").is_file());

        let manifest = fs::read_to_string(output_dir.join("manifest.json")).unwrap();
        assert!(manifest.contains(JSONL_MANIFEST_SCHEMA));
        assert!(manifest.contains("\"snapshot\": \"snap_test\""));

        fs::remove_dir_all(output_dir).unwrap();
    }
}
