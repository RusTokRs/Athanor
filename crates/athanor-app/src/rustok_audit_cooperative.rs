use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::Result;
use athanor_core::{CanonicalSnapshot, OperationContext, OperationContextCancellation};
use athanor_domain::{Diagnostic, DiagnosticKind, Entity, RelationKind};

use crate::graph::{
    RUSTOK_FBA_AUDIT_SCHEMA, RUSTOK_FFA_AUDIT_SCHEMA, RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA,
    RustokFbaAudit, RustokFbaAuditModule, RustokFbaAuditSummary, RustokFfaAudit,
    RustokFfaAuditSummary, RustokFfaAuditSurface, RustokPageBuilderAudit,
    RustokPageBuilderAuditConsumer, RustokPageBuilderAuditSummary,
};

const RUSTOK_AUDIT_POLL_INTERVAL: usize = 64;

pub(crate) fn build_rustok_ffa_audit_with_operation_context(
    snapshot: &CanonicalSnapshot,
    operation: &OperationContext,
) -> Result<RustokFfaAudit> {
    build_ffa_with_checkpoint(snapshot, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

pub(crate) fn build_rustok_fba_audit_with_operation_context(
    snapshot: &CanonicalSnapshot,
    operation: &OperationContext,
) -> Result<RustokFbaAudit> {
    build_fba_with_checkpoint(snapshot, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

pub(crate) fn build_rustok_page_builder_audit_with_operation_context(
    snapshot: &CanonicalSnapshot,
    operation: &OperationContext,
) -> Result<RustokPageBuilderAudit> {
    build_page_builder_with_checkpoint(snapshot, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

fn build_ffa_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokFfaAudit> {
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_AUDIT_POLL_INTERVAL)?;
    let surface_index = ffa_surface_index(snapshot, &mut poller)?;
    let diagnostics = ffa_diagnostics(snapshot, None, None, &mut poller)?;
    let diagnostics_by_surface = diagnostics_by_surface(&diagnostics, &mut poller)?;
    let mut surfaces = Vec::with_capacity(surface_index.len());

    for ((module, surface), details) in surface_index {
        poller.step()?;
        let key = (module.clone(), surface.clone());
        let mut layers = details.layers.into_iter().collect::<Vec<_>>();
        let mut files = details.files.into_iter().collect::<Vec<_>>();
        layers.sort();
        files.sort();
        let diagnostics = diagnostics_by_surface
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let shape = ffa_shape(&layers);
        let actionable = !matches!(shape.as_str(), "host_wiring" | "scaffold");
        let core_present = layers.iter().any(|layer| layer == "core");
        let transport_present = layers.iter().any(|layer| layer == "transport");
        let ui_adapter_present = layers.iter().any(|layer| layer == "ui_leptos");
        let host_wiring_present = layers.iter().any(|layer| layer == "host_wiring");
        let requirements_met = if actionable {
            [core_present, transport_present, ui_adapter_present]
                .into_iter()
                .filter(|present| *present)
                .count()
        } else {
            0
        };
        let requirements_total = usize::from(actionable) * 3;
        surfaces.push(RustokFfaAuditSurface {
            id: format!("ffa_surface://{module}/{surface}"),
            module,
            surface,
            shape,
            actionable,
            requirements_met,
            requirements_total,
            completion_percent: completion_percent(requirements_met, requirements_total),
            core_present,
            transport_present,
            ui_adapter_present,
            host_wiring_present,
            diagnostics_open: diagnostics.len(),
            layers,
            files,
            diagnostics,
        });
    }
    poller.checkpoint()?;
    surfaces
        .sort_by(|left, right| (&left.module, &left.surface).cmp(&(&right.module, &right.surface)));

    let mut actionable_surfaces = 0;
    let mut scaffold_surfaces = 0;
    let mut host_wiring_surfaces = 0;
    let mut core_transport_ui = 0;
    let mut diagnostics_open = 0;
    let mut requirements_met = 0;
    let mut requirements_total = 0;
    let mut missing_core = 0;
    let mut missing_transport = 0;
    let mut missing_ui_adapter = 0;
    for surface in &surfaces {
        poller.step()?;
        if !matches!(surface.shape.as_str(), "host_wiring" | "scaffold") {
            actionable_surfaces += 1;
        }
        if surface.shape == "scaffold" {
            scaffold_surfaces += 1;
        }
        if surface.shape == "host_wiring" {
            host_wiring_surfaces += 1;
        }
        if surface.shape == "core_transport_ui" {
            core_transport_ui += 1;
        }
        diagnostics_open += surface.diagnostics.len();
        requirements_met += surface.requirements_met;
        requirements_total += surface.requirements_total;
        if surface.actionable && !surface.core_present {
            missing_core += 1;
        }
        if surface.actionable && !surface.transport_present {
            missing_transport += 1;
        }
        if surface.actionable && !surface.ui_adapter_present {
            missing_ui_adapter += 1;
        }
    }
    poller.finish()?;

    Ok(RustokFfaAudit {
        schema: RUSTOK_FFA_AUDIT_SCHEMA.to_string(),
        snapshot: snapshot_id(snapshot),
        summary: RustokFfaAuditSummary {
            observed_surfaces: surfaces.len(),
            surfaces_total: actionable_surfaces,
            core_transport_ui,
            incomplete: actionable_surfaces.saturating_sub(core_transport_ui),
            requirements_met,
            requirements_total,
            completion_percent: completion_percent(requirements_met, requirements_total),
            missing_core,
            missing_transport,
            missing_ui_adapter,
            scaffold_surfaces,
            host_wiring_surfaces,
            diagnostics_open,
        },
        surfaces,
    })
}

fn build_fba_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokFbaAudit> {
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_AUDIT_POLL_INTERVAL)?;
    let module_index = fba_module_index(snapshot, &mut poller)?;
    let diagnostics = fba_diagnostics(snapshot, None, &mut poller)?;
    let diagnostics_by_module = diagnostics_by_module(&diagnostics, &mut poller)?;
    let mut modules = Vec::with_capacity(module_index.len());

    for (module, details) in module_index {
        poller.step()?;
        let mut ports = details.ports.into_iter().collect::<Vec<_>>();
        let mut operations = details.operations.into_iter().collect::<Vec<_>>();
        let mut dependencies = details.dependencies.into_iter().collect::<Vec<_>>();
        ports.sort();
        operations.sort();
        dependencies.sort();
        let diagnostics = diagnostics_by_module
            .get(&module)
            .cloned()
            .unwrap_or_default();
        let diagnostic_absent =
            |kind: &str| !diagnostics.iter().any(|diagnostic| diagnostic == kind);
        let has_declared_ports = details.declared_ports > 0;
        let has_declared_operations = details.declared_operations > 0;
        let has_dependencies = !dependencies.is_empty();
        let port_code_present = has_declared_ports.then_some(details.port_code_present);
        let port_traits_present =
            has_declared_ports.then(|| diagnostic_absent("rustok_fba_port_trait_missing"));
        let port_traits_are_present = port_traits_present.unwrap_or(false);
        let operations_implemented = has_declared_operations.then(|| {
            port_traits_are_present && diagnostic_absent("rustok_fba_port_operation_missing")
        });
        let context_present = has_declared_ports.then(|| {
            port_traits_are_present && diagnostic_absent("rustok_fba_context_missing")
        });
        let error_present = has_declared_ports
            .then(|| port_traits_are_present && diagnostic_absent("rustok_fba_error_missing"));
        let policy_present = details
            .requires_policy
            .then(|| port_traits_are_present && diagnostic_absent("rustok_fba_policy_missing"));
        let evidence_present = details
            .registry_present
            .then(|| diagnostic_absent("rustok_fba_evidence_missing"));
        let contract_tests_present =
            has_declared_ports.then(|| diagnostic_absent("rustok_fba_contract_tests_missing"));
        let write_idempotency_present = details
            .requires_idempotency
            .then(|| diagnostic_absent("rustok_fba_write_idempotency_missing"));
        let dependencies_resolved = has_dependencies
            .then(|| diagnostic_absent("rustok_fba_consumer_provider_unresolved"));
        let requirement_values = [
            port_code_present,
            port_traits_present,
            operations_implemented,
            context_present,
            error_present,
            policy_present,
            evidence_present,
            contract_tests_present,
            write_idempotency_present,
            dependencies_resolved,
        ];
        let requirements_total = if details.registry_present {
            1 + requirement_values
                .iter()
                .filter(|value| value.is_some())
                .count()
        } else {
            0
        };
        let requirements_met = if details.registry_present {
            1 + requirement_values
                .iter()
                .filter(|value| **value == Some(true))
                .count()
        } else {
            0
        };
        modules.push(RustokFbaAuditModule {
            id: format!("fba_module://{module}"),
            module,
            role: details.role,
            status: details.status,
            registry_present: details.registry_present,
            requirements_met,
            requirements_total,
            completion_percent: completion_percent(requirements_met, requirements_total),
            port_code_present,
            port_traits_present,
            operations_implemented,
            context_present,
            error_present,
            policy_present,
            evidence_present,
            contract_tests_present,
            write_idempotency_present,
            dependencies_resolved,
            contract_version: details.contract_version,
            ports,
            operations,
            dependencies,
            diagnostics,
        });
    }
    poller.checkpoint()?;
    modules.sort_by(|left, right| left.module.cmp(&right.module));

    let mut provider_modules = 0;
    let mut consumer_modules = 0;
    let mut registered_modules = 0;
    let mut registered_module_ids = BTreeSet::new();
    let mut in_progress_modules = 0;
    let mut status_unknown_modules = 0;
    let mut ports_total = 0;
    let mut operations_total = 0;
    let mut diagnostics_open = 0;
    let mut requirements_met = 0;
    let mut requirements_total = 0;
    let mut modules_with_port_code = 0;
    let mut modules_with_complete_operations = 0;
    let mut modules_with_evidence = 0;
    let mut dependency_edges_total = 0;

    for module in &modules {
        poller.step()?;
        if module.role.as_deref() == Some("provider") {
            provider_modules += 1;
        }
        if matches!(
            module.role.as_deref(),
            Some("consumer") | Some("orchestrator_consumer") | Some("consumer_support_adapter")
        ) {
            consumer_modules += 1;
        }
        if module.registry_present {
            registered_modules += 1;
            registered_module_ids.insert(module.module.as_str());
        }
        if module.status.as_deref() == Some("in_progress") {
            in_progress_modules += 1;
        }
        if module.status.is_none() {
            status_unknown_modules += 1;
        }
        ports_total += module.ports.len();
        operations_total += module.operations.len();
        diagnostics_open += module.diagnostics.len();
        requirements_met += module.requirements_met;
        requirements_total += module.requirements_total;
        if module.port_code_present == Some(true) {
            modules_with_port_code += 1;
        }
        if module.operations_implemented == Some(true) {
            modules_with_complete_operations += 1;
        }
        if module.evidence_present == Some(true) {
            modules_with_evidence += 1;
        }
        dependency_edges_total += module.dependencies.len();
    }

    let mut dependency_edges_resolved = 0;
    for module in &modules {
        for dependency in &module.dependencies {
            poller.step()?;
            if dependency
                .split_once(':')
                .is_some_and(|(provider, _)| registered_module_ids.contains(provider))
            {
                dependency_edges_resolved += 1;
            }
        }
    }
    poller.finish()?;

    Ok(RustokFbaAudit {
        schema: RUSTOK_FBA_AUDIT_SCHEMA.to_string(),
        snapshot: snapshot_id(snapshot),
        summary: RustokFbaAuditSummary {
            modules_total: modules.len(),
            registered_modules,
            dependency_only_modules: modules.len().saturating_sub(registered_modules),
            in_progress_modules,
            status_unknown_modules,
            requirements_met,
            requirements_total,
            completion_percent: completion_percent(requirements_met, requirements_total),
            modules_with_port_code,
            modules_with_complete_operations,
            modules_with_evidence,
            dependency_edges_resolved,
            dependency_edges_total,
            provider_modules,
            consumer_modules,
            ports_total,
            operations_total,
            diagnostics_open,
        },
        modules,
    })
}

fn build_page_builder_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokPageBuilderAudit> {
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_AUDIT_POLL_INTERVAL)?;
    let diagnostics = page_builder_diagnostics(snapshot, None, &mut poller)?;
    let diagnostics_by_module = diagnostics_by_module(&diagnostics, &mut poller)?;
    let mut providers = Vec::new();
    let mut consumers = Vec::new();
    let mut contracts = Vec::new();
    let mut capabilities = Vec::new();
    let mut fallback_profiles = Vec::new();
    let mut wave_evidence = Vec::new();

    for entity in &snapshot.entities {
        poller.step()?;
        if !is_page_builder_entity(entity) {
            continue;
        }
        let stable = entity.stable_key.0.clone();
        match entity.kind {
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_provider" =>
            {
                providers.push(stable);
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_consumer" =>
            {
                let module = stable
                    .strip_prefix("page_builder_consumer://")
                    .unwrap_or(entity.name.as_str())
                    .to_string();
                consumers.push(RustokPageBuilderAuditConsumer {
                    id: stable,
                    module: module.clone(),
                    diagnostics: diagnostics_by_module
                        .get(&module)
                        .cloned()
                        .unwrap_or_default(),
                });
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_contract" =>
            {
                contracts.push(stable);
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_capability" =>
            {
                capabilities.push(stable);
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_fallback_profile" =>
            {
                fallback_profiles.push(stable);
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_wave_evidence" =>
            {
                wave_evidence.push(stable);
            }
            _ => {}
        }
    }

    poller.checkpoint()?;
    providers.sort();
    consumers.sort_by(|left, right| left.module.cmp(&right.module));
    contracts.sort();
    capabilities.sort();
    fallback_profiles.sort();
    wave_evidence.sort();
    let mut diagnostic_ids = Vec::with_capacity(diagnostics.len());
    for diagnostic in &diagnostics {
        poller.step()?;
        diagnostic_ids.push(diagnostic.id.0.clone());
    }
    poller.finish()?;

    Ok(RustokPageBuilderAudit {
        schema: RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA.to_string(),
        snapshot: snapshot_id(snapshot),
        summary: RustokPageBuilderAuditSummary {
            providers_total: providers.len(),
            consumers_total: consumers.len(),
            contracts_total: contracts.len(),
            capabilities_total: capabilities.len(),
            fallback_profiles_total: fallback_profiles.len(),
            wave_evidence_total: wave_evidence.len(),
            diagnostics_open: diagnostic_ids.len(),
        },
        providers,
        consumers,
        contracts,
        capabilities,
        fallback_profiles,
        wave_evidence,
        diagnostics: diagnostic_ids,
    })
}

#[derive(Debug, Clone, Default)]
struct FfaSurfaceDetails {
    layers: BTreeSet<String>,
    files: BTreeSet<String>,
}

fn ffa_surface_index<F>(
    snapshot: &CanonicalSnapshot,
    poller: &mut CheckpointPoller<F>,
) -> Result<BTreeMap<(String, String), FfaSurfaceDetails>>
where
    F: FnMut() -> Result<()>,
{
    let entity_by_id = entity_by_id(snapshot, poller)?;
    let mut index = BTreeMap::<(String, String), FfaSurfaceDetails>::new();
    for entity in &snapshot.entities {
        poller.step()?;
        if let Some((module, surface)) = parse_ffa_surface_key(&entity.stable_key.0) {
            index
                .entry((module.to_string(), surface.to_string()))
                .or_default();
        }
    }
    for relation in &snapshot.relations {
        poller.step()?;
        let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
            continue;
        };
        let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
            continue;
        };
        if matches!(relation.kind, RelationKind::Contains)
            && let (Some((module, surface)), Some((_, _, role))) = (
                parse_ffa_surface_key(&from.stable_key.0),
                parse_ffa_layer_key(&to.stable_key.0),
            )
        {
            index
                .entry((module.to_string(), surface.to_string()))
                .or_default()
                .layers
                .insert(role.to_string());
        }
        if matches!(relation.kind, RelationKind::ImplementedBy)
            && let Some((module, surface, role)) = parse_ffa_layer_key(&from.stable_key.0)
            && to.stable_key.0.starts_with("file://")
        {
            let details = index
                .entry((module.to_string(), surface.to_string()))
                .or_default();
            details.layers.insert(role.to_string());
            details.files.insert(to.stable_key.0.clone());
        }
    }
    Ok(index)
}

#[derive(Debug, Default)]
struct FbaModuleDetails {
    registry_present: bool,
    port_code_present: bool,
    declared_ports: usize,
    declared_operations: usize,
    requires_policy: bool,
    requires_idempotency: bool,
    role: Option<String>,
    status: Option<String>,
    contract_version: Option<String>,
    ports: BTreeSet<String>,
    operations: BTreeSet<String>,
    dependencies: BTreeSet<String>,
}

fn fba_module_index<F>(
    snapshot: &CanonicalSnapshot,
    poller: &mut CheckpointPoller<F>,
) -> Result<BTreeMap<String, FbaModuleDetails>>
where
    F: FnMut() -> Result<()>,
{
    let mut index = BTreeMap::<String, FbaModuleDetails>::new();
    for entity in &snapshot.entities {
        poller.step()?;
        if let Some(module) = parse_fba_module_key(&entity.stable_key.0) {
            let details = index.entry(module.to_string()).or_default();
            if details.role.is_none() {
                details.role = entity
                    .payload
                    .get("role")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
            }
        }
        if let Some((module, _)) = parse_fba_contract_key(&entity.stable_key.0) {
            let details = index.entry(module.to_string()).or_default();
            details.contract_version = entity
                .payload
                .get("contract_version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
        }
        if let Some((module, port)) = parse_fba_port_key(&entity.stable_key.0) {
            index
                .entry(module.to_string())
                .or_default()
                .ports
                .insert(port.to_string());
        }
        if let Some((module, port, operation)) = parse_fba_operation_key(&entity.stable_key.0) {
            index
                .entry(module.to_string())
                .or_default()
                .operations
                .insert(format!("{port}.{operation}"));
        }
        if let Some((consumer, provider, profile)) = parse_fba_dependency_key(&entity.stable_key.0)
        {
            index
                .entry(consumer.to_string())
                .or_default()
                .dependencies
                .insert(format!("{provider}:{profile}"));
        }
    }

    let diagnostics = fba_diagnostics(snapshot, None, poller)?;
    for diagnostic in diagnostics {
        poller.step()?;
        if let Some(module) = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
        {
            index.entry(module.to_string()).or_default();
        }
    }

    for fact in &snapshot.facts {
        poller.step()?;
        if matches!(&fact.kind, athanor_domain::FactKind::Other(kind) if kind == "rustok_fba_registry")
            && let Some(module) = fact.value.get("module").and_then(serde_json::Value::as_str)
        {
            let details = index.entry(module.to_string()).or_default();
            details.registry_present = true;
            if let Some(ports) = fact
                .value
                .get("ports")
                .and_then(serde_json::Value::as_array)
            {
                details.declared_ports = ports.len();
                details.declared_operations = 0;
                details.requires_policy = false;
                details.requires_idempotency = false;
                for port in ports {
                    poller.step()?;
                    details.declared_operations += port
                        .get("operations")
                        .and_then(serde_json::Value::as_array)
                        .map_or(0, Vec::len);
                    details.requires_policy |= port
                        .get("deadline_required")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false);
                    details.requires_idempotency |= port
                        .get("idempotency_required")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false);
                }
            }
            details.role = fact
                .value
                .get("role")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or(details.role.take());
            details.status = fact
                .value
                .get("status")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or(details.status.take());
            details.contract_version = fact
                .value
                .get("contract_version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or(details.contract_version.take());
        }
        if matches!(&fact.kind, athanor_domain::FactKind::Other(kind) if kind == "rustok_fba_port_code")
            && let Some(module) = fact.value.get("module").and_then(serde_json::Value::as_str)
        {
            index
                .entry(module.to_string())
                .or_default()
                .port_code_present = true;
        }
    }
    Ok(index)
}

fn entity_by_id<'a, F>(
    snapshot: &'a CanonicalSnapshot,
    poller: &mut CheckpointPoller<F>,
) -> Result<HashMap<&'a str, &'a Entity>>
where
    F: FnMut() -> Result<()>,
{
    let mut entities = HashMap::with_capacity(snapshot.entities.len());
    for entity in &snapshot.entities {
        poller.step()?;
        entities.insert(entity.id.0.as_str(), entity);
    }
    Ok(entities)
}

fn ffa_diagnostics<F>(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    surface: Option<&str>,
    poller: &mut CheckpointPoller<F>,
) -> Result<Vec<Diagnostic>>
where
    F: FnMut() -> Result<()>,
{
    let mut diagnostics = Vec::new();
    for diagnostic in &snapshot.diagnostics {
        poller.step()?;
        let matches_kind = matches!(
            &diagnostic.kind,
            DiagnosticKind::Other(kind) if kind.starts_with("rustok_ffa_")
        );
        let matches_module = module.is_none_or(|module| {
            diagnostic
                .payload
                .get("module")
                .and_then(serde_json::Value::as_str)
                == Some(module)
        });
        let matches_surface = surface.is_none_or(|surface| {
            diagnostic
                .payload
                .get("surface")
                .and_then(serde_json::Value::as_str)
                == Some(surface)
        });
        if matches_kind && matches_module && matches_surface {
            diagnostics.push(diagnostic.clone());
        }
    }
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    Ok(diagnostics)
}

fn fba_diagnostics<F>(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    poller: &mut CheckpointPoller<F>,
) -> Result<Vec<Diagnostic>>
where
    F: FnMut() -> Result<()>,
{
    let mut diagnostics = Vec::new();
    for diagnostic in &snapshot.diagnostics {
        poller.step()?;
        let matches_kind = matches!(
            &diagnostic.kind,
            DiagnosticKind::Other(kind) if kind.starts_with("rustok_fba_")
        );
        let matches_module = module.is_none_or(|module| {
            diagnostic
                .payload
                .get("module")
                .and_then(serde_json::Value::as_str)
                == Some(module)
        });
        if matches_kind && matches_module {
            diagnostics.push(diagnostic.clone());
        }
    }
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    Ok(diagnostics)
}

fn page_builder_diagnostics<F>(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    poller: &mut CheckpointPoller<F>,
) -> Result<Vec<Diagnostic>>
where
    F: FnMut() -> Result<()>,
{
    let mut diagnostics = Vec::new();
    for diagnostic in &snapshot.diagnostics {
        poller.step()?;
        let matches_kind = matches!(
            &diagnostic.kind,
            DiagnosticKind::Other(kind) if kind.starts_with("rustok_page_builder_")
        );
        let matches_module = module.is_none_or(|module| {
            diagnostic
                .payload
                .get("module")
                .and_then(serde_json::Value::as_str)
                == Some(module)
        });
        if matches_kind && matches_module {
            diagnostics.push(diagnostic.clone());
        }
    }
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    Ok(diagnostics)
}

fn diagnostics_by_surface<F>(
    diagnostics: &[Diagnostic],
    poller: &mut CheckpointPoller<F>,
) -> Result<BTreeMap<(String, String), Vec<String>>>
where
    F: FnMut() -> Result<()>,
{
    let mut by_surface = BTreeMap::<(String, String), Vec<String>>::new();
    for diagnostic in diagnostics {
        poller.step()?;
        let Some(module) = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(surface) = diagnostic
            .payload
            .get("surface")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        by_surface
            .entry((module.to_string(), surface.to_string()))
            .or_default()
            .push(serialized_name(&diagnostic.kind));
    }
    Ok(by_surface)
}

fn diagnostics_by_module<F>(
    diagnostics: &[Diagnostic],
    poller: &mut CheckpointPoller<F>,
) -> Result<BTreeMap<String, Vec<String>>>
where
    F: FnMut() -> Result<()>,
{
    let mut by_module = BTreeMap::<String, Vec<String>>::new();
    for diagnostic in diagnostics {
        poller.step()?;
        let Some(module) = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        by_module
            .entry(module.to_string())
            .or_default()
            .push(serialized_name(&diagnostic.kind));
    }
    Ok(by_module)
}

fn is_page_builder_entity(entity: &Entity) -> bool {
    matches!(
        &entity.kind,
        athanor_domain::EntityKind::Other(kind) if kind.starts_with("rustok_page_builder_")
    )
}

fn parse_ffa_surface_key(stable_key: &str) -> Option<(&str, &str)> {
    let rest = stable_key.strip_prefix("ffa_surface://")?;
    let mut parts = rest.split('/');
    let module = parts.next()?;
    let surface = parts.next()?;
    parts.next().is_none().then_some((module, surface))
}

fn parse_ffa_layer_key(stable_key: &str) -> Option<(&str, &str, &str)> {
    let rest = stable_key.strip_prefix("ffa_layer://")?;
    let mut parts = rest.split('/');
    let module = parts.next()?;
    let surface = parts.next()?;
    let role = parts.next()?;
    parts.next().is_none().then_some((module, surface, role))
}

fn parse_fba_module_key(stable_key: &str) -> Option<&str> {
    let rest = stable_key.strip_prefix("fba_module://")?;
    (!rest.contains('/')).then_some(rest)
}

fn parse_fba_contract_key(stable_key: &str) -> Option<(&str, &str)> {
    let rest = stable_key.strip_prefix("fba_contract://")?;
    let mut parts = rest.splitn(2, '/');
    Some((parts.next()?, parts.next()?))
}

fn parse_fba_port_key(stable_key: &str) -> Option<(&str, &str)> {
    let rest = stable_key.strip_prefix("fba_port://")?;
    let mut parts = rest.split('/');
    let module = parts.next()?;
    let port = parts.next()?;
    parts.next().is_none().then_some((module, port))
}

fn parse_fba_operation_key(stable_key: &str) -> Option<(&str, &str, &str)> {
    let rest = stable_key.strip_prefix("fba_operation://")?;
    let mut parts = rest.split('/');
    let module = parts.next()?;
    let port = parts.next()?;
    let operation = parts.next()?;
    parts.next().is_none().then_some((module, port, operation))
}

fn parse_fba_dependency_key(stable_key: &str) -> Option<(&str, &str, &str)> {
    let rest = stable_key.strip_prefix("fba_dependency://")?;
    let mut parts = rest.split('/');
    let consumer = parts.next()?;
    let provider = parts.next()?;
    let profile = parts.next()?;
    parts
        .next()
        .is_none()
        .then_some((consumer, provider, profile))
}

fn ffa_shape(layers: &[String]) -> String {
    if layers.iter().any(|layer| layer == "host_wiring") {
        return "host_wiring".to_string();
    }
    let has_core = layers.iter().any(|layer| layer == "core");
    let has_transport = layers.iter().any(|layer| layer == "transport");
    let has_ui = layers.iter().any(|layer| layer == "ui_leptos");
    if !has_core
        && !has_transport
        && !has_ui
        && layers
            .iter()
            .all(|layer| matches!(layer.as_str(), "crate_root" | "manifest"))
    {
        return "scaffold".to_string();
    }
    match (has_core, has_transport, has_ui) {
        (true, true, true) => "core_transport_ui",
        (true, true, false) => "core_transport",
        (true, false, true) => "core_ui",
        (false, true, true) => "transport_ui",
        (true, false, false) => "core_only",
        (false, true, false) => "transport_only",
        (false, false, true) => "ui_only",
        (false, false, false) => "none",
    }
    .to_string()
}

fn completion_percent(completed: usize, total: usize) -> Option<u8> {
    (total > 0).then(|| ((completed * 100 + total / 2) / total).min(100) as u8)
}

fn snapshot_id(snapshot: &CanonicalSnapshot) -> String {
    snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone())
}

fn serialized_name(value: &impl serde::Serialize) -> String {
    let Ok(value) = serde_json::to_value(value) else {
        return "unknown".to_string();
    };
    if let Some(name) = value.as_str() {
        return name.to_string();
    }
    value
        .get("other")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string())
}

struct CheckpointPoller<F> {
    checkpoint: F,
    interval: usize,
    remaining: usize,
}

impl<F> CheckpointPoller<F>
where
    F: FnMut() -> Result<()>,
{
    fn new(mut checkpoint: F, interval: usize) -> Result<Self> {
        checkpoint()?;
        let interval = interval.max(1);
        Ok(Self {
            checkpoint,
            interval,
            remaining: interval,
        })
    }

    fn step(&mut self) -> Result<()> {
        self.remaining -= 1;
        if self.remaining == 0 {
            self.checkpoint()?;
            self.remaining = self.interval;
        }
        Ok(())
    }

    fn checkpoint(&mut self) -> Result<()> {
        (self.checkpoint)()?;
        self.remaining = self.interval;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.checkpoint()
    }
}

#[cfg(test)]
mod tests {
    use athanor_core::CoreError;
    use athanor_domain::{
        Diagnostic, DiagnosticId, DiagnosticStatus, EntityId, EntityKind, Fact, FactId, FactKind,
        Relation, RelationId, RelationStatus, Severity, SnapshotId, StableKey,
    };
    use serde_json::json;

    use super::*;
    use crate::graph::{
        build_rustok_fba_audit, build_rustok_ffa_audit, build_rustok_page_builder_audit,
    };

    #[test]
    fn cooperative_audits_match_legacy_outputs() {
        let surface = entity(
            "ffa-surface",
            "ffa_surface://catalog/product",
            "rustok_ffa_surface",
        );
        let layer = entity(
            "ffa-layer",
            "ffa_layer://catalog/product/core",
            "rustok_ffa_layer",
        );
        let file = entity("file", "file://src/product.rs", "file");
        let fba_module = entity("fba-module", "fba_module://catalog", "rustok_fba_module");
        let fba_port = entity("fba-port", "fba_port://catalog/read", "rustok_fba_port");
        let fba_operation = entity(
            "fba-operation",
            "fba_operation://catalog/read/get",
            "rustok_fba_operation",
        );
        let fba_dependency = entity(
            "fba-dependency",
            "fba_dependency://consumer/catalog/native",
            "rustok_fba_dependency",
        );
        let page_provider = entity(
            "page-provider",
            "page_builder_provider://page_builder",
            "rustok_page_builder_provider",
        );
        let page_consumer = entity(
            "page-consumer",
            "page_builder_consumer://catalog",
            "rustok_page_builder_consumer",
        );
        let page_contract = entity(
            "page-contract",
            "page_builder_contract://catalog/read",
            "rustok_page_builder_contract",
        );
        let page_capability = entity(
            "page-capability",
            "page_builder_capability://catalog/read",
            "rustok_page_builder_capability",
        );
        let page_fallback = entity(
            "page-fallback",
            "page_builder_fallback_profile://catalog/default",
            "rustok_page_builder_fallback_profile",
        );
        let page_wave = entity(
            "page-wave",
            "page_builder_wave_evidence://catalog/wave-1",
            "rustok_page_builder_wave_evidence",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_audit".to_string())),
            entities: vec![
                surface.clone(),
                layer.clone(),
                file.clone(),
                fba_module.clone(),
                fba_port,
                fba_operation,
                fba_dependency,
                page_provider,
                page_consumer,
                page_contract,
                page_capability,
                page_fallback,
                page_wave,
            ],
            facts: vec![
                fact(
                    "fba-registry",
                    FactKind::Other("rustok_fba_registry".to_string()),
                    &fba_module,
                    json!({
                        "module": "catalog",
                        "role": "provider",
                        "status": "complete",
                        "contract_version": "v1",
                        "ports": [{
                            "operations": [{}],
                            "deadline_required": true,
                            "idempotency_required": true
                        }]
                    }),
                ),
                fact(
                    "fba-port-code",
                    FactKind::Other("rustok_fba_port_code".to_string()),
                    &fba_module,
                    json!({ "module": "catalog" }),
                ),
            ],
            relations: vec![
                relation("contains", RelationKind::Contains, &surface, &layer),
                relation("implemented", RelationKind::ImplementedBy, &layer, &file),
            ],
            diagnostics: vec![
                diagnostic(
                    "ffa-diagnostic",
                    "rustok_ffa_transport_missing",
                    json!({
                        "module": "catalog",
                        "surface": "product",
                        "role": "transport",
                        "path": "src/product.rs"
                    }),
                ),
                diagnostic(
                    "fba-diagnostic",
                    "rustok_fba_policy_missing",
                    json!({ "module": "catalog", "port": "read" }),
                ),
                diagnostic(
                    "page-diagnostic",
                    "rustok_page_builder_contract_missing",
                    json!({ "module": "catalog" }),
                ),
            ],
        };
        let operation = OperationContext::new("rustok-audit-parity");

        assert_eq!(
            build_rustok_ffa_audit_with_operation_context(&snapshot, &operation).unwrap(),
            build_rustok_ffa_audit(&snapshot)
        );
        assert_eq!(
            build_rustok_fba_audit_with_operation_context(&snapshot, &operation).unwrap(),
            build_rustok_fba_audit(&snapshot)
        );
        assert_eq!(
            build_rustok_page_builder_audit_with_operation_context(&snapshot, &operation).unwrap(),
            build_rustok_page_builder_audit(&snapshot)
        );
    }

    #[test]
    fn cancellation_after_multiple_batches_stops_audit_builder() {
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_large_audit".to_string())),
            entities: (0..800)
                .map(|index| {
                    entity(
                        &format!("module-{index}"),
                        &format!("fba_module://module-{index}"),
                        "rustok_fba_module",
                    )
                })
                .collect(),
            ..CanonicalSnapshot::default()
        };
        let operation = OperationContext::new("rustok-audit-mid-cancel");
        let cancellation = operation.cancellation_handle().unwrap();
        let mut checkpoints = 0;

        let error = build_fba_with_checkpoint(&snapshot, || {
            checkpoints += 1;
            if checkpoints == 3 {
                cancellation.cancel();
            }
            operation.check_active().map_err(anyhow::Error::new)
        })
        .expect_err("cancelled audit build must stop after bounded batches");

        assert!(checkpoints >= 3);
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    fn entity(id: &str, stable_key: &str, kind: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind: if kind == "file" {
                EntityKind::File
            } else {
                EntityKind::Other(kind.to_string())
            },
            name: id.to_string(),
            title: None,
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }

    fn fact(id: &str, kind: FactKind, subject: &Entity, value: serde_json::Value) -> Fact {
        Fact {
            id: FactId(id.to_string()),
            kind,
            subject: subject.id.clone(),
            object: None,
            value,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_audit".to_string()),
            extractor: "test".to_string(),
            confidence: 1.0,
        }
    }

    fn relation(id: &str, kind: RelationKind, from: &Entity, to: &Entity) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind,
            from: from.id.clone(),
            to: to.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_audit".to_string()),
            payload: json!({}),
        }
    }

    fn diagnostic(id: &str, kind: &str, payload: serde_json::Value) -> Diagnostic {
        Diagnostic {
            id: DiagnosticId(id.to_string()),
            kind: DiagnosticKind::Other(kind.to_string()),
            severity: Severity::Medium,
            status: DiagnosticStatus::Open,
            title: id.to_string(),
            message: id.to_string(),
            entities: Vec::new(),
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_audit".to_string()),
            suggested_fix: None,
            payload,
        }
    }
}
