use anyhow::{Context, Result};
use athanor_core::{
    AffectedSubset, CheckInput, Checker, ExtractInput, Extractor, KnowledgeStore, LinkInput,
    Linker, SourceFile, SourceProvider,
};
use athanor_domain::{Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId};

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
        let files = self.discover_sources().await?;
        let snapshot = self
            .store
            .begin_snapshot(repo.clone(), base)
            .await
            .context("failed to begin snapshot")?;
        let (entities, facts) = self.extract(&repo, &snapshot, &files).await?;
        let affected_extracted = AffectedSubset::from_extracted(entities.clone(), facts.clone());
        let relations = self
            .link(&snapshot, &entities, &facts, &affected_extracted)
            .await?;
        let affected_checked = affected_extracted.with_relations(relations.clone());
        let diagnostics = self
            .check(&snapshot, &entities, &facts, &relations, &affected_checked)
            .await?;

        self.store
            .put_entities(snapshot.clone(), entities.clone())
            .await
            .context("failed to store entities")?;
        self.store
            .put_facts(snapshot.clone(), facts.clone())
            .await
            .context("failed to store facts")?;
        self.store
            .put_relations(snapshot.clone(), relations.clone())
            .await
            .context("failed to store relations")?;
        self.store
            .put_diagnostics(snapshot.clone(), diagnostics.clone())
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
            relations,
            diagnostics,
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

        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }

    async fn extract(
        &self,
        repo: &RepoId,
        snapshot: &SnapshotId,
        files: &[SourceFile],
    ) -> Result<(Vec<Entity>, Vec<Fact>)> {
        let mut entities = Vec::new();
        let mut facts = Vec::new();

        for file in files {
            for extractor in &self.extractors {
                if !extractor.supports(file) {
                    continue;
                }

                let output = extractor
                    .extract(ExtractInput {
                        repo: repo.clone(),
                        snapshot: snapshot.clone(),
                        source: file.clone(),
                    })
                    .await
                    .with_context(|| {
                        format!("extractor {} failed for {}", extractor.name(), file.path)
                    })?;

                entities.extend(output.entities);
                facts.extend(output.facts);
            }
        }

        Ok((entities, facts))
    }

    async fn link(
        &self,
        snapshot: &SnapshotId,
        entities: &[Entity],
        facts: &[Fact],
        affected: &AffectedSubset,
    ) -> Result<Vec<Relation>> {
        let mut relations = Vec::new();

        for linker in &self.linkers {
            relations.extend(
                linker
                    .link(LinkInput {
                        snapshot: snapshot.clone(),
                        entities: entities.to_vec(),
                        facts: facts.to_vec(),
                        affected: affected.clone(),
                    })
                    .await
                    .with_context(|| format!("linker {} failed", linker.name()))?,
            );
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
            diagnostics.extend(
                checker
                    .check(CheckInput {
                        snapshot: snapshot.clone(),
                        entities: entities.to_vec(),
                        facts: facts.to_vec(),
                        relations: relations.to_vec(),
                        affected: affected.clone(),
                    })
                    .await
                    .with_context(|| format!("checker {} failed", checker.name()))?,
            );
        }

        Ok(diagnostics)
    }
}
