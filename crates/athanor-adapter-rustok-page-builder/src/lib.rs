use std::collections::{BTreeMap, BTreeSet, HashMap};

use async_trait::async_trait;
use athanor_core::{
    CheckInput, Checker, CoreResult, ExtractInput, ExtractOutput, Extractor, LinkInput, Linker,
    SourceFile,
};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, Fact, FactId, FactKind, Relation, RelationId, RelationKind, RelationStatus, Severity,
    SnapshotId, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const PAGE_BUILDER_EXTRACTOR_ID: &str = "rustok_page_builder";
pub const PAGE_BUILDER_LINKER_ID: &str = "rustok_page_builder_linker";
pub const PAGE_BUILDER_CHECKER_ID: &str = "rustok_page_builder_checker";

pub const PAGE_BUILDER_PROVIDER_FACT_KIND: &str = "rustok_page_builder_provider_registry";
pub const PAGE_BUILDER_CONSUMER_FACT_KIND: &str = "rustok_page_builder_consumer_manifest";
pub const PAGE_BUILDER_ADAPTER_SEAM_FACT_KIND: &str = "rustok_page_builder_adapter_seams";
pub const PAGE_BUILDER_WAVE_EVIDENCE_FACT_KIND: &str = "rustok_page_builder_wave_evidence";
pub const PAGE_BUILDER_CONTENT_FACT_KIND: &str = "rustok_page_builder_content_surface";
pub const PAGE_BUILDER_FSD_FACT_KIND: &str = "rustok_page_builder_fsd_surface";

pub const PAGE_BUILDER_PROVIDER_ENTITY_KIND: &str = "rustok_page_builder_provider";
pub const PAGE_BUILDER_CONSUMER_ENTITY_KIND: &str = "rustok_page_builder_consumer";
pub const PAGE_BUILDER_CONTRACT_ENTITY_KIND: &str = "rustok_page_builder_contract";
pub const PAGE_BUILDER_CAPABILITY_ENTITY_KIND: &str = "rustok_page_builder_capability";
pub const PAGE_BUILDER_FALLBACK_PROFILE_ENTITY_KIND: &str = "rustok_page_builder_fallback_profile";
pub const PAGE_BUILDER_WAVE_EVIDENCE_ENTITY_KIND: &str = "rustok_page_builder_wave_evidence";
pub const PAGE_BUILDER_ADAPTER_SEAM_ENTITY_KIND: &str = "rustok_page_builder_adapter_seam";
pub const PAGE_BUILDER_CONTENT_SURFACE_ENTITY_KIND: &str = "rustok_page_builder_content_surface";
pub const PAGE_BUILDER_FSD_SURFACE_ENTITY_KIND: &str = "rustok_page_builder_fsd_surface";

#[derive(Debug, Clone, Default)]
pub struct RustokPageBuilderExtractor;

#[derive(Debug, Clone, Default)]
pub struct RustokPageBuilderLinker;

#[derive(Debug, Clone, Default)]
pub struct RustokPageBuilderChecker;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderProviderMarker {
    pub schema: String,
    pub path: String,
    pub module: String,
    pub manifest_module: Option<String>,
    pub contract: String,
    pub builder_contract_version: Option<String>,
    pub consumer_min_version: Option<String>,
    pub capabilities: Vec<String>,
    pub fallback_profiles: Vec<String>,
    pub health_states: Vec<String>,
    pub degradation_reasons: Vec<String>,
    pub consumers: Vec<PageBuilderRegistryConsumer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderRegistryConsumer {
    pub module: String,
    pub crate_name: Option<String>,
    pub provider_module: Option<String>,
    pub contract: Option<String>,
    pub contract_version: Option<String>,
    pub builder_contract_version: Option<String>,
    pub consumer_min_version: Option<String>,
    pub capabilities: Vec<String>,
    pub rollout_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderConsumerManifestMarker {
    pub schema: String,
    pub path: String,
    pub module: String,
    pub provider_module: Option<String>,
    pub contract: Option<String>,
    pub contract_version: Option<String>,
    pub builder_contract_version: Option<String>,
    pub consumer_min_version: Option<String>,
    pub capabilities: Vec<String>,
    pub fallback_profiles: Vec<String>,
    pub degraded_modes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderAdapterSeamMarker {
    pub schema: String,
    pub path: String,
    pub module: String,
    pub seams: Vec<String>,
    pub canonical_entrypoints: Vec<String>,
    pub blocked_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderWaveEvidenceMarker {
    pub schema: String,
    pub path: String,
    pub module: String,
    pub wave: Option<u64>,
    pub mode: Option<String>,
    pub created_at: Option<String>,
    pub fallback_profiles: Vec<String>,
    pub trace_profiles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderContentMarker {
    pub schema: String,
    pub path: String,
    pub module: String,
    pub format: String,
    pub first_marker_line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderFsdMarker {
    pub schema: String,
    pub path: String,
    pub module: String,
    pub surface: String,
    pub role: String,
    pub has_leptos_marker: bool,
    pub has_component_marker: bool,
    pub has_server_fn_marker: bool,
    pub calls_raw_transport: bool,
    pub calls_transport_facade: bool,
    pub host_wiring: bool,
    pub first_marker_line: Option<u32>,
}

#[async_trait]
impl Extractor for RustokPageBuilderExtractor {
    fn name(&self) -> &str {
        PAGE_BUILDER_EXTRACTOR_ID
    }

    fn supports(&self, source: &SourceFile) -> bool {
        let path = normalize_path(&source.path);
        is_provider_registry_path(&path)
            || is_adapter_seams_path(&path)
            || is_wave_evidence_path(&path)
            || is_consumer_manifest_path(&path)
            || classify_fsd_path(&path).is_some()
            || is_host_mount_source_path(&path)
            || is_content_source_path(&path)
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let path = normalize_path(&input.source.path);
        if is_provider_registry_path(&path) {
            return Ok(extract_provider_registry(
                &input.source,
                &input.snapshot,
                &path,
            ));
        }
        if is_adapter_seams_path(&path) {
            return Ok(extract_adapter_seams(&input.source, &input.snapshot, &path));
        }
        if is_wave_evidence_path(&path) {
            return Ok(extract_wave_evidence(&input.source, &input.snapshot, &path));
        }
        if is_consumer_manifest_path(&path) {
            return Ok(extract_consumer_manifest(
                &input.source,
                &input.snapshot,
                &path,
            ));
        }
        if let Some((module, surface, role, host_wiring)) = classify_fsd_path(&path) {
            return Ok(extract_fsd_surface(
                &input.source,
                &input.snapshot,
                &path,
                module,
                surface,
                role,
                host_wiring,
            ));
        }
        if is_host_mount_source_path(&path) {
            return Ok(extract_host_mount_surfaces(
                &input.source,
                &input.snapshot,
                &path,
            ));
        }
        if is_content_source_path(&path) {
            return Ok(extract_content_surface(
                &input.source,
                &input.snapshot,
                &path,
            ));
        }
        Ok(ExtractOutput::default())
    }
}

#[async_trait]
impl Linker for RustokPageBuilderLinker {
    fn name(&self) -> &str {
        PAGE_BUILDER_LINKER_ID
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
        let entity_by_id = input
            .entities
            .iter()
            .map(|entity| (entity.id.0.as_str(), entity))
            .collect::<HashMap<_, _>>();
        let mut relations = Vec::new();
        let mut seen = BTreeSet::new();

        for fact in &*input.facts {
            match fact.kind {
                FactKind::Other(ref kind) if kind == PAGE_BUILDER_PROVIDER_FACT_KIND => {
                    let Some(marker) = provider_from_fact(fact) else {
                        continue;
                    };
                    let Some(file) = entity_by_id.get(fact.subject.0.as_str()) else {
                        continue;
                    };
                    let provider = provider_entity(&marker.module, &input.snapshot, None);
                    push_relation(
                        &mut relations,
                        &mut seen,
                        &input.snapshot,
                        RelationKind::ImplementedBy,
                        &provider.id,
                        &file.id,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                        json!({"schema":"rustok.page_builder.relation.v1","kind":"provider_evidenced_by"}),
                    );
                    let contract = contract_entity(&marker.contract, &input.snapshot, None);
                    push_relation(
                        &mut relations,
                        &mut seen,
                        &input.snapshot,
                        RelationKind::Contains,
                        &provider.id,
                        &contract.id,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                        json!({"schema":"rustok.page_builder.relation.v1","kind":"provider_exposes_contract"}),
                    );
                    for capability in &marker.capabilities {
                        let capability = capability_entity(capability, &input.snapshot, None);
                        push_relation(
                            &mut relations,
                            &mut seen,
                            &input.snapshot,
                            RelationKind::Contains,
                            &contract.id,
                            &capability.id,
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                            json!({"schema":"rustok.page_builder.relation.v1","kind":"contract_exposes_capability"}),
                        );
                    }
                    for profile in &marker.fallback_profiles {
                        let profile = fallback_profile_entity(profile, &input.snapshot, None);
                        push_relation(
                            &mut relations,
                            &mut seen,
                            &input.snapshot,
                            RelationKind::Contains,
                            &contract.id,
                            &profile.id,
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                            json!({"schema":"rustok.page_builder.relation.v1","kind":"contract_has_fallback_profile"}),
                        );
                    }
                    for consumer in &marker.consumers {
                        let consumer_entity =
                            consumer_entity(&consumer.module, &input.snapshot, None);
                        push_relation(
                            &mut relations,
                            &mut seen,
                            &input.snapshot,
                            RelationKind::Other(
                                "rustok_page_builder_consumer_requires_provider".to_string(),
                            ),
                            &consumer_entity.id,
                            &provider.id,
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                            json!({"schema":"rustok.page_builder.relation.v1","kind":"consumer_requires_provider"}),
                        );
                    }
                }
                FactKind::Other(ref kind) if kind == PAGE_BUILDER_CONSUMER_FACT_KIND => {
                    let Some(marker) = consumer_from_fact(fact) else {
                        continue;
                    };
                    let Some(file) = entity_by_id.get(fact.subject.0.as_str()) else {
                        continue;
                    };
                    let consumer = consumer_entity(&marker.module, &input.snapshot, None);
                    push_relation(
                        &mut relations,
                        &mut seen,
                        &input.snapshot,
                        RelationKind::ImplementedBy,
                        &consumer.id,
                        &file.id,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                        json!({"schema":"rustok.page_builder.relation.v1","kind":"consumer_evidenced_by"}),
                    );
                    for profile in &marker.fallback_profiles {
                        let profile = fallback_profile_entity(profile, &input.snapshot, None);
                        push_relation(
                            &mut relations,
                            &mut seen,
                            &input.snapshot,
                            RelationKind::Other(
                                "rustok_page_builder_consumer_uses_fallback_profile".to_string(),
                            ),
                            &consumer.id,
                            &profile.id,
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                            json!({"schema":"rustok.page_builder.relation.v1","kind":"consumer_uses_fallback_profile"}),
                        );
                    }
                }
                FactKind::Other(ref kind) if kind == PAGE_BUILDER_ADAPTER_SEAM_FACT_KIND => {
                    let Some(marker) = adapter_seam_from_fact(fact) else {
                        continue;
                    };
                    let Some(file) = entity_by_id.get(fact.subject.0.as_str()) else {
                        continue;
                    };
                    for seam in &marker.seams {
                        let seam = adapter_seam_entity(seam, &input.snapshot, None);
                        push_relation(
                            &mut relations,
                            &mut seen,
                            &input.snapshot,
                            RelationKind::ImplementedBy,
                            &seam.id,
                            &file.id,
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                            json!({"schema":"rustok.page_builder.relation.v1","kind":"adapter_seam_evidenced_by"}),
                        );
                    }
                }
                FactKind::Other(ref kind) if kind == PAGE_BUILDER_WAVE_EVIDENCE_FACT_KIND => {
                    let Some(marker) = wave_from_fact(fact) else {
                        continue;
                    };
                    let Some(file) = entity_by_id.get(fact.subject.0.as_str()) else {
                        continue;
                    };
                    let wave =
                        wave_evidence_entity(&marker.module, marker.wave, &input.snapshot, None);
                    push_relation(
                        &mut relations,
                        &mut seen,
                        &input.snapshot,
                        RelationKind::ImplementedBy,
                        &wave.id,
                        &file.id,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                        json!({"schema":"rustok.page_builder.relation.v1","kind":"wave_evidence_evidenced_by"}),
                    );
                    let consumer = consumer_entity(&marker.module, &input.snapshot, None);
                    push_relation(
                        &mut relations,
                        &mut seen,
                        &input.snapshot,
                        RelationKind::Other("rustok_page_builder_wave_covers_consumer".to_string()),
                        &wave.id,
                        &consumer.id,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                        json!({"schema":"rustok.page_builder.relation.v1","kind":"wave_evidence_covers_consumer"}),
                    );
                    for profile in &marker.fallback_profiles {
                        let profile = fallback_profile_entity(profile, &input.snapshot, None);
                        push_relation(
                            &mut relations,
                            &mut seen,
                            &input.snapshot,
                            RelationKind::Other(
                                "rustok_page_builder_wave_covers_profile".to_string(),
                            ),
                            &wave.id,
                            &profile.id,
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                            json!({"schema":"rustok.page_builder.relation.v1","kind":"wave_evidence_covers_profile"}),
                        );
                    }
                }
                FactKind::Other(ref kind) if kind == PAGE_BUILDER_CONTENT_FACT_KIND => {
                    let Some(marker) = content_from_fact(fact) else {
                        continue;
                    };
                    let surface = content_surface_entity(
                        &marker.module,
                        &marker.format,
                        &input.snapshot,
                        None,
                    );
                    if let Some(file) = entity_by_id.get(fact.subject.0.as_str()) {
                        push_relation(
                            &mut relations,
                            &mut seen,
                            &input.snapshot,
                            RelationKind::ImplementedBy,
                            &surface.id,
                            &file.id,
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                            json!({"schema":"rustok.page_builder.relation.v1","kind":"content_surface_uses_format"}),
                        );
                    }
                }
                FactKind::Other(ref kind) if kind == PAGE_BUILDER_FSD_FACT_KIND => {
                    let Some(marker) = fsd_from_fact(fact) else {
                        continue;
                    };
                    let surface =
                        fsd_surface_entity(&marker.module, &marker.surface, &input.snapshot, None);
                    if let Some(file) = entity_by_id.get(fact.subject.0.as_str()) {
                        push_relation(
                            &mut relations,
                            &mut seen,
                            &input.snapshot,
                            RelationKind::ImplementedBy,
                            &surface.id,
                            &file.id,
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                            json!({"schema":"rustok.page_builder.relation.v1","kind":"fsd_surface_implemented_by","role":marker.role}),
                        );
                        if marker.host_wiring {
                            push_relation(
                                &mut relations,
                                &mut seen,
                                &input.snapshot,
                                RelationKind::Other(
                                    "rustok_page_builder_host_mounts_module_surface".to_string(),
                                ),
                                &file.id,
                                &surface.id,
                                fact.evidence.clone(),
                                fact.ownership.clone(),
                                json!({"schema":"rustok.page_builder.relation.v1","kind":"host_mounts_module_owned_surface","role":marker.role}),
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(relations)
    }
}

#[async_trait]
impl Checker for RustokPageBuilderChecker {
    fn name(&self) -> &str {
        PAGE_BUILDER_CHECKER_ID
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let providers = input
            .facts
            .iter()
            .filter(|fact| is_fact_kind(fact, PAGE_BUILDER_PROVIDER_FACT_KIND))
            .filter_map(provider_from_fact)
            .collect::<Vec<_>>();
        let consumers = input
            .facts
            .iter()
            .filter(|fact| is_fact_kind(fact, PAGE_BUILDER_CONSUMER_FACT_KIND))
            .filter_map(|fact| consumer_from_fact(fact).map(|marker| (marker, fact.clone())))
            .collect::<Vec<_>>();
        let seams = input
            .facts
            .iter()
            .filter(|fact| is_fact_kind(fact, PAGE_BUILDER_ADAPTER_SEAM_FACT_KIND))
            .filter_map(adapter_seam_from_fact)
            .collect::<Vec<_>>();
        let waves = input
            .facts
            .iter()
            .filter(|fact| is_fact_kind(fact, PAGE_BUILDER_WAVE_EVIDENCE_FACT_KIND))
            .filter_map(|fact| wave_from_fact(fact).map(|marker| (marker, fact.clone())))
            .collect::<Vec<_>>();
        let content_markers = input
            .facts
            .iter()
            .filter(|fact| is_fact_kind(fact, PAGE_BUILDER_CONTENT_FACT_KIND))
            .filter_map(|fact| content_from_fact(fact).map(|marker| (marker, fact.clone())))
            .collect::<Vec<_>>();
        let fsd_markers = input
            .facts
            .iter()
            .filter(|fact| is_fact_kind(fact, PAGE_BUILDER_FSD_FACT_KIND))
            .filter_map(|fact| fsd_from_fact(fact).map(|marker| (marker, fact.clone())))
            .collect::<Vec<_>>();

        let mut diagnostics = Vec::new();
        let Some(provider) = providers
            .iter()
            .find(|provider| provider.module == "page_builder")
        else {
            diagnostics.push(simple_diagnostic(
                &input.snapshot,
                "rustok_page_builder_registry_missing",
                Severity::High,
                "Page Builder registry is missing",
                "crates/rustok-page-builder/contracts/page-builder-fba-registry.json was not indexed",
                "page_builder",
                Some("crates/rustok-page-builder/contracts/page-builder-fba-registry.json"),
                Vec::new(),
            ));
            return Ok(diagnostics);
        };

        let provider_consumers = provider
            .consumers
            .iter()
            .map(|consumer| (consumer.module.as_str(), consumer))
            .collect::<BTreeMap<_, _>>();
        let provider_caps = provider
            .capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let provider_profiles = provider
            .fallback_profiles
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();

        for (consumer, fact) in &consumers {
            if consumer.provider_module.as_deref() != Some("page-builder") {
                continue;
            }
            let Some(registry_consumer) = provider_consumers.get(consumer.module.as_str()) else {
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_consumer_registry_drift",
                    Severity::High,
                    "Page Builder consumer is not registered",
                    format!(
                        "{} depends on page-builder but is not declared in the provider registry",
                        consumer.module
                    ),
                    &consumer.module,
                    Some(&consumer.path),
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
                continue;
            };
            if consumer.contract.as_deref() != Some(provider.contract.as_str())
                || registry_consumer.contract.as_deref() != Some(provider.contract.as_str())
            {
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_contract_version_drift",
                    Severity::High,
                    "Page Builder contract drift",
                    format!(
                        "{} does not match provider contract {}",
                        consumer.module, provider.contract
                    ),
                    &consumer.module,
                    Some(&consumer.path),
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
            if consumer.builder_contract_version != provider.builder_contract_version
                || consumer.consumer_min_version != provider.consumer_min_version
            {
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_contract_version_drift",
                    Severity::High,
                    "Page Builder version drift",
                    format!(
                        "{} has builder/consumer version drift against provider",
                        consumer.module
                    ),
                    &consumer.module,
                    Some(&consumer.path),
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
            let consumer_caps = consumer
                .capabilities
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            if !consumer_caps.is_empty() && consumer_caps != provider_caps {
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_capability_drift",
                    Severity::Medium,
                    "Page Builder capability drift",
                    format!(
                        "{} capability set differs from provider registry",
                        consumer.module
                    ),
                    &consumer.module,
                    Some(&consumer.path),
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
            let consumer_profiles = consumer
                .fallback_profiles
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            if !consumer_profiles.is_empty() && consumer_profiles != provider_profiles {
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_fallback_profile_drift",
                    Severity::Medium,
                    "Page Builder fallback profile drift",
                    format!(
                        "{} fallback profiles differ from provider registry",
                        consumer.module
                    ),
                    &consumer.module,
                    Some(&consumer.path),
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
        }

        for required in [
            "PageBuilderProjectStore",
            "PageBuilderRenderingAdapter",
            "AdapterBackedPageBuilderService",
        ] {
            if !seams
                .iter()
                .any(|seam| seam.seams.iter().any(|value| value == required))
            {
                diagnostics.push(simple_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_adapter_seam_missing",
                    Severity::High,
                    format!("Page Builder adapter seam {required} is missing"),
                    "page_builder",
                    &provider.module,
                    Some(&provider.path),
                    Vec::new(),
                ));
            }
        }

        for consumer in provider
            .consumers
            .iter()
            .filter(|consumer| consumer.module == "pages" || consumer.module == "forum")
        {
            if !waves.iter().any(|(wave, _)| wave.module == consumer.module) {
                diagnostics.push(simple_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_wave_evidence_missing",
                    Severity::Medium,
                    format!(
                        "Page Builder wave evidence for {} is missing",
                        consumer.module
                    ),
                    "consumer has no indexed wave evidence packet",
                    &consumer.module,
                    Some(&provider.path),
                    Vec::new(),
                ));
            }
        }
        for (wave, fact) in &waves {
            if wave
                .fallback_profiles
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                != provider_profiles
            {
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_fallback_profile_drift",
                    Severity::Medium,
                    "Wave evidence fallback profile drift",
                    format!(
                        "{} wave evidence fallback profiles differ from provider registry",
                        wave.module
                    ),
                    &wave.module,
                    Some(&wave.path),
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
            if is_stale_evidence(wave) {
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_wave_evidence_stale",
                    Severity::Medium,
                    "Page Builder wave evidence is stale",
                    format!("{} wave evidence is marked stale or expired", wave.module),
                    &wave.module,
                    Some(&wave.path),
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
        }

        for (marker, fact) in &content_markers {
            if marker.module == "pages" && marker.format != "grapesjs_v1" {
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_content_format_drift",
                    Severity::Medium,
                    "Pages builder content format drift",
                    "pages visual builder surfaces should use grapesjs_v1".to_string(),
                    &marker.module,
                    Some(&marker.path),
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
            if matches!(marker.module.as_str(), "forum" | "blog") && marker.format == "grapesjs_v1"
            {
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_content_format_drift",
                    Severity::Medium,
                    "Rich-text module uses visual builder format",
                    format!("{} should remain on rt_json_v1 for rich-text content unless explicitly migrated", marker.module),
                    &marker.module,
                    Some(&marker.path),
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
        }

        let mut fsd_by_surface =
            BTreeMap::<(String, String), Vec<(PageBuilderFsdMarker, Fact)>>::new();
        for (marker, fact) in fsd_markers {
            fsd_by_surface
                .entry((marker.module.clone(), marker.surface.clone()))
                .or_default()
                .push((marker, fact));
        }
        for ((module, surface), markers) in fsd_by_surface {
            let roles = markers
                .iter()
                .map(|(marker, _)| marker.role.as_str())
                .collect::<BTreeSet<_>>();
            for (marker, fact) in &markers {
                if marker.role == "core"
                    && (marker.has_leptos_marker
                        || marker.has_component_marker
                        || marker.has_server_fn_marker)
                {
                    diagnostics.push(marker_diagnostic(
                        &input.snapshot,
                        "rustok_page_builder_fsd_core_leaks_ui",
                        Severity::High,
                        "Page Builder FSD core leaks UI/runtime markers",
                        format!("{module}/{surface} core contains Leptos/component/server markers"),
                        &module,
                        Some(&marker.path),
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                    ));
                }
                if matches!(marker.role.as_str(), "ui_leptos" | "ui_support")
                    && marker.calls_raw_transport
                    && !marker.calls_transport_facade
                {
                    diagnostics.push(marker_diagnostic(
                        &input.snapshot,
                        "rustok_page_builder_fsd_transport_missing",
                        Severity::High,
                        "Page Builder UI bypasses transport facade",
                        format!("{module}/{surface} UI calls raw transport"),
                        &module,
                        Some(&marker.path),
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                    ));
                }
                if marker.host_wiring && marker.has_component_marker {
                    diagnostics.push(marker_diagnostic(
                        &input.snapshot,
                        "rustok_page_builder_host_owns_module_ui",
                        Severity::Medium,
                        "Host owns Page Builder module UI",
                        format!("{module}/{surface} host wiring contains component markers"),
                        &module,
                        Some(&marker.path),
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                    ));
                }
            }
            if roles.contains("ui_leptos") && !roles.contains("transport") {
                let (_, fact) = &markers[0];
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_fsd_transport_missing",
                    Severity::Medium,
                    "Page Builder FSD surface is missing transport",
                    format!("{module}/{surface} has UI adapter without transport"),
                    &module,
                    None,
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
            if (roles.contains("core") || roles.contains("transport"))
                && !roles.contains("ui_leptos")
            {
                let (_, fact) = &markers[0];
                diagnostics.push(marker_diagnostic(
                    &input.snapshot,
                    "rustok_page_builder_fsd_ui_adapter_missing",
                    Severity::Medium,
                    "Page Builder FSD surface is missing explicit UI adapter",
                    format!("{module}/{surface} should expose ui/leptos adapter"),
                    &module,
                    None,
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
        }

        diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        Ok(diagnostics)
    }
}

fn extract_provider_registry(
    source: &SourceFile,
    snapshot: &SnapshotId,
    path: &str,
) -> ExtractOutput {
    let Some(content) = source.content.as_deref() else {
        return ExtractOutput::default();
    };
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return ExtractOutput::default();
    };
    let Some(marker) = provider_marker_from_value(path, &value) else {
        return ExtractOutput::default();
    };
    let file = file_entity(source, &snapshot.0);
    let mut entities = vec![
        file.clone(),
        provider_entity(&marker.module, snapshot, Some(path)),
        contract_entity(&marker.contract, snapshot, Some(path)),
    ];
    for capability in &marker.capabilities {
        entities.push(capability_entity(capability, snapshot, Some(path)));
    }
    for profile in &marker.fallback_profiles {
        entities.push(fallback_profile_entity(profile, snapshot, Some(path)));
    }
    for consumer in &marker.consumers {
        entities.push(consumer_entity(&consumer.module, snapshot, Some(path)));
    }
    dedup_entities(&mut entities);
    ExtractOutput {
        entities,
        facts: vec![Fact {
            id: fact_id(PAGE_BUILDER_PROVIDER_FACT_KIND, &file.stable_key.0),
            kind: FactKind::Other(PAGE_BUILDER_PROVIDER_FACT_KIND.to_string()),
            subject: file.id,
            object: None,
            value: serde_json::to_value(marker).expect("provider marker serializes"),
            evidence: vec![evidence_for_file(
                path,
                PAGE_BUILDER_EXTRACTOR_ID,
                Some(1),
                Some(1),
            )],
            ownership: ownership_for_file(path),
            snapshot: snapshot.clone(),
            extractor: PAGE_BUILDER_EXTRACTOR_ID.to_string(),
            confidence: 1.0,
        }],
        diagnostics: Vec::new(),
    }
}

fn extract_adapter_seams(source: &SourceFile, snapshot: &SnapshotId, path: &str) -> ExtractOutput {
    let Some(content) = source.content.as_deref() else {
        return ExtractOutput::default();
    };
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return ExtractOutput::default();
    };
    let module = value
        .get("module")
        .and_then(Value::as_str)
        .unwrap_or("page_builder")
        .to_string();
    let mut seams = BTreeSet::new();
    collect_string(value.pointer("/seams/persistence/trait"), &mut seams);
    collect_string(value.pointer("/seams/rendering/trait"), &mut seams);
    collect_string(
        value.pointer("/seams/adapter_backed_service/type"),
        &mut seams,
    );
    let marker = PageBuilderAdapterSeamMarker {
        schema: "rustok.page_builder.adapter_seams.v1".to_string(),
        path: path.to_string(),
        module,
        seams: seams.into_iter().collect(),
        canonical_entrypoints: string_array(value.get("canonical_entrypoints")),
        blocked_patterns: string_array(value.get("blocked_patterns")),
    };
    let file = file_entity(source, &snapshot.0);
    let mut entities = vec![file.clone()];
    for seam in &marker.seams {
        entities.push(adapter_seam_entity(seam, snapshot, Some(path)));
    }
    dedup_entities(&mut entities);
    ExtractOutput {
        entities,
        facts: vec![Fact {
            id: fact_id(PAGE_BUILDER_ADAPTER_SEAM_FACT_KIND, &file.stable_key.0),
            kind: FactKind::Other(PAGE_BUILDER_ADAPTER_SEAM_FACT_KIND.to_string()),
            subject: file.id,
            object: None,
            value: serde_json::to_value(marker).expect("adapter seam marker serializes"),
            evidence: vec![evidence_for_file(
                path,
                PAGE_BUILDER_EXTRACTOR_ID,
                Some(1),
                Some(1),
            )],
            ownership: ownership_for_file(path),
            snapshot: snapshot.clone(),
            extractor: PAGE_BUILDER_EXTRACTOR_ID.to_string(),
            confidence: 1.0,
        }],
        diagnostics: Vec::new(),
    }
}

fn extract_wave_evidence(source: &SourceFile, snapshot: &SnapshotId, path: &str) -> ExtractOutput {
    let Some(content) = source.content.as_deref() else {
        return ExtractOutput::default();
    };
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return ExtractOutput::default();
    };
    if value.get("artifact").and_then(Value::as_str) != Some("page_builder_wave_evidence_packet") {
        return ExtractOutput::default();
    }
    let module = value
        .get("module_slug")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .pointer("/metadata/consumer/module_slug")
                .and_then(Value::as_str)
        })
        .unwrap_or("pages")
        .to_string();
    let marker = PageBuilderWaveEvidenceMarker {
        schema: "rustok.page_builder.wave_evidence.v1".to_string(),
        path: path.to_string(),
        module: module.clone(),
        wave: json_u64(value.get("wave")),
        mode: value
            .get("mode")
            .and_then(Value::as_str)
            .map(str::to_string),
        created_at: value
            .get("created_at")
            .and_then(Value::as_str)
            .map(str::to_string),
        fallback_profiles: value
            .pointer("/metadata/consumer/fallback_matrix")
            .map(|value| string_array(Some(value)))
            .unwrap_or_else(|| {
                value
                    .pointer("/fallback/profiles")
                    .and_then(Value::as_array)
                    .map(|profiles| {
                        profiles
                            .iter()
                            .filter_map(|profile| {
                                profile
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .map(str::to_string)
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }),
        trace_profiles: value
            .pointer("/observability/trace_samples")
            .and_then(Value::as_array)
            .map(|traces| {
                traces
                    .iter()
                    .filter_map(|trace| {
                        trace
                            .get("profile")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    let file = file_entity(source, &snapshot.0);
    let mut entities = vec![
        file.clone(),
        wave_evidence_entity(&module, marker.wave, snapshot, Some(path)),
        consumer_entity(&module, snapshot, Some(path)),
    ];
    for profile in &marker.fallback_profiles {
        entities.push(fallback_profile_entity(profile, snapshot, Some(path)));
    }
    dedup_entities(&mut entities);
    ExtractOutput {
        entities,
        facts: vec![Fact {
            id: fact_id(PAGE_BUILDER_WAVE_EVIDENCE_FACT_KIND, &file.stable_key.0),
            kind: FactKind::Other(PAGE_BUILDER_WAVE_EVIDENCE_FACT_KIND.to_string()),
            subject: file.id,
            object: None,
            value: serde_json::to_value(marker).expect("wave evidence marker serializes"),
            evidence: vec![evidence_for_file(
                path,
                PAGE_BUILDER_EXTRACTOR_ID,
                Some(1),
                Some(1),
            )],
            ownership: ownership_for_file(path),
            snapshot: snapshot.clone(),
            extractor: PAGE_BUILDER_EXTRACTOR_ID.to_string(),
            confidence: 1.0,
        }],
        diagnostics: Vec::new(),
    }
}

fn extract_consumer_manifest(
    source: &SourceFile,
    snapshot: &SnapshotId,
    path: &str,
) -> ExtractOutput {
    let Some(module) = module_from_crate_path(path) else {
        return ExtractOutput::default();
    };
    let Some(content) = source.content.as_deref() else {
        return ExtractOutput::default();
    };
    let Ok(value) = toml::from_str::<toml::Value>(content) else {
        return ExtractOutput::default();
    };
    let Some(manifest) = page_builder_consumer_manifest(&value) else {
        return ExtractOutput::default();
    };
    let marker = PageBuilderConsumerManifestMarker {
        schema: "rustok.page_builder.consumer_manifest.v1".to_string(),
        path: path.to_string(),
        module: module.clone(),
        provider_module: manifest.provider_module,
        contract: manifest.contract,
        contract_version: manifest.contract_version,
        builder_contract_version: manifest.builder_contract_version,
        consumer_min_version: manifest.consumer_min_version,
        capabilities: manifest.capabilities,
        fallback_profiles: manifest.fallback_profiles,
        degraded_modes: manifest.degraded_modes,
    };
    let file = file_entity(source, &snapshot.0);
    let mut entities = vec![file.clone(), consumer_entity(&module, snapshot, Some(path))];
    for profile in &marker.fallback_profiles {
        entities.push(fallback_profile_entity(profile, snapshot, Some(path)));
    }
    dedup_entities(&mut entities);
    ExtractOutput {
        entities,
        facts: vec![Fact {
            id: fact_id(PAGE_BUILDER_CONSUMER_FACT_KIND, &file.stable_key.0),
            kind: FactKind::Other(PAGE_BUILDER_CONSUMER_FACT_KIND.to_string()),
            subject: file.id,
            object: None,
            value: serde_json::to_value(marker).expect("consumer marker serializes"),
            evidence: vec![evidence_for_file(
                path,
                PAGE_BUILDER_EXTRACTOR_ID,
                Some(1),
                Some(1),
            )],
            ownership: ownership_for_file(path),
            snapshot: snapshot.clone(),
            extractor: PAGE_BUILDER_EXTRACTOR_ID.to_string(),
            confidence: 1.0,
        }],
        diagnostics: Vec::new(),
    }
}

fn extract_content_surface(
    source: &SourceFile,
    snapshot: &SnapshotId,
    path: &str,
) -> ExtractOutput {
    let Some(module) = module_from_crate_path(path) else {
        return ExtractOutput::default();
    };
    let content = source.content.as_deref().unwrap_or_default();
    let mut formats = BTreeSet::new();
    for format in ["grapesjs_v1", "rt_json_v1"] {
        if content.contains(format) {
            formats.insert(format.to_string());
        }
    }
    if formats.is_empty() {
        return ExtractOutput::default();
    }
    let file = file_entity(source, &snapshot.0);
    let mut entities = vec![file.clone()];
    let mut facts = Vec::new();
    for format in formats {
        let marker = PageBuilderContentMarker {
            schema: "rustok.page_builder.content_surface.v1".to_string(),
            path: path.to_string(),
            module: module.clone(),
            format: format.clone(),
            first_marker_line: find_first_line(content, &format),
        };
        entities.push(content_surface_entity(
            &module,
            &format,
            snapshot,
            Some(path),
        ));
        facts.push(Fact {
            id: fact_id(
                PAGE_BUILDER_CONTENT_FACT_KIND,
                &format!("{}:{format}", file.stable_key.0),
            ),
            kind: FactKind::Other(PAGE_BUILDER_CONTENT_FACT_KIND.to_string()),
            subject: file.id.clone(),
            object: None,
            value: serde_json::to_value(marker).expect("content marker serializes"),
            evidence: vec![evidence_for_file(
                path,
                PAGE_BUILDER_EXTRACTOR_ID,
                find_first_line(content, &format),
                find_first_line(content, &format),
            )],
            ownership: ownership_for_file(path),
            snapshot: snapshot.clone(),
            extractor: PAGE_BUILDER_EXTRACTOR_ID.to_string(),
            confidence: 1.0,
        });
    }
    dedup_entities(&mut entities);
    ExtractOutput {
        entities,
        facts,
        diagnostics: Vec::new(),
    }
}

fn extract_fsd_surface(
    source: &SourceFile,
    snapshot: &SnapshotId,
    path: &str,
    module: String,
    surface: String,
    role: String,
    host_wiring: bool,
) -> ExtractOutput {
    let content = source.content.as_deref().unwrap_or_default();
    let marker = PageBuilderFsdMarker {
        schema: "rustok.page_builder.fsd_surface.v1".to_string(),
        path: path.to_string(),
        module: module.clone(),
        surface: surface.clone(),
        role: role.clone(),
        has_leptos_marker: contains_any(content, &["leptos::", "#[component]", "view!"]),
        has_component_marker: contains_any(content, &["#[component]", "view!", "IntoView"]),
        has_server_fn_marker: contains_any(content, &["#[server]", "ServerFn"]),
        calls_raw_transport: contains_any(
            content,
            &[
                "crate::api::",
                "execute_graphql",
                "use_query",
                "useMutation",
            ],
        ),
        calls_transport_facade: contains_any(
            content,
            &[
                "transport::",
                "Transport",
                "with_fallback",
                "graphql_adapter",
            ],
        ),
        host_wiring,
        first_marker_line: first_marker_line(content),
    };
    let file = file_entity(source, &snapshot.0);
    ExtractOutput {
        entities: vec![
            file.clone(),
            fsd_surface_entity(&module, &surface, snapshot, Some(path)),
        ],
        facts: vec![Fact {
            id: fact_id(
                PAGE_BUILDER_FSD_FACT_KIND,
                &format!("{}:{role}", file.stable_key.0),
            ),
            kind: FactKind::Other(PAGE_BUILDER_FSD_FACT_KIND.to_string()),
            subject: file.id,
            object: None,
            value: serde_json::to_value(marker).expect("fsd marker serializes"),
            evidence: vec![evidence_for_file(
                path,
                PAGE_BUILDER_EXTRACTOR_ID,
                first_marker_line(content),
                first_marker_line(content),
            )],
            ownership: ownership_for_file(path),
            snapshot: snapshot.clone(),
            extractor: PAGE_BUILDER_EXTRACTOR_ID.to_string(),
            confidence: 1.0,
        }],
        diagnostics: Vec::new(),
    }
}

fn extract_host_mount_surfaces(
    source: &SourceFile,
    snapshot: &SnapshotId,
    path: &str,
) -> ExtractOutput {
    let content = source.content.as_deref().unwrap_or_default();
    let mounted = detect_host_mounts(content);
    if mounted.is_empty() {
        return ExtractOutput::default();
    }
    let file = file_entity(source, &snapshot.0);
    let mut entities = vec![file.clone()];
    let mut facts = Vec::new();
    for (module, surface) in mounted {
        let marker = PageBuilderFsdMarker {
            schema: "rustok.page_builder.fsd_surface.v1".to_string(),
            path: path.to_string(),
            module: module.clone(),
            surface: surface.clone(),
            role: "host_mount".to_string(),
            has_leptos_marker: contains_any(content, &["leptos::", "#[component]", "view!"]),
            has_component_marker: contains_any(content, &["#[component]", "view!", "IntoView"]),
            has_server_fn_marker: contains_any(content, &["#[server]", "ServerFn"]),
            calls_raw_transport: contains_any(
                content,
                &[
                    "crate::api::",
                    "execute_graphql",
                    "use_query",
                    "useMutation",
                ],
            ),
            calls_transport_facade: contains_any(
                content,
                &[
                    "transport::",
                    "Transport",
                    "with_fallback",
                    "graphql_adapter",
                ],
            ),
            host_wiring: true,
            first_marker_line: first_marker_line(content),
        };
        entities.push(fsd_surface_entity(&module, &surface, snapshot, Some(path)));
        facts.push(Fact {
            id: fact_id(
                PAGE_BUILDER_FSD_FACT_KIND,
                &format!("{}:{module}:{surface}:host_mount", file.stable_key.0),
            ),
            kind: FactKind::Other(PAGE_BUILDER_FSD_FACT_KIND.to_string()),
            subject: file.id.clone(),
            object: None,
            value: serde_json::to_value(marker).expect("host mount marker serializes"),
            evidence: vec![evidence_for_file(
                path,
                PAGE_BUILDER_EXTRACTOR_ID,
                first_marker_line(content),
                first_marker_line(content),
            )],
            ownership: ownership_for_file(path),
            snapshot: snapshot.clone(),
            extractor: PAGE_BUILDER_EXTRACTOR_ID.to_string(),
            confidence: 0.85,
        });
    }
    dedup_entities(&mut entities);
    ExtractOutput {
        entities,
        facts,
        diagnostics: Vec::new(),
    }
}

fn provider_marker_from_value(path: &str, value: &Value) -> Option<PageBuilderProviderMarker> {
    let provider = value.get("provider")?;
    Some(PageBuilderProviderMarker {
        schema: "rustok.page_builder.provider_registry.v1".to_string(),
        path: path.to_string(),
        module: provider.get("module_slug")?.as_str()?.to_string(),
        manifest_module: provider
            .get("manifest_module")
            .and_then(Value::as_str)
            .map(str::to_string),
        contract: provider.get("contract")?.as_str()?.to_string(),
        builder_contract_version: provider
            .get("builder_contract_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        consumer_min_version: provider
            .get("consumer_min_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        capabilities: string_array(provider.get("capabilities")),
        fallback_profiles: string_array(value.get("fallback_profiles")),
        health_states: string_array(provider.get("health_states")),
        degradation_reasons: string_array(provider.get("degradation_reasons")),
        consumers: value
            .get("consumers")
            .and_then(Value::as_array)
            .map(|consumers| {
                consumers
                    .iter()
                    .filter_map(registry_consumer_from_value)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn registry_consumer_from_value(value: &Value) -> Option<PageBuilderRegistryConsumer> {
    Some(PageBuilderRegistryConsumer {
        module: value.get("module_slug")?.as_str()?.to_string(),
        crate_name: value
            .get("crate")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_module: value
            .get("provider_module")
            .and_then(Value::as_str)
            .map(str::to_string),
        contract: value
            .get("contract")
            .and_then(Value::as_str)
            .map(str::to_string),
        contract_version: value
            .get("contract_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        builder_contract_version: value
            .get("builder_contract_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        consumer_min_version: value
            .get("consumer_min_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        capabilities: string_array(value.get("capabilities")),
        rollout_state: value
            .get("rollout_state")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn provider_from_fact(fact: &Fact) -> Option<PageBuilderProviderMarker> {
    serde_json::from_value(fact.value.clone()).ok()
}

fn consumer_from_fact(fact: &Fact) -> Option<PageBuilderConsumerManifestMarker> {
    serde_json::from_value(fact.value.clone()).ok()
}

fn adapter_seam_from_fact(fact: &Fact) -> Option<PageBuilderAdapterSeamMarker> {
    serde_json::from_value(fact.value.clone()).ok()
}

fn wave_from_fact(fact: &Fact) -> Option<PageBuilderWaveEvidenceMarker> {
    serde_json::from_value(fact.value.clone()).ok()
}

fn content_from_fact(fact: &Fact) -> Option<PageBuilderContentMarker> {
    serde_json::from_value(fact.value.clone()).ok()
}

fn fsd_from_fact(fact: &Fact) -> Option<PageBuilderFsdMarker> {
    serde_json::from_value(fact.value.clone()).ok()
}

fn is_fact_kind(fact: &Fact, expected: &str) -> bool {
    matches!(&fact.kind, FactKind::Other(kind) if kind == expected)
}

fn is_provider_registry_path(path: &str) -> bool {
    path == "crates/rustok-page-builder/contracts/page-builder-fba-registry.json"
}

fn is_adapter_seams_path(path: &str) -> bool {
    path == "crates/rustok-page-builder/contracts/page-builder-adapter-seams.json"
}

fn is_wave_evidence_path(path: &str) -> bool {
    (path.starts_with("crates/rustok-page-builder/contracts/evidence/")
        || (path.starts_with("crates/rustok-") && path.contains("/contracts/evidence/")))
        && path.ends_with(".json")
        && path.contains("wave")
}

fn is_consumer_manifest_path(path: &str) -> bool {
    path.starts_with("crates/rustok-") && path.ends_with("/rustok-module.toml")
}

fn is_content_source_path(path: &str) -> bool {
    path.starts_with("crates/rustok-")
        && matches!(path.rsplit('.').next(), Some("rs"))
        && !path.contains("/tests/")
        && !path.contains("/docs/")
        && (path.contains("rustok-pages")
            || path.contains("rustok-forum")
            || path.contains("rustok-blog")
            || path.contains("rustok-content"))
}

fn is_host_mount_source_path(path: &str) -> bool {
    path.starts_with("apps/")
        && matches!(
            path.rsplit('.').next(),
            Some("rs" | "tsx" | "ts" | "jsx" | "js")
        )
}

fn classify_fsd_path(path: &str) -> Option<(String, String, String, bool)> {
    let module = module_from_crate_path(path)?;
    if !matches!(module.as_str(), "pages" | "forum" | "blog" | "content") {
        return None;
    }
    let rest = path.strip_prefix(&format!("crates/rustok-{module}/"))?;
    let (surface, surface_rest) = if let Some(rest) = rest.strip_prefix("admin/") {
        ("admin".to_string(), rest)
    } else if let Some(rest) = rest.strip_prefix("storefront/") {
        ("storefront".to_string(), rest)
    } else {
        return None;
    };
    let role = if surface_rest == "src/core.rs" || surface_rest.starts_with("src/core/") {
        "core"
    } else if surface_rest == "src/transport.rs"
        || surface_rest.starts_with("src/transport/")
        || surface_rest == "src/api.rs"
    {
        "transport"
    } else if surface_rest == "src/ui/leptos.rs" || surface_rest.starts_with("src/ui/leptos/") {
        "ui_leptos"
    } else if surface_rest.starts_with("src/ui/") {
        "ui_support"
    } else {
        return None;
    };
    Some((module, surface, role.to_string(), false))
}

fn detect_host_mounts(content: &str) -> Vec<(String, String)> {
    let mut mounts = BTreeSet::new();
    for module in ["pages", "forum", "blog", "content"] {
        let crate_ident = format!("rustok_{module}");
        let pascal_module = pascal_case(module);
        for surface in ["admin", "storefront"] {
            let pascal_surface = pascal_case(surface);
            let markers = [
                format!("{crate_ident}::{surface}"),
                format!("{crate_ident}::ui::{surface}"),
                format!("{crate_ident}::{pascal_surface}"),
                format!("{pascal_module}{pascal_surface}"),
            ];
            if markers.iter().any(|marker| content.contains(marker)) {
                mounts.insert((module.to_string(), surface.to_string()));
            }
        }
    }
    mounts.into_iter().collect()
}

fn pascal_case(value: &str) -> String {
    let mut out = String::new();
    for part in value.split('_') {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

fn module_from_crate_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("crates/rustok-")?;
    let module = rest.split('/').next()?;
    Some(module.replace('-', "_"))
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn json_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn collect_string(value: Option<&Value>, out: &mut BTreeSet<String>) {
    if let Some(value) = value.and_then(Value::as_str) {
        out.insert(value.to_string());
    }
}

fn toml_string(value: &toml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
}

fn toml_string_array(value: &toml::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

struct ConsumerManifestFields {
    provider_module: Option<String>,
    contract: Option<String>,
    contract_version: Option<String>,
    builder_contract_version: Option<String>,
    consumer_min_version: Option<String>,
    capabilities: Vec<String>,
    fallback_profiles: Vec<String>,
    degraded_modes: Vec<String>,
}

fn page_builder_consumer_manifest(value: &toml::Value) -> Option<ConsumerManifestFields> {
    let fba = value.get("fba").and_then(|fba| fba.get("builder_consumer"));
    let dep = value
        .get("dependencies")
        .and_then(|deps| deps.get("page_builder"));
    let source = fba.or(dep)?;
    Some(ConsumerManifestFields {
        provider_module: toml_string(source, "provider_module")
            .or_else(|| toml_string(source, "module"))
            .or_else(|| dep.and_then(|dep| toml_string(dep, "provider_module")))
            .or_else(|| dep.and_then(|dep| toml_string(dep, "module"))),
        contract: toml_string(source, "contract")
            .or_else(|| dep.and_then(|dep| toml_string(dep, "contract"))),
        contract_version: toml_string(source, "contract_version")
            .or_else(|| dep.and_then(|dep| toml_string(dep, "contract_version"))),
        builder_contract_version: toml_string(source, "builder_contract_version")
            .or_else(|| dep.and_then(|dep| toml_string(dep, "builder_contract_version"))),
        consumer_min_version: toml_string(source, "consumer_min_version")
            .or_else(|| dep.and_then(|dep| toml_string(dep, "consumer_min_version"))),
        capabilities: non_empty_vec(toml_string_array(source, "capabilities"))
            .or_else(|| dep.map(|dep| toml_string_array(dep, "required_capabilities")))
            .or_else(|| dep.map(|dep| toml_string_array(dep, "capabilities")))
            .unwrap_or_default(),
        fallback_profiles: non_empty_vec(toml_string_array(source, "fallback_profiles"))
            .or_else(|| toml_table_keys(source, "toggle_profiles"))
            .or_else(|| dep.map(|dep| toml_string_array(dep, "fallback_profiles")))
            .unwrap_or_default(),
        degraded_modes: non_empty_vec(toml_string_array(source, "degraded_modes"))
            .or_else(|| toml_table_keys(source, "degraded_modes"))
            .unwrap_or_default(),
    })
}

fn non_empty_vec(values: Vec<String>) -> Option<Vec<String>> {
    (!values.is_empty()).then_some(values)
}

fn toml_table_keys(value: &toml::Value, key: &str) -> Option<Vec<String>> {
    value
        .get(key)
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().cloned().collect())
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn find_first_line(content: &str, needle: &str) -> Option<u32> {
    content
        .lines()
        .enumerate()
        .find_map(|(index, line)| line.contains(needle).then_some(index as u32 + 1))
}

fn first_marker_line(content: &str) -> Option<u32> {
    content
        .lines()
        .enumerate()
        .find_map(|(index, line)| (!line.trim().is_empty()).then_some(index as u32 + 1))
}

fn is_stale_evidence(marker: &PageBuilderWaveEvidenceMarker) -> bool {
    marker.mode.as_deref() == Some("stale")
        || marker
            .created_at
            .as_deref()
            .is_some_and(|date| date.starts_with("1970-"))
}

fn fact_id(kind: &str, seed: &str) -> FactId {
    FactId(format!("fact_{kind}_{:016x}", stable_hash(seed.as_bytes())))
}

fn entity(
    kind: &str,
    stable_key: String,
    name: String,
    title: String,
    _snapshot: &SnapshotId,
    path: Option<&str>,
    payload: Value,
) -> Entity {
    Entity {
        id: EntityId(format!(
            "ent_{kind}_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        stable_key: StableKey(stable_key),
        kind: EntityKind::Other(kind.to_string()),
        name,
        title: Some(title),
        source: None,
        language: None,
        aliases: Vec::new(),
        ownership: path.map_or_else(Vec::new, ownership_for_file),
        payload,
    }
}

fn provider_entity(module: &str, snapshot: &SnapshotId, path: Option<&str>) -> Entity {
    entity(
        PAGE_BUILDER_PROVIDER_ENTITY_KIND,
        format!("page_builder_provider://{module}"),
        module.to_string(),
        format!("RusTok Page Builder {module} provider"),
        snapshot,
        path,
        json!({"schema":"rustok.page_builder.provider.v1","module":module}),
    )
}

fn consumer_entity(module: &str, snapshot: &SnapshotId, path: Option<&str>) -> Entity {
    entity(
        PAGE_BUILDER_CONSUMER_ENTITY_KIND,
        format!("page_builder_consumer://{module}"),
        module.to_string(),
        format!("RusTok Page Builder {module} consumer"),
        snapshot,
        path,
        json!({"schema":"rustok.page_builder.consumer.v1","module":module}),
    )
}

fn contract_entity(contract: &str, snapshot: &SnapshotId, path: Option<&str>) -> Entity {
    entity(
        PAGE_BUILDER_CONTRACT_ENTITY_KIND,
        format!("page_builder_contract://{contract}"),
        contract.to_string(),
        format!("RusTok Page Builder {contract} contract"),
        snapshot,
        path,
        json!({"schema":"rustok.page_builder.contract.v1","contract":contract}),
    )
}

fn capability_entity(capability: &str, snapshot: &SnapshotId, path: Option<&str>) -> Entity {
    entity(
        PAGE_BUILDER_CAPABILITY_ENTITY_KIND,
        format!("page_builder_capability://{capability}"),
        capability.to_string(),
        format!("RusTok Page Builder {capability} capability"),
        snapshot,
        path,
        json!({"schema":"rustok.page_builder.capability.v1","capability":capability}),
    )
}

fn fallback_profile_entity(profile: &str, snapshot: &SnapshotId, path: Option<&str>) -> Entity {
    entity(
        PAGE_BUILDER_FALLBACK_PROFILE_ENTITY_KIND,
        format!("page_builder_fallback_profile://{profile}"),
        profile.to_string(),
        format!("RusTok Page Builder {profile} fallback profile"),
        snapshot,
        path,
        json!({"schema":"rustok.page_builder.fallback_profile.v1","profile":profile}),
    )
}

fn wave_evidence_entity(
    module: &str,
    wave: Option<u64>,
    snapshot: &SnapshotId,
    path: Option<&str>,
) -> Entity {
    let wave_name = wave.map_or_else(|| "unknown".to_string(), |wave| format!("wave{wave}"));
    entity(
        PAGE_BUILDER_WAVE_EVIDENCE_ENTITY_KIND,
        format!("page_builder_wave_evidence://{module}/{wave_name}"),
        format!("{module}/{wave_name}"),
        format!("RusTok Page Builder {module} {wave_name} evidence"),
        snapshot,
        path,
        json!({"schema":"rustok.page_builder.wave_evidence_entity.v1","module":module,"wave":wave}),
    )
}

fn adapter_seam_entity(seam: &str, snapshot: &SnapshotId, path: Option<&str>) -> Entity {
    entity(
        PAGE_BUILDER_ADAPTER_SEAM_ENTITY_KIND,
        format!("page_builder_adapter_seam://{seam}"),
        seam.to_string(),
        format!("RusTok Page Builder {seam} adapter seam"),
        snapshot,
        path,
        json!({"schema":"rustok.page_builder.adapter_seam.v1","seam":seam}),
    )
}

fn content_surface_entity(
    module: &str,
    format: &str,
    snapshot: &SnapshotId,
    path: Option<&str>,
) -> Entity {
    entity(
        PAGE_BUILDER_CONTENT_SURFACE_ENTITY_KIND,
        format!("page_builder_content_surface://{module}/{format}"),
        format!("{module}/{format}"),
        format!("RusTok Page Builder {module} {format} content surface"),
        snapshot,
        path,
        json!({"schema":"rustok.page_builder.content_surface_entity.v1","module":module,"format":format}),
    )
}

fn fsd_surface_entity(
    module: &str,
    surface: &str,
    snapshot: &SnapshotId,
    path: Option<&str>,
) -> Entity {
    entity(
        PAGE_BUILDER_FSD_SURFACE_ENTITY_KIND,
        format!("page_builder_fsd_surface://{module}/{surface}"),
        format!("{module}/{surface}"),
        format!("RusTok Page Builder {module} {surface} FSD surface"),
        snapshot,
        path,
        json!({"schema":"rustok.page_builder.fsd_surface_entity.v1","module":module,"surface":surface}),
    )
}

fn dedup_entities(entities: &mut Vec<Entity>) {
    let mut seen = BTreeSet::new();
    entities.retain(|entity| seen.insert(entity.id.0.clone()));
}

#[allow(clippy::too_many_arguments)]
fn push_relation(
    relations: &mut Vec<Relation>,
    seen: &mut BTreeSet<String>,
    snapshot: &SnapshotId,
    kind: RelationKind,
    from: &EntityId,
    to: &EntityId,
    evidence: Vec<Evidence>,
    ownership: Vec<athanor_domain::Ownership>,
    payload: Value,
) {
    let key = format!("{}:{}:{}", serialized_relation_kind(&kind), from.0, to.0);
    if !seen.insert(key.clone()) {
        return;
    }
    relations.push(Relation {
        id: RelationId(format!("rel_{:016x}", stable_hash(key.as_bytes()))),
        kind,
        from: from.clone(),
        to: to.clone(),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence,
        ownership,
        snapshot: snapshot.clone(),
        payload,
    });
}

fn serialized_relation_kind(kind: &RelationKind) -> String {
    match kind {
        RelationKind::Other(value) => value.clone(),
        other => format!("{other:?}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn marker_diagnostic(
    snapshot: &SnapshotId,
    kind: &str,
    severity: Severity,
    title: &str,
    message: String,
    module: &str,
    path: Option<&str>,
    evidence: Vec<Evidence>,
    ownership: Vec<athanor_domain::Ownership>,
) -> Diagnostic {
    let seed = format!("{kind}:{module}:{}", path.unwrap_or(""));
    Diagnostic {
        id: DiagnosticId(format!("diag_{:016x}", stable_hash(seed.as_bytes()))),
        kind: DiagnosticKind::Other(kind.to_string()),
        severity,
        status: DiagnosticStatus::Open,
        title: title.to_string(),
        message,
        entities: Vec::new(),
        evidence,
        ownership,
        snapshot: snapshot.clone(),
        suggested_fix: None,
        payload: json!({
            "schema": "rustok.page_builder.diagnostic.v1",
            "module": module,
            "path": path,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn simple_diagnostic(
    snapshot: &SnapshotId,
    kind: &str,
    severity: Severity,
    title: impl Into<String>,
    message: impl Into<String>,
    module: &str,
    path: Option<&str>,
    evidence: Vec<Evidence>,
) -> Diagnostic {
    marker_diagnostic(
        snapshot,
        kind,
        severity,
        &title.into(),
        message.into(),
        module,
        path,
        if evidence.is_empty() {
            path.map(|path| evidence_for_file(path, PAGE_BUILDER_CHECKER_ID, Some(1), Some(1)))
                .into_iter()
                .collect()
        } else {
            evidence
        },
        path.map_or_else(Vec::new, ownership_for_file),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use athanor_core::{CheckInput, LinkInput};
    use athanor_domain::{RepoId, SnapshotId};
    use std::sync::Arc;

    fn source(path: &str, content: &str) -> SourceFile {
        SourceFile {
            path: path.to_string(),
            language_hint: None,
            content_hash: Some("hash".to_string()),
            content: Some(content.to_string()),
        }
    }

    fn input(path: &str, content: &str, snapshot: SnapshotId) -> ExtractInput {
        ExtractInput {
            repo: RepoId("repo".to_string()),
            source: source(path, content),
            snapshot,
        }
    }

    #[tokio::test]
    async fn parses_provider_registry_with_capabilities() {
        let snapshot = SnapshotId("snap".to_string());
        let output = RustokPageBuilderExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                source: source(
                    "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
                    r#"{"provider":{"module_slug":"page_builder","manifest_module":"page-builder","contract":"grapesjs_v1","builder_contract_version":"1.0","consumer_min_version":"1.0","capabilities":["preview","tree","properties","publish"]},"consumers":[{"module_slug":"pages","crate":"rustok-pages","provider_module":"page-builder","contract":"grapesjs_v1","contract_version":"1.0","builder_contract_version":"1.0","consumer_min_version":"1.0","capabilities":["preview","tree","properties","publish"]},{"module_slug":"forum","crate":"rustok-forum","provider_module":"page-builder","contract":"grapesjs_v1","contract_version":"1.0","builder_contract_version":"1.0","consumer_min_version":"1.0","capabilities":["preview","tree","properties","publish"]}],"fallback_profiles":["all_on","publish_off","preview_off","builder_off"]}"#,
                ),
                snapshot: snapshot.clone(),
            })
            .await
            .unwrap();
        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.stable_key.0 == "page_builder_provider://page_builder")
        );
        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.stable_key.0 == "page_builder_contract://grapesjs_v1")
        );
        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.stable_key.0 == "page_builder_capability://publish")
        );
        assert_eq!(output.facts.len(), 1);
    }

    #[tokio::test]
    async fn links_consumers_to_provider() {
        let snapshot = SnapshotId("snap".to_string());
        let extracted = RustokPageBuilderExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                source: source(
                    "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
                    r#"{"provider":{"module_slug":"page_builder","contract":"grapesjs_v1","capabilities":["preview"]},"consumers":[{"module_slug":"pages","provider_module":"page-builder"}],"fallback_profiles":["all_on"]}"#,
                ),
                snapshot: snapshot.clone(),
            })
            .await
            .unwrap();
        let relations = RustokPageBuilderLinker
            .link(LinkInput {
                snapshot,
                entities: Arc::new(extracted.entities),
                facts: Arc::new(extracted.facts),
                affected: Default::default(),
            })
            .await
            .unwrap();
        assert!(relations.iter().any(|relation| matches!(&relation.kind, RelationKind::Other(kind) if kind == "rustok_page_builder_consumer_requires_provider")));
    }

    #[tokio::test]
    async fn detects_contract_version_drift() {
        let snapshot = SnapshotId("snap".to_string());
        let mut facts = Vec::new();
        for (path, content) in [
            (
                "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
                r#"{"provider":{"module_slug":"page_builder","contract":"grapesjs_v1","builder_contract_version":"1.0","consumer_min_version":"1.0","capabilities":["preview"],"health_states":["ready"]},"consumers":[{"module_slug":"pages","provider_module":"page-builder","contract":"grapesjs_v1","contract_version":"1.0","builder_contract_version":"1.0","consumer_min_version":"1.0","capabilities":["preview"]}],"fallback_profiles":["all_on"]}"#,
            ),
            (
                "crates/rustok-pages/rustok-module.toml",
                "[dependencies.page_builder]\nprovider_module = \"page-builder\"\ncontract = \"rt_json_v1\"\nbuilder_contract_version = \"0.9\"\nconsumer_min_version = \"1.0\"\ncapabilities = [\"preview\"]\nfallback_profiles = [\"all_on\"]\n",
            ),
        ] {
            let output = RustokPageBuilderExtractor
                .extract(input(path, content, snapshot.clone()))
                .await
                .unwrap();
            facts.extend(output.facts);
        }
        let diagnostics = RustokPageBuilderChecker
            .check(CheckInput {
                entities: Arc::new(Vec::new()),
                facts: Arc::new(facts),
                relations: Arc::new(Vec::new()),
                affected: Default::default(),
                snapshot,
            })
            .await
            .unwrap();
        assert!(diagnostics.iter().any(|diagnostic| matches!(&diagnostic.kind, DiagnosticKind::Other(kind) if kind == "rustok_page_builder_contract_version_drift")));
    }

    #[tokio::test]
    async fn detects_core_leptos_and_missing_layers() {
        let snapshot = SnapshotId("snap".to_string());
        let output = RustokPageBuilderExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                source: source(
                    "crates/rustok-pages/admin/src/core.rs",
                    "use leptos::*;\n#[component]\npub fn Broken() {}\n",
                ),
                snapshot: snapshot.clone(),
            })
            .await
            .unwrap();
        let diagnostics = RustokPageBuilderChecker
            .check(CheckInput {
                entities: Arc::new(Vec::new()),
                facts: Arc::new(output.facts),
                relations: Arc::new(Vec::new()),
                affected: Default::default(),
                snapshot,
            })
            .await
            .unwrap();
        assert!(diagnostics.iter().any(|diagnostic| matches!(&diagnostic.kind, DiagnosticKind::Other(kind) if kind == "rustok_page_builder_registry_missing" || kind == "rustok_page_builder_fsd_core_leaks_ui")));
    }

    #[tokio::test]
    async fn detects_host_owned_module_ui() {
        let snapshot = SnapshotId("snap".to_string());
        let mut facts = Vec::new();
        for (path, content) in [
            (
                "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
                r#"{"provider":{"module_slug":"page_builder","contract":"grapesjs_v1","capabilities":["preview"]},"consumers":[],"fallback_profiles":["all_on"]}"#,
            ),
            (
                "apps/rustok-admin/src/pages.rs",
                "use rustok_pages::admin::PagesAdmin;\n#[component]\npub fn PagesScreen() -> impl IntoView { view! { <PagesAdmin/> } }\n",
            ),
        ] {
            let output = RustokPageBuilderExtractor
                .extract(input(path, content, snapshot.clone()))
                .await
                .unwrap();
            facts.extend(output.facts);
        }
        let diagnostics = RustokPageBuilderChecker
            .check(CheckInput {
                entities: Arc::new(Vec::new()),
                facts: Arc::new(facts),
                relations: Arc::new(Vec::new()),
                affected: Default::default(),
                snapshot,
            })
            .await
            .unwrap();
        assert!(diagnostics.iter().any(|diagnostic| matches!(&diagnostic.kind, DiagnosticKind::Other(kind) if kind == "rustok_page_builder_host_owns_module_ui")));
    }
}
