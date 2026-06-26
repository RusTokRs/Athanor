use std::collections::{BTreeSet, HashMap};

use async_trait::async_trait;
use athanor_core::{
    CheckInput, Checker, CoreResult, ExtractInput, ExtractOutput, Extractor, LinkInput, Linker,
    SourceFile,
};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, Fact, FactId, FactKind, LanguageCode, Ownership, Relation, RelationId, RelationKind,
    RelationStatus, Severity, SnapshotId, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const FBA_EXTRACTOR_ID: &str = "rustok_fba";
pub const FBA_LINKER_ID: &str = "rustok_fba_linker";
pub const FBA_CHECKER_ID: &str = "rustok_fba_checker";
pub const FBA_REGISTRY_FACT_KIND: &str = "rustok_fba_registry";
pub const FBA_PORT_CODE_FACT_KIND: &str = "rustok_fba_port_code";
pub const FBA_MODULE_ENTITY_KIND: &str = "rustok_fba_module";
pub const FBA_CONTRACT_ENTITY_KIND: &str = "rustok_fba_contract";
pub const FBA_PORT_ENTITY_KIND: &str = "rustok_fba_port";
pub const FBA_OPERATION_ENTITY_KIND: &str = "rustok_fba_operation";
pub const FBA_PROFILE_ENTITY_KIND: &str = "rustok_fba_profile";
pub const FBA_DEPENDENCY_ENTITY_KIND: &str = "rustok_fba_dependency";

#[derive(Debug, Clone, Default)]
pub struct RustokFbaExtractor;

#[derive(Debug, Clone, Default)]
pub struct RustokFbaLinker;

#[derive(Debug, Clone, Default)]
pub struct RustokFbaChecker;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FbaRegistryMarker {
    pub schema: String,
    pub path: String,
    pub module: String,
    pub role: String,
    pub status: Option<String>,
    pub contract_version: Option<String>,
    pub ports: Vec<FbaPortSpec>,
    pub consumers: Vec<FbaConsumerSpec>,
    pub providers: Vec<FbaProviderDependency>,
    pub profiles: Vec<String>,
    pub evidence_paths: Vec<String>,
    pub in_process_impl_source: Option<String>,
    pub contract_tests_status: Option<String>,
    pub contract_test_cases: Vec<FbaContractTestCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FbaPortSpec {
    pub name: String,
    pub owner: Option<String>,
    pub operations: Vec<String>,
    pub context: Option<String>,
    pub error: Option<String>,
    pub idempotency_required: bool,
    pub deadline_required: bool,
    pub read_operations: Vec<String>,
    pub write_operations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FbaConsumerSpec {
    pub module: String,
    pub profile: Option<String>,
    pub fallback_profiles: Vec<String>,
    pub degraded_modes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FbaProviderDependency {
    pub module: String,
    pub contract_version: Option<String>,
    pub ports: Vec<String>,
    pub profiles: Vec<String>,
    pub fallback_profiles: Vec<String>,
    pub degraded_modes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FbaContractTestCase {
    pub operation: String,
    pub profiles: Vec<String>,
    pub assertions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FbaPortCodeMarker {
    pub schema: String,
    pub path: String,
    pub module: String,
    pub ports: Vec<FbaCodePort>,
    pub has_port_context: bool,
    pub has_port_error: bool,
    pub has_read_policy: bool,
    pub has_write_policy: bool,
    pub first_marker_line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FbaCodePort {
    pub name: String,
    pub operations: Vec<String>,
}

#[async_trait]
impl Extractor for RustokFbaExtractor {
    fn name(&self) -> &str {
        FBA_EXTRACTOR_ID
    }

    fn supports(&self, source: &SourceFile) -> bool {
        let path = normalize_path(&source.path);
        is_fba_registry_path(&path) || is_rustok_ports_path(&path)
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let path = normalize_path(&input.source.path);
        if is_fba_registry_path(&path) {
            return Ok(extract_registry(&input.source, &input.snapshot, &path));
        }
        if is_rustok_ports_path(&path) {
            return Ok(extract_port_code(&input.source, &input.snapshot, &path));
        }
        Ok(ExtractOutput::default())
    }
}

#[async_trait]
impl Linker for RustokFbaLinker {
    fn name(&self) -> &str {
        FBA_LINKER_ID
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
        let entity_by_id = input
            .entities
            .iter()
            .map(|entity| (entity.id.0.as_str(), entity))
            .collect::<HashMap<_, _>>();
        let mut relations = Vec::new();
        let mut seen = BTreeSet::new();

        for fact in input.facts.iter().filter(|fact| is_registry_fact(fact)) {
            let Some(registry) = registry_from_fact(fact) else {
                continue;
            };
            let Some(file) = entity_by_id.get(fact.subject.0.as_str()) else {
                continue;
            };
            let module = fba_module_entity(&registry.module, &input.snapshot, Some(&registry.path));
            push_relation(
                &mut relations,
                &mut seen,
                &input.snapshot,
                RelationKind::ImplementedBy,
                &module.id,
                &file.id,
                fact.evidence.clone(),
                fact.ownership.clone(),
                json!({"schema": "rustok.fba.relation.v1", "kind": "evidenced_by"}),
            );

            if let Some(contract_version) = &registry.contract_version {
                let contract = fba_contract_entity(
                    &registry.module,
                    contract_version,
                    &input.snapshot,
                    Some(&registry.path),
                );
                push_relation(
                    &mut relations,
                    &mut seen,
                    &input.snapshot,
                    RelationKind::Contains,
                    &module.id,
                    &contract.id,
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                    json!({"schema": "rustok.fba.relation.v1", "kind": "module_provides_contract"}),
                );

                for port in &registry.ports {
                    let port_entity = fba_port_entity(
                        &registry.module,
                        &port.name,
                        &input.snapshot,
                        Some(&registry.path),
                    );
                    push_relation(
                        &mut relations,
                        &mut seen,
                        &input.snapshot,
                        RelationKind::Contains,
                        &contract.id,
                        &port_entity.id,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                        json!({"schema": "rustok.fba.relation.v1", "kind": "contract_exposes_port"}),
                    );
                    for operation in &port.operations {
                        let operation_entity = fba_operation_entity(
                            &registry.module,
                            &port.name,
                            operation,
                            &input.snapshot,
                            Some(&registry.path),
                        );
                        push_relation(
                            &mut relations,
                            &mut seen,
                            &input.snapshot,
                            RelationKind::Contains,
                            &port_entity.id,
                            &operation_entity.id,
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                            json!({"schema": "rustok.fba.relation.v1", "kind": "port_has_operation"}),
                        );
                    }
                }

                for profile in &registry.profiles {
                    let profile_entity = fba_profile_entity(
                        &registry.module,
                        profile,
                        &input.snapshot,
                        Some(&registry.path),
                    );
                    push_relation(
                        &mut relations,
                        &mut seen,
                        &input.snapshot,
                        RelationKind::Contains,
                        &contract.id,
                        &profile_entity.id,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                        json!({"schema": "rustok.fba.relation.v1", "kind": "contract_has_profile"}),
                    );
                }
            }

            for consumer in &registry.consumers {
                if let Some(profile) = &consumer.profile {
                    let dependency = fba_dependency_entity(
                        &consumer.module,
                        &registry.module,
                        profile,
                        &input.snapshot,
                        Some(&registry.path),
                    );
                    push_relation(
                        &mut relations,
                        &mut seen,
                        &input.snapshot,
                        RelationKind::Other("rustok_fba_consumer_requires_provider".to_string()),
                        &dependency.id,
                        &module.id,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                        json!({
                            "schema": "rustok.fba.relation.v1",
                            "consumer": consumer.module,
                            "provider": registry.module,
                            "profile": profile,
                        }),
                    );
                }
            }

            for provider in &registry.providers {
                for profile in provider
                    .profiles
                    .iter()
                    .chain(provider.fallback_profiles.iter())
                {
                    let dependency = fba_dependency_entity(
                        &registry.module,
                        &provider.module,
                        profile,
                        &input.snapshot,
                        Some(&registry.path),
                    );
                    let provider_module =
                        fba_module_entity(&provider.module, &input.snapshot, Some(&registry.path));
                    push_relation(
                        &mut relations,
                        &mut seen,
                        &input.snapshot,
                        RelationKind::Other("rustok_fba_consumer_requires_provider".to_string()),
                        &dependency.id,
                        &provider_module.id,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                        json!({
                            "schema": "rustok.fba.relation.v1",
                            "consumer": registry.module,
                            "provider": provider.module,
                            "profile": profile,
                        }),
                    );
                }
            }
        }

        for fact in input.facts.iter().filter(|fact| is_port_code_fact(fact)) {
            let Some(marker) = port_code_from_fact(fact) else {
                continue;
            };
            for port in marker.ports {
                let port_entity = fba_port_entity(
                    &marker.module,
                    &port.name,
                    &input.snapshot,
                    Some(&marker.path),
                );
                push_relation(
                    &mut relations,
                    &mut seen,
                    &input.snapshot,
                    RelationKind::ImplementedBy,
                    &port_entity.id,
                    &fact.subject,
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                    json!({"schema": "rustok.fba.relation.v1", "kind": "implemented_by"}),
                );
            }
        }

        Ok(relations)
    }
}

#[async_trait]
impl Checker for RustokFbaChecker {
    fn name(&self) -> &str {
        FBA_CHECKER_ID
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let registries = input
            .facts
            .iter()
            .filter(|fact| is_registry_fact(fact))
            .filter_map(|fact| registry_from_fact(fact).map(|marker| (fact, marker)))
            .collect::<Vec<_>>();
        let code_markers = input
            .facts
            .iter()
            .filter(|fact| is_port_code_fact(fact))
            .filter_map(|fact| port_code_from_fact(fact).map(|marker| (fact, marker)))
            .collect::<Vec<_>>();
        let code_by_module = merged_code_markers_by_module(&code_markers);
        let registry_modules = registries
            .iter()
            .map(|(_, marker)| marker.module.as_str())
            .collect::<BTreeSet<_>>();
        let registry_paths = registries
            .iter()
            .map(|(_, marker)| marker.path.as_str())
            .collect::<BTreeSet<_>>();

        let mut diagnostics = Vec::new();

        for (fact, registry) in &registries {
            if registry.ports.is_empty()
                && registry.role == "provider"
                && registry.contract_version.is_some()
                && registry.module != "page_builder"
            {
                diagnostics.push(diagnostic(
                    &input.snapshot,
                    "rustok_fba_port_trait_missing",
                    Severity::Medium,
                    "FBA registry declares provider role without ports",
                    format!(
                        "{} is a FBA {} registry but does not declare any ports",
                        registry.module, registry.role
                    ),
                    registry,
                    None,
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }

            if registry.evidence_paths.is_empty() {
                diagnostics.push(diagnostic(
                    &input.snapshot,
                    "rustok_fba_evidence_missing",
                    Severity::Low,
                    "FBA registry has no explicit evidence paths",
                    format!(
                        "{} FBA registry should point at verifier/docs evidence",
                        registry.module
                    ),
                    registry,
                    None,
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }

            if !registry.ports.is_empty()
                && (registry.contract_tests_status.is_none()
                    || registry.contract_test_cases.is_empty())
            {
                diagnostics.push(diagnostic(
                    &input.snapshot,
                    "rustok_fba_contract_tests_missing",
                    Severity::Medium,
                    "FBA contract tests are not locked",
                    format!(
                        "{} FBA registry should declare contract_tests.status and cases",
                        registry.module
                    ),
                    registry,
                    None,
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }

            let code = code_by_module.get(registry.module.as_str());
            let code_ports = code
                .map(|marker| {
                    marker
                        .ports
                        .iter()
                        .map(|port| (port.name.as_str(), port))
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default();

            for port in &registry.ports {
                let Some(code_port) = code_ports.get(port.name.as_str()) else {
                    diagnostics.push(diagnostic(
                        &input.snapshot,
                        "rustok_fba_port_trait_missing",
                        Severity::High,
                        format!("FBA port trait {} is missing in code", port.name),
                        format!(
                            "{} declares {} but no matching trait was found in src/ports",
                            registry.module, port.name
                        ),
                        registry,
                        Some(&port.name),
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                    ));
                    continue;
                };

                for operation in &port.operations {
                    if !code_port
                        .operations
                        .iter()
                        .any(|candidate| candidate == operation)
                    {
                        diagnostics.push(diagnostic(
                            &input.snapshot,
                            "rustok_fba_port_operation_missing",
                            Severity::High,
                            format!("FBA operation {operation} is missing in code"),
                            format!("{} declares {}.{operation}, but the method was not found in src/ports", registry.module, port.name),
                            registry,
                            Some(&port.name),
                            fact.evidence.clone(),
                            fact.ownership.clone(),
                        ));
                    }
                }

                if !port
                    .context
                    .as_deref()
                    .is_some_and(|context| context.ends_with("::PortContext"))
                    || !code.is_some_and(|marker| marker.has_port_context)
                {
                    diagnostics.push(diagnostic(
                        &input.snapshot,
                        "rustok_fba_context_missing",
                        Severity::High,
                        format!("FBA port {} does not use shared PortContext", port.name),
                        format!("{} must declare and implement the shared rustok_api::ports::PortContext boundary", port.name),
                        registry,
                        Some(&port.name),
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                    ));
                }

                if !port
                    .error
                    .as_deref()
                    .is_some_and(|error| error.ends_with("::PortError"))
                    || !code.is_some_and(|marker| marker.has_port_error)
                {
                    diagnostics.push(diagnostic(
                        &input.snapshot,
                        "rustok_fba_error_missing",
                        Severity::High,
                        format!("FBA port {} does not use shared PortError", port.name),
                        format!("{} must declare and implement the shared rustok_api::ports::PortError boundary", port.name),
                        registry,
                        Some(&port.name),
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                    ));
                }

                if port.deadline_required
                    && !code.is_some_and(|marker| marker.has_read_policy || marker.has_write_policy)
                {
                    diagnostics.push(diagnostic(
                        &input.snapshot,
                        "rustok_fba_policy_missing",
                        Severity::Medium,
                        format!("FBA port {} has no shared PortCallPolicy enforcement", port.name),
                        format!("{} requires deadline/policy metadata but code lacks require_policy calls", port.name),
                        registry,
                        Some(&port.name),
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                    ));
                }

                if port.idempotency_required {
                    for operation in write_operations_for(port) {
                        let has_assertion = registry.contract_test_cases.iter().any(|case| {
                            case.operation == operation
                                && case.assertions.iter().any(|assertion| {
                                    matches!(
                                        assertion.as_str(),
                                        "write_idempotency_required"
                                            | "write_idempotency_preserved"
                                    )
                                })
                        });
                        if !has_assertion {
                            diagnostics.push(diagnostic(
                                &input.snapshot,
                                "rustok_fba_write_idempotency_missing",
                                Severity::Medium,
                                format!("FBA write operation {operation} lacks idempotency assertion"),
                                format!("{} requires write idempotency but {operation} has no contract-test assertion", port.name),
                                registry,
                                Some(&port.name),
                                fact.evidence.clone(),
                                fact.ownership.clone(),
                            ));
                        }
                    }
                }
            }

            for provider in &registry.providers {
                if !registry_modules.contains(provider.module.as_str())
                    && !registry_paths.contains(provider_registry_path(&provider.module).as_str())
                {
                    diagnostics.push(diagnostic(
                        &input.snapshot,
                        "rustok_fba_consumer_provider_unresolved",
                        Severity::High,
                        format!("FBA provider dependency {} is unresolved", provider.module),
                        format!(
                            "{} depends on provider {}, but no provider registry was indexed",
                            registry.module, provider.module
                        ),
                        registry,
                        None,
                        fact.evidence.clone(),
                        fact.ownership.clone(),
                    ));
                }
            }
        }

        for (fact, marker) in &code_markers {
            if marker.module == "core" {
                continue;
            }
            if !registry_modules.contains(marker.module.as_str()) {
                diagnostics.push(diagnostic(
                    &input.snapshot,
                    "rustok_fba_registry_missing",
                    Severity::Medium,
                    "FBA code has no registry",
                    format!(
                        "{} has src/ports FBA markers but no contracts/*-fba-registry.json",
                        marker.module
                    ),
                    &FbaRegistryMarker::from_code_marker(marker),
                    None,
                    fact.evidence.clone(),
                    fact.ownership.clone(),
                ));
            }
        }

        diagnostics.sort_by_key(|diagnostic| diagnostic.id.0.clone());
        diagnostics.dedup_by(|left, right| left.id == right.id);
        Ok(diagnostics)
    }
}

impl FbaRegistryMarker {
    fn from_code_marker(marker: &FbaPortCodeMarker) -> Self {
        Self {
            schema: "rustok.fba.registry.v1".to_string(),
            path: marker.path.clone(),
            module: marker.module.clone(),
            role: "code".to_string(),
            status: None,
            contract_version: None,
            ports: Vec::new(),
            consumers: Vec::new(),
            providers: Vec::new(),
            profiles: Vec::new(),
            evidence_paths: Vec::new(),
            in_process_impl_source: None,
            contract_tests_status: None,
            contract_test_cases: Vec::new(),
        }
    }
}

fn extract_registry(source: &SourceFile, snapshot: &SnapshotId, path: &str) -> ExtractOutput {
    let content = source.content.as_deref().unwrap_or_default();
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return ExtractOutput::default();
    };
    let Some(marker) = registry_marker_from_value(path, &value) else {
        return ExtractOutput::default();
    };

    let file = file_entity(source, &snapshot.0);
    let mut entities = vec![
        file.clone(),
        fba_module_entity(&marker.module, snapshot, Some(path)),
    ];
    if let Some(contract_version) = &marker.contract_version {
        entities.push(fba_contract_entity(
            &marker.module,
            contract_version,
            snapshot,
            Some(path),
        ));
    }
    for port in &marker.ports {
        entities.push(fba_port_entity(
            &marker.module,
            &port.name,
            snapshot,
            Some(path),
        ));
        for operation in &port.operations {
            entities.push(fba_operation_entity(
                &marker.module,
                &port.name,
                operation,
                snapshot,
                Some(path),
            ));
        }
    }
    for profile in &marker.profiles {
        entities.push(fba_profile_entity(
            &marker.module,
            profile,
            snapshot,
            Some(path),
        ));
    }
    for consumer in &marker.consumers {
        if let Some(profile) = &consumer.profile {
            entities.push(fba_dependency_entity(
                &consumer.module,
                &marker.module,
                profile,
                snapshot,
                Some(path),
            ));
        }
    }
    for provider in &marker.providers {
        entities.push(fba_module_entity(&provider.module, snapshot, Some(path)));
        for profile in provider
            .profiles
            .iter()
            .chain(provider.fallback_profiles.iter())
        {
            entities.push(fba_dependency_entity(
                &marker.module,
                &provider.module,
                profile,
                snapshot,
                Some(path),
            ));
        }
    }
    dedup_entities(&mut entities);

    let fact = Fact {
        id: FactId(format!(
            "fact_rustok_fba_registry_{:016x}",
            stable_hash(format!("{}:{}", file.stable_key.0, marker.module).as_bytes())
        )),
        kind: FactKind::Other(FBA_REGISTRY_FACT_KIND.to_string()),
        subject: file.id,
        object: None,
        value: serde_json::to_value(marker).expect("FbaRegistryMarker serializes"),
        evidence: vec![evidence_for_file(path, FBA_EXTRACTOR_ID, Some(1), Some(1))],
        ownership: ownership_for_file(path),
        snapshot: snapshot.clone(),
        extractor: FBA_EXTRACTOR_ID.to_string(),
        confidence: 1.0,
    };

    ExtractOutput {
        entities,
        facts: vec![fact],
    }
}

fn extract_port_code(source: &SourceFile, snapshot: &SnapshotId, path: &str) -> ExtractOutput {
    let Some(module) = module_from_crate_path(path) else {
        return ExtractOutput::default();
    };
    if module == "core" {
        return ExtractOutput::default();
    }
    let content = source.content.as_deref().unwrap_or_default();
    let ports = extract_code_ports(content);
    if ports.is_empty() && !contains_any(content, &["PortContext", "PortError", "PortCallPolicy"]) {
        return ExtractOutput::default();
    }
    let file = file_entity(source, &snapshot.0);
    let marker = FbaPortCodeMarker {
        schema: "rustok.fba.port_code.v1".to_string(),
        path: path.to_string(),
        module: module.clone(),
        ports: ports.clone(),
        has_port_context: content.contains("PortContext"),
        has_port_error: content.contains("PortError"),
        has_read_policy: content.contains("PortCallPolicy::read()"),
        has_write_policy: content.contains("PortCallPolicy::write()"),
        first_marker_line: first_marker_line(content),
    };
    let mut entities = vec![
        file.clone(),
        fba_module_entity(&module, snapshot, Some(path)),
    ];
    for port in &ports {
        entities.push(fba_port_entity(&module, &port.name, snapshot, Some(path)));
        for operation in &port.operations {
            entities.push(fba_operation_entity(
                &module,
                &port.name,
                operation,
                snapshot,
                Some(path),
            ));
        }
    }
    dedup_entities(&mut entities);
    let fact = Fact {
        id: FactId(format!(
            "fact_rustok_fba_port_code_{:016x}",
            stable_hash(format!("{}:ports", file.stable_key.0).as_bytes())
        )),
        kind: FactKind::Other(FBA_PORT_CODE_FACT_KIND.to_string()),
        subject: file.id,
        object: None,
        value: serde_json::to_value(marker).expect("FbaPortCodeMarker serializes"),
        evidence: vec![evidence_for_file(
            path,
            FBA_EXTRACTOR_ID,
            first_marker_line(content),
            first_marker_line(content),
        )],
        ownership: ownership_for_file(path),
        snapshot: snapshot.clone(),
        extractor: FBA_EXTRACTOR_ID.to_string(),
        confidence: 1.0,
    };

    ExtractOutput {
        entities,
        facts: vec![fact],
    }
}

fn registry_marker_from_value(path: &str, value: &Value) -> Option<FbaRegistryMarker> {
    let module = value
        .get("module")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .pointer("/provider/module_slug")
                .and_then(Value::as_str)
        })?
        .to_string();
    let role = value
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("provider")
        .to_string();
    let contract_version = value
        .get("contract_version")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/provider/contract").and_then(Value::as_str))
        .map(str::to_string);
    let ports: Vec<FbaPortSpec> = value
        .get("ports")
        .and_then(Value::as_array)
        .map(|ports| ports.iter().filter_map(port_spec_from_value).collect())
        .unwrap_or_default();
    let consumers: Vec<FbaConsumerSpec> = value
        .get("consumers")
        .and_then(Value::as_array)
        .map(|consumers| consumers.iter().filter_map(consumer_from_value).collect())
        .unwrap_or_default();
    let providers: Vec<FbaProviderDependency> = value
        .get("providers")
        .or_else(|| value.get("provider_dependencies"))
        .and_then(Value::as_array)
        .map(|providers| {
            providers
                .iter()
                .filter_map(provider_dependency_from_value)
                .collect()
        })
        .unwrap_or_default();
    let mut profiles = BTreeSet::new();
    collect_string_array(value.get("fallback_profiles"), &mut profiles);
    for consumer in &consumers {
        if let Some(profile) = &consumer.profile {
            profiles.insert(profile.clone());
        }
        for profile in &consumer.fallback_profiles {
            profiles.insert(profile.clone());
        }
    }
    for provider in &providers {
        for profile in provider
            .profiles
            .iter()
            .chain(provider.fallback_profiles.iter())
        {
            profiles.insert(profile.clone());
        }
    }
    let evidence_paths = evidence_paths(value);
    let contract_tests = value.get("contract_tests");
    Some(FbaRegistryMarker {
        schema: "rustok.fba.registry.v1".to_string(),
        path: path.to_string(),
        module,
        role,
        status: value
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_string),
        contract_version,
        ports,
        consumers,
        providers,
        profiles: profiles.into_iter().collect(),
        evidence_paths,
        in_process_impl_source: value
            .pointer("/in_process_provider_impl/source")
            .and_then(Value::as_str)
            .map(str::to_string),
        contract_tests_status: contract_tests
            .and_then(|tests| tests.get("status"))
            .and_then(Value::as_str)
            .map(str::to_string),
        contract_test_cases: contract_tests
            .and_then(|tests| tests.get("cases"))
            .and_then(Value::as_array)
            .map(|cases| cases.iter().filter_map(contract_case_from_value).collect())
            .unwrap_or_default(),
    })
}

fn port_spec_from_value(value: &Value) -> Option<FbaPortSpec> {
    let name = value.get("name")?.as_str()?.to_string();
    Some(FbaPortSpec {
        name,
        owner: value
            .get("owner")
            .and_then(Value::as_str)
            .map(str::to_string),
        operations: string_array(value.get("operations")),
        context: value
            .get("context")
            .and_then(Value::as_str)
            .map(str::to_string),
        error: value
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_string),
        idempotency_required: value
            .get("idempotency_required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        deadline_required: value
            .get("deadline_required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        read_operations: string_array(value.get("read_operations")),
        write_operations: string_array(value.get("write_operations")),
    })
}

fn consumer_from_value(value: &Value) -> Option<FbaConsumerSpec> {
    Some(FbaConsumerSpec {
        module: value.get("module")?.as_str()?.to_string(),
        profile: value
            .get("profile")
            .and_then(Value::as_str)
            .map(str::to_string),
        fallback_profiles: string_array(value.get("fallback_profiles")),
        degraded_modes: string_array(value.get("degraded_modes")),
    })
}

fn provider_dependency_from_value(value: &Value) -> Option<FbaProviderDependency> {
    Some(FbaProviderDependency {
        module: value.get("module")?.as_str()?.to_string(),
        contract_version: value
            .get("contract_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        ports: string_array(value.get("ports")),
        profiles: string_array(value.get("profiles")),
        fallback_profiles: string_array(value.get("fallback_profiles")),
        degraded_modes: string_array(value.get("degraded_modes")),
    })
}

fn contract_case_from_value(value: &Value) -> Option<FbaContractTestCase> {
    Some(FbaContractTestCase {
        operation: value.get("operation")?.as_str()?.to_string(),
        profiles: string_array(value.get("profiles")),
        assertions: string_array(value.get("assertions")),
    })
}

fn evidence_paths(value: &Value) -> Vec<String> {
    let mut paths = BTreeSet::new();
    if let Some(evidence) = value.get("evidence").and_then(Value::as_object) {
        for value in evidence.values() {
            if let Some(path) = value.as_str()
                && (path.contains('/') || path.contains('\\'))
            {
                paths.insert(normalize_path(path));
            }
        }
    }
    if let Some(path) = value
        .pointer("/contract_tests/source")
        .and_then(Value::as_str)
        && (path.contains('/') || path.contains('\\'))
    {
        paths.insert(normalize_path(path));
    }
    paths.into_iter().collect()
}

fn extract_code_ports(content: &str) -> Vec<FbaCodePort> {
    let mut ports = Vec::new();
    let lines = content.lines().collect::<Vec<_>>();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if let Some(name) = trait_name(line) {
            let mut operations = Vec::new();
            index += 1;
            while index < lines.len() {
                let next = lines[index].trim();
                if next.starts_with("pub trait ") || next.starts_with("impl ") {
                    break;
                }
                if next.starts_with('}') {
                    break;
                }
                if let Some(operation) = method_name(next) {
                    operations.push(operation);
                }
                index += 1;
            }
            ports.push(FbaCodePort { name, operations });
            continue;
        }
        index += 1;
    }
    ports
}

fn trait_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("pub trait ")?;
    let name = rest
        .split(|ch: char| ch == ':' || ch == '<' || ch == '{' || ch.is_whitespace())
        .next()?;
    name.ends_with("Port").then(|| name.to_string())
}

fn method_name(line: &str) -> Option<String> {
    let rest = line
        .strip_prefix("async fn ")
        .or_else(|| line.strip_prefix("fn "))?;
    let name = rest
        .split(|ch: char| ch == '(' || ch == '<' || ch.is_whitespace())
        .next()?;
    (!name.is_empty()).then(|| name.to_string())
}

fn write_operations_for(port: &FbaPortSpec) -> Vec<String> {
    if !port.write_operations.is_empty() {
        return port.write_operations.clone();
    }
    port.operations
        .iter()
        .filter(|operation| {
            !port.read_operations.iter().any(|read| read == *operation)
                && !operation.starts_with("read_")
                && !operation.starts_with("get_")
                && !operation.starts_with("list_")
                && !operation.starts_with("check_")
                && !operation.starts_with("execute_")
                && !operation.starts_with("suggest")
        })
        .cloned()
        .collect()
}

fn fba_module_entity(module: &str, snapshot: &SnapshotId, path: Option<&str>) -> Entity {
    let stable_key = format!("fba_module://{module}");
    Entity {
        id: EntityId(format!(
            "ent_fba_module_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        stable_key: StableKey(stable_key),
        kind: EntityKind::Other(FBA_MODULE_ENTITY_KIND.to_string()),
        name: module.to_string(),
        title: Some(format!("RusTok FBA {module} module")),
        source: path.map(source_location),
        language: None,
        aliases: Vec::new(),
        ownership: path.map_or_else(Vec::new, ownership_for_file),
        payload: json!({"schema": "rustok.fba.module.v1", "module": module, "snapshot": snapshot.0}),
    }
}

fn fba_contract_entity(
    module: &str,
    contract_version: &str,
    snapshot: &SnapshotId,
    path: Option<&str>,
) -> Entity {
    let stable_key = format!("fba_contract://{module}/{contract_version}");
    Entity {
        id: EntityId(format!(
            "ent_fba_contract_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        stable_key: StableKey(stable_key),
        kind: EntityKind::Other(FBA_CONTRACT_ENTITY_KIND.to_string()),
        name: format!("{module}/{contract_version}"),
        title: Some(format!("RusTok FBA {module} {contract_version} contract")),
        source: path.map(source_location),
        language: None,
        aliases: Vec::new(),
        ownership: path.map_or_else(Vec::new, ownership_for_file),
        payload: json!({
            "schema": "rustok.fba.contract.v1",
            "module": module,
            "contract_version": contract_version,
            "snapshot": snapshot.0,
        }),
    }
}

fn fba_port_entity(module: &str, port: &str, snapshot: &SnapshotId, path: Option<&str>) -> Entity {
    let stable_key = format!("fba_port://{module}/{port}");
    Entity {
        id: EntityId(format!(
            "ent_fba_port_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        stable_key: StableKey(stable_key),
        kind: EntityKind::Other(FBA_PORT_ENTITY_KIND.to_string()),
        name: format!("{module}/{port}"),
        title: Some(format!("RusTok FBA {module} {port} port")),
        source: path.map(source_location),
        language: Some(LanguageCode("rust".to_string())),
        aliases: Vec::new(),
        ownership: path.map_or_else(Vec::new, ownership_for_file),
        payload: json!({"schema": "rustok.fba.port.v1", "module": module, "port": port, "snapshot": snapshot.0}),
    }
}

fn fba_operation_entity(
    module: &str,
    port: &str,
    operation: &str,
    snapshot: &SnapshotId,
    path: Option<&str>,
) -> Entity {
    let stable_key = format!("fba_operation://{module}/{port}/{operation}");
    Entity {
        id: EntityId(format!(
            "ent_fba_operation_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        stable_key: StableKey(stable_key),
        kind: EntityKind::Other(FBA_OPERATION_ENTITY_KIND.to_string()),
        name: format!("{module}/{port}/{operation}"),
        title: Some(format!("RusTok FBA {module} {port}.{operation} operation")),
        source: path.map(source_location),
        language: Some(LanguageCode("rust".to_string())),
        aliases: Vec::new(),
        ownership: path.map_or_else(Vec::new, ownership_for_file),
        payload: json!({
            "schema": "rustok.fba.operation.v1",
            "module": module,
            "port": port,
            "operation": operation,
            "snapshot": snapshot.0,
        }),
    }
}

fn fba_profile_entity(
    module: &str,
    profile: &str,
    snapshot: &SnapshotId,
    path: Option<&str>,
) -> Entity {
    let stable_key = format!("fba_profile://{module}/{profile}");
    Entity {
        id: EntityId(format!(
            "ent_fba_profile_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        stable_key: StableKey(stable_key),
        kind: EntityKind::Other(FBA_PROFILE_ENTITY_KIND.to_string()),
        name: format!("{module}/{profile}"),
        title: Some(format!("RusTok FBA {module} {profile} profile")),
        source: path.map(source_location),
        language: None,
        aliases: Vec::new(),
        ownership: path.map_or_else(Vec::new, ownership_for_file),
        payload: json!({"schema": "rustok.fba.profile.v1", "module": module, "profile": profile, "snapshot": snapshot.0}),
    }
}

fn fba_dependency_entity(
    consumer: &str,
    provider: &str,
    profile: &str,
    snapshot: &SnapshotId,
    path: Option<&str>,
) -> Entity {
    let stable_key = format!("fba_dependency://{consumer}/{provider}/{profile}");
    Entity {
        id: EntityId(format!(
            "ent_fba_dependency_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        stable_key: StableKey(stable_key),
        kind: EntityKind::Other(FBA_DEPENDENCY_ENTITY_KIND.to_string()),
        name: format!("{consumer}/{provider}/{profile}"),
        title: Some(format!(
            "RusTok FBA {consumer}->{provider} {profile} dependency"
        )),
        source: path.map(source_location),
        language: None,
        aliases: Vec::new(),
        ownership: path.map_or_else(Vec::new, ownership_for_file),
        payload: json!({
            "schema": "rustok.fba.dependency.v1",
            "consumer": consumer,
            "provider": provider,
            "profile": profile,
            "snapshot": snapshot.0,
        }),
    }
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
    ownership: Vec<Ownership>,
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

#[allow(clippy::too_many_arguments)]
fn diagnostic(
    snapshot: &SnapshotId,
    kind: &str,
    severity: Severity,
    title: impl Into<String>,
    message: impl Into<String>,
    registry: &FbaRegistryMarker,
    port: Option<&str>,
    evidence: Vec<Evidence>,
    ownership: Vec<Ownership>,
) -> Diagnostic {
    let port_key = port.unwrap_or("-");
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_rustok_fba_{:016x}",
            stable_hash(
                format!(
                    "{}:{}:{}:{}",
                    kind, registry.module, registry.path, port_key
                )
                .as_bytes()
            )
        )),
        kind: DiagnosticKind::Other(kind.to_string()),
        severity,
        status: DiagnosticStatus::Open,
        title: title.into(),
        message: message.into(),
        entities: Vec::new(),
        evidence,
        ownership,
        snapshot: snapshot.clone(),
        suggested_fix: None,
        payload: json!({
            "schema": "rustok.fba.diagnostic.v1",
            "module": registry.module,
            "role": registry.role,
            "contract_version": registry.contract_version,
            "port": port,
            "path": registry.path,
        }),
    }
}

fn dedup_entities(entities: &mut Vec<Entity>) {
    let mut seen = BTreeSet::new();
    entities.retain(|entity| seen.insert(entity.id.0.clone()));
}

fn source_location(path: &str) -> SourceLocation {
    SourceLocation {
        path: path.to_string(),
        line_start: None,
        line_end: None,
    }
}

fn serialized_relation_kind(kind: &RelationKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "other".to_string())
}

fn merged_code_markers_by_module(
    code_markers: &[(&Fact, FbaPortCodeMarker)],
) -> HashMap<String, FbaPortCodeMarker> {
    let mut by_module = HashMap::<String, FbaPortCodeMarker>::new();
    for (_, marker) in code_markers {
        by_module
            .entry(marker.module.clone())
            .and_modify(|existing| merge_code_marker(existing, marker))
            .or_insert_with(|| marker.clone());
    }
    by_module
}

fn merge_code_marker(existing: &mut FbaPortCodeMarker, marker: &FbaPortCodeMarker) {
    existing.has_port_context |= marker.has_port_context;
    existing.has_port_error |= marker.has_port_error;
    existing.has_read_policy |= marker.has_read_policy;
    existing.has_write_policy |= marker.has_write_policy;
    existing.first_marker_line = existing.first_marker_line.or(marker.first_marker_line);
    for port in &marker.ports {
        match existing
            .ports
            .iter_mut()
            .find(|existing_port| existing_port.name == port.name)
        {
            Some(existing_port) => {
                for operation in &port.operations {
                    if !existing_port
                        .operations
                        .iter()
                        .any(|existing| existing == operation)
                    {
                        existing_port.operations.push(operation.clone());
                    }
                }
            }
            None => existing.ports.push(port.clone()),
        }
    }
}

fn registry_from_fact(fact: &Fact) -> Option<FbaRegistryMarker> {
    serde_json::from_value(fact.value.clone()).ok()
}

fn port_code_from_fact(fact: &Fact) -> Option<FbaPortCodeMarker> {
    serde_json::from_value(fact.value.clone()).ok()
}

fn is_registry_fact(fact: &Fact) -> bool {
    matches!(&fact.kind, FactKind::Other(kind) if kind == FBA_REGISTRY_FACT_KIND)
}

fn is_port_code_fact(fact: &Fact) -> bool {
    matches!(&fact.kind, FactKind::Other(kind) if kind == FBA_PORT_CODE_FACT_KIND)
}

fn is_fba_registry_path(path: &str) -> bool {
    path.starts_with("crates/rustok-")
        && path.contains("/contracts/")
        && path.ends_with("-fba-registry.json")
}

fn is_rustok_ports_path(path: &str) -> bool {
    path.starts_with("crates/rustok-")
        && (path.ends_with("/src/ports.rs")
            || (path.contains("/src/ports/") && path.ends_with(".rs")))
}

fn module_from_crate_path(path: &str) -> Option<String> {
    let parts = path.split('/').collect::<Vec<_>>();
    let crates_index = parts.iter().position(|part| *part == "crates")?;
    let crate_name = *parts.get(crates_index + 1)?;
    crate_name
        .strip_prefix("rustok-")
        .map(|module| module.to_string())
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn provider_registry_path(module: &str) -> String {
    format!("crates/rustok-{module}/contracts/{module}-fba-registry.json")
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn first_marker_line(content: &str) -> Option<u32> {
    content.lines().enumerate().find_map(|(index, line)| {
        contains_any(
            line,
            &["pub trait ", "PortContext", "PortError", "PortCallPolicy"],
        )
        .then_some(index as u32 + 1)
    })
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

fn collect_string_array(value: Option<&Value>, out: &mut BTreeSet<String>) {
    for item in string_array(value) {
        out.insert(item);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use athanor_core::{AffectedSubset, CheckInput, ExtractInput, LinkInput};
    use athanor_domain::{EvidenceStatus, RepoId, SnapshotId};

    use super::*;

    fn source(path: &str, content: &str) -> SourceFile {
        SourceFile {
            path: path.to_string(),
            language_hint: Some("rust".to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(content.to_string()),
        }
    }

    #[tokio::test]
    async fn discovers_provider_registry_ports_and_operations() {
        let source = SourceFile {
            path: "crates/rustok-inventory/contracts/inventory-fba-registry.json".to_string(),
            language_hint: Some("json".to_string()),
            content_hash: None,
            content: Some(
                r#"{
                    "schema_version": 1,
                    "module": "inventory",
                    "role": "provider",
                    "status": "in_progress",
                    "contract_version": "inventory.reservation.v1",
                    "ports": [{
                        "name": "InventoryReservationPort",
                        "operations": ["check_availability", "reserve_inventory"],
                        "context": "rustok_api::ports::PortContext",
                        "error": "rustok_api::ports::PortError",
                        "idempotency_required": true,
                        "deadline_required": true
                    }],
                    "contract_tests": {
                        "status": "planned_cases_locked",
                        "cases": [{"operation": "reserve_inventory", "assertions": ["write_idempotency_required"]}]
                    },
                    "evidence": {"verifier": "scripts/verify/verify-ecommerce-fba-registries.mjs"}
                }"#.to_string(),
            ),
        };

        let output = RustokFbaExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                snapshot: SnapshotId("snap".to_string()),
                source,
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "fba_port://inventory/InventoryReservationPort"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0
                == "fba_operation://inventory/InventoryReservationPort/reserve_inventory"
        }));
        assert_eq!(output.facts.len(), 1);
    }

    #[tokio::test]
    async fn detects_port_trait_operations_from_code() {
        let source = SourceFile {
            path: "crates/rustok-cart/src/ports.rs".to_string(),
            language_hint: Some("rust".to_string()),
            content_hash: None,
            content: Some(
                r#"
                use rustok_api::{PortCallPolicy, PortContext, PortError};
                pub trait CartSnapshotReadPort: Send + Sync {
                    async fn read_cart_checkout_snapshot(&self, context: PortContext) -> Result<(), PortError>;
                }
                impl CartSnapshotReadPort for CartService {
                    async fn read_cart_checkout_snapshot(&self, context: PortContext) -> Result<(), PortError> {
                        context.require_policy(PortCallPolicy::read())?;
                        Ok(())
                    }
                }
                "#
                .to_string(),
            ),
        };

        let output = RustokFbaExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                snapshot: SnapshotId("snap".to_string()),
                source,
            })
            .await
            .unwrap();
        let marker: FbaPortCodeMarker =
            serde_json::from_value(output.facts[0].value.clone()).unwrap();

        assert_eq!(marker.module, "cart");
        assert_eq!(marker.ports[0].name, "CartSnapshotReadPort");
        assert_eq!(
            marker.ports[0].operations,
            vec!["read_cart_checkout_snapshot"]
        );
        assert!(marker.has_port_context);
        assert!(marker.has_read_policy);
    }

    #[test]
    fn supports_flat_mod_and_nested_port_paths() {
        let extractor = RustokFbaExtractor;

        assert!(extractor.supports(&source(
            "crates/rustok-cart/src/ports.rs",
            "pub trait CartSnapshotReadPort {}",
        )));
        assert!(extractor.supports(&source(
            "crates/rustok-cart/src/ports/mod.rs",
            "pub trait CartSnapshotReadPort {}",
        )));
        assert!(extractor.supports(&source(
            "crates\\rustok-cart\\src\\ports\\cart\\read.rs",
            "pub trait CartSnapshotReadPort {}",
        )));
    }

    #[tokio::test]
    async fn detects_port_trait_operations_from_mod_rs() {
        let output = RustokFbaExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                snapshot: SnapshotId("snap".to_string()),
                source: source(
                    "crates/rustok-cart/src/ports/mod.rs",
                    r#"
                    use rustok_api::{PortContext, PortError};
                    pub trait CartSnapshotReadPort: Send + Sync {
                        async fn read_cart_checkout_snapshot(&self, context: PortContext) -> Result<(), PortError>;
                    }
                    "#,
                ),
            })
            .await
            .unwrap();
        let marker: FbaPortCodeMarker =
            serde_json::from_value(output.facts[0].value.clone()).unwrap();

        assert_eq!(marker.path, "crates/rustok-cart/src/ports/mod.rs");
        assert_eq!(marker.module, "cart");
        assert_eq!(marker.ports[0].name, "CartSnapshotReadPort");
        assert_eq!(
            marker.ports[0].operations,
            vec!["read_cart_checkout_snapshot"]
        );
    }

    #[tokio::test]
    async fn checker_merges_split_port_code_files_per_module() {
        let registry = registry_fact(
            "inventory",
            "crates/rustok-inventory/contracts/inventory-fba-registry.json",
            vec![FbaPortSpec {
                name: "InventoryReservationPort".to_string(),
                owner: None,
                operations: vec![
                    "check_availability".to_string(),
                    "reserve_inventory".to_string(),
                ],
                context: Some("rustok_api::ports::PortContext".to_string()),
                error: Some("rustok_api::ports::PortError".to_string()),
                idempotency_required: false,
                deadline_required: true,
                read_operations: vec!["check_availability".to_string()],
                write_operations: vec!["reserve_inventory".to_string()],
            }],
        );
        let trait_file = code_fact_with_path(
            "inventory",
            "crates/rustok-inventory/src/ports/mod.rs",
            vec![FbaCodePort {
                name: "InventoryReservationPort".to_string(),
                operations: vec!["check_availability".to_string()],
            }],
            true,
            true,
            false,
            false,
        );
        let impl_file = code_fact_with_path(
            "inventory",
            "crates/rustok-inventory/src/ports/reservation.rs",
            vec![FbaCodePort {
                name: "InventoryReservationPort".to_string(),
                operations: vec!["reserve_inventory".to_string()],
            }],
            false,
            false,
            false,
            true,
        );

        let diagnostics = RustokFbaChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap".to_string()),
                entities: Arc::new(Vec::new()),
                facts: Arc::new(vec![registry, trait_file, impl_file]),
                relations: Arc::new(Vec::new()),
                affected: AffectedSubset::default(),
            })
            .await
            .unwrap();

        assert!(!diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.kind,
            DiagnosticKind::Other(ref kind)
                if kind == "rustok_fba_port_trait_missing"
                    || kind == "rustok_fba_port_operation_missing"
                    || kind == "rustok_fba_context_missing"
                    || kind == "rustok_fba_error_missing"
                    || kind == "rustok_fba_policy_missing"
        )));
    }

    #[tokio::test]
    async fn checker_reports_missing_port_operation() {
        let registry = registry_fact(
            "inventory",
            "crates/rustok-inventory/contracts/inventory-fba-registry.json",
            vec![FbaPortSpec {
                name: "InventoryReservationPort".to_string(),
                owner: None,
                operations: vec!["reserve_inventory".to_string()],
                context: Some("rustok_api::ports::PortContext".to_string()),
                error: Some("rustok_api::ports::PortError".to_string()),
                idempotency_required: false,
                deadline_required: false,
                read_operations: Vec::new(),
                write_operations: Vec::new(),
            }],
        );
        let code = code_fact(
            "inventory",
            vec![FbaCodePort {
                name: "InventoryReservationPort".to_string(),
                operations: vec!["check_availability".to_string()],
            }],
        );

        let diagnostics = RustokFbaChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap".to_string()),
                entities: Arc::new(Vec::new()),
                facts: Arc::new(vec![registry, code]),
                relations: Arc::new(Vec::new()),
                affected: AffectedSubset::default(),
            })
            .await
            .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.kind
                == DiagnosticKind::Other("rustok_fba_port_operation_missing".to_string())
        }));
    }

    #[tokio::test]
    async fn linker_connects_contract_port_operation_and_file() {
        let source = SourceFile {
            path: "crates/rustok-cart/contracts/cart-fba-registry.json".to_string(),
            language_hint: Some("json".to_string()),
            content_hash: None,
            content: Some(
                r#"{
                    "module": "cart",
                    "role": "provider",
                    "contract_version": "cart.checkout_snapshot.v1",
                    "ports": [{"name": "CartSnapshotReadPort", "operations": ["read_cart_checkout_snapshot"]}],
                    "contract_tests": {"status": "planned_cases_locked", "cases": [{"operation": "read_cart_checkout_snapshot"}]}
                }"#.to_string(),
            ),
        };
        let extracted = RustokFbaExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                snapshot: SnapshotId("snap".to_string()),
                source,
            })
            .await
            .unwrap();

        let relations = RustokFbaLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap".to_string()),
                entities: Arc::new(extracted.entities),
                facts: Arc::new(extracted.facts),
                affected: AffectedSubset::default(),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .any(|relation| matches!(relation.kind, RelationKind::Contains))
        );
        assert!(
            relations
                .iter()
                .all(|relation| !relation.evidence.is_empty() && !relation.ownership.is_empty())
        );
    }

    fn registry_fact(module: &str, path: &str, ports: Vec<FbaPortSpec>) -> Fact {
        let marker = FbaRegistryMarker {
            schema: "rustok.fba.registry.v1".to_string(),
            path: path.to_string(),
            module: module.to_string(),
            role: "provider".to_string(),
            status: Some("in_progress".to_string()),
            contract_version: Some(format!("{module}.v1")),
            ports,
            consumers: Vec::new(),
            providers: Vec::new(),
            profiles: Vec::new(),
            evidence_paths: vec!["docs/modules/registry.md".to_string()],
            in_process_impl_source: None,
            contract_tests_status: Some("planned_cases_locked".to_string()),
            contract_test_cases: Vec::new(),
        };
        Fact {
            id: FactId(format!("fact_{module}")),
            kind: FactKind::Other(FBA_REGISTRY_FACT_KIND.to_string()),
            subject: EntityId(format!("ent_file_{module}")),
            object: None,
            value: serde_json::to_value(marker).unwrap(),
            evidence: vec![Evidence {
                source_file: Some(path.to_string()),
                line_start: Some(1),
                line_end: Some(1),
                extractor: Some("test".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: EvidenceStatus::Verified,
            }],
            ownership: ownership_for_file(path),
            snapshot: SnapshotId("snap".to_string()),
            extractor: "test".to_string(),
            confidence: 1.0,
        }
    }

    fn code_fact(module: &str, ports: Vec<FbaCodePort>) -> Fact {
        let path = format!("crates/rustok-{module}/src/ports.rs");
        code_fact_with_path(module, &path, ports, true, true, true, true)
    }

    fn code_fact_with_path(
        module: &str,
        path: &str,
        ports: Vec<FbaCodePort>,
        has_port_context: bool,
        has_port_error: bool,
        has_read_policy: bool,
        has_write_policy: bool,
    ) -> Fact {
        let marker = FbaPortCodeMarker {
            schema: "rustok.fba.port_code.v1".to_string(),
            path: path.to_string(),
            module: module.to_string(),
            ports,
            has_port_context,
            has_port_error,
            has_read_policy,
            has_write_policy,
            first_marker_line: Some(1),
        };
        Fact {
            id: FactId(format!("fact_code_{module}")),
            kind: FactKind::Other(FBA_PORT_CODE_FACT_KIND.to_string()),
            subject: EntityId(format!("ent_file_code_{module}")),
            object: None,
            value: serde_json::to_value(marker).unwrap(),
            evidence: vec![Evidence {
                source_file: Some(path.to_string()),
                line_start: Some(1),
                line_end: Some(1),
                extractor: Some("test".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: EvidenceStatus::Verified,
            }],
            ownership: ownership_for_file(path),
            snapshot: SnapshotId("snap".to_string()),
            extractor: "test".to_string(),
            confidence: 1.0,
        }
    }
}
