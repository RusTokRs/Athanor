use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshot;
use athanor_domain::EntityKind;
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

    pub fn write_canonical_snapshot(
        &self,
        snapshot: &CanonicalSnapshot,
    ) -> Result<JsonlReadModelReport> {
        let snapshot_id = snapshot
            .snapshot
            .as_ref()
            .context("canonical snapshot has no snapshot id")?;
        let mut entities = snapshot.entities.clone();
        let mut facts = snapshot.facts.clone();
        let mut relations = snapshot.relations.clone();
        let mut diagnostics = snapshot.diagnostics.clone();
        entities.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        facts.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));

        write_jsonl(&self.output_dir.join("entities.jsonl"), &entities)?;
        write_jsonl(&self.output_dir.join("facts.jsonl"), &facts)?;
        write_jsonl(&self.output_dir.join("relations.jsonl"), &relations)?;
        write_jsonl(&self.output_dir.join("diagnostics.jsonl"), &diagnostics)?;

        let files_indexed = entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::File)
            .count();
        let report = JsonlReadModelReport {
            output_dir: self.output_dir.clone(),
            snapshot: snapshot_id.0.clone(),
            files_indexed,
            changed_files: 0,
            unchanged_files: files_indexed,
            removed_files: 0,
            entities: entities.len(),
            facts: facts.len(),
            relations: relations.len(),
            diagnostics: diagnostics.len(),
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
    use athanor_domain::{Entity, EntityId, SnapshotId, StableKey};

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
            metrics: crate::IndexPipelineMetrics::default(),
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

    #[test]
    fn writes_loaded_canonical_snapshot_deterministically() {
        let output_dir = std::env::temp_dir().join(format!(
            "athanor-canonical-jsonl-read-model-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![Entity {
                id: EntityId("ent_file".to_string()),
                stable_key: StableKey("file://README.md".to_string()),
                kind: EntityKind::File,
                name: "README.md".to_string(),
                title: None,
                source: None,
                language: None,
                aliases: Vec::new(),
                ownership: Vec::new(),
                payload: json!({}),
            }],
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };

        let report = JsonlReadModelWriter::new(&output_dir)
            .write_canonical_snapshot(&snapshot)
            .unwrap();

        assert_eq!(report.snapshot, "snap_test");
        assert_eq!(report.files_indexed, 1);
        assert_eq!(report.unchanged_files, 1);
        assert!(
            fs::read_to_string(output_dir.join("entities.jsonl"))
                .unwrap()
                .contains("file://README.md")
        );
        fs::remove_dir_all(output_dir).unwrap();
    }
}
