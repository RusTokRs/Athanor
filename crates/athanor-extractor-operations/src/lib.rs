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
        assert!(!is_dotenv_path("src/lib.rs"));
    }
}
