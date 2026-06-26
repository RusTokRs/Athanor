use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use athanor_adapter_rustok_fba::{RustokFbaChecker, RustokFbaExtractor, RustokFbaLinker};
use athanor_adapter_rustok_ffa::{RustokFfaChecker, RustokFfaExtractor, RustokFfaLinker};
use athanor_adapter_rustok_page_builder::{
    RustokPageBuilderChecker, RustokPageBuilderExtractor, RustokPageBuilderLinker,
};
use athanor_checker_api::{
    ApiConsistencyChecker, DeploymentDocsChecker, EnvDocsChecker, RunbookConsistencyChecker,
    ScriptDocsChecker,
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
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use crate::IndexPipeline;

pub const ADAPTER_MANIFEST_SCHEMA: &str = "athanor.adapter_manifest";
pub const ADAPTER_TRUST_SCHEMA: &str = "athanor.adapter_trust.v1";

type SourceFactory = Box<dyn Fn(&Path) -> Box<dyn SourceProvider> + Send + Sync>;
type ExtractorFactory = Box<dyn Fn() -> Box<dyn Extractor> + Send + Sync>;
type LinkerFactory = Box<dyn Fn() -> Box<dyn Linker> + Send + Sync>;
type CheckerFactory = Box<dyn Fn() -> Box<dyn Checker> + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterPluginManifest {
    pub schema: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub adapters: Vec<AdapterPluginEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Clone)]
pub struct AdapterTrustOptions {
    pub trust_path: PathBuf,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AdapterTrustListOptions {
    pub root: PathBuf,
    pub trust_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrustedAdapterPlugin {
    pub manifest_path: PathBuf,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdapterTrustRegistry {
    pub schema: String,
    #[serde(default)]
    pub trusted_plugins: Vec<TrustedAdapterPlugin>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AdapterTrustReport {
    pub schema: String,
    pub trust_path: PathBuf,
    pub plugins: Vec<AdapterTrustStatus>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AdapterTrustStatus {
    pub manifest_path: PathBuf,
    pub name: String,
    pub version: Option<String>,
    pub has_external_process: bool,
    pub trusted: bool,
    pub content_hash: String,
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
            .builtin_checker_runbook_consistency()
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
            (AdapterPluginKind::Extractor, "builtin.extractor.rustok_ffa") => {
                Ok(self.builtin_extractor_rustok_ffa())
            }
            (AdapterPluginKind::Extractor, "builtin.extractor.rustok_fba") => {
                Ok(self.builtin_extractor_rustok_fba())
            }
            (AdapterPluginKind::Extractor, "builtin.extractor.rustok_page_builder") => {
                Ok(self.builtin_extractor_rustok_page_builder())
            }
            (AdapterPluginKind::Linker, "builtin.linker.markdown_containment") => {
                Ok(self.builtin_linker_markdown_containment())
            }
            (AdapterPluginKind::Linker, "builtin.linker.api_knowledge") => {
                Ok(self.builtin_linker_api_knowledge())
            }
            (AdapterPluginKind::Linker, "builtin.linker.rust") => Ok(self.builtin_linker_rust()),
            (AdapterPluginKind::Linker, "builtin.linker.rustok_ffa") => {
                Ok(self.builtin_linker_rustok_ffa())
            }
            (AdapterPluginKind::Linker, "builtin.linker.rustok_fba") => {
                Ok(self.builtin_linker_rustok_fba())
            }
            (AdapterPluginKind::Linker, "builtin.linker.rustok_page_builder") => {
                Ok(self.builtin_linker_rustok_page_builder())
            }
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
            (AdapterPluginKind::Checker, "builtin.checker.runbook_consistency") => {
                Ok(self.builtin_checker_runbook_consistency())
            }
            (AdapterPluginKind::Checker, "builtin.checker.rustok_ffa") => {
                Ok(self.builtin_checker_rustok_ffa())
            }
            (AdapterPluginKind::Checker, "builtin.checker.rustok_fba") => {
                Ok(self.builtin_checker_rustok_fba())
            }
            (AdapterPluginKind::Checker, "builtin.checker.rustok_page_builder") => {
                Ok(self.builtin_checker_rustok_page_builder())
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
        let command = ProcessCommand::from_manifest(manifest_dir, command)?;
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
        let command = ProcessCommand::from_manifest(manifest_dir, command)?;
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
        let command = ProcessCommand::from_manifest(manifest_dir, command)?;
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
        let command = ProcessCommand::from_manifest(manifest_dir, command)?;
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

    fn builtin_extractor_rustok_ffa(self) -> Self {
        self.register_extractor_id("builtin.extractor.rustok_ffa", || {
            Box::new(RustokFfaExtractor)
        })
    }

    fn builtin_extractor_rustok_fba(self) -> Self {
        self.register_extractor_id("builtin.extractor.rustok_fba", || {
            Box::new(RustokFbaExtractor)
        })
    }

    fn builtin_extractor_rustok_page_builder(self) -> Self {
        self.register_extractor_id("builtin.extractor.rustok_page_builder", || {
            Box::new(RustokPageBuilderExtractor)
        })
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

    fn builtin_linker_rustok_ffa(self) -> Self {
        self.register_linker_id("builtin.linker.rustok_ffa", || Box::new(RustokFfaLinker))
    }

    fn builtin_linker_rustok_fba(self) -> Self {
        self.register_linker_id("builtin.linker.rustok_fba", || Box::new(RustokFbaLinker))
    }

    fn builtin_linker_rustok_page_builder(self) -> Self {
        self.register_linker_id("builtin.linker.rustok_page_builder", || {
            Box::new(RustokPageBuilderLinker)
        })
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

    fn builtin_checker_runbook_consistency(self) -> Self {
        self.register_checker_id("builtin.checker.runbook_consistency", || {
            Box::new(RunbookConsistencyChecker)
        })
    }

    fn builtin_checker_rustok_ffa(self) -> Self {
        self.register_checker_id("builtin.checker.rustok_ffa", || Box::new(RustokFfaChecker))
    }

    fn builtin_checker_rustok_fba(self) -> Self {
        self.register_checker_id("builtin.checker.rustok_fba", || Box::new(RustokFbaChecker))
    }

    fn builtin_checker_rustok_page_builder(self) -> Self {
        self.register_checker_id("builtin.checker.rustok_page_builder", || {
            Box::new(RustokPageBuilderChecker)
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
    allow_external_process: bool,
    allowed_external_process_programs: Vec<PathBuf>,
    adapter_trust_path: Option<PathBuf>,
}

impl RuntimeBuilder {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            registry: AdapterRegistry::built_in(),
            allow_external_process: false,
            allowed_external_process_programs: Vec::new(),
            adapter_trust_path: None,
        }
    }

    pub fn with_registry(mut self, registry: AdapterRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn allow_external_process(mut self, allowed: bool) -> Self {
        self.allow_external_process = allowed;
        self
    }

    pub fn allowed_external_process_programs(
        mut self,
        programs: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> Self {
        self.allowed_external_process_programs = programs.into_iter().map(Into::into).collect();
        self
    }

    pub fn adapter_trust_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.adapter_trust_path = Some(path.into());
        self
    }

    pub fn with_discovered_plugins(mut self) -> Result<Self> {
        let mut trust_registry = None;

        for plugin in discover_adapter_plugins(&self.root)? {
            let external = plugin
                .manifest
                .adapters
                .iter()
                .filter(|adapter| adapter.enabled && adapter.command.is_some())
                .collect::<Vec<_>>();
            if !external.is_empty() && !self.allow_external_process {
                bail!(
                    "external process adapters are disabled; set [adapters] allow_external_process = true to enable plugin `{}`",
                    plugin.manifest.name
                );
            }
            if !external.is_empty() {
                let allowlist = resolve_external_process_allowlist(
                    &self.root,
                    &self.allowed_external_process_programs,
                )?;
                for adapter in &external {
                    let command = adapter
                        .command
                        .as_ref()
                        .expect("external adapter has command");
                    let resolved = ProcessCommand::from_manifest(
                        plugin.manifest_path.parent().unwrap_or(&self.root),
                        command,
                    )?;
                    if !allowlist.contains(&resolved.program) {
                        bail!(
                            "external process adapter `{}` command {} is not in [adapters].external_process_allowlist",
                            adapter.id,
                            resolved.program.display()
                        );
                    }
                }
                if trust_registry.is_none() {
                    let trust_path = match &self.adapter_trust_path {
                        Some(path) => path.clone(),
                        None => default_adapter_trust_path()?,
                    };
                    trust_registry = Some(load_adapter_trust_registry(&trust_path)?);
                }
                let registry = trust_registry
                    .as_ref()
                    .expect("adapter trust registry was loaded for external adapters");
                if !is_adapter_plugin_trusted(registry, &plugin)? {
                    bail!(
                        "external process adapter plugin `{}` is not trusted; run `ath plugins trust {}` to trust this manifest version",
                        plugin.manifest.name,
                        plugin.manifest_path.display()
                    );
                }
            }
            for adapter in external {
                warn!(
                    plugin = %plugin.manifest.name,
                    adapter = %adapter.id,
                    "external process adapter execution is enabled"
                );
            }
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

pub fn default_adapter_trust_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ATHANOR_ADAPTER_TRUST") {
        if path.is_empty() {
            bail!("ATHANOR_ADAPTER_TRUST must not be empty");
        }
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "cannot determine user home directory; set ATHANOR_ADAPTER_TRUST explicitly"
            )
        })?;
    Ok(PathBuf::from(home).join(".athanor/adapter-trust.json"))
}

pub fn list_adapter_plugin_trust(options: AdapterTrustListOptions) -> Result<AdapterTrustReport> {
    let root = options.root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize project root {}",
            options.root.display()
        )
    })?;
    let registry = load_adapter_trust_registry(&options.trust_path)?;
    let mut plugins = discover_adapter_plugins(root)?
        .into_iter()
        .map(|plugin| adapter_trust_status(&registry, &plugin))
        .collect::<Result<Vec<_>>>()?;
    plugins.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));

    Ok(AdapterTrustReport {
        schema: ADAPTER_TRUST_SCHEMA.to_string(),
        trust_path: options.trust_path,
        plugins,
    })
}

pub fn trust_adapter_plugin(options: AdapterTrustOptions) -> Result<AdapterTrustReport> {
    let plugin = read_adapter_plugin_manifest(options.manifest_path)?;
    let mut registry = load_adapter_trust_registry(&options.trust_path)?;
    let trusted = trusted_plugin_record(&plugin)?;

    registry
        .trusted_plugins
        .retain(|entry| entry.manifest_path != trusted.manifest_path);
    registry.trusted_plugins.push(trusted);
    sort_trusted_plugins(&mut registry.trusted_plugins);
    write_adapter_trust_registry(&options.trust_path, &registry)?;

    Ok(AdapterTrustReport {
        schema: ADAPTER_TRUST_SCHEMA.to_string(),
        trust_path: options.trust_path,
        plugins: vec![adapter_trust_status(&registry, &plugin)?],
    })
}

pub fn untrust_adapter_plugin(options: AdapterTrustOptions) -> Result<AdapterTrustReport> {
    let plugin = read_adapter_plugin_manifest(options.manifest_path)?;
    let mut registry = load_adapter_trust_registry(&options.trust_path)?;
    let manifest_path = plugin.manifest_path.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize adapter manifest {}",
            plugin.manifest_path.display()
        )
    })?;
    let previous_len = registry.trusted_plugins.len();
    registry
        .trusted_plugins
        .retain(|entry| entry.manifest_path != manifest_path);
    if registry.trusted_plugins.len() == previous_len {
        bail!(
            "adapter plugin manifest is not trusted: {}",
            manifest_path.display()
        );
    }
    write_adapter_trust_registry(&options.trust_path, &registry)?;

    Ok(AdapterTrustReport {
        schema: ADAPTER_TRUST_SCHEMA.to_string(),
        trust_path: options.trust_path,
        plugins: vec![adapter_trust_status(&registry, &plugin)?],
    })
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

fn load_adapter_trust_registry(path: &Path) -> Result<AdapterTrustRegistry> {
    if !path.exists() {
        return Ok(AdapterTrustRegistry {
            schema: ADAPTER_TRUST_SCHEMA.to_string(),
            trusted_plugins: Vec::new(),
        });
    }

    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut registry: AdapterTrustRegistry = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if registry.schema != ADAPTER_TRUST_SCHEMA {
        bail!(
            "unsupported adapter trust schema `{}` in {}",
            registry.schema,
            path.display()
        );
    }
    for entry in &registry.trusted_plugins {
        if !entry.manifest_path.is_absolute() {
            bail!(
                "trusted adapter manifest path is not absolute: {}",
                entry.manifest_path.display()
            );
        }
        if entry.content_hash.trim().is_empty() {
            bail!(
                "trusted adapter manifest {} has empty content hash",
                entry.manifest_path.display()
            );
        }
    }
    sort_trusted_plugins(&mut registry.trusted_plugins);
    Ok(registry)
}

fn write_adapter_trust_registry(path: &Path, registry: &AdapterTrustRegistry) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("adapter trust path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create adapter trust directory {}",
            parent.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid adapter trust path: {}", path.display()))?;
    let staging = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let backup = parent.join(format!(".{file_name}.backup-{}", std::process::id()));
    if staging.exists() {
        fs::remove_file(&staging)
            .with_context(|| format!("failed to remove stale staging {}", staging.display()))?;
    }
    if backup.exists() {
        fs::remove_file(&backup)
            .with_context(|| format!("failed to remove stale backup {}", backup.display()))?;
    }
    let content = serde_json::to_string_pretty(registry)?;
    fs::write(&staging, format!("{content}\n"))
        .with_context(|| format!("failed to write staged adapter trust {}", staging.display()))?;
    if path.exists() {
        fs::rename(path, &backup).with_context(|| {
            format!("failed to stage previous adapter trust {}", path.display())
        })?;
    }
    if let Err(error) = fs::rename(&staging, path) {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&staging);
        return Err(error)
            .with_context(|| format!("failed to publish adapter trust {}", path.display()));
    }
    if backup.exists() {
        fs::remove_file(&backup)
            .with_context(|| format!("failed to remove backup {}", backup.display()))?;
    }
    Ok(())
}

fn trusted_plugin_record(plugin: &DiscoveredAdapterPlugin) -> Result<TrustedAdapterPlugin> {
    Ok(TrustedAdapterPlugin {
        manifest_path: plugin.manifest_path.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize adapter manifest {}",
                plugin.manifest_path.display()
            )
        })?,
        content_hash: adapter_manifest_hash(&plugin.manifest_path)?,
    })
}

fn adapter_trust_status(
    registry: &AdapterTrustRegistry,
    plugin: &DiscoveredAdapterPlugin,
) -> Result<AdapterTrustStatus> {
    let content_hash = adapter_manifest_hash(&plugin.manifest_path)?;
    Ok(AdapterTrustStatus {
        manifest_path: plugin.manifest_path.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize adapter manifest {}",
                plugin.manifest_path.display()
            )
        })?,
        name: plugin.manifest.name.clone(),
        version: plugin.manifest.version.clone(),
        has_external_process: plugin
            .manifest
            .adapters
            .iter()
            .any(|adapter| adapter.enabled && adapter.command.is_some()),
        trusted: is_adapter_plugin_trusted(registry, plugin)?,
        content_hash,
    })
}

fn is_adapter_plugin_trusted(
    registry: &AdapterTrustRegistry,
    plugin: &DiscoveredAdapterPlugin,
) -> Result<bool> {
    let trusted = trusted_plugin_record(plugin)?;
    Ok(registry.trusted_plugins.iter().any(|entry| {
        entry.manifest_path == trusted.manifest_path && entry.content_hash == trusted.content_hash
    }))
}

fn adapter_manifest_hash(path: &Path) -> Result<String> {
    let content = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let digest = Sha256::digest(&content);
    Ok(hex_encode(&digest))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn sort_trusted_plugins(plugins: &mut [TrustedAdapterPlugin]) {
    plugins.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
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
    fn from_manifest(manifest_dir: &Path, command: &AdapterProcessCommand) -> Result<Self> {
        let program = resolve_manifest_program(manifest_dir, &command.program)?;

        Ok(Self {
            program,
            args: command.args.clone(),
        })
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
    run_process_adapter_with_limits(
        adapter_kind,
        adapter_id,
        command,
        input,
        ProcessLimits::default(),
    )
}

#[derive(Debug, Clone, Copy)]
struct ProcessLimits {
    timeout: Duration,
    max_stdin_bytes: usize,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_stdin_bytes: 8 * 1024 * 1024,
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
        }
    }
}

struct LimitedProcessOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

fn run_process_adapter_with_limits<I, O>(
    adapter_kind: &str,
    adapter_id: &str,
    command: &ProcessCommand,
    input: &I,
    limits: ProcessLimits,
) -> CoreResult<O>
where
    I: Serialize,
    O: for<'de> Deserialize<'de>,
{
    let input_bytes = serde_json::to_vec(input).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to serialize input for external {adapter_kind} {adapter_id}: {error}"
        ))
    })?;
    if input_bytes.len() > limits.max_stdin_bytes {
        return Err(CoreError::Adapter(format!(
            "input for external {adapter_kind} {adapter_id} exceeded {} bytes",
            limits.max_stdin_bytes
        )));
    }

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

    let stdout = child.stdout.take().ok_or_else(|| {
        CoreError::Adapter(format!(
            "failed to open stdout for external {adapter_kind} {adapter_id}"
        ))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        CoreError::Adapter(format!(
            "failed to open stderr for external {adapter_kind} {adapter_id}"
        ))
    })?;
    let stdout_limit = limits.max_stdout_bytes;
    let stderr_limit = limits.max_stderr_bytes;
    let stdout_reader = thread::spawn(move || read_limited(stdout, stdout_limit));
    let stderr_reader = thread::spawn(move || read_limited(stderr, stderr_limit));

    {
        let mut stdin = child.stdin.take().ok_or_else(|| {
            CoreError::Adapter(format!(
                "failed to open stdin for external {adapter_kind} {adapter_id}"
            ))
        })?;
        stdin.write_all(&input_bytes).map_err(|error| {
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

    let deadline = Instant::now() + limits.timeout;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            CoreError::Adapter(format!(
                "failed to wait for external {adapter_kind} {adapter_id}: {error}"
            ))
        })? {
            break status;
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(CoreError::Adapter(format!(
                "external {adapter_kind} {adapter_id} timed out after {} ms",
                limits.timeout.as_millis()
            )));
        }

        thread::sleep(Duration::from_millis(10));
    };

    let (stdout, stdout_truncated) = stdout_reader.join().map_err(|_| {
        CoreError::Adapter(format!(
            "failed to read stdout for external {adapter_kind} {adapter_id}"
        ))
    })??;
    let (stderr, stderr_truncated) = stderr_reader.join().map_err(|_| {
        CoreError::Adapter(format!(
            "failed to read stderr for external {adapter_kind} {adapter_id}"
        ))
    })??;

    let output = LimitedProcessOutput {
        status,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    };
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

    if output.stdout_truncated {
        return Err(CoreError::Adapter(format!(
            "external {adapter_kind} {adapter_id} stdout exceeded {} bytes",
            limits.max_stdout_bytes
        )));
    }

    if output.stderr_truncated {
        return Err(CoreError::Adapter(format!(
            "external {adapter_kind} {adapter_id} stderr exceeded {} bytes",
            limits.max_stderr_bytes
        )));
    }

    serde_json::from_slice(&output.stdout).map_err(|error| {
        CoreError::Adapter(format!(
            "failed to parse output from external {adapter_kind} {adapter_id}: {error}"
        ))
    })
}

fn read_limited(mut reader: impl Read, max_bytes: usize) -> CoreResult<(Vec<u8>, bool)> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;

    loop {
        let read = reader.read(&mut buffer).map_err(|error| {
            CoreError::Adapter(format!("failed to read external process output: {error}"))
        })?;
        if read == 0 {
            break;
        }

        let remaining = max_bytes.saturating_sub(output.len());
        if read > remaining {
            output.extend_from_slice(&buffer[..remaining]);
            truncated = true;
            break;
        }

        output.extend_from_slice(&buffer[..read]);
    }

    Ok((output, truncated))
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

fn resolve_manifest_program(manifest_dir: &Path, program: &str) -> Result<PathBuf> {
    if program.trim().is_empty() {
        bail!("adapter command program must not be empty");
    }

    let path = PathBuf::from(program);
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!("adapter command program must not contain parent directory components");
    }

    if path.is_relative() && !(program.contains('/') || program.contains('\\')) {
        bail!("adapter command program must be an explicit absolute or manifest-relative path");
    }

    if path.is_relative() && (program.contains('/') || program.contains('\\')) {
        let base = manifest_dir
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", manifest_dir.display()))?;
        let resolved = manifest_dir
            .join(path)
            .canonicalize()
            .with_context(|| format!("failed to canonicalize adapter command program {program}"))?;
        if !resolved.starts_with(&base) {
            bail!(
                "adapter command program {} escapes manifest directory {}",
                resolved.display(),
                base.display()
            );
        }
        Ok(resolved)
    } else {
        let resolved = path.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize adapter command program {}",
                path.display()
            )
        })?;
        Ok(resolved)
    }
}

fn resolve_external_process_allowlist(
    root: &Path,
    programs: &[PathBuf],
) -> Result<BTreeSet<PathBuf>> {
    programs
        .iter()
        .map(|program| {
            let path = if program.is_absolute() {
                program.clone()
            } else {
                root.join(program)
            };
            path.canonicalize().with_context(|| {
                format!(
                    "failed to canonicalize external process allowlist entry {}",
                    path.display()
                )
            })
        })
        .collect()
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
    use std::time::Duration;

    use athanor_domain::{EntityKind, RelationKind, RepoId, SnapshotBase};
    use athanor_store_memory::MemoryKnowledgeStore;
    use serde_json::Value;

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

    #[test]
    fn discovered_external_process_adapters_require_explicit_opt_in_and_trust() {
        let root = std::env::temp_dir().join(format!(
            "athanor-runtime-process-policy-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let manifest_dir = root.join(".athanor/adapters");
        fs::create_dir_all(&manifest_dir).unwrap();
        let manifest = AdapterPluginManifest {
            schema: ADAPTER_MANIFEST_SCHEMA.to_string(),
            name: "external-policy".to_string(),
            version: None,
            adapters: vec![AdapterPluginEntry {
                id: "external.extractor.empty".to_string(),
                kind: AdapterPluginKind::Extractor,
                enabled: true,
                command: Some(empty_output_command()),
                supports_extensions: vec!["rs".to_string()],
            }],
        };
        fs::write(
            manifest_dir.join("external.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let trust_path = root.join("state/adapter-trust.json");

        let error = RuntimeBuilder::new(&root)
            .adapter_trust_path(&trust_path)
            .with_discovered_plugins()
            .err()
            .expect("external process adapter should be rejected by default");
        assert!(
            error
                .to_string()
                .contains("external process adapters are disabled")
        );
        let error = RuntimeBuilder::new(&root)
            .adapter_trust_path(&trust_path)
            .allow_external_process(true)
            .allowed_external_process_programs([empty_output_program()])
            .with_discovered_plugins()
            .err()
            .expect("untrusted external process adapter should be rejected");
        assert!(error.to_string().contains("is not trusted"));

        trust_adapter_plugin(AdapterTrustOptions {
            trust_path: trust_path.clone(),
            manifest_path: manifest_dir.join("external.json"),
        })
        .expect("trusting plugin should succeed");
        let error = RuntimeBuilder::new(&root)
            .adapter_trust_path(&trust_path)
            .allow_external_process(true)
            .with_discovered_plugins()
            .err()
            .expect("external process adapter without allowlist should be rejected");
        assert!(error.to_string().contains("external_process_allowlist"));

        RuntimeBuilder::new(&root)
            .adapter_trust_path(trust_path)
            .allow_external_process(true)
            .allowed_external_process_programs([empty_output_program()])
            .with_discovered_plugins()
            .expect("explicit opt-in, trust, and allowlist should allow external process adapters");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trusted_plugin_requires_matching_manifest_hash() {
        let root = std::env::temp_dir().join(format!(
            "athanor-runtime-process-trust-hash-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let manifest_dir = root.join(".athanor/adapters");
        fs::create_dir_all(&manifest_dir).unwrap();
        let manifest_path = manifest_dir.join("external.json");
        let mut manifest = AdapterPluginManifest {
            schema: ADAPTER_MANIFEST_SCHEMA.to_string(),
            name: "external-trust-hash".to_string(),
            version: None,
            adapters: vec![AdapterPluginEntry {
                id: "external.extractor.empty".to_string(),
                kind: AdapterPluginKind::Extractor,
                enabled: true,
                command: Some(empty_output_command()),
                supports_extensions: vec!["rs".to_string()],
            }],
        };
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let trust_path = root.join("state/adapter-trust.json");
        trust_adapter_plugin(AdapterTrustOptions {
            trust_path: trust_path.clone(),
            manifest_path: manifest_path.clone(),
        })
        .unwrap();

        let trusted = list_adapter_plugin_trust(AdapterTrustListOptions {
            root: root.clone(),
            trust_path: trust_path.clone(),
        })
        .unwrap();
        assert!(trusted.plugins[0].trusted);

        manifest.version = Some("0.2.0".to_string());
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let changed = list_adapter_plugin_trust(AdapterTrustListOptions {
            root: root.clone(),
            trust_path,
        })
        .unwrap();
        assert!(!changed.plugins[0].trusted);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unknown_manifest_fields() {
        let content = serde_json::json!({
            "schema": ADAPTER_MANIFEST_SCHEMA,
            "name": "unknown-field",
            "unexpected": true,
            "adapters": []
        })
        .to_string();

        let error = serde_json::from_str::<AdapterPluginManifest>(&content)
            .expect_err("unknown manifest field should fail deserialization");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_bare_process_commands() {
        let command = AdapterProcessCommand {
            program: "sh".to_string(),
            args: Vec::new(),
        };
        let error = ProcessCommand::from_manifest(Path::new("."), &command)
            .expect_err("bare command should be rejected");

        assert!(error.to_string().contains("explicit absolute"));
    }

    #[test]
    fn rejects_parent_directory_process_commands() {
        let command = AdapterProcessCommand {
            program: "../adapter".to_string(),
            args: Vec::new(),
        };
        let error = ProcessCommand::from_manifest(Path::new("."), &command)
            .expect_err("parent directory command should be rejected");

        assert!(error.to_string().contains("parent directory"));
    }

    #[test]
    fn external_process_timeout_is_reported() {
        let command = sleep_command();
        let error = run_process_adapter_with_limits::<_, Value>(
            "checker",
            "external.checker.sleep",
            &command,
            &serde_json::json!({}),
            ProcessLimits {
                timeout: Duration::from_millis(50),
                max_stdin_bytes: 1024,
                max_stdout_bytes: 1024,
                max_stderr_bytes: 1024,
            },
        )
        .expect_err("sleeping process should time out");

        assert!(error.to_string().contains("timed out"));
    }

    #[test]
    fn external_process_oversized_stdout_is_reported() {
        let command = stdout_bytes_command(2048);
        let error = run_process_adapter_with_limits::<_, Value>(
            "checker",
            "external.checker.big_stdout",
            &command,
            &serde_json::json!({}),
            ProcessLimits {
                timeout: Duration::from_secs(5),
                max_stdin_bytes: 1024,
                max_stdout_bytes: 32,
                max_stderr_bytes: 1024,
            },
        )
        .expect_err("oversized stdout should fail");

        assert!(error.to_string().contains("stdout exceeded"));
    }

    #[test]
    fn external_process_nonzero_exit_reports_bounded_stderr() {
        let command = failing_command();
        let error = run_process_adapter_with_limits::<_, Value>(
            "checker",
            "external.checker.fail",
            &command,
            &serde_json::json!({}),
            ProcessLimits {
                timeout: Duration::from_secs(5),
                max_stdin_bytes: 1024,
                max_stdout_bytes: 1024,
                max_stderr_bytes: 1024,
            },
        )
        .expect_err("non-zero process should fail");

        let message = error.to_string();
        assert!(message.contains("exited with"));
        assert!(message.contains("intentional failure"));
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

    #[cfg(windows)]
    fn empty_output_program() -> PathBuf {
        powershell_path()
    }

    #[cfg(not(windows))]
    fn empty_output_command() -> AdapterProcessCommand {
        sh_json_command("{\"entities\":[],\"facts\":[]}")
    }

    #[cfg(not(windows))]
    fn empty_output_program() -> PathBuf {
        sh_path()
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
            program: powershell_path().display().to_string(),
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
            program: sh_path().display().to_string(),
            args: vec![
                "-c".to_string(),
                format!(
                    "cat >/dev/null; printf '%s' '{}'",
                    json.replace('\'', "'\\''")
                ),
            ],
        }
    }

    #[cfg(windows)]
    fn sleep_command() -> ProcessCommand {
        ProcessCommand {
            program: powershell_path(),
            args: vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "$input | Out-Null; Start-Sleep -Seconds 5".to_string(),
            ],
        }
    }

    #[cfg(not(windows))]
    fn sleep_command() -> ProcessCommand {
        ProcessCommand {
            program: sh_path(),
            args: vec!["-c".to_string(), "cat >/dev/null; sleep 5".to_string()],
        }
    }

    #[cfg(windows)]
    fn stdout_bytes_command(bytes: usize) -> ProcessCommand {
        ProcessCommand {
            program: powershell_path(),
            args: vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                format!("$input | Out-Null; [Console]::Out.Write(('x' * {bytes}))"),
            ],
        }
    }

    #[cfg(not(windows))]
    fn stdout_bytes_command(bytes: usize) -> ProcessCommand {
        ProcessCommand {
            program: sh_path(),
            args: vec![
                "-c".to_string(),
                format!("cat >/dev/null; yes x | tr -d '\\n' | head -c {bytes}"),
            ],
        }
    }

    #[cfg(windows)]
    fn failing_command() -> ProcessCommand {
        ProcessCommand {
            program: powershell_path(),
            args: vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "$input | Out-Null; [Console]::Error.Write('intentional failure'); exit 7"
                    .to_string(),
            ],
        }
    }

    #[cfg(not(windows))]
    fn failing_command() -> ProcessCommand {
        ProcessCommand {
            program: sh_path(),
            args: vec![
                "-c".to_string(),
                "cat >/dev/null; printf '%s' 'intentional failure' >&2; exit 7".to_string(),
            ],
        }
    }

    #[cfg(windows)]
    fn powershell_path() -> PathBuf {
        let path = PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
        assert!(
            path.is_file(),
            "powershell.exe not found at {}",
            path.display()
        );
        path
    }

    #[cfg(not(windows))]
    fn sh_path() -> PathBuf {
        for candidate in ["/bin/sh", "/usr/bin/sh"] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return path;
            }
        }
        panic!("sh executable not found");
    }
}
