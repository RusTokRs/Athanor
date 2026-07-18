use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_core::{Checker, Extractor, KnowledgeStore, Linker, SourceProvider};

use super::model::{AdapterPluginEntry, AdapterPluginKind, AdapterPluginManifest};
use super::process_adapter;
use super::process_adapter_support::{ProcessCommand, normalize_extension};
use crate::IndexPipeline;

pub(super) type SourceFactory = Box<dyn Fn(&Path) -> Box<dyn SourceProvider> + Send + Sync>;
pub(super) type ExtractorFactory = Box<dyn Fn() -> Box<dyn Extractor> + Send + Sync>;
pub(super) type LinkerFactory = Box<dyn Fn() -> Box<dyn Linker> + Send + Sync>;
pub(super) type CheckerFactory = Box<dyn Fn() -> Box<dyn Checker> + Send + Sync>;

pub type AdapterRegistryFactory = fn() -> AdapterRegistry;
pub type BuiltinAdapterResolver =
    fn(AdapterRegistry, AdapterPluginKind, &str) -> Option<AdapterRegistry>;

pub struct AdapterRegistry {
    adapter_ids: BTreeSet<String>,
    source_factories: Vec<SourceFactory>,
    pub(super) extractor_factories: Vec<ExtractorFactory>,
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

    pub fn with_plugin_manifest(self, manifest: &AdapterPluginManifest) -> Result<Self> {
        self.with_plugin_manifest_at(manifest, Path::new("."))
    }

    pub fn with_plugin_manifest_at(
        self,
        manifest: &AdapterPluginManifest,
        manifest_dir: &Path,
    ) -> Result<Self> {
        self.with_plugin_manifest_at_using(manifest, manifest_dir, None, false)
    }

    pub(super) fn with_plugin_manifest_at_using(
        mut self,
        manifest: &AdapterPluginManifest,
        manifest_dir: &Path,
        builtin_resolver: Option<BuiltinAdapterResolver>,
        clear_external_process_environment: bool,
    ) -> Result<Self> {
        super::plugin_discovery::validate(manifest)?;

        for adapter in manifest.adapters.iter().filter(|adapter| adapter.enabled) {
            if self.adapter_ids.contains(&adapter.id) {
                continue;
            }
            self = self
                .with_adapter_entry_using(
                    adapter,
                    manifest_dir,
                    builtin_resolver,
                    clear_external_process_environment,
                )
                .with_context(|| {
                    format!(
                        "failed to register adapter {} from plugin {}",
                        adapter.id, manifest.name
                    )
                })?;
        }

        Ok(self)
    }

    fn with_adapter_entry_using(
        self,
        adapter: &AdapterPluginEntry,
        manifest_dir: &Path,
        builtin_resolver: Option<BuiltinAdapterResolver>,
        clear_external_process_environment: bool,
    ) -> Result<Self> {
        if adapter.command.is_none()
            && let Some(resolver) = builtin_resolver
        {
            if let Some(registry) = resolver(self, adapter.kind, adapter.id.as_str()) {
                return Ok(registry);
            }

            bail!(
                "unknown {:?} adapter id {} and no command was provided",
                adapter.kind,
                adapter.id
            );
        }

        match adapter.kind {
            AdapterPluginKind::Source => self.external_process_source(
                adapter,
                manifest_dir,
                clear_external_process_environment,
            ),
            AdapterPluginKind::Extractor => self.external_process_extractor(
                adapter,
                manifest_dir,
                clear_external_process_environment,
            ),
            AdapterPluginKind::Linker => self.external_process_linker(
                adapter,
                manifest_dir,
                clear_external_process_environment,
            ),
            AdapterPluginKind::Checker => self.external_process_checker(
                adapter,
                manifest_dir,
                clear_external_process_environment,
            ),
        }
    }

    fn external_process_source(
        self,
        adapter: &AdapterPluginEntry,
        manifest_dir: &Path,
        clear_environment: bool,
    ) -> Result<Self> {
        let Some(command) = &adapter.command else {
            bail!(
                "unknown source adapter id {} and no command was provided",
                adapter.id
            );
        };
        let command =
            ProcessCommand::from_manifest_with_sandbox(manifest_dir, command, clear_environment)?;
        let id = adapter.id.clone();

        Ok(self.register_source_id(&adapter.id, move |root| {
            process_adapter::source(id.clone(), command.clone(), root.to_path_buf())
        }))
    }

    fn external_process_extractor(
        self,
        adapter: &AdapterPluginEntry,
        manifest_dir: &Path,
        clear_environment: bool,
    ) -> Result<Self> {
        let Some(command) = &adapter.command else {
            bail!(
                "unknown extractor adapter id {} and no command was provided",
                adapter.id
            );
        };
        let command =
            ProcessCommand::from_manifest_with_sandbox(manifest_dir, command, clear_environment)?;
        let id = adapter.id.clone();
        let supports_extensions = adapter
            .supports_extensions
            .iter()
            .map(normalize_extension)
            .collect::<BTreeSet<_>>();

        Ok(self.register_extractor_id(&adapter.id, move || {
            process_adapter::extractor(id.clone(), command.clone(), supports_extensions.clone())
        }))
    }

    fn external_process_linker(
        self,
        adapter: &AdapterPluginEntry,
        manifest_dir: &Path,
        clear_environment: bool,
    ) -> Result<Self> {
        let Some(command) = &adapter.command else {
            bail!(
                "unknown linker adapter id {} and no command was provided",
                adapter.id
            );
        };
        let command =
            ProcessCommand::from_manifest_with_sandbox(manifest_dir, command, clear_environment)?;
        let id = adapter.id.clone();

        Ok(self.register_linker_id(&adapter.id, move || {
            process_adapter::linker(id.clone(), command.clone())
        }))
    }

    fn external_process_checker(
        self,
        adapter: &AdapterPluginEntry,
        manifest_dir: &Path,
        clear_environment: bool,
    ) -> Result<Self> {
        let Some(command) = &adapter.command else {
            bail!(
                "unknown checker adapter id {} and no command was provided",
                adapter.id
            );
        };
        let command =
            ProcessCommand::from_manifest_with_sandbox(manifest_dir, command, clear_environment)?;
        let id = adapter.id.clone();

        Ok(self.register_checker_id(&adapter.id, move || {
            process_adapter::checker(id.clone(), command.clone())
        }))
    }

    pub fn register_source_id(
        mut self,
        id: &str,
        factory: impl Fn(&Path) -> Box<dyn SourceProvider> + Send + Sync + 'static,
    ) -> Self {
        if self.adapter_ids.insert(id.to_string()) {
            self.source_factories.push(Box::new(factory));
        }
        self
    }

    pub fn register_extractor_id(
        mut self,
        id: &str,
        factory: impl Fn() -> Box<dyn Extractor> + Send + Sync + 'static,
    ) -> Self {
        if self.adapter_ids.insert(id.to_string()) {
            self.extractor_factories.push(Box::new(factory));
        }
        self
    }

    pub fn register_linker_id(
        mut self,
        id: &str,
        factory: impl Fn() -> Box<dyn Linker> + Send + Sync + 'static,
    ) -> Self {
        if self.adapter_ids.insert(id.to_string()) {
            self.linker_factories.push(Box::new(factory));
        }
        self
    }

    pub fn register_checker_id(
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
        Self::empty()
    }
}
