use std::path::{Path, PathBuf};

use athanor_checker_markdown::MarkdownStructureChecker;
use athanor_core::{Checker, Extractor, KnowledgeStore, Linker, SourceProvider};
use athanor_extractor_basic::FileExtractor;
use athanor_extractor_markdown::MarkdownExtractor;
use athanor_linker_markdown::MarkdownContainmentLinker;
use athanor_source_fs::LocalFileSystemSource;

use crate::IndexPipeline;

type SourceFactory = Box<dyn Fn(&Path) -> Box<dyn SourceProvider> + Send + Sync>;
type ExtractorFactory = Box<dyn Fn() -> Box<dyn Extractor> + Send + Sync>;
type LinkerFactory = Box<dyn Fn() -> Box<dyn Linker> + Send + Sync>;
type CheckerFactory = Box<dyn Fn() -> Box<dyn Checker> + Send + Sync>;

pub struct AdapterRegistry {
    source_factories: Vec<SourceFactory>,
    extractor_factories: Vec<ExtractorFactory>,
    linker_factories: Vec<LinkerFactory>,
    checker_factories: Vec<CheckerFactory>,
}

impl AdapterRegistry {
    pub fn empty() -> Self {
        Self {
            source_factories: Vec::new(),
            extractor_factories: Vec::new(),
            linker_factories: Vec::new(),
            checker_factories: Vec::new(),
        }
    }

    pub fn built_in() -> Self {
        Self::empty()
            .source(|root| Box::new(LocalFileSystemSource::new(root)))
            .extractor(|| Box::new(FileExtractor))
            .extractor(|| Box::new(MarkdownExtractor))
            .linker(|| Box::new(MarkdownContainmentLinker))
            .checker(|| Box::new(MarkdownStructureChecker))
    }

    pub fn source(
        mut self,
        factory: impl Fn(&Path) -> Box<dyn SourceProvider> + Send + Sync + 'static,
    ) -> Self {
        self.source_factories.push(Box::new(factory));
        self
    }

    pub fn extractor(
        mut self,
        factory: impl Fn() -> Box<dyn Extractor> + Send + Sync + 'static,
    ) -> Self {
        self.extractor_factories.push(Box::new(factory));
        self
    }

    pub fn linker(mut self, factory: impl Fn() -> Box<dyn Linker> + Send + Sync + 'static) -> Self {
        self.linker_factories.push(Box::new(factory));
        self
    }

    pub fn checker(
        mut self,
        factory: impl Fn() -> Box<dyn Checker> + Send + Sync + 'static,
    ) -> Self {
        self.checker_factories.push(Box::new(factory));
        self
    }

    pub fn build_index_pipeline(
        &self,
        root: &Path,
        store: impl KnowledgeStore + 'static,
    ) -> IndexPipeline {
        let mut pipeline = IndexPipeline::new(store);

        for factory in &self.source_factories {
            pipeline = pipeline.boxed_source(factory(root));
        }

        for factory in &self.extractor_factories {
            pipeline = pipeline.boxed_extractor(factory());
        }

        for factory in &self.linker_factories {
            pipeline = pipeline.boxed_linker(factory());
        }

        for factory in &self.checker_factories {
            pipeline = pipeline.boxed_checker(factory());
        }

        pipeline
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::built_in()
    }
}

pub struct RuntimeBuilder {
    root: PathBuf,
    registry: AdapterRegistry,
}

impl RuntimeBuilder {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            registry: AdapterRegistry::built_in(),
        }
    }

    pub fn with_registry(mut self, registry: AdapterRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn build_index_pipeline(self, store: impl KnowledgeStore + 'static) -> IndexPipeline {
        self.registry.build_index_pipeline(&self.root, store)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use athanor_domain::{EntityKind, RelationKind, RepoId, SnapshotBase};
    use athanor_store_memory::MemoryKnowledgeStore;

    use super::*;

    #[tokio::test]
    async fn builds_builtin_index_pipeline() {
        let root = std::env::temp_dir().join(format!(
            "athanor-runtime-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/runtime.md"), "# Runtime\n\n## Registry\n").unwrap();

        let output = RuntimeBuilder::new(&root)
            .build_index_pipeline(MemoryKnowledgeStore::new())
            .run(
                RepoId("repo_runtime_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();

        assert_eq!(output.files.len(), 1);
        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.kind == EntityKind::File)
        );
        assert!(
            output
                .relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Contains)
        );

        fs::remove_dir_all(root).unwrap();
    }
}
