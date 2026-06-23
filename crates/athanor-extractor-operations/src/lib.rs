use std::collections::BTreeMap;

use async_trait::async_trait;
use athanor_core::{CoreResult, ExtractInput, ExtractOutput, Extractor, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct OperationsExtractor;

#[async_trait]
impl Extractor for OperationsExtractor {
    fn name(&self) -> &str {
        "operations"
    }

    fn supports(&self, source: &SourceFile) -> bool {
        is_dotenv_path(&source.path)
            || is_cargo_manifest_path(&source.path)
            || is_makefile_path(&source.path)
            || is_dockerfile_path(&source.path)
            || is_shell_script_path(&source.path)
            || is_docker_compose_path(&source.path)
            || is_kubernetes_manifest_path(&source.path)
            || is_database_migration_path(&source.path)
            || is_github_actions_workflow_path(&source.path)
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };
        let file_id = file_entity(&input.source, &input.snapshot.0).id;
        let mut entities = Vec::new();
        let mut facts = Vec::new();

        if is_dotenv_path(&input.source.path) {
            extract_env_declarations(
                self.name(),
                &input,
                &file_id,
                "dotenv",
                parse_dotenv_declarations(content),
                &mut entities,
                &mut facts,
            );
        }

        if is_cargo_manifest_path(&input.source.path) {
            extract_cargo_manifest(
                self.name(),
                &input,
                &file_id,
                content,
                &mut entities,
                &mut facts,
            );
        }

        if is_makefile_path(&input.source.path) {
            extract_makefile_targets(
                self.name(),
                &input,
                &file_id,
                content,
                &mut entities,
                &mut facts,
            );
        }

        if is_dockerfile_path(&input.source.path) {
            extract_dockerfile(
                self.name(),
                &input,
                &file_id,
                content,
                &mut entities,
                &mut facts,
            );
        }

        if is_shell_script_path(&input.source.path) {
            extract_env_declarations(
                self.name(),
                &input,
                &file_id,
                "shell",
                parse_shell_env_declarations(content),
                &mut entities,
                &mut facts,
            );
            extract_shell_functions(
                self.name(),
                &input,
                &file_id,
                content,
                &mut entities,
                &mut facts,
            );
        }

        if is_docker_compose_path(&input.source.path) {
            extract_docker_compose(
                self.name(),
                &input,
                &file_id,
                content,
                &mut entities,
                &mut facts,
            );
        }

        if is_kubernetes_manifest_path(&input.source.path) {
            extract_kubernetes_manifest(
                self.name(),
                &input,
                &file_id,
                content,
                &mut entities,
                &mut facts,
            );
        }

        if is_database_migration_path(&input.source.path) {
            extract_database_migration(
                self.name(),
                &input,
                &file_id,
                content,
                &mut entities,
                &mut facts,
            );
        }

        if is_github_actions_workflow_path(&input.source.path) {
            extract_github_actions_workflow(
                self.name(),
                &input,
                &file_id,
                content,
                &mut entities,
                &mut facts,
            );
        }

        Ok(ExtractOutput { entities, facts })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EnvDeclaration {
    line: u32,
    has_value: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MakeTarget {
    name: String,
    line: u32,
    prerequisites: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoManifest {
    package: Option<CargoPackage>,
    workspace: Option<CargoWorkspace>,
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoPackage {
    name: String,
    version: Option<String>,
    edition: Option<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoWorkspace {
    members: Vec<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoDependency {
    name: String,
    section: String,
    version: Option<String>,
    path: Option<String>,
    git: Option<String>,
    registry: Option<String>,
    package: Option<String>,
    optional: Option<bool>,
    features: Vec<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DockerInstruction {
    keyword: String,
    value: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComposeService {
    name: String,
    image: Option<String>,
    build: Option<String>,
    command: Option<String>,
    entrypoint: Option<String>,
    line: u32,
    environment: BTreeMap<String, EnvDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KubernetesResource {
    kind: String,
    name: String,
    namespace: Option<String>,
    line: u32,
    images: Vec<String>,
    containers: Vec<String>,
    commands: Vec<KubernetesContainerCommand>,
    environment: BTreeMap<String, EnvDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KubernetesContainerCommand {
    container: String,
    instruction: String,
    value: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DatabaseMigration {
    name: String,
    line: u32,
    created_tables: Vec<DatabaseTableDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DatabaseTableDeclaration {
    name: String,
    schema: Option<String>,
    line: u32,
    if_not_exists: bool,
}

impl DatabaseTableDeclaration {
    fn qualified_name(&self) -> String {
        self.schema.as_ref().map_or_else(
            || self.name.clone(),
            |schema| format!("{schema}.{}", self.name),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubActionsWorkflow {
    name: Option<String>,
    line: u32,
    environment: BTreeMap<String, EnvDeclaration>,
    jobs: Vec<GithubActionsJob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubActionsJob {
    id: String,
    name: Option<String>,
    runs_on: Vec<String>,
    line: u32,
    environment: BTreeMap<String, EnvDeclaration>,
    steps: Vec<GithubActionsStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubActionsStep {
    index: usize,
    name: Option<String>,
    kind: GithubActionsStepKind,
    line: u32,
    environment: BTreeMap<String, EnvDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GithubActionsStepKind {
    Run(String),
    Uses(String),
}

fn extract_env_declarations(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    source_kind: &str,
    declarations: BTreeMap<String, EnvDeclaration>,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    for (name, declaration) in declarations {
        let stable_key = StableKey(format!("env://{name}"));
        let entity_id = env_var_entity_id(&stable_key);
        let ownership = ownership_for_file(&input.source.path);

        entities.push(Entity {
            id: entity_id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::EnvVar,
            name: name.clone(),
            title: None,
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(declaration.line),
                line_end: Some(declaration.line),
            }),
            language: Some(LanguageCode(source_kind.to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "name": name,
                "source_kind": source_kind,
                "has_default": declaration.has_value,
                "value_redacted": declaration.has_value,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_env_var_declared_{:016x}",
                stable_hash(
                    format!("{}\0{}\0{}", stable_key.0, input.source.path, source_kind).as_bytes()
                )
            )),
            kind: FactKind::EnvVarUsed,
            subject: entity_id,
            object: Some(file_id.clone()),
            value: json!({
                "name": name,
                "mechanism": source_kind,
                "source_kind": source_kind,
                "has_default": declaration.has_value,
                "value_redacted": declaration.has_value,
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(declaration.line),
                Some(declaration.line),
            )],
            ownership,
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });
    }
}

fn extract_makefile_targets(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    for target in parse_makefile_targets(content) {
        let stable_key = StableKey(format!(
            "script-command://{}#target:{}",
            input.source.path, target.name
        ));
        let entity_id = script_command_entity_id(&stable_key);
        let ownership = ownership_for_file(&input.source.path);

        entities.push(Entity {
            id: entity_id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::ScriptCommand,
            name: target.name.clone(),
            title: Some(format!("make {}", target.name)),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(target.line),
                line_end: Some(target.line),
            }),
            language: Some(LanguageCode("makefile".to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "command_kind": "make_target",
                "target": target.name,
                "prerequisites": target.prerequisites,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_script_command_defined_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            )),
            kind: FactKind::SymbolDefined,
            subject: entity_id,
            object: Some(file_id.clone()),
            value: json!({
                "stable_key": stable_key.0,
                "path": input.source.path,
                "source_kind": "makefile",
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(target.line),
                Some(target.line),
            )],
            ownership,
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });
    }
}

fn extract_cargo_manifest(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let Some(manifest) = parse_cargo_manifest(content) else {
        return;
    };

    if let Some(package) = manifest.package {
        let stable_key = StableKey(format!(
            "package://{}#{}",
            input.source.path,
            sanitize_key_fragment(&package.name)
        ));
        push_package_entity_and_fact(
            extractor,
            input,
            file_id,
            stable_key,
            package.name.clone(),
            Some(format!("Cargo package {}", package.name)),
            package.line,
            json!({
                "package_kind": "cargo_package",
                "name": package.name,
                "version": package.version,
                "edition": package.edition,
            }),
            json!({
                "path": input.source.path,
                "source_kind": "cargo_manifest",
                "package_kind": "cargo_package",
            }),
            entities,
            facts,
        );
    }

    if let Some(workspace) = manifest.workspace {
        let stable_key = StableKey(format!("package://{}#workspace", input.source.path));
        push_package_entity_and_fact(
            extractor,
            input,
            file_id,
            stable_key,
            "workspace".to_string(),
            Some("Cargo workspace".to_string()),
            workspace.line,
            json!({
                "package_kind": "cargo_workspace",
                "members": workspace.members,
            }),
            json!({
                "path": input.source.path,
                "source_kind": "cargo_manifest",
                "package_kind": "cargo_workspace",
            }),
            entities,
            facts,
        );
    }

    for dependency in manifest.dependencies {
        let stable_key = StableKey(format!(
            "dependency://{}#{}:{}",
            input.source.path,
            sanitize_key_fragment(&dependency.section),
            sanitize_key_fragment(&dependency.name)
        ));
        let entity_id = dependency_entity_id(&stable_key);
        let ownership = ownership_for_file(&input.source.path);
        entities.push(Entity {
            id: entity_id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::Dependency,
            name: dependency.name.clone(),
            title: Some(format!("Cargo dependency {}", dependency.name)),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(dependency.line),
                line_end: Some(dependency.line),
            }),
            language: Some(LanguageCode("cargo".to_string())),
            aliases: dependency.package.iter().cloned().collect(),
            ownership: ownership.clone(),
            payload: json!({
                "dependency_kind": "cargo_dependency",
                "name": dependency.name,
                "section": dependency.section,
                "version": dependency.version,
                "path": dependency.path,
                "git": dependency.git,
                "registry": dependency.registry,
                "package": dependency.package,
                "optional": dependency.optional,
                "features": dependency.features,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_dependency_defined_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            )),
            kind: FactKind::SymbolDefined,
            subject: entity_id,
            object: Some(file_id.clone()),
            value: json!({
                "stable_key": stable_key.0,
                "path": input.source.path,
                "source_kind": "cargo_manifest",
                "section": dependency.section,
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(dependency.line),
                Some(dependency.line),
            )],
            ownership,
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });
    }
}

fn extract_dockerfile(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let instructions = parse_dockerfile_instructions(content);
    for (index, instruction) in instructions.iter().enumerate() {
        match instruction.keyword.as_str() {
            "FROM" => {
                let stage = docker_stage_name(&instruction.value)
                    .unwrap_or_else(|| format!("stage-{}", index + 1));
                let stable_key = StableKey(format!("docker://{}#{}", input.source.path, stage));
                let entity_id = EntityId(format!(
                    "ent_docker_service_{:016x}",
                    stable_hash(stable_key.0.as_bytes())
                ));
                let ownership = ownership_for_file(&input.source.path);

                entities.push(Entity {
                    id: entity_id.clone(),
                    stable_key: stable_key.clone(),
                    kind: EntityKind::DockerService,
                    name: stage.clone(),
                    title: Some(format!("Docker stage {stage}")),
                    source: Some(SourceLocation {
                        path: input.source.path.clone(),
                        line_start: Some(instruction.line),
                        line_end: Some(instruction.line),
                    }),
                    language: Some(LanguageCode("dockerfile".to_string())),
                    aliases: Vec::new(),
                    ownership: ownership.clone(),
                    payload: json!({
                        "service_kind": "dockerfile_stage",
                        "image": docker_base_image(&instruction.value),
                        "stage": stage,
                    }),
                });

                facts.push(Fact {
                    id: FactId(format!(
                        "fact_docker_stage_defined_{:016x}",
                        stable_hash(stable_key.0.as_bytes())
                    )),
                    kind: FactKind::SymbolDefined,
                    subject: entity_id,
                    object: Some(file_id.clone()),
                    value: json!({
                        "stable_key": stable_key.0,
                        "path": input.source.path,
                        "source_kind": "dockerfile",
                    }),
                    evidence: vec![evidence_for_file(
                        &input.source.path,
                        extractor,
                        Some(instruction.line),
                        Some(instruction.line),
                    )],
                    ownership,
                    snapshot: input.snapshot.clone(),
                    extractor: extractor.to_string(),
                    confidence: 1.0,
                });
            }
            "RUN" | "CMD" | "ENTRYPOINT" => {
                let stable_key = StableKey(format!(
                    "script-command://{}#dockerfile:{}:{}",
                    input.source.path,
                    instruction.keyword.to_ascii_lowercase(),
                    instruction.line
                ));
                let entity_id = script_command_entity_id(&stable_key);
                let ownership = ownership_for_file(&input.source.path);

                entities.push(Entity {
                    id: entity_id.clone(),
                    stable_key: stable_key.clone(),
                    kind: EntityKind::ScriptCommand,
                    name: format!("{} {}", instruction.keyword, instruction.line),
                    title: Some(instruction.keyword.clone()),
                    source: Some(SourceLocation {
                        path: input.source.path.clone(),
                        line_start: Some(instruction.line),
                        line_end: Some(instruction.line),
                    }),
                    language: Some(LanguageCode("dockerfile".to_string())),
                    aliases: Vec::new(),
                    ownership: ownership.clone(),
                    payload: json!({
                        "command_kind": "dockerfile_instruction",
                        "instruction": instruction.keyword,
                        "command": instruction.value,
                    }),
                });

                facts.push(Fact {
                    id: FactId(format!(
                        "fact_script_command_defined_{:016x}",
                        stable_hash(stable_key.0.as_bytes())
                    )),
                    kind: FactKind::SymbolDefined,
                    subject: entity_id,
                    object: Some(file_id.clone()),
                    value: json!({
                        "stable_key": stable_key.0,
                        "path": input.source.path,
                        "source_kind": "dockerfile",
                        "instruction": instruction.keyword,
                    }),
                    evidence: vec![evidence_for_file(
                        &input.source.path,
                        extractor,
                        Some(instruction.line),
                        Some(instruction.line),
                    )],
                    ownership,
                    snapshot: input.snapshot.clone(),
                    extractor: extractor.to_string(),
                    confidence: 1.0,
                });
            }
            "ENV" => extract_env_declarations(
                extractor,
                input,
                file_id,
                "dockerfile",
                parse_dockerfile_env_declarations(instruction),
                entities,
                facts,
            ),
            _ => {}
        }
    }
}

fn extract_shell_functions(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    for function in parse_shell_functions(content) {
        let stable_key = StableKey(format!(
            "script-command://{}#shell-function:{}",
            input.source.path, function.name
        ));
        let entity_id = script_command_entity_id(&stable_key);
        let ownership = ownership_for_file(&input.source.path);

        entities.push(Entity {
            id: entity_id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::ScriptCommand,
            name: function.name.clone(),
            title: Some(format!("shell function {}", function.name)),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(function.line),
                line_end: Some(function.line),
            }),
            language: Some(LanguageCode("shell".to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "command_kind": "shell_function",
                "function": function.name,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_script_command_defined_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            )),
            kind: FactKind::SymbolDefined,
            subject: entity_id,
            object: Some(file_id.clone()),
            value: json!({
                "stable_key": stable_key.0,
                "path": input.source.path,
                "source_kind": "shell",
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(function.line),
                Some(function.line),
            )],
            ownership,
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });
    }
}

fn extract_docker_compose(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let mut environment = BTreeMap::new();
    for service in parse_docker_compose_services(content) {
        for (name, declaration) in &service.environment {
            environment
                .entry(name.clone())
                .or_insert_with(|| declaration.clone());
        }

        let stable_key = StableKey(format!(
            "docker://{}#compose-service:{}",
            input.source.path,
            sanitize_key_fragment(&service.name)
        ));
        let entity_id = EntityId(format!(
            "ent_docker_service_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        ));
        let ownership = ownership_for_file(&input.source.path);

        entities.push(Entity {
            id: entity_id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::DockerService,
            name: service.name.clone(),
            title: Some(format!("Compose service {}", service.name)),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(service.line),
                line_end: Some(service.line),
            }),
            language: Some(LanguageCode("docker-compose".to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "service_kind": "docker_compose_service",
                "service": &service.name,
                "image": &service.image,
                "build": &service.build,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_docker_compose_service_defined_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            )),
            kind: FactKind::SymbolDefined,
            subject: entity_id.clone(),
            object: Some(file_id.clone()),
            value: json!({
                "stable_key": stable_key.0,
                "path": input.source.path,
                "source_kind": "docker_compose",
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(service.line),
                Some(service.line),
            )],
            ownership: ownership.clone(),
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });

        for (instruction, command) in [
            ("command", service.command.as_deref()),
            ("entrypoint", service.entrypoint.as_deref()),
        ]
        .into_iter()
        .filter_map(|(instruction, command)| command.map(|command| (instruction, command)))
        {
            let stable_key = StableKey(format!(
                "script-command://{}#compose:{}:{}",
                input.source.path,
                sanitize_key_fragment(&service.name),
                instruction
            ));
            let command_entity_id = script_command_entity_id(&stable_key);
            entities.push(Entity {
                id: command_entity_id.clone(),
                stable_key: stable_key.clone(),
                kind: EntityKind::ScriptCommand,
                name: format!("{} {}", service.name, instruction),
                title: Some(format!("compose {instruction}")),
                source: Some(SourceLocation {
                    path: input.source.path.clone(),
                    line_start: Some(service.line),
                    line_end: Some(service.line),
                }),
                language: Some(LanguageCode("docker-compose".to_string())),
                aliases: Vec::new(),
                ownership: ownership.clone(),
                payload: json!({
                    "command_kind": "docker_compose_service_instruction",
                    "service": &service.name,
                    "instruction": instruction,
                    "command": command,
                }),
            });

            facts.push(Fact {
                id: FactId(format!(
                    "fact_script_command_defined_{:016x}",
                    stable_hash(stable_key.0.as_bytes())
                )),
                kind: FactKind::SymbolDefined,
                subject: command_entity_id,
                object: Some(file_id.clone()),
                value: json!({
                    "stable_key": stable_key.0,
                    "path": input.source.path,
                    "source_kind": "docker_compose",
                    "service": &service.name,
                    "instruction": instruction,
                }),
                evidence: vec![evidence_for_file(
                    &input.source.path,
                    extractor,
                    Some(service.line),
                    Some(service.line),
                )],
                ownership: ownership.clone(),
                snapshot: input.snapshot.clone(),
                extractor: extractor.to_string(),
                confidence: 1.0,
            });
        }
    }

    extract_env_declarations(
        extractor,
        input,
        file_id,
        "docker_compose",
        environment,
        entities,
        facts,
    );
}

fn extract_kubernetes_manifest(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let mut environment = BTreeMap::new();
    for resource in parse_kubernetes_resources(content) {
        for (name, declaration) in &resource.environment {
            environment
                .entry(name.clone())
                .or_insert_with(|| declaration.clone());
        }

        let identity = resource.namespace.as_ref().map_or_else(
            || resource.name.clone(),
            |namespace| format!("{namespace}:{}", resource.name),
        );
        let stable_key = StableKey(format!(
            "kubernetes://{}#{}:{}",
            input.source.path,
            sanitize_key_fragment(&resource.kind),
            sanitize_key_fragment(&identity)
        ));
        let entity_id = EntityId(format!(
            "ent_docker_service_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        ));
        let ownership = ownership_for_file(&input.source.path);

        entities.push(Entity {
            id: entity_id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::DockerService,
            name: resource.name.clone(),
            title: Some(format!("Kubernetes {} {}", resource.kind, resource.name)),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(resource.line),
                line_end: Some(resource.line),
            }),
            language: Some(LanguageCode("kubernetes".to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "service_kind": "kubernetes_resource",
                "kind": &resource.kind,
                "name": &resource.name,
                "namespace": &resource.namespace,
                "images": &resource.images,
                "containers": &resource.containers,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_kubernetes_resource_defined_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            )),
            kind: FactKind::SymbolDefined,
            subject: entity_id,
            object: Some(file_id.clone()),
            value: json!({
                "stable_key": stable_key.0,
                "path": input.source.path,
                "source_kind": "kubernetes",
                "kind": &resource.kind,
                "name": &resource.name,
                "namespace": &resource.namespace,
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(resource.line),
                Some(resource.line),
            )],
            ownership: ownership.clone(),
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });

        for command in resource.commands {
            let command_key = StableKey(format!(
                "script-command://{}#kubernetes:{}:{}:container:{}:{}",
                input.source.path,
                sanitize_key_fragment(&resource.kind),
                sanitize_key_fragment(&identity),
                sanitize_key_fragment(&command.container),
                command.instruction
            ));
            push_script_command_entity_and_fact(
                extractor,
                input,
                file_id,
                command_key,
                format!("{} {}", command.container, command.instruction),
                Some(format!(
                    "Kubernetes container {} {}",
                    command.container, command.instruction
                )),
                command.line,
                "kubernetes",
                json!({
                    "command_kind": "kubernetes_container_instruction",
                    "resource_kind": &resource.kind,
                    "resource": &resource.name,
                    "namespace": &resource.namespace,
                    "container": command.container,
                    "instruction": command.instruction,
                    "value": command.value,
                }),
                json!({
                    "path": input.source.path,
                    "source_kind": "kubernetes",
                    "resource_kind": &resource.kind,
                    "resource": &resource.name,
                    "namespace": &resource.namespace,
                }),
                entities,
                facts,
            );
        }
    }

    extract_env_declarations(
        extractor,
        input,
        file_id,
        "kubernetes",
        environment,
        entities,
        facts,
    );
}

fn extract_database_migration(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let migration = parse_database_migration(&input.source.path, content);
    let migration_key = StableKey(format!("db-migration://{}", input.source.path));
    let migration_id = EntityId(format!(
        "ent_db_migration_{:016x}",
        stable_hash(migration_key.0.as_bytes())
    ));
    let ownership = ownership_for_file(&input.source.path);

    entities.push(Entity {
        id: migration_id.clone(),
        stable_key: migration_key.clone(),
        kind: EntityKind::DbMigration,
        name: migration.name.clone(),
        title: Some(format!("Database migration {}", migration.name)),
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(migration.line),
            line_end: Some(migration.line),
        }),
        language: Some(LanguageCode("sql".to_string())),
        aliases: Vec::new(),
        ownership: ownership.clone(),
        payload: json!({
            "migration_kind": "sql_migration",
            "path": input.source.path,
            "created_tables": migration
                .created_tables
                .iter()
                .map(|table| table.qualified_name())
                .collect::<Vec<_>>(),
        }),
    });

    facts.push(Fact {
        id: FactId(format!(
            "fact_db_migration_defined_{:016x}",
            stable_hash(migration_key.0.as_bytes())
        )),
        kind: FactKind::SymbolDefined,
        subject: migration_id.clone(),
        object: Some(file_id.clone()),
        value: json!({
            "stable_key": migration_key.0,
            "path": input.source.path,
            "source_kind": "database_migration",
        }),
        evidence: vec![evidence_for_file(
            &input.source.path,
            extractor,
            Some(migration.line),
            Some(migration.line),
        )],
        ownership: ownership.clone(),
        snapshot: input.snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    });

    for table in migration.created_tables {
        let table_key = StableKey(format!(
            "db-table://{}#{}",
            input.source.path,
            sanitize_key_fragment(&table.qualified_name())
        ));
        let table_id = EntityId(format!(
            "ent_db_table_{:016x}",
            stable_hash(table_key.0.as_bytes())
        ));

        entities.push(Entity {
            id: table_id.clone(),
            stable_key: table_key.clone(),
            kind: EntityKind::DbTable,
            name: table.qualified_name(),
            title: Some(format!("Database table {}", table.qualified_name())),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(table.line),
                line_end: Some(table.line),
            }),
            language: Some(LanguageCode("sql".to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "table_kind": "sql_table",
                "name": table.name,
                "schema": table.schema,
                "if_not_exists": table.if_not_exists,
            }),
        });

        facts.push(Fact {
            id: FactId(format!(
                "fact_migration_creates_table_{:016x}",
                stable_hash(format!("{}\0{}", migration_id.0, table_key.0).as_bytes())
            )),
            kind: FactKind::MigrationCreatesTable,
            subject: migration_id.clone(),
            object: Some(table_id),
            value: json!({
                "migration": migration_key.0,
                "table": table_key.0,
                "table_name": table.qualified_name(),
                "source_kind": "database_migration",
                "statement": "create_table",
                "if_not_exists": table.if_not_exists,
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(table.line),
                Some(table.line),
            )],
            ownership: ownership.clone(),
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });
    }
}

fn extract_github_actions_workflow(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let Some(workflow) = parse_github_actions_workflow(content) else {
        return;
    };

    let mut environment = workflow.environment.clone();
    for job in &workflow.jobs {
        for (name, declaration) in &job.environment {
            environment
                .entry(name.clone())
                .or_insert_with(|| declaration.clone());
        }
        for step in &job.steps {
            for (name, declaration) in &step.environment {
                environment
                    .entry(name.clone())
                    .or_insert_with(|| declaration.clone());
            }
        }
    }

    let workflow_name = workflow
        .name
        .clone()
        .unwrap_or_else(|| file_name(&input.source.path).to_string());
    let workflow_key = StableKey(format!(
        "script-command://{}#github-actions:workflow",
        input.source.path
    ));
    push_script_command_entity_and_fact(
        extractor,
        input,
        file_id,
        workflow_key,
        workflow_name.clone(),
        Some(format!("GitHub Actions workflow {workflow_name}")),
        workflow.line,
        "github_actions",
        json!({
            "command_kind": "github_actions_workflow",
            "workflow": workflow_name,
        }),
        json!({
            "path": input.source.path,
            "source_kind": "github_actions",
            "workflow": workflow_name,
        }),
        entities,
        facts,
    );

    for job in workflow.jobs {
        let job_key = StableKey(format!(
            "script-command://{}#github-actions:job:{}",
            input.source.path,
            sanitize_key_fragment(&job.id)
        ));
        let job_name = job.name.clone().unwrap_or_else(|| job.id.clone());
        push_script_command_entity_and_fact(
            extractor,
            input,
            file_id,
            job_key,
            job.id.clone(),
            Some(format!("GitHub Actions job {job_name}")),
            job.line,
            "github_actions",
            json!({
                "command_kind": "github_actions_job",
                "job": &job.id,
                "name": &job.name,
                "runs_on": &job.runs_on,
            }),
            json!({
                "path": input.source.path,
                "source_kind": "github_actions",
                "job": &job.id,
            }),
            entities,
            facts,
        );

        for step in job.steps {
            let (step_kind, value) = match &step.kind {
                GithubActionsStepKind::Run(command) => ("run", command.as_str()),
                GithubActionsStepKind::Uses(action) => ("uses", action.as_str()),
            };
            let step_key = StableKey(format!(
                "script-command://{}#github-actions:job:{}:step:{}:{}",
                input.source.path,
                sanitize_key_fragment(&job.id),
                step.index,
                step_kind
            ));
            let step_name = step
                .name
                .clone()
                .unwrap_or_else(|| format!("{} step {}", job.id, step.index));
            push_script_command_entity_and_fact(
                extractor,
                input,
                file_id,
                step_key,
                step_name.clone(),
                Some(format!("GitHub Actions {step_kind} step {step_name}")),
                step.line,
                "github_actions",
                json!({
                    "command_kind": "github_actions_step",
                    "job": &job.id,
                    "step": step.index,
                    "step_name": &step.name,
                    "step_kind": step_kind,
                    "value": value,
                }),
                json!({
                    "path": input.source.path,
                    "source_kind": "github_actions",
                    "job": &job.id,
                    "step": step.index,
                    "step_kind": step_kind,
                }),
                entities,
                facts,
            );
        }
    }

    extract_env_declarations(
        extractor,
        input,
        file_id,
        "github_actions",
        environment,
        entities,
        facts,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_script_command_entity_and_fact(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    stable_key: StableKey,
    name: String,
    title: Option<String>,
    line: u32,
    language: &str,
    payload: serde_json::Value,
    mut fact_value: serde_json::Value,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let entity_id = script_command_entity_id(&stable_key);
    let ownership = ownership_for_file(&input.source.path);
    entities.push(Entity {
        id: entity_id.clone(),
        stable_key: stable_key.clone(),
        kind: EntityKind::ScriptCommand,
        name,
        title,
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(line),
            line_end: Some(line),
        }),
        language: Some(LanguageCode(language.to_string())),
        aliases: Vec::new(),
        ownership: ownership.clone(),
        payload,
    });

    if let Some(object) = fact_value.as_object_mut() {
        object.insert("stable_key".to_string(), json!(&stable_key.0));
    }

    facts.push(Fact {
        id: FactId(format!(
            "fact_script_command_defined_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        )),
        kind: FactKind::SymbolDefined,
        subject: entity_id,
        object: Some(file_id.clone()),
        value: fact_value,
        evidence: vec![evidence_for_file(
            &input.source.path,
            extractor,
            Some(line),
            Some(line),
        )],
        ownership,
        snapshot: input.snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    });
}

#[allow(clippy::too_many_arguments)]
fn push_package_entity_and_fact(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    stable_key: StableKey,
    name: String,
    title: Option<String>,
    line: u32,
    payload: serde_json::Value,
    mut fact_value: serde_json::Value,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    let entity_id = package_entity_id(&stable_key);
    let ownership = ownership_for_file(&input.source.path);
    entities.push(Entity {
        id: entity_id.clone(),
        stable_key: stable_key.clone(),
        kind: EntityKind::Package,
        name,
        title,
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(line),
            line_end: Some(line),
        }),
        language: Some(LanguageCode("cargo".to_string())),
        aliases: Vec::new(),
        ownership: ownership.clone(),
        payload,
    });

    if let Some(object) = fact_value.as_object_mut() {
        object.insert("stable_key".to_string(), json!(&stable_key.0));
    }

    facts.push(Fact {
        id: FactId(format!(
            "fact_package_defined_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        )),
        kind: FactKind::SymbolDefined,
        subject: entity_id,
        object: Some(file_id.clone()),
        value: fact_value,
        evidence: vec![evidence_for_file(
            &input.source.path,
            extractor,
            Some(line),
            Some(line),
        )],
        ownership,
        snapshot: input.snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    });
}

fn env_var_entity_id(stable_key: &StableKey) -> EntityId {
    EntityId(format!(
        "ent_env_var_{:016x}",
        stable_hash(stable_key.0.as_bytes())
    ))
}

fn script_command_entity_id(stable_key: &StableKey) -> EntityId {
    EntityId(format!(
        "ent_script_command_{:016x}",
        stable_hash(stable_key.0.as_bytes())
    ))
}

fn package_entity_id(stable_key: &StableKey) -> EntityId {
    EntityId(format!(
        "ent_package_{:016x}",
        stable_hash(stable_key.0.as_bytes())
    ))
}

fn dependency_entity_id(stable_key: &StableKey) -> EntityId {
    EntityId(format!(
        "ent_dependency_{:016x}",
        stable_hash(stable_key.0.as_bytes())
    ))
}

fn parse_cargo_manifest(content: &str) -> Option<CargoManifest> {
    let root = toml::from_str::<toml::Table>(content).ok()?;
    let package = root
        .get("package")
        .and_then(toml::Value::as_table)
        .and_then(|package| {
            let name = package.get("name")?.as_str()?.to_string();
            Some(CargoPackage {
                name,
                version: package
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .map(str::to_string),
                edition: package
                    .get("edition")
                    .and_then(toml::Value::as_str)
                    .map(str::to_string),
                line: toml_header_line(content, "package").unwrap_or(1),
            })
        });

    let workspace = root
        .get("workspace")
        .and_then(toml::Value::as_table)
        .map(|workspace| CargoWorkspace {
            members: workspace
                .get("members")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect(),
            line: toml_header_line(content, "workspace").unwrap_or(1),
        });

    let mut dependencies = Vec::new();
    collect_cargo_dependency_sections(&root, "", content, &mut dependencies);
    dependencies.sort_by(|left, right| {
        left.section
            .cmp(&right.section)
            .then_with(|| left.name.cmp(&right.name))
    });

    Some(CargoManifest {
        package,
        workspace,
        dependencies,
    })
}

fn collect_cargo_dependency_sections(
    table: &toml::Table,
    prefix: &str,
    content: &str,
    dependencies: &mut Vec<CargoDependency>,
) {
    for (key, value) in table {
        let section = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{key}")
        };

        if is_cargo_dependency_section(&section) {
            if let Some(dependency_table) = value.as_table() {
                dependencies.extend(parse_cargo_dependencies(
                    content,
                    &section,
                    dependency_table,
                ));
            }
            continue;
        }

        if let Some(nested) = value.as_table() {
            collect_cargo_dependency_sections(nested, &section, content, dependencies);
        }
    }
}

fn is_cargo_dependency_section(section: &str) -> bool {
    matches!(
        section,
        "dependencies" | "dev-dependencies" | "build-dependencies" | "workspace.dependencies"
    ) || section.starts_with("target.")
        && (section.ends_with(".dependencies")
            || section.ends_with(".dev-dependencies")
            || section.ends_with(".build-dependencies"))
}

fn parse_cargo_dependencies(
    content: &str,
    section: &str,
    table: &toml::Table,
) -> Vec<CargoDependency> {
    table
        .iter()
        .map(|(name, value)| {
            let line = toml_key_line(content, section, name)
                .or_else(|| toml_header_line(content, &format!("{section}.{name}")))
                .or_else(|| toml_header_line(content, section))
                .unwrap_or(1);
            let mut dependency = CargoDependency {
                name: name.clone(),
                section: section.to_string(),
                version: None,
                path: None,
                git: None,
                registry: None,
                package: None,
                optional: None,
                features: Vec::new(),
                line,
            };

            match value {
                toml::Value::String(version) => {
                    dependency.version = Some(version.clone());
                }
                toml::Value::Table(table) => {
                    dependency.version = table
                        .get("version")
                        .and_then(toml::Value::as_str)
                        .map(str::to_string);
                    dependency.path = table
                        .get("path")
                        .and_then(toml::Value::as_str)
                        .map(str::to_string);
                    dependency.git = table
                        .get("git")
                        .and_then(toml::Value::as_str)
                        .map(str::to_string);
                    dependency.registry = table
                        .get("registry")
                        .and_then(toml::Value::as_str)
                        .map(str::to_string);
                    dependency.package = table
                        .get("package")
                        .and_then(toml::Value::as_str)
                        .map(str::to_string);
                    dependency.optional = table.get("optional").and_then(toml::Value::as_bool);
                    dependency.features = table
                        .get("features")
                        .and_then(toml::Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_string)
                        .collect();
                }
                _ => {}
            }

            dependency
        })
        .collect()
}

fn toml_header_line(content: &str, section: &str) -> Option<u32> {
    content.lines().enumerate().find_map(|(index, line)| {
        toml_section_name(line)
            .filter(|name| name == section)
            .map(|_| (index + 1) as u32)
    })
}

fn toml_key_line(content: &str, section: &str, key: &str) -> Option<u32> {
    let mut in_section = false;
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(name) = toml_section_name(trimmed) {
            in_section = name == section;
            continue;
        }
        if in_section
            && (trimmed == key
                || trimmed.starts_with(&format!("{key} "))
                || trimmed.starts_with(&format!("{key}."))
                || trimmed.starts_with(&format!("{key}=")))
        {
            return Some((index + 1) as u32);
        }
    }
    None
}

fn toml_section_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return None;
    }
    Some(
        trimmed[1..trimmed.len() - 1]
            .chars()
            .filter(|character| !matches!(character, '\'' | '"'))
            .collect(),
    )
}

fn parse_dotenv_declarations(content: &str) -> BTreeMap<String, EnvDeclaration> {
    let mut declarations = BTreeMap::new();
    for (index, line) in content.lines().enumerate() {
        let line_number = (index + 1) as u32;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let declaration = trimmed
            .strip_prefix("export ")
            .unwrap_or(trimmed)
            .trim_start();
        let Some((key, value)) = declaration.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !is_env_name(key) {
            continue;
        }
        declarations
            .entry(key.to_string())
            .or_insert(EnvDeclaration {
                line: line_number,
                has_value: !value.trim().is_empty(),
            });
    }
    declarations
}

fn parse_makefile_targets(content: &str) -> Vec<MakeTarget> {
    let mut targets = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let line_number = (index + 1) as u32;
        if line.starts_with('\t') {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('.')
            || trimmed.contains(":=")
            || trimmed.contains("+=")
            || trimmed.contains("?=")
            || trimmed.contains("::=")
        {
            continue;
        }
        let Some((left, right)) = trimmed.split_once(':') else {
            continue;
        };
        let left = left.trim();
        if left.is_empty() || left.contains('=') {
            continue;
        }
        for name in left
            .split_whitespace()
            .filter(|name| is_make_target_name(name))
        {
            targets.push(MakeTarget {
                name: name.to_string(),
                line: line_number,
                prerequisites: right
                    .split_whitespace()
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
                    .collect(),
            });
        }
    }
    targets
}

fn parse_dockerfile_instructions(content: &str) -> Vec<DockerInstruction> {
    let mut instructions = Vec::new();
    let mut pending: Option<(String, u32)> = None;

    for (index, raw_line) in content.lines().enumerate() {
        let line_number = (index + 1) as u32;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let continued = line.ends_with('\\');
        let fragment = line.trim_end_matches('\\').trim_end();
        match pending.as_mut() {
            Some((value, _)) => {
                value.push(' ');
                value.push_str(fragment);
            }
            None => pending = Some((fragment.to_string(), line_number)),
        }

        if continued {
            continue;
        }

        let Some((combined, start_line)) = pending.take() else {
            continue;
        };
        let mut parts = combined.splitn(2, char::is_whitespace);
        let keyword = parts.next().unwrap_or_default().to_ascii_uppercase();
        let value = parts.next().unwrap_or_default().trim().to_string();
        if !keyword.is_empty() {
            instructions.push(DockerInstruction {
                keyword,
                value,
                line: start_line,
            });
        }
    }

    instructions
}

fn parse_dockerfile_env_declarations(
    instruction: &DockerInstruction,
) -> BTreeMap<String, EnvDeclaration> {
    let mut declarations = BTreeMap::new();
    let parts = instruction.value.split_whitespace().collect::<Vec<_>>();
    if parts.len() == 2 && !parts[0].contains('=') && is_env_name(parts[0]) {
        declarations.insert(
            parts[0].to_string(),
            EnvDeclaration {
                line: instruction.line,
                has_value: !parts[1].is_empty(),
            },
        );
        return declarations;
    }

    for part in parts {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if is_env_name(key) {
            declarations.insert(
                key.to_string(),
                EnvDeclaration {
                    line: instruction.line,
                    has_value: !value.is_empty(),
                },
            );
        }
    }

    declarations
}

fn parse_shell_env_declarations(content: &str) -> BTreeMap<String, EnvDeclaration> {
    let mut declarations = BTreeMap::new();
    for (index, line) in content.lines().enumerate() {
        let line_number = (index + 1) as u32;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let declaration = trimmed
            .strip_prefix("export ")
            .or_else(|| trimmed.strip_prefix("readonly "))
            .unwrap_or_default()
            .trim_start();
        if declaration.is_empty() {
            continue;
        }
        let Some((key, value)) = declaration.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !is_env_name(key) {
            continue;
        }
        declarations
            .entry(key.to_string())
            .or_insert(EnvDeclaration {
                line: line_number,
                has_value: !value.trim().is_empty(),
            });
    }
    declarations
}

fn parse_shell_functions(content: &str) -> Vec<MakeTarget> {
    let mut functions = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let line_number = (index + 1) as u32;
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(name) = shell_function_name(trimmed) {
            functions.push(MakeTarget {
                name,
                line: line_number,
                prerequisites: Vec::new(),
            });
        }
    }
    functions
}

fn parse_docker_compose_services(content: &str) -> Vec<ComposeService> {
    let Ok(root) = serde_yaml_ng::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let Some(services) = root.get("services").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };

    let mut output = services
        .iter()
        .filter_map(|(name, value)| {
            let service = value.as_object()?;
            let line = yaml_key_line(content, name).unwrap_or(1);
            Some(ComposeService {
                name: name.clone(),
                image: service
                    .get("image")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                build: compose_build_value(service.get("build")),
                command: compose_command_value(service.get("command")),
                entrypoint: compose_command_value(service.get("entrypoint")),
                line,
                environment: compose_environment_declarations(service.get("environment"), line),
            })
        })
        .collect::<Vec<_>>();
    output.sort_by(|left, right| left.name.cmp(&right.name));
    output
}

fn parse_kubernetes_resources(content: &str) -> Vec<KubernetesResource> {
    let mut resources = split_yaml_documents(content)
        .into_iter()
        .filter_map(|(document, start_line)| parse_kubernetes_resource(&document, start_line))
        .collect::<Vec<_>>();
    resources.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.name.cmp(&right.name))
    });
    resources
}

fn parse_kubernetes_resource(document: &str, start_line: u32) -> Option<KubernetesResource> {
    let Ok(root_value) = serde_yaml_ng::from_str::<serde_json::Value>(document) else {
        return None;
    };
    let root = root_value.as_object()?;
    let kind = root.get("kind")?.as_str()?.to_string();
    let metadata = root.get("metadata")?.as_object()?;
    let name = metadata.get("name")?.as_str()?.to_string();
    let namespace = metadata
        .get("namespace")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let line = yaml_key_line(document, "kind")
        .map(|line| start_line + line - 1)
        .unwrap_or(start_line);

    let mut containers = Vec::new();
    collect_kubernetes_containers(&root_value, start_line, &mut containers);

    let mut environment = BTreeMap::new();
    collect_kubernetes_env_declarations(&root_value, start_line, &mut environment);
    if matches!(kind.as_str(), "ConfigMap" | "Secret") {
        collect_kubernetes_data_env(root.get("data"), start_line, &mut environment);
        collect_kubernetes_data_env(root.get("stringData"), start_line, &mut environment);
    }

    Some(KubernetesResource {
        kind,
        name,
        namespace,
        line,
        images: containers
            .iter()
            .filter_map(|container| container.image.clone())
            .collect(),
        containers: containers
            .iter()
            .map(|container| container.name.clone())
            .collect(),
        commands: containers
            .into_iter()
            .flat_map(|container| container.commands)
            .collect(),
        environment,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KubernetesContainer {
    name: String,
    image: Option<String>,
    commands: Vec<KubernetesContainerCommand>,
}

fn collect_kubernetes_containers(
    value: &serde_json::Value,
    start_line: u32,
    containers: &mut Vec<KubernetesContainer>,
) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(values) = object
                .get("containers")
                .and_then(serde_json::Value::as_array)
            {
                for value in values {
                    if let Some(container) = parse_kubernetes_container(value, start_line) {
                        containers.push(container);
                    }
                }
            }
            if let Some(values) = object
                .get("initContainers")
                .and_then(serde_json::Value::as_array)
            {
                for value in values {
                    if let Some(container) = parse_kubernetes_container(value, start_line) {
                        containers.push(container);
                    }
                }
            }
            for value in object.values() {
                collect_kubernetes_containers(value, start_line, containers);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_kubernetes_containers(value, start_line, containers);
            }
        }
        _ => {}
    }
}

fn parse_kubernetes_container(
    value: &serde_json::Value,
    start_line: u32,
) -> Option<KubernetesContainer> {
    let container = value.as_object()?;
    let name = container.get("name")?.as_str()?.to_string();
    let line = start_line;
    let mut commands = Vec::new();
    for instruction in ["command", "args"] {
        if let Some(value) = kubernetes_command_value(container.get(instruction)) {
            commands.push(KubernetesContainerCommand {
                container: name.clone(),
                instruction: instruction.to_string(),
                value,
                line,
            });
        }
    }

    Some(KubernetesContainer {
        name,
        image: container
            .get("image")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        commands,
    })
}

fn collect_kubernetes_env_declarations(
    value: &serde_json::Value,
    start_line: u32,
    declarations: &mut BTreeMap<String, EnvDeclaration>,
) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(values) = object.get("env").and_then(serde_json::Value::as_array) {
                for value in values {
                    let Some(env) = value.as_object() else {
                        continue;
                    };
                    let Some(name) = env.get("name").and_then(serde_json::Value::as_str) else {
                        continue;
                    };
                    if is_env_name(name) {
                        declarations
                            .entry(name.to_string())
                            .or_insert(EnvDeclaration {
                                line: start_line,
                                has_value: env.contains_key("value")
                                    || env.contains_key("valueFrom"),
                            });
                    }
                }
            }
            for value in object.values() {
                collect_kubernetes_env_declarations(value, start_line, declarations);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_kubernetes_env_declarations(value, start_line, declarations);
            }
        }
        _ => {}
    }
}

fn collect_kubernetes_data_env(
    value: Option<&serde_json::Value>,
    line: u32,
    declarations: &mut BTreeMap<String, EnvDeclaration>,
) {
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return;
    };
    for key in object.keys().filter(|key| is_env_name(key)) {
        declarations.entry(key.clone()).or_insert(EnvDeclaration {
            line,
            has_value: true,
        });
    }
}

fn kubernetes_command_value(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Array(values) => {
            let parts = values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>();
            (!parts.is_empty()).then(|| parts.join(" "))
        }
        _ => None,
    }
}

fn split_yaml_documents(content: &str) -> Vec<(String, u32)> {
    let mut documents = Vec::new();
    let mut current = Vec::new();
    let mut start_line = 1;

    for (index, line) in content.lines().enumerate() {
        let line_number = (index + 1) as u32;
        if line.trim() == "---" {
            if !current.is_empty() {
                documents.push((current.join("\n"), start_line));
                current.clear();
            }
            start_line = line_number + 1;
            continue;
        }
        current.push(line.to_string());
    }

    if !current.is_empty() {
        documents.push((current.join("\n"), start_line));
    }

    documents
}

fn parse_database_migration(path: &str, content: &str) -> DatabaseMigration {
    DatabaseMigration {
        name: file_name(path).trim_end_matches(".sql").to_string(),
        line: 1,
        created_tables: parse_created_tables(content),
    }
}

fn parse_created_tables(content: &str) -> Vec<DatabaseTableDeclaration> {
    let mut tables = Vec::new();
    let mut pending = String::new();
    let mut pending_line = 1;

    for (index, line) in content.lines().enumerate() {
        let line_number = (index + 1) as u32;
        let stripped = strip_sql_line_comment(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if pending.is_empty() {
            pending_line = line_number;
        } else {
            pending.push(' ');
        }
        pending.push_str(trimmed);

        if trimmed.contains(';') {
            if let Some(table) = parse_create_table_statement(&pending, pending_line) {
                tables.push(table);
            }
            pending.clear();
        }
    }

    if !pending.is_empty()
        && let Some(table) = parse_create_table_statement(&pending, pending_line)
    {
        tables.push(table);
    }

    tables
}

fn parse_create_table_statement(statement: &str, line: u32) -> Option<DatabaseTableDeclaration> {
    let normalized = statement
        .replace(['\n', '\r', '\t', '(', ';'], " ")
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if normalized.len() < 3 {
        return None;
    }
    if !normalized[0].eq_ignore_ascii_case("create") || !normalized[1].eq_ignore_ascii_case("table")
    {
        return None;
    }

    let mut index = 2;
    let mut if_not_exists = false;
    if normalized.get(index..index + 3).is_some_and(|words| {
        words[0].eq_ignore_ascii_case("if")
            && words[1].eq_ignore_ascii_case("not")
            && words[2].eq_ignore_ascii_case("exists")
    }) {
        if_not_exists = true;
        index += 3;
    }

    let raw_name = normalized.get(index)?;
    let name = raw_name
        .trim_matches('"')
        .trim_matches('`')
        .trim_matches('[')
        .trim_matches(']')
        .to_string();
    if !is_sql_identifier_path(&name) {
        return None;
    }
    let (schema, name) = name
        .rsplit_once('.')
        .map_or((None, name.clone()), |(schema, table)| {
            (Some(schema.to_string()), table.to_string())
        });

    Some(DatabaseTableDeclaration {
        name,
        schema,
        line,
        if_not_exists,
    })
}

fn strip_sql_line_comment(line: &str) -> &str {
    line.split_once("--")
        .map_or(line, |(before_comment, _)| before_comment)
}

fn parse_github_actions_workflow(content: &str) -> Option<GithubActionsWorkflow> {
    let Ok(root) = serde_yaml_ng::from_str::<serde_json::Value>(content) else {
        return None;
    };
    let root = root.as_object()?;
    let jobs = root.get("jobs")?.as_object()?;
    let mut parsed_jobs = jobs
        .iter()
        .filter_map(|(id, value)| {
            let job = value.as_object()?;
            let line = yaml_key_line(content, id).unwrap_or(1);
            let steps = job
                .get("steps")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
                .filter_map(|(index, value)| parse_github_actions_step(content, index + 1, value))
                .collect::<Vec<_>>();
            Some(GithubActionsJob {
                id: id.clone(),
                name: job
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                runs_on: github_actions_runs_on(job.get("runs-on")),
                line,
                environment: compose_environment_declarations(job.get("env"), line),
                steps,
            })
        })
        .collect::<Vec<_>>();
    parsed_jobs.sort_by(|left, right| left.id.cmp(&right.id));

    Some(GithubActionsWorkflow {
        name: root
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        line: yaml_key_line(content, "name").unwrap_or(1),
        environment: compose_environment_declarations(root.get("env"), 1),
        jobs: parsed_jobs,
    })
}

fn parse_github_actions_step(
    content: &str,
    index: usize,
    value: &serde_json::Value,
) -> Option<GithubActionsStep> {
    let step = value.as_object()?;
    let (kind, needle) = if let Some(command) = step.get("run").and_then(serde_json::Value::as_str)
    {
        (GithubActionsStepKind::Run(command.to_string()), "run")
    } else if let Some(action) = step.get("uses").and_then(serde_json::Value::as_str) {
        (GithubActionsStepKind::Uses(action.to_string()), "uses")
    } else {
        return None;
    };
    let line = yaml_key_line(content, needle).unwrap_or(index as u32);
    Some(GithubActionsStep {
        index,
        name: step
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        kind,
        line,
        environment: compose_environment_declarations(step.get("env"), line),
    })
}

fn github_actions_runs_on(value: Option<&serde_json::Value>) -> Vec<String> {
    match value {
        Some(serde_json::Value::String(value)) => vec![value.clone()],
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn compose_build_value(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Object(object) => object
            .get("context")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn compose_command_value(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Array(values) => {
            let parts = values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>();
            (!parts.is_empty()).then(|| parts.join(" "))
        }
        _ => None,
    }
}

fn compose_environment_declarations(
    value: Option<&serde_json::Value>,
    line: u32,
) -> BTreeMap<String, EnvDeclaration> {
    let mut declarations = BTreeMap::new();
    match value {
        Some(serde_json::Value::Object(object)) => {
            for (key, value) in object {
                if is_env_name(key) {
                    declarations.insert(
                        key.clone(),
                        EnvDeclaration {
                            line,
                            has_value: !value.is_null(),
                        },
                    );
                }
            }
        }
        Some(serde_json::Value::Array(values)) => {
            for value in values.iter().filter_map(serde_json::Value::as_str) {
                let (key, has_value) = value
                    .split_once('=')
                    .map_or((value.trim(), false), |(key, value)| {
                        (key.trim(), !value.trim().is_empty())
                    });
                if is_env_name(key) {
                    declarations.insert(key.to_string(), EnvDeclaration { line, has_value });
                }
            }
        }
        _ => {}
    }
    declarations
}

fn yaml_key_line(content: &str, key: &str) -> Option<u32> {
    let needle = format!("{key}:");
    content.lines().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim_start();
        (trimmed == needle || trimmed.starts_with(&format!("{needle} ")))
            .then_some((index + 1) as u32)
    })
}

fn shell_function_name(line: &str) -> Option<String> {
    let declaration = line.strip_prefix("function ").unwrap_or(line).trim_start();
    if let Some((name, rest)) = declaration.split_once("()") {
        let name = name.trim();
        if is_shell_function_name(name) && rest.trim_start().starts_with('{') {
            return Some(name.to_string());
        }
    }

    let mut parts = declaration.split_whitespace();
    let name = parts.next()?.trim_end_matches("()");
    let next = parts.next()?;
    if is_shell_function_name(name) && next == "{" {
        return Some(name.to_string());
    }
    None
}

fn docker_stage_name(value: &str) -> Option<String> {
    let words = value.split_whitespace().collect::<Vec<_>>();
    words
        .windows(2)
        .find(|window| window[0].eq_ignore_ascii_case("as"))
        .map(|window| sanitize_key_fragment(window[1]))
}

fn docker_base_image(value: &str) -> Option<String> {
    value.split_whitespace().next().map(ToString::to_string)
}

fn is_dotenv_path(path: &str) -> bool {
    let file_name = file_name(path);
    file_name == ".env"
        || file_name == ".env.example"
        || file_name.ends_with(".env")
        || file_name.ends_with(".env.example")
}

fn is_cargo_manifest_path(path: &str) -> bool {
    file_name(path) == "Cargo.toml"
}

fn is_makefile_path(path: &str) -> bool {
    let file_name = file_name(path);
    file_name == "Makefile" || file_name == "makefile" || file_name.ends_with(".mk")
}

fn is_dockerfile_path(path: &str) -> bool {
    let file_name = file_name(path);
    file_name == "Dockerfile" || file_name.ends_with(".Dockerfile")
}

fn is_shell_script_path(path: &str) -> bool {
    let file_name = file_name(path);
    file_name.ends_with(".sh") || file_name.ends_with(".bash") || file_name.ends_with(".zsh")
}

fn is_docker_compose_path(path: &str) -> bool {
    let file_name = file_name(path).to_ascii_lowercase();
    matches!(
        file_name.as_str(),
        "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml"
    ) || file_name.ends_with(".compose.yml")
        || file_name.ends_with(".compose.yaml")
}

fn is_kubernetes_manifest_path(path: &str) -> bool {
    if is_docker_compose_path(path) || is_github_actions_workflow_path(path) {
        return false;
    }
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    if !(normalized.ends_with(".yml") || normalized.ends_with(".yaml")) {
        return false;
    }
    let file_name = file_name(&normalized);
    matches!(
        file_name,
        "deployment.yml"
            | "deployment.yaml"
            | "deploy.yml"
            | "deploy.yaml"
            | "service.yml"
            | "service.yaml"
            | "ingress.yml"
            | "ingress.yaml"
            | "configmap.yml"
            | "configmap.yaml"
            | "secret.yml"
            | "secret.yaml"
            | "namespace.yml"
            | "namespace.yaml"
            | "job.yml"
            | "job.yaml"
            | "cronjob.yml"
            | "cronjob.yaml"
            | "statefulset.yml"
            | "statefulset.yaml"
            | "daemonset.yml"
            | "daemonset.yaml"
    ) || normalized.starts_with("k8s/")
        || normalized.contains("/k8s/")
        || normalized.starts_with("kubernetes/")
        || normalized.contains("/kubernetes/")
        || normalized.starts_with("deploy/")
        || normalized.starts_with("deployments/")
}

fn is_database_migration_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    if !normalized.ends_with(".sql") {
        return false;
    }
    let file_name = file_name(&normalized);
    normalized.contains("/migrations/")
        || normalized.starts_with("migrations/")
        || normalized.contains("/db/")
        || normalized.starts_with("db/")
        || normalized.contains("/sqlx/")
        || normalized.contains("/diesel/")
        || normalized.contains("/prisma/")
        || file_name.starts_with("migration_")
        || file_name.starts_with("migrate_")
        || file_name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
}

fn is_github_actions_workflow_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.starts_with(".github/workflows/")
        && (normalized.ends_with(".yml") || normalized.ends_with(".yaml"))
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn is_make_target_name(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | '/')
        })
}

fn is_shell_function_name(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

fn sanitize_key_fragment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn is_env_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_uppercase())
        && chars.all(|character| {
            character == '_' || character.is_ascii_uppercase() || character.is_ascii_digit()
        })
}

fn is_sql_identifier_path(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
                })
        })
}

#[cfg(test)]
mod tests {
    use athanor_domain::{RepoId, SnapshotId};

    use super::*;

    #[tokio::test]
    async fn extracts_dotenv_environment_variables_without_values() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: ".env.example".to_string(),
                    language_hint: None,
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "# example\nDATABASE_URL=postgres://example\nexport API_TOKEN=\nBAD-KEY=value\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let keys = output
            .entities
            .iter()
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["env://API_TOKEN", "env://DATABASE_URL"]);
        assert_eq!(output.facts.len(), 2);
        assert!(
            output
                .entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::EnvVar)
                .all(|entity| entity.payload.get("value").is_none())
        );
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }

    #[tokio::test]
    async fn extracts_makefile_targets_as_script_commands() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "Makefile".to_string(),
                    language_hint: None,
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        ".PHONY: test\nbuild test: src/lib.rs\n\tcargo test\nNAME := ignored\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let commands = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ScriptCommand)
            .map(|entity| entity.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(commands, vec!["build", "test"]);
        assert_eq!(output.facts.len(), 2);
        assert!(
            output
                .facts
                .iter()
                .all(|fact| fact.kind == FactKind::SymbolDefined)
        );
    }

    #[tokio::test]
    async fn extracts_cargo_manifest_packages_and_dependencies() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "crates/example/Cargo.toml".to_string(),
                    language_hint: Some("toml".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "[package]\nname = \"example\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde.workspace = true\nlocal-crate = { path = \"../local-crate\", package = \"local_crate\" }\n\n[dev-dependencies]\ntokio = { version = \"1\", features = [\"macros\"], optional = true }\n\n[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Package
                && entity.stable_key.0 == "package://crates/example/Cargo.toml#example"
                && entity.payload["version"] == json!("0.1.0")
        }));
        let dependency_keys = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Dependency)
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            dependency_keys,
            vec![
                "dependency://crates/example/Cargo.toml#dependencies:local-crate",
                "dependency://crates/example/Cargo.toml#dependencies:serde",
                "dependency://crates/example/Cargo.toml#dev-dependencies:tokio",
                "dependency://crates/example/Cargo.toml#target.cfg-unix-.dependencies:libc",
            ]
        );
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Dependency
                && entity.name == "local-crate"
                && entity.aliases == vec!["local_crate"]
                && entity.payload["path"] == json!("../local-crate")
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Dependency
                && entity.name == "tokio"
                && entity.payload["features"] == json!(["macros"])
                && entity.payload["optional"] == json!(true)
        }));
        assert!(
            output
                .facts
                .iter()
                .all(|fact| fact.kind == FactKind::SymbolDefined
                    && !fact.evidence.is_empty()
                    && !fact.ownership.is_empty())
        );
    }

    #[tokio::test]
    async fn extracts_cargo_workspace_manifest() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "Cargo.toml".to_string(),
                    language_hint: Some("toml".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "[workspace]\nmembers = [\"crates/example\"]\n\n[workspace.dependencies]\nserde = \"1\"\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Package
                && entity.stable_key.0 == "package://Cargo.toml#workspace"
                && entity.payload["members"] == json!(["crates/example"])
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Dependency
                && entity.stable_key.0 == "dependency://Cargo.toml#workspace.dependencies:serde"
                && entity.payload["version"] == json!("1")
        }));
    }

    #[tokio::test]
    async fn extracts_dockerfile_stages_commands_and_env_without_values() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "Dockerfile".to_string(),
                    language_hint: None,
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "FROM rust:1.95 AS builder\nENV DATABASE_URL=postgres://example API_TOKEN=\nRUN cargo build\nCMD [\"ath\"]\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::DockerService
                && entity.stable_key.0 == "docker://Dockerfile#builder"
        }));
        assert_eq!(
            output
                .entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::ScriptCommand)
                .count(),
            2
        );
        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.stable_key.0 == "env://DATABASE_URL")
        );
        assert!(
            output
                .entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::EnvVar)
                .all(|entity| entity.payload.get("value").is_none())
        );
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }

    #[tokio::test]
    async fn extracts_shell_functions_and_exported_environment_variables() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "scripts/deploy.sh".to_string(),
                    language_hint: None,
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "#!/usr/bin/env bash\nexport DATABASE_URL=postgres://example\nbuild_app() {\n  cargo build\n}\nfunction deploy {\n  ./deploy\n}\nreadonly API_TOKEN=\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let commands = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ScriptCommand)
            .map(|entity| (entity.stable_key.0.as_str(), entity.name.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            commands,
            vec![
                (
                    "script-command://scripts/deploy.sh#shell-function:build_app",
                    "build_app"
                ),
                (
                    "script-command://scripts/deploy.sh#shell-function:deploy",
                    "deploy"
                ),
            ]
        );
        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.stable_key.0 == "env://DATABASE_URL")
        );
        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.stable_key.0 == "env://API_TOKEN")
        );
        assert!(
            output
                .entities
                .iter()
                .all(|entity| entity.payload.get("value").is_none())
        );
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }

    #[tokio::test]
    async fn extracts_docker_compose_services_commands_and_env_without_values() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "docker-compose.yml".to_string(),
                    language_hint: Some("yaml".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "services:\n  web:\n    image: example/web:latest\n    command: [\"ath\", \"serve\"]\n    environment:\n      DATABASE_URL: postgres://example\n      API_TOKEN:\n  worker:\n    build:\n      context: .\n    entrypoint: ./worker\n    environment:\n      - DATABASE_URL=postgres://example\n      - WORKER_CONCURRENCY=4\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::DockerService
                && entity.stable_key.0 == "docker://docker-compose.yml#compose-service:web"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::DockerService
                && entity.stable_key.0 == "docker://docker-compose.yml#compose-service:worker"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ScriptCommand
                && entity.stable_key.0 == "script-command://docker-compose.yml#compose:web:command"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ScriptCommand
                && entity.stable_key.0
                    == "script-command://docker-compose.yml#compose:worker:entrypoint"
        }));
        let env_keys = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::EnvVar)
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            env_keys,
            vec![
                "env://API_TOKEN",
                "env://DATABASE_URL",
                "env://WORKER_CONCURRENCY"
            ]
        );
        assert!(
            output
                .entities
                .iter()
                .all(|entity| entity.payload.get("value").is_none())
        );
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }

    #[tokio::test]
    async fn extracts_kubernetes_resources_commands_and_env_without_values() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "k8s/deployment.yaml".to_string(),
                    language_hint: Some("yaml".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: api\n  namespace: prod\nspec:\n  template:\n    spec:\n      containers:\n        - name: web\n          image: example/api:latest\n          command: [\"ath\", \"serve\"]\n          env:\n            - name: DATABASE_URL\n              value: postgres://example\n            - name: SECRET_TOKEN\n              valueFrom:\n                secretKeyRef:\n                  name: api-secret\n                  key: token\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: api\nspec:\n  ports:\n    - port: 80\n---\napiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: app-config\ndata:\n  APP_CONFIG: enabled\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::DockerService
                && entity.stable_key.0 == "kubernetes://k8s/deployment.yaml#Deployment:prod-api"
                && entity.payload["images"] == json!(["example/api:latest"])
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::DockerService
                && entity.stable_key.0 == "kubernetes://k8s/deployment.yaml#Service:api"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::DockerService
                && entity.stable_key.0 == "kubernetes://k8s/deployment.yaml#ConfigMap:app-config"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ScriptCommand
                && entity.stable_key.0
                    == "script-command://k8s/deployment.yaml#kubernetes:Deployment:prod-api:container:web:command"
                && entity.payload["value"] == json!("ath serve")
        }));
        let env_keys = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::EnvVar)
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            env_keys,
            vec![
                "env://APP_CONFIG",
                "env://DATABASE_URL",
                "env://SECRET_TOKEN"
            ]
        );
        assert!(
            output
                .entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::EnvVar)
                .all(|entity| entity.payload.get("value").is_none())
        );
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }

    #[tokio::test]
    async fn extracts_sql_database_migrations_and_created_tables() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "migrations/20260101000000_create_users.sql".to_string(),
                    language_hint: Some("sql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "-- users table\nCREATE TABLE IF NOT EXISTS public.users (\n  id uuid primary key\n);\n\ncreate table audit_events (\n  id bigint primary key\n);\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::DbMigration
                && entity.stable_key.0
                    == "db-migration://migrations/20260101000000_create_users.sql"
        }));
        let table_keys = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::DbTable)
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            table_keys,
            vec![
                "db-table://migrations/20260101000000_create_users.sql#public.users",
                "db-table://migrations/20260101000000_create_users.sql#audit_events",
            ]
        );
        assert!(
            output
                .facts
                .iter()
                .filter(|fact| fact.kind == FactKind::MigrationCreatesTable)
                .count()
                == 2
        );
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }

    #[tokio::test]
    async fn extracts_github_actions_workflow_jobs_steps_and_env_without_values() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: ".github/workflows/ci.yml".to_string(),
                    language_hint: Some("yaml".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "name: CI\n\nenv:\n  CARGO_TERM_COLOR: always\n\njobs:\n  quality:\n    name: Rust quality\n    runs-on: [ubuntu-latest, windows-latest]\n    env:\n      RUST_BACKTRACE: 1\n    steps:\n      - name: Check out\n        uses: actions/checkout@v7\n      - name: Run tests\n        run: cargo test --workspace --quiet\n        env:\n          TEST_TOKEN:\n"
                            .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ScriptCommand
                && entity.stable_key.0
                    == "script-command://.github/workflows/ci.yml#github-actions:workflow"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ScriptCommand
                && entity.stable_key.0
                    == "script-command://.github/workflows/ci.yml#github-actions:job:quality"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ScriptCommand
                && entity.stable_key.0
                    == "script-command://.github/workflows/ci.yml#github-actions:job:quality:step:1:uses"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ScriptCommand
                && entity.stable_key.0
                    == "script-command://.github/workflows/ci.yml#github-actions:job:quality:step:2:run"
                && entity.payload["value"] == json!("cargo test --workspace --quiet")
        }));
        let env_keys = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::EnvVar)
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            env_keys,
            vec![
                "env://CARGO_TERM_COLOR",
                "env://RUST_BACKTRACE",
                "env://TEST_TOKEN"
            ]
        );
        assert!(
            output
                .entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::EnvVar)
                .all(|entity| entity.payload.get("value").is_none())
        );
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }

    #[test]
    fn recognizes_operations_paths() {
        assert!(is_dotenv_path(".env"));
        assert!(is_dotenv_path(".env.example"));
        assert!(is_dotenv_path("config/app.env"));
        assert!(is_cargo_manifest_path("Cargo.toml"));
        assert!(is_cargo_manifest_path("crates/example/Cargo.toml"));
        assert!(is_makefile_path("Makefile"));
        assert!(is_makefile_path("build/tasks.mk"));
        assert!(is_dockerfile_path("Dockerfile"));
        assert!(is_dockerfile_path("build/app.Dockerfile"));
        assert!(is_shell_script_path("scripts/deploy.sh"));
        assert!(is_shell_script_path("scripts/deploy.bash"));
        assert!(is_shell_script_path("scripts/deploy.zsh"));
        assert!(is_docker_compose_path("docker-compose.yml"));
        assert!(is_docker_compose_path("compose.yaml"));
        assert!(is_docker_compose_path("deploy/app.compose.yml"));
        assert!(is_kubernetes_manifest_path("k8s/deployment.yaml"));
        assert!(is_kubernetes_manifest_path("deploy/service.yml"));
        assert!(is_kubernetes_manifest_path("manifests/secret.yaml"));
        assert!(!is_kubernetes_manifest_path("docker-compose.yml"));
        assert!(is_database_migration_path(
            "migrations/20260101000000_create_users.sql"
        ));
        assert!(is_database_migration_path("db/schema/migration_init.sql"));
        assert!(!is_database_migration_path("docs/query.sql"));
        assert!(is_github_actions_workflow_path(".github/workflows/ci.yml"));
        assert!(is_github_actions_workflow_path(
            ".github/workflows/security.yaml"
        ));
        assert!(!is_dotenv_path("src/lib.rs"));
    }
}
