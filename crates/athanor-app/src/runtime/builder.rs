use std::path::PathBuf;

use anyhow::{Result, bail};
use athanor_core::{KnowledgeStore, SourceProvider};
use tracing::warn;

use super::process_adapter_support::{
    ProcessCommand, resolve_external_process_allowlist,
};
use super::{AdapterRegistry, BuiltinAdapterResolver};
use crate::{IndexPipeline, RuntimeComposition};

pub struct RuntimeBuilder {
    root: PathBuf,
    registry: AdapterRegistry,
    allow_external_process: bool,
    clear_external_process_environment: bool,
    allowed_external_process_programs: Vec<PathBuf>,
    adapter_trust_path: Option<PathBuf>,
    builtin_adapter_resolver: Option<BuiltinAdapterResolver>,
}

impl RuntimeBuilder {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            registry: AdapterRegistry::built_in(),
            allow_external_process: false,
            clear_external_process_environment: false,
            allowed_external_process_programs: Vec::new(),
            adapter_trust_path: None,
            builtin_adapter_resolver: None,
        }
    }

    /// Starts a builder from an explicit application composition.
    ///
    /// Unlike [`Self::new`], this does not consult process-global adapter factories.
    pub fn from_composition(root: impl Into<PathBuf>, composition: &RuntimeComposition) -> Self {
        Self {
            root: root.into(),
            registry: composition.adapter_registry(),
            allow_external_process: false,
            clear_external_process_environment: false,
            allowed_external_process_programs: Vec::new(),
            adapter_trust_path: None,
            builtin_adapter_resolver: Some(composition.builtin_adapter_resolver()),
        }
    }

    pub fn with_registry(mut self, registry: AdapterRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn with_builtin_adapter_resolver(mut self, resolver: BuiltinAdapterResolver) -> Self {
        self.builtin_adapter_resolver = Some(resolver);
        self
    }

    pub fn allow_external_process(mut self, allowed: bool) -> Self {
        self.allow_external_process = allowed;
        self
    }

    /// Enables the opt-in clean-environment external process sandbox profile.
    pub fn clear_external_process_environment(mut self, enabled: bool) -> Self {
        self.clear_external_process_environment = enabled;
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

        for plugin in super::discover_adapter_plugins(&self.root)? {
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
                        None => super::default_adapter_trust_path()?,
                    };
                    trust_registry = Some(super::plugin_trust_registry::load(&trust_path)?);
                }
                let registry = trust_registry
                    .as_ref()
                    .expect("adapter trust registry was loaded for external adapters");
                if !super::plugin_trust_status::is_trusted(
                    registry,
                    &plugin,
                    super::trust::trusted_plugin_record,
                )? {
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
            self.registry = self.registry.with_plugin_manifest_at_using(
                &plugin.manifest,
                manifest_dir,
                self.builtin_adapter_resolver,
                self.clear_external_process_environment,
            )?;
        }

        Ok(self)
    }

    pub fn build_index_pipeline(self, store: impl KnowledgeStore + 'static) -> IndexPipeline {
        self.registry.build_index_pipeline(&self.root, store)
    }

    pub fn build_extraction_pipeline(
        self,
        source: Box<dyn SourceProvider>,
        store: impl KnowledgeStore + 'static,
    ) -> IndexPipeline {
        let mut pipeline = IndexPipeline::new(store).boxed_source(source);
        for factory in &self.registry.extractor_factories {
            pipeline = pipeline.boxed_extractor(factory());
        }
        pipeline
    }
}
