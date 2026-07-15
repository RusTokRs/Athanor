use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{EntityKind, GenerationId};
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
    pub generation: String,
    pub files_indexed: usize,
    pub changed_files: usize,
    pub unchanged_files: usize,
    pub removed_files: usize,
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
}

/// A published read-model replacement whose previous generation remains recoverable until
/// [`Self::finalize`] is called.
#[derive(Debug)]
pub struct PreparedJsonlReadModel {
    report: JsonlReadModelReport,
    output_dir: PathBuf,
    backup: Option<PathBuf>,
}

impl PreparedJsonlReadModel {
    pub fn report(&self) -> &JsonlReadModelReport {
        &self.report
    }

    pub fn finalize(mut self) -> Result<JsonlReadModelReport> {
        if let Some(backup) = self.backup.take() {
            fs::remove_dir_all(&backup).with_context(|| {
                format!("failed to remove read model backup {}", backup.display())
            })?;
        }
        Ok(self.report)
    }

    pub fn rollback(mut self) -> Result<()> {
        if self.output_dir.exists() {
            fs::remove_dir_all(&self.output_dir).with_context(|| {
                format!(
                    "failed to remove unpublished read model {}",
                    self.output_dir.display()
                )
            })?;
        }
        if let Some(backup) = self.backup.take() {
            fs::rename(&backup, &self.output_dir).with_context(|| {
                format!("failed to restore read model backup {}", backup.display())
            })?;
        }
        Ok(())
    }
}

impl JsonlReadModelWriter {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    pub fn write(&self, output: &IndexPipelineOutput) -> Result<JsonlReadModelReport> {
        self.prepare(output)?.finalize()
    }

    pub fn prepare(&self, output: &IndexPipelineOutput) -> Result<PreparedJsonlReadModel> {
        self.prepare_with_publication_id(output, &publication_nonce())
    }

    pub fn prepare_with_publication_id(
        &self,
        output: &IndexPipelineOutput,
        publication_id: &str,
    ) -> Result<PreparedJsonlReadModel> {
        let generation = GenerationId::for_snapshot(&output.snapshot);
        let report = JsonlReadModelReport {
            output_dir: self.output_dir.clone(),
            snapshot: output.snapshot.0.clone(),
            generation: generation.0,
            files_indexed: output.files.len(),
            changed_files: output.affected_files.changed.len(),
            unchanged_files: output.affected_files.unchanged.len(),
            removed_files: output.affected_files.removed.len(),
            entities: output.entities.len(),
            facts: output.facts.len(),
            relations: output.relations.len(),
            diagnostics: output.diagnostics.len(),
        };

        let manifest_report = report.clone();
        self.publish(report, publication_id, move |staging| {
            write_jsonl(&staging.join("entities.jsonl"), &output.entities)?;
            write_jsonl(&staging.join("facts.jsonl"), &output.facts)?;
            write_jsonl(&staging.join("relations.jsonl"), &output.relations)?;
            write_jsonl(&staging.join("diagnostics.jsonl"), &output.diagnostics)?;
            write_manifest(&staging.join("manifest.json"), &manifest_report)
        })
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

        let files_indexed = entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::File)
            .count();
        let generation = GenerationId::for_snapshot(snapshot_id);
        let report = JsonlReadModelReport {
            output_dir: self.output_dir.clone(),
            snapshot: snapshot_id.0.clone(),
            generation: generation.0,
            files_indexed,
            changed_files: 0,
            unchanged_files: files_indexed,
            removed_files: 0,
            entities: entities.len(),
            facts: facts.len(),
            relations: relations.len(),
            diagnostics: diagnostics.len(),
        };
        let manifest_report = report.clone();
        self.publish(report, &publication_nonce(), move |staging| {
            write_jsonl(&staging.join("entities.jsonl"), &entities)?;
            write_jsonl(&staging.join("facts.jsonl"), &facts)?;
            write_jsonl(&staging.join("relations.jsonl"), &relations)?;
            write_jsonl(&staging.join("diagnostics.jsonl"), &diagnostics)?;
            write_manifest(&staging.join("manifest.json"), &manifest_report)
        })?
        .finalize()
    }

    /// Builds every read-model file in a sibling staging directory before replacing the current
    /// directory. Readers therefore see either the prior complete model or the new complete one.
    fn publish(
        &self,
        report: JsonlReadModelReport,
        publication_id: &str,
        write: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<PreparedJsonlReadModel> {
        let parent = self.output_dir.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "read model output has no parent: {}",
                self.output_dir.display()
            )
        })?;
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        let name = self
            .output_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                anyhow::anyhow!("invalid read model path: {}", self.output_dir.display())
            })?;
        let staging = parent.join(format!(".{name}.staging-{publication_id}"));
        let backup = parent.join(format!(".{name}.backup-{publication_id}"));

        if let Err(error) = write(&staging) {
            let _ = fs::remove_dir_all(&staging);
            return Err(error).context("failed to stage JSONL read model");
        }
        // The manifest is written last and is the staged completeness marker.
        if !staging.join("manifest.json").is_file() {
            let _ = fs::remove_dir_all(&staging);
            anyhow::bail!(
                "staged JSONL read model for {} has no manifest",
                report.snapshot
            );
        }

        if self.output_dir.exists() {
            fs::rename(&self.output_dir, &backup).with_context(|| {
                format!(
                    "failed to stage previous read model {}",
                    self.output_dir.display()
                )
            })?;
        }
        if let Err(error) = fs::rename(&staging, &self.output_dir) {
            if backup.exists() {
                let _ = fs::rename(&backup, &self.output_dir);
            }
            let _ = fs::remove_dir_all(&staging);
            return Err(error).with_context(|| {
                format!(
                    "failed to publish JSONL read model {}",
                    self.output_dir.display()
                )
            });
        }
        Ok(PreparedJsonlReadModel {
            report,
            output_dir: self.output_dir.clone(),
            backup: backup.exists().then_some(backup),
        })
    }
}

fn publication_nonce() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

fn write_jsonl<T: Serialize>(path: &Path, items: &[T]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, file);

    for item in items {
        serde_json::to_writer(&mut writer, item)
            .with_context(|| format!("failed to write JSON to {}", path.display()))?;
        writer
            .write_all(b"\n")
            .with_context(|| format!("failed to write newline to {}", path.display()))?;
    }
    writer
        .flush()
        .with_context(|| format!("failed to flush {}", path.display()))?;

    Ok(())
}

fn write_manifest(path: &Path, report: &JsonlReadModelReport) -> Result<()> {
    let manifest = json!({
        "schema": JSONL_MANIFEST_SCHEMA,
        "snapshot": report.snapshot,
        "generation": report.generation,
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
    fn writes_empty_jsonl_read_model_with_generation_identity() {
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
        assert_eq!(report.generation, "gen_snap_test");
        assert_eq!(report.files_indexed, 0);
        assert!(output_dir.join("entities.jsonl").is_file());
        assert!(output_dir.join("facts.jsonl").is_file());
        assert!(output_dir.join("relations.jsonl").is_file());
        assert!(output_dir.join("diagnostics.jsonl").is_file());

        let manifest = fs::read_to_string(output_dir.join("manifest.json")).unwrap();
        assert!(manifest.contains(JSONL_MANIFEST_SCHEMA));
        assert!(manifest.contains("\"snapshot\": \"snap_test\""));
        assert!(manifest.contains("\"generation\": \"gen_snap_test\""));

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
        assert_eq!(report.generation, "gen_snap_test");
        assert_eq!(report.files_indexed, 1);
        assert_eq!(report.unchanged_files, 1);
        assert!(
            fs::read_to_string(output_dir.join("entities.jsonl"))
                .unwrap()
                .contains("file://README.md")
        );
        fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn replaces_the_entire_previous_read_model_generation() {
        let output_dir = std::env::temp_dir().join(format!(
            "athanor-jsonl-read-model-replace-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("obsolete.jsonl"), "obsolete\n").unwrap();
        let output = IndexPipelineOutput {
            snapshot: SnapshotId("snap_replace".to_string()),
            files: Vec::new(),
            entities: Vec::new(),
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
            affected_files: crate::AffectedFileSet::default(),
            metrics: crate::IndexPipelineMetrics::default(),
        };

        JsonlReadModelWriter::new(&output_dir)
            .write(&output)
            .unwrap();

        assert!(!output_dir.join("obsolete.jsonl").exists());
        assert!(output_dir.join("manifest.json").is_file());
        fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn prepared_publication_rolls_back_to_the_previous_generation() {
        let output_dir = std::env::temp_dir().join(format!(
            "athanor-jsonl-read-model-rollback-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(
            output_dir.join("manifest.json"),
            r#"{"snapshot":"snap_old"}"#,
        )
        .unwrap();
        let output = IndexPipelineOutput {
            snapshot: SnapshotId("snap_new".to_string()),
            files: Vec::new(),
            entities: Vec::new(),
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
            affected_files: crate::AffectedFileSet::default(),
            metrics: crate::IndexPipelineMetrics::default(),
        };

        JsonlReadModelWriter::new(&output_dir)
            .prepare(&output)
            .unwrap()
            .rollback()
            .unwrap();

        assert!(
            fs::read_to_string(output_dir.join("manifest.json"))
                .unwrap()
                .contains("snap_old")
        );
        fs::remove_dir_all(output_dir).unwrap();
    }
}
