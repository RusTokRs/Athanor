use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_checker_markdown::MarkdownStructureChecker;
use athanor_core::{Checker, Extractor, KnowledgeStore, Linker, SourceProvider};
use athanor_extractor_basic::FileExtractor;
use athanor_extractor_markdown::MarkdownExtractor;
use athanor_linker_markdown::MarkdownContainmentLinker;
use athanor_source_fs::LocalFileSystemSource;
use serde::{Deserialize, Serialize};

use crate::IndexPipeline;

pub const ADAPTER_PLUGIN_MANIFEST_SCHEMA: &str = "athanor.adapter_plugin.v1";

type SourceFactory = Box<dyn Fn(&Path) -> Box<dyn SourceProvider> + Send + Sync>;
type ExtractorFactory = Box<dyn Fn() -> Box<dyn Extractor> + Send + Sync>;
type LinkerFactory = Box<dyn Fn() -> Box<dyn Linker> + Send + Sync>;
type CheckerFactory = Box<dyn Fn() -> Box<dyn Checker> + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPluginManifest {
    pub schema: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub adapters: Vec<AdapterPluginEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPluginEntry {
    pub id: String,
    pub kind: AdapterPluginKind,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterPluginKind {
    Source,
    Extractor,
    Linker,
    Checker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredAdapterPlugin {
    pub manifest_path: PathBuf,
    pub manifest: AdapterPluginManifest,
}

pub struct AdapterRegistry {
    adapter_ids: BTreeSet<String>,
    source_factories: Vec<SourceFactory>,
    extractor_factories: Vec<ExtractorFactory>,
    linker_factories: Vec<LinkerFactory>,
    checker_factories: Vec<CheckerFactory>,
}

impl AdapterRegistry {
    pub fn empty() -> Self {
        Self {
            adapter_ids: BTreeSet::new(),
            source_factories: Vec::new(),
            extractor_factories: Vec::new(),
            linker_factories: Vec::new(),
            checker_factories: Vec::new(),
        }
    }

    pub fn built_in() -> Self {
        Self::empty()
            .builtin_source_local_filesystem()
            .builtin_extractor_file()
            .builtin_extractor_markdown()
            .builtin_linker_markdown_containment()
            .builtin_checker_markdown_structure()
    }

    pub fn with_plugin_manifest(mut self, manifest: &AdapterPluginManifest) -> Result<Self> {
        validate_adapter_plugin_manifest(manifest)?;

        for adapter in manifest.adapters.iter().filter(|adapter| adapter.enabled) {
            self = self
                .with_adapter_id(adapter.kind, &adapter.id)
                .with_context(|| {
                    format!(
                        "failed to register adapter {} from plugin {}",
                        adapter.id, manifest.name
                    )
                })?;
        }

        Ok(self)
    }

    fn with_adapter_id(self, kind: AdapterPluginKind, id: &str) -> Result<Self> {
        match (kind, id) {
            (AdapterPluginKind::Source, "builtin.source.local_filesystem") => {
                Ok(self.builtin_source_local_filesystem())
            }
            (AdapterPluginKind::Extractor, "builtin.extractor.file") => {
                Ok(self.builtin_extractor_file())
            }
            (AdapterPluginKind::Extractor, "builtin.extractor.markdown") => {
                Ok(self.builtin_extractor_markdown())
            }
            (AdapterPluginKind::Linker, "builtin.linker.markdown_containment") => {
                Ok(self.builtin_linker_markdown_containment())
            }
            (AdapterPluginKind::Checker, "builtin.checker.markdown_structure") => {
                Ok(self.builtin_checker_markdown_structure())
            }
            _ => bail!("unknown {kind:?} adapter id {id}"),
        }
    }

    fn builtin_source_local_filesystem(self) -> Self {
        self.register_source_id("builtin.source.local_filesystem", |root| {
            Box::new(LocalFileSystemSource::new(root))
        })
    }

    fn builtin_extractor_file(self) -> Self {
        self.register_extractor_id("builtin.extractor.file", || Box::new(FileExtractor))
    }

    fn builtin_extractor_markdown(self) -> Self {
        self.register_extractor_id("builtin.extractor.markdown", || Box::new(MarkdownExtractor))
    }

    fn builtin_linker_markdown_containment(self) -> Self {
        self.register_linker_id("builtin.linker.markdown_containment", || {
            Box::new(MarkdownContainmentLinker)
        })
    }

    fn builtin_checker_markdown_structure(self) -> Self {
        self.register_checker_id("builtin.checker.markdown_structure", || {
            Box::new(MarkdownStructureChecker)
        })
    }

    fn register_source_id(
        mut self,
        id: &str,
        factory: impl Fn(&Path) -> Box<dyn SourceProvider> + Send + Sync + 'static,
    ) -> Self {
        if self.adapter_ids.insert(id.to_string()) {
            self.source_factories.push(Box::new(factory));
        }

        self
    }

    fn register_extractor_id(
        mut self,
        id: &str,
        factory: impl Fn() -> Box<dyn Extractor> + Send + Sync + 'static,
    ) -> Self {
        if self.adapter_ids.insert(id.to_string()) {
            self.extractor_factories.push(Box::new(factory));
        }

        self
    }

    fn register_linker_id(
        mut self,
        id: &str,
        factory: impl Fn() -> Box<dyn Linker> + Send + Sync + 'static,
    ) -> Self {
        if self.adapter_ids.insert(id.to_string()) {
            self.linker_factories.push(Box::new(factory));
        }

        self
    }

    fn register_checker_id(
        mut self,
        id: &str,
        factory: impl Fn() -> Box<dyn Checker> + Send + Sync + 'static,
    ) -> Self {
        if self.adapter_ids.insert(id.to_string()) {
            self.checker_factories.push(Box::new(factory));
        }

        self
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

    pub fn with_discovered_plugins(mut self) -> Result<Self> {
        for plugin in discover_adapter_plugins(&self.root)? {
            self.registry = self.registry.with_plugin_manifest(&plugin.manifest)?;
        }

        Ok(self)
    }

    pub fn build_index_pipeline(self, store: impl KnowledgeStore + 'static) -> IndexPipeline {
        self.registry.build_index_pipeline(&self.root, store)
    }
}

pub fn discover_adapter_plugins(root: impl AsRef<Path>) -> Result<Vec<DiscoveredAdapterPlugin>> {
    let root = root.as_ref();
    let mut manifest_paths = Vec::new();
    let adapters_dir = root.join(".athanor/adapters");
    let plugins_dir = root.join(".athanor/plugins");

    if adapters_dir.is_dir() {
        for entry in fs::read_dir(&adapters_dir)
            .with_context(|| format!("failed to read {}", adapters_dir.display()))?
        {
            let path = entry?.path();

            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                manifest_paths.push(path);
            }
        }
    }

    if plugins_dir.is_dir() {
        for entry in fs::read_dir(&plugins_dir)
            .with_context(|| format!("failed to read {}", plugins_dir.display()))?
        {
            let path = entry?.path().join("athanor-adapter.json");

            if path.is_file() {
                manifest_paths.push(path);
            }
        }
    }

    manifest_paths.sort();
    manifest_paths
        .into_iter()
        .map(read_adapter_plugin_manifest)
        .collect()
}

fn read_adapter_plugin_manifest(path: PathBuf) -> Result<DiscoveredAdapterPlugin> {
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: AdapterPluginManifest = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_adapter_plugin_manifest(&manifest)
        .with_context(|| format!("invalid adapter plugin manifest {}", path.display()))?;

    Ok(DiscoveredAdapterPlugin {
        manifest_path: path,
        manifest,
    })
}

fn validate_adapter_plugin_manifest(manifest: &AdapterPluginManifest) -> Result<()> {
    if manifest.schema != ADAPTER_PLUGIN_MANIFEST_SCHEMA {
        bail!(
            "unsupported adapter plugin manifest schema {}; expected {}",
            manifest.schema,
            ADAPTER_PLUGIN_MANIFEST_SCHEMA
        );
    }

    if manifest.name.trim().is_empty() {
        bail!("adapter plugin manifest name must not be empty");
    }

    Ok(())
}

fn enabled_by_default() -> bool {
    true
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

    #[tokio::test]
    async fn builds_pipeline_from_plugin_manifest() {
        let root = std::env::temp_dir().join(format!(
            "athanor-runtime-plugin-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        let manifest = AdapterPluginManifest {
            schema: ADAPTER_PLUGIN_MANIFEST_SCHEMA.to_string(),
            name: "test-plugin".to_string(),
            version: Some("0.1.0".to_string()),
            adapters: vec![
                AdapterPluginEntry {
                    id: "builtin.source.local_filesystem".to_string(),
                    kind: AdapterPluginKind::Source,
                    enabled: true,
                },
                AdapterPluginEntry {
                    id: "builtin.extractor.file".to_string(),
                    kind: AdapterPluginKind::Extractor,
                    enabled: true,
                },
            ],
        };

        let output = RuntimeBuilder::new(&root)
            .with_registry(
                AdapterRegistry::empty()
                    .with_plugin_manifest(&manifest)
                    .unwrap(),
            )
            .build_index_pipeline(MemoryKnowledgeStore::new())
            .run(
                RepoId("repo_runtime_plugin_test".to_string()),
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

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn plugin_manifest_does_not_duplicate_builtin_adapter_ids() {
        let root = std::env::temp_dir().join(format!(
            "athanor-runtime-plugin-dedupe-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        let manifest = AdapterPluginManifest {
            schema: ADAPTER_PLUGIN_MANIFEST_SCHEMA.to_string(),
            name: "duplicate-builtins".to_string(),
            version: None,
            adapters: vec![AdapterPluginEntry {
                id: "builtin.extractor.file".to_string(),
                kind: AdapterPluginKind::Extractor,
                enabled: true,
            }],
        };

        let output = RuntimeBuilder::new(&root)
            .with_registry(
                AdapterRegistry::built_in()
                    .with_plugin_manifest(&manifest)
                    .unwrap(),
            )
            .build_index_pipeline(MemoryKnowledgeStore::new())
            .run(
                RepoId("repo_runtime_plugin_dedupe_test".to_string()),
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
        assert_eq!(
            output
                .facts
                .iter()
                .filter(|fact| fact.extractor == "file")
                .count(),
            1
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discovers_adapter_plugin_manifests() {
        let root = std::env::temp_dir().join(format!(
            "athanor-runtime-plugin-discovery-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let manifest_path = root.join(".athanor/adapters/files.json");

        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::json!({
                "schema": ADAPTER_PLUGIN_MANIFEST_SCHEMA,
                "name": "files",
                "adapters": [
                    {
                        "id": "builtin.extractor.file",
                        "kind": "extractor"
                    }
                ]
            })
            .to_string(),
        )
        .unwrap();

        let plugins = discover_adapter_plugins(&root).unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest_path, manifest_path);
        assert_eq!(plugins[0].manifest.name, "files");
        assert!(plugins[0].manifest.adapters[0].enabled);

        fs::remove_dir_all(root).unwrap();
    }
}
