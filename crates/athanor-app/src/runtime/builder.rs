use std::path::PathBuf;

use anyhow::{Result, bail};
use athanor_core::{KnowledgeStore, SourceProvider};
use tracing::warn;

use super::process_adapter_support::{
    ProcessLimits, resolve_external_process_allowlist, resolve_manifest_program,
};
use super::registry::PluginRegistrationRequest;
use super::{
    AdapterRegistry, AdapterTrustRegistry, BuiltinAdapterResolver, DiscoveredAdapterPlugin,
    TrustedAdapterExecutable,
};
use crate::config::AdaptersConfig;
use crate::{IndexPipeline, RuntimeComposition};

pub struct RuntimeBuilder {
    root: PathBuf,
    registry: AdapterRegistry,
    allow_external_process: bool,
    clear_external_process_environment: bool,
    allowed_external_process_programs: Vec<PathBuf>,
    adapter_trust_path: Option<PathBuf>,
    builtin_adapter_resolver: Option<BuiltinAdapterResolver>,
    disabled_adapter_ids: Vec<String>,
    process_limits: ProcessLimits,
}

impl RuntimeBuilder {
    /// Starts a builder with an empty registry in production.
    ///
    /// Unit tests receive a fresh local test composition so legacy test helpers remain deterministic
    /// without process-global installation. Production callers should use [`Self::from_composition`]
    /// or provide an explicit registry and built-in resolver.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();

        #[cfg(test)]
        {
            Self::from_composition(root, &crate::test_runtime::composition())
        }

        #[cfg(not(test))]
        Self {
            root,
            registry: AdapterRegistry::empty(),
            allow_external_process: false,
            clear_external_process_environment: false,
            allowed_external_process_programs: Vec::new(),
            adapter_trust_path: None,
            builtin_adapter_resolver: None,
            disabled_adapter_ids: Vec::new(),
            process_limits: ProcessLimits::default(),
        }
    }

    /// Starts a builder from an explicit application composition.
    pub fn from_composition(root: impl Into<PathBuf>, composition: &RuntimeComposition) -> Self {
        Self {
            root: root.into(),
            registry: composition.adapter_registry(),
            allow_external_process: false,
            clear_external_process_environment: false,
            allowed_external_process_programs: Vec::new(),
            adapter_trust_path: None,
            builtin_adapter_resolver: Some(composition.builtin_adapter_resolver()),
            disabled_adapter_ids: Vec::new(),
            process_limits: ProcessLimits::default(),
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

    pub fn with_adapters_config(mut self, config: &AdaptersConfig) -> Self {
        self.allow_external_process = config.allow_external_process;
        self.clear_external_process_environment = matches!(
            config.external_process_sandbox,
            crate::config::ExternalProcessSandboxProfile::CleanEnvironment
        );
        self.allowed_external_process_programs = config
            .external_process_allowlist
            .iter()
            .map(PathBuf::from)
            .collect();
        self.disabled_adapter_ids = config.disabled.clone();
        self.process_limits = ProcessLimits {
            timeout: std::time::Duration::from_millis(config.external_process_timeout_ms),
            max_stdin_bytes: config.external_process_max_stdin_bytes,
            max_stdout_bytes: config.external_process_max_stdout_bytes,
            max_stderr_bytes: config.external_process_max_stderr_bytes,
        };
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

    pub fn disabled_adapters(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.disabled_adapter_ids = ids.into_iter().map(Into::into).collect();
        self
    }

    pub fn adapter_trust_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.adapter_trust_path = Some(path.into());
        self
    }

    pub fn with_discovered_plugins(mut self) -> Result<Self> {
        let mut trust_registry = None;
        for plugin in super::discover_adapter_plugins(&self.root)? {
            let trusted_payload = self.bind_external_plugin(&plugin, &mut trust_registry)?;
            let manifest_dir = plugin.manifest_path.parent().unwrap_or(&self.root);
            self.registry = self.registry.with_plugin_manifest_at_using(
                PluginRegistrationRequest {
                    manifest: &plugin.manifest,
                    manifest_dir,
                    builtin_resolver: self.builtin_adapter_resolver,
                    clear_external_process_environment: self.clear_external_process_environment,
                    trusted_payload: trusted_payload.as_deref(),
                    limits: self.process_limits,
                },
            )?;
        }
        Ok(self)
    }

    fn bind_external_plugin(
        &self,
        plugin: &DiscoveredAdapterPlugin,
        trust_registry: &mut Option<AdapterTrustRegistry>,
    ) -> Result<Option<Vec<TrustedAdapterExecutable>>> {
        let external = plugin
            .manifest
            .adapters
            .iter()
            .filter(|adapter| adapter.enabled && adapter.command.is_some())
            .collect::<Vec<_>>();
        if external.is_empty() {
            return Ok(None);
        }
        if !self.allow_external_process {
            bail!(
                "external process adapters are disabled; set [adapters] allow_external_process = true to enable plugin `{}`",
                plugin.manifest.name
            );
        }
        let allowlist = resolve_external_process_allowlist(
            &self.root,
            &self.allowed_external_process_programs,
        )?;
        let manifest_dir = plugin.manifest_path.parent().unwrap_or(&self.root);
        for adapter in &external {
            let command = adapter.command.as_ref().expect(
                "external adapters were already filtered to entries that declare command",
            );
            let resolved = resolve_manifest_program(manifest_dir, &command.program)?;
            if !allowlist.contains(&resolved) {
                bail!(
                    "external process adapter `{}` command {} is not in [adapters].external_process_allowlist",
                    adapter.id,
                    resolved.display()
                );
            }
        }
        if trust_registry.is_none() {
            let trust_path = match &self.adapter_trust_path {
                Some(path) => path.clone(),
                None => super::default_adapter_trust_path()?,
            };
            *trust_registry = Some(super::plugin_trust_registry::load(&trust_path)?);
        }
        let registry = trust_registry
            .as_ref()
            .expect("adapter trust registry was loaded for external adapters");
        let trusted = super::trust::trusted_plugin_record(plugin)?;
        if !super::plugin_trust_status::record_matches(registry, &trusted) {
            bail!(
                "external process adapter plugin `{}` is not trusted; run `ath plugins trust {}` to trust this manifest version",
                plugin.manifest.name,
                plugin.manifest_path.display()
            );
        }
        for adapter in external {
            warn!(
                plugin = %plugin.manifest.name,
                adapter = %adapter.id,
                "external process adapter execution is enabled"
            );
        }
        Ok(Some(trusted.executable_hashes))
    }

    pub fn build_index_pipeline(self, store: impl KnowledgeStore + 'static) -> IndexPipeline {
        let this = self.apply_disabled();
        this.registry.build_index_pipeline(&this.root, store)
    }

    pub fn build_extraction_pipeline(
        self,
        source: Box<dyn SourceProvider>,
        store: impl KnowledgeStore + 'static,
    ) -> IndexPipeline {
        let this = self.apply_disabled();
        let mut pipeline = IndexPipeline::new(store).boxed_source(source);
        for factory in this.registry.extractor_factories() {
            pipeline = pipeline.boxed_extractor(factory());
        }
        pipeline
    }

    fn apply_disabled(mut self) -> Self {
        self.registry = self.registry.without_adapters(&self.disabled_adapter_ids);
        self
    }
}
