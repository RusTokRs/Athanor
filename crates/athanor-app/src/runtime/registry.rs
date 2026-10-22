use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_core::{Checker, Extractor, KnowledgeStore, Linker, SourceProvider};

use super::model::{AdapterPluginEntry, AdapterPluginKind, AdapterPluginManifest};
use super::process_adapter;
use super::process_adapter_support::{ProcessCommand, ProcessLimits, normalize_extension};
use super::TrustedAdapterExecutable;
use crate::IndexPipeline;

pub(super) type SourceFactory = Box<dyn Fn(&Path) -> Box<dyn SourceProvider> + Send + Sync>;
pub(super) type ExtractorFactory = Box<dyn Fn() -> Box<dyn Extractor> + Send + Sync>;
pub(super) type LinkerFactory = Box<dyn Fn() -> Box<dyn Linker> + Send + Sync>;
pub(super) type CheckerFactory = Box<dyn Fn() -> Box<dyn Checker> + Send + Sync>;

pub type AdapterRegistryFactory = fn() -> AdapterRegistry;
pub type BuiltinAdapterResolver =
    fn(AdapterRegistry, AdapterPluginKind, &str) -> Option<AdapterRegistry>;

struct NamedFactory<T> {
    id: String,
    factory: T,
}

pub(super) struct PluginRegistrationRequest<'a> {
    pub manifest: &'a AdapterPluginManifest,
    pub manifest_dir: &'a Path,
    pub builtin_resolver: Option<BuiltinAdapterResolver>,
    pub clear_external_process_environment: bool,
    pub trusted_payload: Option<&'a [TrustedAdapterExecutable]>,
    pub limits: ProcessLimits,
}

pub struct AdapterRegistry {
    adapter_ids: BTreeSet<String>,
    source_factories: Vec<NamedFactory<SourceFactory>>,
    extractor_factories: Vec<NamedFactory<ExtractorFactory>>,
    linker_factories: Vec<NamedFactory<LinkerFactory>>,
    checker_factories: Vec<NamedFactory<CheckerFactory>>,
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
        self.with_plugin_manifest_at_using(PluginRegistrationRequest {
            manifest,
            manifest_dir,
            builtin_resolver: None,
            clear_external_process_environment: false,
            trusted_payload: None,
            limits: ProcessLimits::default(),
        })
    }

    pub(super) fn with_plugin_manifest_at_using(
        mut self,
        request: PluginRegistrationRequest<'_>,
    ) -> Result<Self> {
        super::plugin_discovery::validate(request.manifest)?;

        for adapter in request
            .manifest
            .adapters
            .iter()
            .filter(|adapter| adapter.enabled)
        {
            if self.adapter_ids.contains(&adapter.id) {
                if adapter.command.is_some() {
                    bail!(
                        "adapter id {} from plugin {} is already registered and cannot be overridden by an external process adapter",
                        adapter.id,
                        request.manifest.name
                    );
                }
                continue;
            }
            self = self
                .with_adapter_entry_using(adapter, &request)
                .with_context(|| {
                    format!(
                        "failed to register adapter {} from plugin {}",
                        adapter.id, request.manifest.name
                    )
                })?;
        }

        Ok(self)
    }

    pub fn without_adapters(mut self, ids: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        for id in ids {
            let id = id.as_ref();
            if self.adapter_ids.remove(id) {
                self.source_factories.retain(|factory| factory.id != id);
                self.extractor_factories.retain(|factory| factory.id != id);
                self.linker_factories.retain(|factory| factory.id != id);
                self.checker_factories.retain(|factory| factory.id != id);
            }
        }
        self
    }

    pub fn contains_adapter(&self, id: &str) -> bool {
        self.adapter_ids.contains(id)
    }

    fn with_adapter_entry_using(
        self,
        adapter: &AdapterPluginEntry,
        request: &PluginRegistrationRequest<'_>,
    ) -> Result<Self> {
        if adapter.command.is_none()
            && let Some(resolver) = request.builtin_resolver
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
            AdapterPluginKind::Source => self.external_process_source(adapter, request),
            AdapterPluginKind::Extractor => self.external_process_extractor(adapter, request),
            AdapterPluginKind::Linker => self.external_process_linker(adapter, request),
            AdapterPluginKind::Checker => self.external_process_checker(adapter, request),
        }
    }

    fn external_process_source(
        self,
        adapter: &AdapterPluginEntry,
        request: &PluginRegistrationRequest<'_>,
    ) -> Result<Self> {
        let command = process_command(adapter, request)?;
        let id = adapter.id.clone();
        Ok(self.register_source_id(&adapter.id, move |root| {
            process_adapter::source(id.clone(), command.clone(), root.to_path_buf())
        }))
    }

    fn external_process_extractor(
        self,
        adapter: &AdapterPluginEntry,
        request: &PluginRegistrationRequest<'_>,
    ) -> Result<Self> {
        let command = process_command(adapter, request)?;
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
        request: &PluginRegistrationRequest<'_>,
    ) -> Result<Self> {
        let command = process_command(adapter, request)?;
        let id = adapter.id.clone();
        Ok(self.register_linker_id(&adapter.id, move || {
            process_adapter::linker(id.clone(), command.clone())
        }))
    }

    fn external_process_checker(
        self,
        adapter: &AdapterPluginEntry,
        request: &PluginRegistrationRequest<'_>,
    ) -> Result<Self> {
        let command = process_command(adapter, request)?;
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
        self.push_named(
            id,
            Box::new(factory),
            |registry| &mut registry.source_factories,
        );
        self
    }

    pub fn register_extractor_id(
        mut self,
        id: &str,
        factory: impl Fn() -> Box<dyn Extractor> + Send + Sync + 'static,
    ) -> Self {
        self.push_named(
            id,
            Box::new(factory),
            |registry| &mut registry.extractor_factories,
        );
        self
    }

    pub fn register_linker_id(
        mut self,
        id: &str,
        factory: impl Fn() -> Box<dyn Linker> + Send + Sync + 'static,
    ) -> Self {
        self.push_named(
            id,
            Box::new(factory),
            |registry| &mut registry.linker_factories,
        );
        self
    }

    pub fn register_checker_id(
        mut self,
        id: &str,
        factory: impl Fn() -> Box<dyn Checker> + Send + Sync + 'static,
    ) -> Self {
        self.push_named(
            id,
            Box::new(factory),
            |registry| &mut registry.checker_factories,
        );
        self
    }

    pub(super) fn extractor_factories(&self) -> impl Iterator<Item = &ExtractorFactory> {
        self.extractor_factories.iter().map(|named| &named.factory)
    }

    pub fn build_index_pipeline(
        &self,
        root: &Path,
        store: impl KnowledgeStore + 'static,
    ) -> IndexPipeline {
        let mut pipeline = IndexPipeline::new(store);
        for named in &self.source_factories {
            pipeline = pipeline.boxed_source((named.factory)(root));
        }
        for named in &self.extractor_factories {
            pipeline = pipeline.boxed_extractor((named.factory)());
        }
        for named in &self.linker_factories {
            pipeline = pipeline.boxed_linker((named.factory)());
        }
        for named in &self.checker_factories {
            pipeline = pipeline.boxed_checker((named.factory)());
        }
        pipeline
    }

    fn push_named<T>(
        &mut self,
        id: &str,
        factory: T,
        factories: impl FnOnce(&mut Self) -> &mut Vec<NamedFactory<T>>,
    ) {
        let inserted = self.adapter_ids.insert(id.to_string());
        if inserted {
            factories(self).push(NamedFactory {
                id: id.to_string(),
                factory,
            });
        }
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::empty()
    }
}

fn process_command(
    adapter: &AdapterPluginEntry,
    request: &PluginRegistrationRequest<'_>,
) -> Result<ProcessCommand> {
    let Some(command) = &adapter.command else {
        bail!(
            "unknown {:?} adapter id {} and no command was provided",
            adapter.kind,
            adapter.id
        );
    };
    ProcessCommand::from_manifest_with_options(
        request.manifest_dir,
        command,
        request.clear_external_process_environment,
        request.limits,
        request.trusted_payload,
    )
}
