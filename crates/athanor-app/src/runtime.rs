use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use athanor_checker_api::{
    ApiConsistencyChecker, DeploymentDocsChecker, EnvDocsChecker, ScriptDocsChecker,
};
use athanor_checker_markdown::MarkdownStructureChecker;
use athanor_core::{
    CheckInput, Checker, CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor,
    KnowledgeStore, LinkInput, Linker, SourceFile, SourceProvider,
};
use athanor_extractor_basic::FileExtractor;
use athanor_extractor_markdown::MarkdownExtractor;
use athanor_extractor_openapi::OpenApiExtractor;
use athanor_extractor_operations::OperationsExtractor;
use athanor_extractor_rust::RustExtractor;
use athanor_linker_api::ApiKnowledgeLinker;
use athanor_linker_markdown::MarkdownContainmentLinker;
use athanor_linker_rust::RustLinker;
use athanor_source_fs::LocalFileSystemSource;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::IndexPipeline;

pub const ADAPTER_MANIFEST_SCHEMA: &str = "athanor.adapter_manifest";

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
    #[serde(default)]
    pub command: Option<AdapterProcessCommand>,
    #[serde(default)]
    pub supports_extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterProcessCommand {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
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
            .builtin_extractor_openapi()
            .builtin_extractor_operations()
            .builtin_extractor_rust()
            .builtin_linker_markdown_containment()
            .builtin_linker_api_knowledge()
            .builtin_linker_rust()
            .builtin_checker_markdown_structure()
            .builtin_checker_api_consistency()
            .builtin_checker_env_docs()
            .builtin_checker_script_docs()
            .builtin_checker_deployment_docs()
    }

    pub fn with_plugin_manifest(self, manifest: &AdapterPluginManifest) -> Result<Self> {
        self.with_plugin_manifest_at(manifest, Path::new("."))
    }

    pub fn with_plugin_manifest_at(
        mut self,
        manifest: &AdapterPluginManifest,
        manifest_dir: &Path,
    ) -> Result<Self> {
        validate_adapter_plugin_manifest(manifest)?;

        for adapter in manifest.adapters.iter().filter(|adapter| adapter.enabled) {
            self = self
                .with_adapter_entry(adapter, manifest_dir)
                .with_context(|| {
                    format!(
                        "failed to register adapter {} from plugin {}",
                        adapter.id, manifest.name
                    )
                })?;
        }

        Ok(self)
    }

    fn with_adapter_entry(self, adapter: &AdapterPluginEntry, manifest_dir: &Path) -> Result<Self> {
        match (adapter.kind, adapter.id.as_str()) {
            (AdapterPluginKind::Source, "builtin.source.local_filesystem") => {
                Ok(self.builtin_source_local_filesystem())
            }
            (AdapterPluginKind::Extractor, "builtin.extractor.file") => {
                Ok(self.builtin_extractor_file())
            }
            (AdapterPluginKind::Extractor, "builtin.extractor.markdown") => {
                Ok(self.builtin_extractor_markdown())
            }
            (AdapterPluginKind::Extractor, "builtin.extractor.openapi") => {
                Ok(self.builtin_extractor_openapi())
            }
            (AdapterPluginKind::Extractor, "builtin.extractor.operations") => {
                Ok(self.builtin_extractor_operations())
            }
            (AdapterPluginKind::Extractor, "builtin.extractor.rust") => {
                Ok(self.builtin_extractor_rust())
            }
            (AdapterPluginKind::Linker, "builtin.linker.markdown_containment") => {
                Ok(self.builtin_linker_markdown_containment())
            }
            (AdapterPluginKind::Linker, "builtin.linker.api_knowledge") => {
                Ok(self.builtin_linker_api_knowledge())
            }
            (AdapterPluginKind::Linker, "builtin.linker.rust") => Ok(self.builtin_linker_rust()),
            (AdapterPluginKind::Checker, "builtin.checker.markdown_structure") => {
                Ok(self.builtin_checker_markdown_structure())
            }
            (AdapterPluginKind::Checker, "builtin.checker.api_consistency") => {
                Ok(self.builtin_checker_api_consistency())
            }
            (AdapterPluginKind::Checker, "builtin.checker.env_docs") => {
                Ok(self.builtin_checker_env_docs())
            }
            (AdapterPluginKind::Checker, "builtin.checker.script_docs") => {
                Ok(self.builtin_checker_script_docs())
            }
            (AdapterPluginKind::Checker, "builtin.checker.deployment_docs") => {
                Ok(self.builtin_checker_deployment_docs())
            }
            (AdapterPluginKind::Source, _) => self.external_process_source(adapter, manifest_dir),
            (AdapterPluginKind::Extractor, _) => {
                self.external_process_extractor(adapter, manifest_dir)
            }
            (AdapterPluginKind::Linker, _) => self.external_process_linker(adapter, manifest_dir),
            (AdapterPluginKind::Checker, _) => self.external_process_checker(adapter, manifest_dir),
        }
    }

    fn external_process_source(
        self,
        adapter: &AdapterPluginEntry,
        manifest_dir: &Path,
    ) -> Result<Self> {
        let Some(command) = &adapter.command else {
            bail!(
                "unknown source adapter id {} and no command was provided",
                adapter.id
            );
        };
        let command = ProcessCommand::from_manifest(manifest_dir, command);
        let id = adapter.id.clone();

        Ok(self.register_source_id(&adapter.id, move |root| {
            Box::new(ProcessSource {
                id: id.clone(),
                command: command.clone(),
                root: root.to_path_buf(),
            })
        }))
    }

    fn external_process_extractor(
        self,
        adapter: &AdapterPluginEntry,
        manifest_dir: &Path,
    ) -> Result<Self> {
        let Some(command) = &adapter.command else {
            bail!(
                "unknown extractor adapter id {} and no command was provided",
                adapter.id
            );
        };
        let command = ProcessCommand::from_manifest(manifest_dir, command);
        let id = adapter.id.clone();
        let supports_extensions = adapter
            .supports_extensions
            .iter()
            .map(normalize_extension)
            .collect::<BTreeSet<_>>();

        Ok(self.register_extractor_id(&adapter.id, move || {
            Box::new(ProcessExtractor {
                id: id.clone(),
                command: command.clone(),
                supports_extensions: supports_extensions.clone(),
            })
        }))
    }

    fn external_process_linker(
        self,
        adapter: &AdapterPluginEntry,
        manifest_dir: &Path,
    ) -> Result<Self> {
        let Some(command) = &adapter.command else {
            bail!(
                "unknown linker adapter id {} and no command was provided",
                adapter.id
            );
        };
        let command = ProcessCommand::from_manifest(manifest_dir, command);
        let id = adapter.id.clone();

        Ok(self.register_linker_id(&adapter.id, move || {
            Box::new(ProcessLinker {
                id: id.clone(),
                command: command.clone(),
            })
        }))
    }

    fn external_process_checker(
        self,
        adapter: &AdapterPluginEntry,
        manifest_dir: &Path,
    ) -> Result<Self> {
        let Some(command) = &adapter.command else {
            bail!(
                "unknown checker adapter id {} and no command was provided",
                adapter.id
            );
        };
        let command = ProcessCommand::from_manifest(manifest_dir, command);
        let id = adapter.id.clone();

        Ok(self.register_checker_id(&adapter.id, move || {
            Box::new(ProcessChecker {
                id: id.clone(),
                command: command.clone(),
            })
        }))
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

    fn builtin_extractor_openapi(self) -> Self {
        self.register_extractor_id("builtin.extractor.openapi", || Box::new(OpenApiExtractor))
    }

    fn builtin_extractor_operations(self) -> Self {
        self.register_extractor_id("builtin.extractor.operations", || {
            Box::new(OperationsExtractor)
        })
    }

    fn builtin_extractor_rust(self) -> Self {
        self.register_extractor_id("builtin.extractor.rust", || Box::new(RustExtractor))
    }

    fn builtin_linker_markdown_containment(self) -> Self {
        self.register_linker_id("builtin.linker.markdown_containment", || {
            Box::new(MarkdownContainmentLinker)
        })
    }

    fn builtin_linker_api_knowledge(self) -> Self {
        self.register_linker_id("builtin.linker.api_knowledge", || {
            Box::new(ApiKnowledgeLinker)
        })
    }

    fn builtin_linker_rust(self) -> Self {
        self.register_linker_id("builtin.linker.rust", || Box::new(RustLinker))
    }

    fn builtin_checker_markdown_structure(self) -> Self {
        self.register_checker_id("builtin.checker.markdown_structure", || {
            Box::new(MarkdownStructureChecker)
        })
    }

    fn builtin_checker_api_consistency(self) -> Self {
        self.register_checker_id("builtin.checker.api_consistency", || {
            Box::new(ApiConsistencyChecker)
        })
    }

    fn builtin_checker_env_docs(self) -> Self {
        self.register_checker_id("builtin.checker.env_docs", || Box::new(EnvDocsChecker))
    }

    fn builtin_checker_script_docs(self) -> Self {
        self.register_checker_id("builtin.checker.script_docs", || {
            Box::new(ScriptDocsChecker)
        })
    }

    fn builtin_checker_deployment_docs(self) -> Self {
        self.register_checker_id("builtin.checker.deployment_docs", || {
            Box::new(DeploymentDocsChecker)
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
            let manifest_dir = plugin.manifest_path.parent().unwrap_or(&self.root);
            self.registry = self
                .registry
                .with_plugin_manifest_at(&plugin.manifest, manifest_dir)?;
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
    if manifest.schema != ADAPTER_MANIFEST_SCHEMA {
        bail!(
            "unsupported adapter plugin manifest schema {}; expected {}",
            manifest.schema,
            ADAPTER_MANIFEST_SCHEMA
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessCommand {
    program: PathBuf,
    args: Vec<String>,
}

impl ProcessCommand {
    fn from_manifest(manifest_dir: &Path, command: &AdapterProcessCommand) -> Self {
        let program = resolve_manifest_program(manifest_dir, &command.program);

        Self {
            program,
            args: command.args.clone(),
        }
    }
}

struct ProcessExtractor {
    id: String,
    command: ProcessCommand,
    supports_extensions: BTreeSet<String>,
}

struct ProcessSource {
    id: String,
    command: ProcessCommand,
    root: PathBuf,
}

#[derive(Serialize)]
struct SourceDiscoverInput<'a> {
    root: &'a Path,
}

struct ProcessLinker {
    id: String,
    command: ProcessCommand,
}

struct ProcessChecker {
    id: String,
    command: ProcessCommand,
}

#[async_trait::async_trait]
impl SourceProvider for ProcessSource {
    fn name(&self) -> &str {
        &self.id
    }

    async fn discover(&self) -> CoreResult<Vec<SourceFile>> {
        run_process_adapter(
            "source",
            &self.id,
            &self.command,
            &SourceDiscoverInput { root: &self.root },
        )
    }
}

#[async_trait::async_trait]
impl Extractor for ProcessExtractor {
    fn name(&self) -> &str {
        &self.id
    }

    fn supports(&self, source: &SourceFile) -> bool {
        if self.supports_extensions.is_empty() {
            return true;
        }

        Path::new(&source.path)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(normalize_extension)
            .is_some_and(|extension| self.supports_extensions.contains(&extension))
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        run_process_adapter("extractor", &self.id, &self.command, &input)
    }
}

#[async_trait::async_trait]
impl Linker for ProcessLinker {
    fn name(&self) -> &str {
        &self.id
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<athanor_domain::Relation>> {
        run_process_adapter("linker", &self.id, &self.command, &input)
    }
}

#[async_trait::async_trait]
impl Checker for ProcessChecker {
    fn name(&self) -> &str {
        &self.id
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<athanor_domain::Diagnostic>> {
        run_process_adapter("checker", &self.id, &self.command, &input)
    }
}

fn run_process_adapter<I, O>(
    adapter_kind: &str,
    adapter_id: &str,
    command: &ProcessCommand,
    input: &I,
) -> CoreResult<O>
where
    I: Serialize,
    O: for<'de> Deserialize<'de>,
{
    let mut child = Command::new(&command.program)
        .args(&command.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            CoreError::Adapter(format!(
                "failed to spawn external {adapter_kind} {adapter_id}: {error}"
            ))
        })?;

    {
        let mut stdin = child.stdin.take().ok_or_else(|| {
            CoreError::Adapter(format!(
                "failed to open stdin for external {adapter_kind} {adapter_id}"
            ))
        })?;
        serde_json::to_writer(&mut stdin, input).map_err(|error| {
            CoreError::Adapter(format!(
                "failed to write input for external {adapter_kind} {adapter_id}: {error}"
            ))
        })?;
        stdin.write_all(b"\n").map_err(|error| {
            CoreError::Adapter(format!(
                "failed to finish input for external {adapter_kind} {adapter_id}: {error}"
            ))
        })?;
    }

    let output = child.wait_with_output().map_err(|error| {
        CoreError::Adapter(format!(
            "failed to wait for external {adapter_kind} {adapter_id}: {error}"
        ))
    })?;
    let stdout = process_output_excerpt(&output.stdout);
    let stderr = process_output_excerpt(&output.stderr);

    if !stdout.is_empty() {
        debug!(
            adapter_kind,
            adapter_id,
            stdout = %stdout,
            "external process adapter stdout"
        );
    }

    if !stderr.is_empty() {
        warn!(
            adapter_kind,
            adapter_id,
            stderr = %stderr,
            "external process adapter stderr"
        );
    }

    if !output.status.success() {
        return Err(CoreError::Adapter(format!(
            "external {adapter_kind} {adapter_id} exited with {}; stderr: {}",
            output.status, stderr
        )));
    }

    serde_json::from_slice(&output.stdout).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to parse output from external {adapter_kind} {adapter_id}: {error}"
        ))
    })
}

fn process_output_excerpt(bytes: &[u8]) -> String {
    const MAX_PROCESS_OUTPUT_LOG_CHARS: usize = 4096;

    let value = String::from_utf8_lossy(bytes);
    let trimmed = value.trim();
    let mut excerpt = trimmed
        .chars()
        .take(MAX_PROCESS_OUTPUT_LOG_CHARS)
        .collect::<String>();

    if trimmed.chars().count() > MAX_PROCESS_OUTPUT_LOG_CHARS {
        excerpt.push_str("...");
    }

    excerpt
}

fn resolve_manifest_program(manifest_dir: &Path, program: &str) -> PathBuf {
    let path = PathBuf::from(program);

    if path.is_relative() && (program.contains('/') || program.contains('\\')) {
        manifest_dir.join(path)
    } else {
        path
    }
}

fn normalize_extension(extension: impl AsRef<str>) -> String {
    extension
        .as_ref()
        .trim_start_matches('.')
        .to_ascii_lowercase()
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
            schema: ADAPTER_MANIFEST_SCHEMA.to_string(),
            name: "test-plugin".to_string(),
            version: Some("0.1.0".to_string()),
            adapters: vec![
                AdapterPluginEntry {
                    id: "builtin.source.local_filesystem".to_string(),
                    kind: AdapterPluginKind::Source,
                    enabled: true,
                    command: None,
                    supports_extensions: Vec::new(),
                },
                AdapterPluginEntry {
                    id: "builtin.extractor.file".to_string(),
                    kind: AdapterPluginKind::Extractor,
                    enabled: true,
                    command: None,
                    supports_extensions: Vec::new(),
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
            schema: ADAPTER_MANIFEST_SCHEMA.to_string(),
            name: "duplicate-builtins".to_string(),
            version: None,
            adapters: vec![AdapterPluginEntry {
                id: "builtin.extractor.file".to_string(),
                kind: AdapterPluginKind::Extractor,
                enabled: true,
                command: None,
                supports_extensions: Vec::new(),
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

    #[tokio::test]
    async fn loads_external_process_extractor_from_manifest() {
        let root = std::env::temp_dir().join(format!(
            "athanor-runtime-process-extractor-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        let manifest = AdapterPluginManifest {
            schema: ADAPTER_MANIFEST_SCHEMA.to_string(),
            name: "process-extractor".to_string(),
            version: None,
            adapters: vec![
                AdapterPluginEntry {
                    id: "builtin.source.local_filesystem".to_string(),
                    kind: AdapterPluginKind::Source,
                    enabled: true,
                    command: None,
                    supports_extensions: Vec::new(),
                },
                AdapterPluginEntry {
                    id: "external.extractor.empty".to_string(),
                    kind: AdapterPluginKind::Extractor,
                    enabled: true,
                    command: Some(empty_output_command()),
                    supports_extensions: vec!["rs".to_string()],
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
                RepoId("repo_runtime_process_extractor_test".to_string()),
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
        assert!(output.entities.is_empty());
        assert!(output.facts.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn loads_external_process_source_from_manifest() {
        let root = std::env::temp_dir().join(format!(
            "athanor-runtime-process-source-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(&root).unwrap();
        let manifest = AdapterPluginManifest {
            schema: ADAPTER_MANIFEST_SCHEMA.to_string(),
            name: "process-source".to_string(),
            version: None,
            adapters: vec![
                AdapterPluginEntry {
                    id: "external.source.virtual".to_string(),
                    kind: AdapterPluginKind::Source,
                    enabled: true,
                    command: Some(source_output_command()),
                    supports_extensions: Vec::new(),
                },
                AdapterPluginEntry {
                    id: "builtin.extractor.file".to_string(),
                    kind: AdapterPluginKind::Extractor,
                    enabled: true,
                    command: None,
                    supports_extensions: Vec::new(),
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
                RepoId("repo_runtime_process_source_test".to_string()),
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
        assert_eq!(output.files[0].path, "virtual/readme.md");
        assert_eq!(output.files[0].language_hint.as_deref(), Some("markdown"));
        assert_eq!(output.entities.len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn loads_external_process_linker_and_checker_from_manifest() {
        let root = std::env::temp_dir().join(format!(
            "athanor-runtime-process-downstream-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        let manifest = AdapterPluginManifest {
            schema: ADAPTER_MANIFEST_SCHEMA.to_string(),
            name: "process-downstream".to_string(),
            version: None,
            adapters: vec![
                AdapterPluginEntry {
                    id: "builtin.source.local_filesystem".to_string(),
                    kind: AdapterPluginKind::Source,
                    enabled: true,
                    command: None,
                    supports_extensions: Vec::new(),
                },
                AdapterPluginEntry {
                    id: "builtin.extractor.file".to_string(),
                    kind: AdapterPluginKind::Extractor,
                    enabled: true,
                    command: None,
                    supports_extensions: Vec::new(),
                },
                AdapterPluginEntry {
                    id: "external.linker.empty".to_string(),
                    kind: AdapterPluginKind::Linker,
                    enabled: true,
                    command: Some(empty_array_command()),
                    supports_extensions: Vec::new(),
                },
                AdapterPluginEntry {
                    id: "external.checker.empty".to_string(),
                    kind: AdapterPluginKind::Checker,
                    enabled: true,
                    command: Some(empty_array_command()),
                    supports_extensions: Vec::new(),
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
                RepoId("repo_runtime_process_downstream_test".to_string()),
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
        assert_eq!(output.entities.len(), 1);
        assert!(output.relations.is_empty());
        assert!(output.diagnostics.is_empty());

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
                "schema": ADAPTER_MANIFEST_SCHEMA,
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

    #[cfg(windows)]
    fn empty_output_command() -> AdapterProcessCommand {
        powershell_json_command("{\"entities\":[],\"facts\":[]}")
    }

    #[cfg(not(windows))]
    fn empty_output_command() -> AdapterProcessCommand {
        sh_json_command("{\"entities\":[],\"facts\":[]}")
    }

    #[cfg(windows)]
    fn source_output_command() -> AdapterProcessCommand {
        powershell_json_command(
            "[{\"path\":\"virtual/readme.md\",\"language_hint\":\"markdown\",\"content_hash\":\"test:1\",\"content\":\"# Virtual\"}]",
        )
    }

    #[cfg(not(windows))]
    fn source_output_command() -> AdapterProcessCommand {
        sh_json_command(
            "[{\"path\":\"virtual/readme.md\",\"language_hint\":\"markdown\",\"content_hash\":\"test:1\",\"content\":\"# Virtual\"}]",
        )
    }

    #[cfg(windows)]
    fn empty_array_command() -> AdapterProcessCommand {
        powershell_json_command("[]")
    }

    #[cfg(not(windows))]
    fn empty_array_command() -> AdapterProcessCommand {
        sh_json_command("[]")
    }

    #[cfg(windows)]
    fn powershell_json_command(json: &str) -> AdapterProcessCommand {
        AdapterProcessCommand {
            program: "powershell".to_string(),
            args: vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                format!("$input | Out-Null; '{}'", json.replace('\'', "''")),
            ],
        }
    }

    #[cfg(not(windows))]
    fn sh_json_command(json: &str) -> AdapterProcessCommand {
        AdapterProcessCommand {
            program: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                format!(
                    "cat >/dev/null; printf '%s' '{}'",
                    json.replace('\'', "'\\''")
                ),
            ],
        }
    }
}
