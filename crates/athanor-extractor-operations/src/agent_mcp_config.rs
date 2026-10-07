use athanor_core::ExtractInput;
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use serde_json::json;

use super::{json_key_line, sanitize_key_fragment, script_command_entity_id};

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentMcpConfig {
    tool: &'static str,
    servers: Vec<AgentMcpServer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentMcpServer {
    name: String,
    command: String,
    args: Vec<String>,
    line: u32,
}

// Verification gate note: Slice 8I keeps first-party agent-tool MCP configuration
// extraction bounded and fail-closed.
pub(super) fn is_agent_mcp_config_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized == ".codex.json" || normalized == "antigravity.json"
}

pub(super) fn extract_agent_mcp_config(
    extractor: &str,
    input: &ExtractInput,
    file_id: &EntityId,
    content: &str,
    entities: &mut Vec<Entity>,
    facts: &mut Vec<Fact>,
) {
    if !is_agent_mcp_config_path(&input.source.path) {
        return;
    }
    let Some(config) = parse_agent_mcp_config(&input.source.path, content) else {
        return;
    };

    for server in config.servers {
        let stable_key = StableKey(format!(
            "script-command://{}#agent-mcp-server:{}",
            input.source.path,
            sanitize_key_fragment(&server.name)
        ));
        let entity_id = script_command_entity_id(&stable_key);
        let ownership = ownership_for_file(&input.source.path);
        let tool_title = agent_tool_title(config.tool);

        entities.push(Entity {
            id: entity_id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::ScriptCommand,
            name: server.name.clone(),
            title: Some(format!("{} MCP server {}", tool_title, server.name)),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(server.line),
                line_end: Some(server.line),
            }),
            language: Some(LanguageCode("json".to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "command_kind": "agent_mcp_server",
                "agent_tool": config.tool,
                "server": server.name.clone(),
                "command": server.command.clone(),
                "args": server.args,
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
                "source_kind": "agent_mcp_config",
                "agent_tool": config.tool,
                "server": server.name,
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                extractor,
                Some(server.line),
                Some(server.line),
            )],
            ownership,
            snapshot: input.snapshot.clone(),
            extractor: extractor.to_string(),
            confidence: 1.0,
        });
    }
}

fn agent_tool_title(tool: &str) -> &str {
    match tool {
        "codex" => "Codex",
        _ => "Antigravity",
    }
}

fn parse_agent_mcp_config(path: &str, content: &str) -> Option<AgentMcpConfig> {
    let normalized_path = path.replace('\\', "/").to_ascii_lowercase();
    let root: serde_json::Value = serde_json::from_str(content).ok()?;
    let object = root.as_object()?;
    let (tool, servers_object) = match normalized_path.as_str() {
        ".codex.json" => {
            let servers = object
                .get("mcp")?
                .as_object()?
                .get("servers")?
                .as_object()?;
            ("codex", servers)
        }
        "antigravity.json" => ("antigravity", object.get("mcpServers")?.as_object()?),
        _ => return None,
    };

    let servers = servers_object
        .iter()
        .map(|(name, value)| {
            let server = value.as_object()?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            let command = server.get("command")?.as_str()?.trim().to_string();
            if command.is_empty() {
                return None;
            }
            let args = match server.get("args") {
                None => Vec::new(),
                Some(args) => args
                    .as_array()?
                    .iter()
                    .map(|arg| Some(arg.as_str()?.to_string()))
                    .collect::<Option<Vec<_>>>()?,
            };
            Some(AgentMcpServer {
                name: name.to_string(),
                command,
                args,
                line: json_key_line(content, name).unwrap_or(1),
            })
        })
        .collect::<Option<Vec<_>>>()?;

    if servers.is_empty() {
        return None;
    }

    Some(AgentMcpConfig { tool, servers })
}

#[cfg(test)]
mod tests {
    use athanor_core::{Extractor, SourceFile};
    use athanor_domain::{EntityKind, FactKind, RepoId, SnapshotId};

    use super::*;
    use crate::OperationsExtractor;

    #[test]
    fn recognizes_only_first_party_agent_mcp_config_paths() {
        assert!(is_agent_mcp_config_path(".codex.json"));
        assert!(is_agent_mcp_config_path("antigravity.json"));
        assert!(!is_agent_mcp_config_path("scripts/.codex.json"));
        assert!(!is_agent_mcp_config_path("config/antigravity.json"));
        assert!(!is_agent_mcp_config_path("codex.json"));
        assert!(!is_agent_mcp_config_path(".mcp.json"));
        assert!(!is_agent_mcp_config_path("claude.json"));
    }

    #[test]
    fn parses_bounded_codex_and_antigravity_schemas() {
        let codex = parse_agent_mcp_config(
            ".codex.json",
            "{\n  \"mcp\": {\n    \"servers\": {\n      \"athanor\": {\n        \"command\": \"cargo\",\n        \"args\": [\"run\", \"--bin\", \"ath\"]\n      }\n    }\n  }\n}\n",
        )
        .unwrap();
        assert_eq!(codex.tool, "codex");
        assert_eq!(codex.servers.len(), 1);
        assert_eq!(codex.servers[0].name, "athanor");
        assert_eq!(codex.servers[0].command, "cargo");
        assert_eq!(codex.servers[0].args, vec!["run", "--bin", "ath"]);
        assert_eq!(codex.servers[0].line, 4);

        let antigravity = parse_agent_mcp_config(
            "antigravity.json",
            "{\n  \"mcpServers\": {\n    \"athanor\": {\n      \"command\": \"cargo\"\n    }\n  }\n}\n",
        )
        .unwrap();
        assert_eq!(antigravity.tool, "antigravity");
        assert_eq!(antigravity.servers.len(), 1);
        assert_eq!(antigravity.servers[0].name, "athanor");
        assert!(antigravity.servers[0].args.is_empty());
        assert_eq!(antigravity.servers[0].line, 3);
    }

    #[test]
    fn rejects_unsupported_or_malformed_configs_atomically() {
        // A Codex config must nest servers under `mcp.servers`.
        assert!(parse_agent_mcp_config(".codex.json", "{\"mcpServers\": {}}").is_none());
        // An empty servers map has nothing first-party to project.
        assert!(parse_agent_mcp_config(".codex.json", "{\"mcp\": {\"servers\": {}}}").is_none());
        // A server without a command is rejected atomically.
        assert!(
            parse_agent_mcp_config(
                ".codex.json",
                "{\"mcp\": {\"servers\": {\"athanor\": {\"args\": []}}}}"
            )
            .is_none()
        );
        // Mixed supported and unsupported servers are rejected atomically.
        assert!(
            parse_agent_mcp_config(
                ".codex.json",
                "{\"mcp\": {\"servers\": {\"ok\": {\"command\": \"cargo\"}, \"bad\": {}}}}"
            )
            .is_none()
        );
        // Non-string arguments are rejected atomically.
        assert!(
            parse_agent_mcp_config(
                ".codex.json",
                "{\"mcp\": {\"servers\": {\"athanor\": {\"command\": \"cargo\", \"args\": [1]}}}}"
            )
            .is_none()
        );
        // Blank server names are rejected.
        assert!(
            parse_agent_mcp_config(
                ".codex.json",
                "{\"mcp\": {\"servers\": {\" \": {\"command\": \"cargo\"}}}}"
            )
            .is_none()
        );
        // Unsupported file names never project.
        assert!(parse_agent_mcp_config("codex.json", "{\"mcpServers\": {}}").is_none());
        // Invalid JSON never projects.
        assert!(parse_agent_mcp_config(".codex.json", "not json").is_none());
    }

    #[tokio::test]
    async fn projects_first_party_codex_mcp_server_with_evidence() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: ".codex.json".to_string(),
                    language_hint: Some("json".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(include_str!("../../../.codex.json").to_string()),
                },
            })
            .await
            .unwrap();

        let commands = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ScriptCommand)
            .collect::<Vec<_>>();
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0].stable_key.0,
            "script-command://.codex.json#agent-mcp-server:athanor"
        );
        assert_eq!(
            commands[0].payload["command_kind"],
            json!("agent_mcp_server")
        );
        assert_eq!(commands[0].payload["agent_tool"], json!("codex"));
        assert_eq!(commands[0].payload["server"], json!("athanor"));
        assert_eq!(commands[0].payload["command"], json!("cargo"));
        assert_eq!(
            commands[0].payload["args"],
            json!(["run", "--bin", "ath", "--quiet", "--", "mcp", "."])
        );

        let defined_facts = output
            .facts
            .iter()
            .filter(|fact| fact.kind == FactKind::SymbolDefined)
            .collect::<Vec<_>>();
        assert_eq!(defined_facts.len(), 1);
        assert_eq!(
            defined_facts[0].value["source_kind"],
            json!("agent_mcp_config")
        );
        assert!(!defined_facts[0].evidence.is_empty());
        assert!(!defined_facts[0].ownership.is_empty());
    }

    #[tokio::test]
    async fn projects_first_party_antigravity_mcp_server_with_evidence() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "antigravity.json".to_string(),
                    language_hint: Some("json".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(include_str!("../../../antigravity.json").to_string()),
                },
            })
            .await
            .unwrap();

        let commands = output
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ScriptCommand)
            .collect::<Vec<_>>();
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0].stable_key.0,
            "script-command://antigravity.json#agent-mcp-server:athanor"
        );
        assert_eq!(
            commands[0].payload["command_kind"],
            json!("agent_mcp_server")
        );
        assert_eq!(commands[0].payload["agent_tool"], json!("antigravity"));
        assert_eq!(commands[0].payload["server"], json!("athanor"));
        assert_eq!(commands[0].payload["command"], json!("cargo"));
        assert_eq!(
            commands[0].payload["args"],
            json!(["run", "--bin", "ath", "--quiet", "--", "mcp", "."])
        );
        let language = commands[0].language.as_ref().map(|code| code.0.as_str());
        assert_eq!(language, Some("json"));

        let defined_facts = output
            .facts
            .iter()
            .filter(|fact| fact.kind == FactKind::SymbolDefined)
            .collect::<Vec<_>>();
        assert_eq!(defined_facts.len(), 1);
        assert_eq!(defined_facts[0].value["agent_tool"], json!("antigravity"));
        assert!(!defined_facts[0].evidence.is_empty());
        assert!(!defined_facts[0].ownership.is_empty());
    }

    #[tokio::test]
    async fn does_not_project_unrelated_json_files() {
        let output = OperationsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "package.json".to_string(),
                    language_hint: Some("json".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        "{\"mcpServers\": {\"athanor\": {\"command\": \"cargo\"}}}".to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.is_empty());
        assert!(output.facts.is_empty());
    }
}
