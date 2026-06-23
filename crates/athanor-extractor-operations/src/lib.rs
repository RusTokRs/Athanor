use std::collections::BTreeMap;
use std::path::Path;

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
            || is_makefile_path(&source.path)
            || is_dockerfile_path(&source.path)
            || is_shell_script_path(&source.path)
            || is_docker_compose_path(&source.path)
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

fn is_github_actions_workflow_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.starts_with(".github/workflows/")
        && (normalized.ends_with(".yml") || normalized.ends_with(".yaml"))
}

fn file_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or(path)
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
        assert!(is_github_actions_workflow_path(".github/workflows/ci.yml"));
        assert!(is_github_actions_workflow_path(
            ".github/workflows/security.yaml"
        ));
        assert!(!is_dotenv_path("src/lib.rs"));
    }
}
