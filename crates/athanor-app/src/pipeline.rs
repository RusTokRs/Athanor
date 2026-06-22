use std::collections::{BTreeSet, HashSet};
use std::fmt;

use anyhow::{Context, Result};
use athanor_core::{
    AffectedSubset, CanonicalSnapshot, CheckInput, Checker, ExtractInput, Extractor,
    KnowledgeStore, LinkInput, Linker, SourceFile, SourceProvider,
};
use athanor_domain::{Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId};
use serde::Serialize;
use serde_json::Value;

use crate::{AffectedFileSet, IndexState};
use futures::stream::{self, StreamExt};

pub struct IndexPipeline {
    store: Box<dyn KnowledgeStore>,
    sources: Vec<Box<dyn SourceProvider>>,
    extractors: Vec<Box<dyn Extractor>>,
    linkers: Vec<Box<dyn Linker>>,
    checkers: Vec<Box<dyn Checker>>,
}

#[derive(Debug, Clone)]
pub struct IndexPipelineOutput {
    pub snapshot: SnapshotId,
    pub files: Vec<SourceFile>,
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
    pub relations: Vec<Relation>,
    pub diagnostics: Vec<Diagnostic>,
    pub affected_files: AffectedFileSet,
}

#[derive(Debug, Clone, Default)]
pub struct IncrementalIndexContext {
    pub previous_state: IndexState,
    pub previous_snapshot: Option<CanonicalSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterValidationReport {
    pub adapter: String,
    pub issues: Vec<AdapterValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterValidationIssue {
    pub object_type: &'static str,
    pub object_id: String,
    pub missing: MissingCanonicalMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingCanonicalMetadata {
    Evidence,
    Ownership,
}

impl AdapterValidationReport {
    fn new(adapter: &str) -> Self {
        Self {
            adapter: adapter.to_string(),
            issues: Vec::new(),
        }
    }

    fn push(
        &mut self,
        object_type: &'static str,
        object_id: impl Into<String>,
        missing: MissingCanonicalMetadata,
    ) {
        self.issues.push(AdapterValidationIssue {
            object_type,
            object_id: object_id.into(),
            missing,
        });
    }

    fn finish(self) -> Result<()> {
        if self.issues.is_empty() {
            Ok(())
        } else {
            Err(anyhow::Error::new(self))
        }
    }
}

impl fmt::Display for AdapterValidationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "adapter {} emitted invalid canonical output",
            self.adapter
        )?;

        if self.issues.is_empty() {
            return Ok(());
        }

        write!(formatter, " ({} issue", self.issues.len())?;

        if self.issues.len() != 1 {
            write!(formatter, "s")?;
        }

        write!(formatter, ")")?;

        for issue in &self.issues {
            write!(
                formatter,
                "; {} {} missing {}",
                issue.object_type, issue.object_id, issue.missing
            )?;
        }

        Ok(())
    }
}

impl std::error::Error for AdapterValidationReport {}

impl fmt::Display for MissingCanonicalMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MissingCanonicalMetadata::Evidence => formatter.write_str("evidence"),
            MissingCanonicalMetadata::Ownership => formatter.write_str("ownership"),
        }
    }
}

impl IndexPipeline {
    pub fn new(store: impl KnowledgeStore + 'static) -> Self {
        Self {
            store: Box::new(store),
            sources: Vec::new(),
            extractors: Vec::new(),
            linkers: Vec::new(),
            checkers: Vec::new(),
        }
    }

    pub fn boxed_source(mut self, source: Box<dyn SourceProvider>) -> Self {
        self.sources.push(source);
        self
    }

    pub fn source(mut self, source: impl SourceProvider + 'static) -> Self {
        self.sources.push(Box::new(source));
        self
    }

    pub fn boxed_extractor(mut self, extractor: Box<dyn Extractor>) -> Self {
        self.extractors.push(extractor);
        self
    }

    pub fn extractor(mut self, extractor: impl Extractor + 'static) -> Self {
        self.extractors.push(Box::new(extractor));
        self
    }

    pub fn boxed_linker(mut self, linker: Box<dyn Linker>) -> Self {
        self.linkers.push(linker);
        self
    }

    pub fn linker(mut self, linker: impl Linker + 'static) -> Self {
        self.linkers.push(Box::new(linker));
        self
    }

    pub fn boxed_checker(mut self, checker: Box<dyn Checker>) -> Self {
        self.checkers.push(checker);
        self
    }

    pub fn checker(mut self, checker: impl Checker + 'static) -> Self {
        self.checkers.push(Box::new(checker));
        self
    }

    pub async fn run(self, repo: RepoId, base: SnapshotBase) -> Result<IndexPipelineOutput> {
        self.run_with_incremental(repo, base, IncrementalIndexContext::default())
            .await
    }

    pub async fn run_with_incremental(
        self,
        repo: RepoId,
        base: SnapshotBase,
        incremental: IncrementalIndexContext,
    ) -> Result<IndexPipelineOutput> {
        let files = self.discover_sources().await?;
        let mut affected_files = incremental.previous_state.affected_files(&files);

        let has_added_files = affected_files
            .changed
            .iter()
            .any(|path| !incremental.previous_state.files.contains_key(path));
        if has_added_files || !affected_files.removed.is_empty() {
            affected_files.changed = files.iter().map(|file| file.path.clone()).collect();
            affected_files.unchanged.clear();
        }

        if incremental.previous_snapshot.is_none() {
            affected_files.changed = files.iter().map(|file| file.path.clone()).collect();
            affected_files.unchanged.clear();
        }

        let snapshot = self
            .store
            .begin_snapshot(repo.clone(), base)
            .await
            .context("failed to begin snapshot")?;
        let files_to_extract = files
            .iter()
            .filter(|file| affected_files.changed.contains(&file.path))
            .cloned()
            .collect::<Vec<_>>();
        let (extracted_entities, extracted_facts) =
            self.extract(&repo, &snapshot, &files_to_extract).await?;
        let affected_extracted =
            AffectedSubset::from_extracted(extracted_entities.clone(), extracted_facts.clone());
        let (mut entities, mut facts, mut prior_relations, mut prior_diagnostics) =
            carried_forward_read_model(
                incremental.previous_snapshot,
                &snapshot,
                &affected_files.changed,
                &affected_files.removed,
            );
        entities.extend(extracted_entities);
        facts.extend(extracted_facts);

        let relations = self
            .link(&snapshot, &entities, &facts, &affected_extracted)
            .await?;
        let mut all_relations_for_check = prior_relations.clone();
        all_relations_for_check.extend(relations.clone());
        let affected_checked = affected_extracted.with_relations(relations.clone());
        let diagnostics = self
            .check(
                &snapshot,
                &entities,
                &facts,
                &all_relations_for_check,
                &affected_checked,
            )
            .await?;
        prior_relations.extend(relations);
        prior_diagnostics.extend(diagnostics);

        self.store
            .put_entities(snapshot.clone(), entities.clone())
            .await
            .context("failed to store entities")?;
        self.store
            .put_facts(snapshot.clone(), facts.clone())
            .await
            .context("failed to store facts")?;
        self.store
            .put_relations(snapshot.clone(), prior_relations.clone())
            .await
            .context("failed to store relations")?;
        self.store
            .put_diagnostics(snapshot.clone(), prior_diagnostics.clone())
            .await
            .context("failed to store diagnostics")?;
        self.store
            .commit_snapshot(snapshot.clone())
            .await
            .context("failed to commit snapshot")?;

        Ok(IndexPipelineOutput {
            snapshot,
            files,
            entities,
            facts,
            relations: prior_relations,
            diagnostics: prior_diagnostics,
            affected_files,
        })
    }

    async fn discover_sources(&self) -> Result<Vec<SourceFile>> {
        let mut files = Vec::new();

        for source in &self.sources {
            files.extend(
                source
                    .discover()
                    .await
                    .with_context(|| format!("source {} failed", source.name()))?,
            );
        }
        entities: &[Entity],
        facts: &[Fact],
        affected: &AffectedSubset,
    ) -> Result<Vec<Relation>> {
        let mut relations = Vec::new();

        for linker in &self.linkers {
            let output = linker
                .link(LinkInput {
                    snapshot: snapshot.clone(),
                    entities: entities.to_vec(),
                    facts: facts.to_vec(),
                    affected: affected.clone(),
                })
                .await
                .with_context(|| format!("linker {} failed", linker.name()))?;

            validate_relations(linker.name(), &output)?;
            relations.extend(output);
        }

        Ok(relations)
    }

    async fn check(
        &self,
        snapshot: &SnapshotId,
        entities: &[Entity],
        facts: &[Fact],
        relations: &[Relation],
        affected: &AffectedSubset,
    ) -> Result<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        for checker in &self.checkers {
            let output = checker
                .check(CheckInput {
                    snapshot: snapshot.clone(),
                    entities: entities.to_vec(),
                    facts: facts.to_vec(),
                    relations: relations.to_vec(),
                    affected: affected.clone(),
                })
                .await
                .with_context(|| format!("checker {} failed", checker.name()))?;

            validate_diagnostics(checker.name(), &output)?;
            diagnostics.extend(output);
        }

        Ok(diagnostics)
    }
}

fn validate_entities(adapter: &str, entities: &[Entity]) -> Result<()> {
    let mut report = AdapterValidationReport::new(adapter);

    for entity in entities {
        if entity.ownership.is_empty() {
            report.push(
                "entity",
                entity.id.0.clone(),
                MissingCanonicalMetadata::Ownership,
            );
        }
    }

    report.finish()
}

fn validate_facts(adapter: &str, facts: &[Fact]) -> Result<()> {
    let mut report = AdapterValidationReport::new(adapter);

    for fact in facts {
        if fact.evidence.is_empty() {
            report.push(
                "fact",
                fact.id.0.clone(),
                MissingCanonicalMetadata::Evidence,
            );
        }

        if fact.ownership.is_empty() {
            report.push(
                "fact",
                fact.id.0.clone(),
                MissingCanonicalMetadata::Ownership,
            );
        }
    }

    report.finish()
}

fn validate_relations(adapter: &str, relations: &[Relation]) -> Result<()> {
    let mut report = AdapterValidationReport::new(adapter);

    for relation in relations {
        if relation.evidence.is_empty() {
            report.push(
                "relation",
                relation.id.0.clone(),
                MissingCanonicalMetadata::Evidence,
            );
        }

        if relation.ownership.is_empty() {
            report.push(
                "relation",
                relation.id.0.clone(),
                MissingCanonicalMetadata::Ownership,
            );
        }
    }

    report.finish()
}

fn validate_diagnostics(adapter: &str, diagnostics: &[Diagnostic]) -> Result<()> {
    let mut report = AdapterValidationReport::new(adapter);

    for diagnostic in diagnostics {
        if diagnostic.evidence.is_empty() {
            report.push(
                "diagnostic",
                diagnostic.id.0.clone(),
                MissingCanonicalMetadata::Evidence,
            );
        }

        if diagnostic.ownership.is_empty() {
            report.push(
                "diagnostic",
                diagnostic.id.0.clone(),
                MissingCanonicalMetadata::Ownership,
            );
        }
    }

    report.finish()
}

fn carried_forward_read_model(
    previous: Option<CanonicalSnapshot>,
    snapshot: &SnapshotId,
    changed_paths: &BTreeSet<String>,
    removed_paths: &BTreeSet<String>,
) -> (Vec<Entity>, Vec<Fact>, Vec<Relation>, Vec<Diagnostic>) {
    let Some(previous) = previous else {
        return (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    };

    let affected_paths = changed_paths
        .iter()
        .chain(removed_paths.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut entities = previous
        .entities
        .into_iter()
        .filter(|entity| !entity_owned_by_any_path(entity, &affected_paths))
        .map(|mut entity| {
            replace_payload_snapshot(&mut entity.payload, snapshot);
            entity
        })
        .collect::<Vec<_>>();
    let entity_ids = entities
        .iter()
        .map(|entity| entity.id.clone())
        .collect::<HashSet<_>>();
    let facts = previous
        .facts
        .into_iter()
        .filter(|fact| {
            entity_ids.contains(&fact.subject)
                && fact
                    .object
                    .as_ref()
                    .is_none_or(|object| entity_ids.contains(object))
                && !fact_owned_by_any_path(fact, &affected_paths)
        })
        .map(|mut fact| {
            fact.snapshot = snapshot.clone();
            fact
        })
        .collect::<Vec<_>>();
    let relations = previous
        .relations
        .into_iter()
        .filter(|relation| {
            entity_ids.contains(&relation.from)
                && entity_ids.contains(&relation.to)
                && !relation_owned_by_any_path(relation, &affected_paths)
        })
        .map(|mut relation| {
            relation.snapshot = snapshot.clone();
            relation
        })
        .collect::<Vec<_>>();
    let diagnostics = previous
        .diagnostics
        .into_iter()
        .filter(|diagnostic| {
            diagnostic
                .entities
                .iter()
                .all(|entity| entity_ids.contains(entity))
                && !diagnostic_owned_by_any_path(diagnostic, &affected_paths)
        })
        .map(|mut diagnostic| {
            diagnostic.snapshot = snapshot.clone();
            diagnostic
        })
        .collect::<Vec<_>>();

    entities.sort_by(|left, right| left.id.0.cmp(&right.id.0));

    (entities, facts, relations, diagnostics)
}

fn entity_owned_by_any_path(entity: &Entity, paths: &BTreeSet<String>) -> bool {
    ownership_belongs_to_any_path(&entity.ownership, paths)
        || belongs_to_any_path(
            entity.source.as_ref().map(|source| source.path.as_str()),
            paths,
        )
}

fn fact_owned_by_any_path(fact: &Fact, paths: &BTreeSet<String>) -> bool {
    ownership_belongs_to_any_path(&fact.ownership, paths)
        || evidence_belongs_to_any_path(&fact.evidence, paths)
}

fn relation_owned_by_any_path(relation: &Relation, paths: &BTreeSet<String>) -> bool {
    ownership_belongs_to_any_path(&relation.ownership, paths)
        || evidence_belongs_to_any_path(&relation.evidence, paths)
}

fn diagnostic_owned_by_any_path(diagnostic: &Diagnostic, paths: &BTreeSet<String>) -> bool {
    ownership_belongs_to_any_path(&diagnostic.ownership, paths)
        || evidence_belongs_to_any_path(&diagnostic.evidence, paths)
}

fn ownership_belongs_to_any_path(
    ownership: &[athanor_domain::Ownership],
    paths: &BTreeSet<String>,
) -> bool {
    ownership
        .iter()
        .any(|owner| paths.contains(&owner.source_file))
}

fn evidence_belongs_to_any_path(
    evidence: &[athanor_domain::Evidence],
    paths: &BTreeSet<String>,
) -> bool {
    evidence
        .iter()
        .any(|evidence| belongs_to_any_path(evidence.source_file.as_deref(), paths))
}

fn belongs_to_any_path(path: Option<&str>, paths: &BTreeSet<String>) -> bool {
    path.is_some_and(|path| paths.contains(path))
}

fn replace_payload_snapshot(value: &mut Value, snapshot: &SnapshotId) {
    if let Value::Object(object) = value
        && object.contains_key("snapshot")
    {
        object.insert("snapshot".to_string(), Value::String(snapshot.0.clone()));
    }
}

#[cfg(test)]
mod tests {
    use athanor_core::CanonicalSnapshot;
    use athanor_domain::{
        EntityId, EntityKind, Evidence, EvidenceStatus, FactId, FactKind, Ownership, RelationId,
        RelationKind, RelationStatus, SourceLocation, StableKey,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn prunes_relation_when_any_owned_source_changes() {
        let snapshot = SnapshotId("snap_next".to_string());
        let left = entity("ent_left", "docs/left.md");
        let right = entity("ent_right", "docs/right.md");
        let relation = Relation {
            id: RelationId("rel_cross_file".to_string()),
            kind: RelationKind::Documents,
            from: left.id.clone(),
            to: right.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: vec![ownership("docs/left.md"), ownership("docs/right.md")],
            snapshot: SnapshotId("snap_previous".to_string()),
            payload: json!({}),
        };

        let (_, _, relations, _) = carried_forward_read_model(
            Some(CanonicalSnapshot {
                snapshot: Some(SnapshotId("snap_previous".to_string())),
                entities: vec![left, right],
                facts: Vec::new(),
                relations: vec![relation],
                diagnostics: Vec::new(),
            }),
            &snapshot,
            &BTreeSet::from(["docs/right.md".to_string()]),
            &BTreeSet::new(),
        );

        assert!(relations.is_empty());
    }

    #[test]
    fn rejects_fact_without_ownership() {
        let mut fact = fact("fact_without_ownership");
        fact.ownership.clear();

        let error = validate_facts("test-extractor", &[fact]).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("fact fact_without_ownership missing ownership")
        );
    }

    #[test]
    fn rejects_relation_without_evidence() {
        let mut relation = relation("rel_without_evidence", "ent_left", "ent_right");
        relation.evidence.clear();

        let error = validate_relations("test-linker", &[relation]).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("relation rel_without_evidence missing evidence")
        );
    }

    #[test]
    fn reports_multiple_validation_issues() {
        let mut first = relation("rel_missing_both", "ent_left", "ent_right");
        first.evidence.clear();
        first.ownership.clear();
        let mut second = relation("rel_missing_ownership", "ent_left", "ent_right");
        second.ownership.clear();

        let error = validate_relations("test-linker", &[first, second]).unwrap_err();
        let message = error.to_string();

        assert!(
            message.contains("adapter test-linker emitted invalid canonical output (3 issues)")
        );
        assert!(message.contains("relation rel_missing_both missing evidence"));
        assert!(message.contains("relation rel_missing_both missing ownership"));
        assert!(message.contains("relation rel_missing_ownership missing ownership"));
    }

    fn entity(id: &str, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("file://{path}")),
            kind: EntityKind::File,
            name: path.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: None,
                line_end: None,
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![ownership(path)],
            payload: json!({}),
        }
    }

    fn fact(id: &str) -> Fact {
        Fact {
            id: FactId(id.to_string()),
            kind: FactKind::FileDiscovered,
            subject: EntityId("ent_left".to_string()),
            object: None,
            value: json!({}),
            evidence: vec![evidence("docs/left.md")],
            ownership: vec![ownership("docs/left.md")],
            snapshot: SnapshotId("snap_test".to_string()),
            extractor: "test".to_string(),
            confidence: 1.0,
        }
    }

    fn relation(id: &str, from: &str, to: &str) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind: RelationKind::Documents,
            from: EntityId(from.to_string()),
            to: EntityId(to.to_string()),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: vec![evidence("docs/left.md")],
            ownership: vec![ownership("docs/left.md")],
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        }
    }

    fn evidence(path: &str) -> Evidence {
        Evidence {
            source_file: Some(path.to_string()),
            line_start: None,
            line_end: None,
            extractor: Some("test".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Verified,
        }
    }

    fn ownership(path: &str) -> Ownership {
        Ownership {
            source_file: path.to_string(),
        }
    }
}
