use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_core::{CoreError, CoreErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{self, AsyncBufReadExt, BufReader};

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

pub async fn run_mcp_server(root: PathBuf) -> Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    eprintln!("Athanor MCP server starting in {}", root.display());

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        match handle_line(&root, &line).await {
            Ok(Some(response)) => {
                println!("{}", response);
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!("Error handling MCP request: {:?}", err);
            }
        }
    }

    Ok(())
}

async fn handle_line(root: &std::path::Path, line: &str) -> Result<Option<String>> {
    let req: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(req) => req,
        Err(err) => {
            let resp = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {}", err),
                    data: None,
                }),
            };
            return Ok(Some(serde_json::to_string(&resp)?));
        }
    };

    let id = req.id.clone();

    if req.id.is_none() {
        return Ok(None);
    }

    let result = handle_request(root, req).await;
    match result {
        Ok(res_val) => {
            let resp = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(res_val),
                error: None,
            };
            Ok(Some(serde_json::to_string(&resp)?))
        }
        Err(err) => {
            let resp = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(json_rpc_error_from_anyhow(&err)),
            };
            Ok(Some(serde_json::to_string(&resp)?))
        }
    }
}

fn json_rpc_error_from_anyhow(error: &anyhow::Error) -> JsonRpcError {
    let core_error = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<CoreError>());
    let (stable_code, retryable) = match core_error {
        Some(error) => (core_error_code_name(error.code()), error.is_retryable()),
        None if error.to_string().contains("deadline") => ("deadline_exceeded", true),
        None if error.to_string().contains("unknown MCP tool")
            || error.to_string().contains("must be") =>
        {
            ("invalid_input", false)
        }
        None => ("internal", false),
    };
    let message = if stable_code == "internal" {
        "internal MCP tool error".to_string()
    } else {
        error.to_string()
    };
    JsonRpcError {
        code: -32603,
        message,
        data: Some(serde_json::json!({
            "code": stable_code,
            "retryable": retryable,
        })),
    }
}

fn core_error_code_name(code: CoreErrorCode) -> &'static str {
    match code {
        CoreErrorCode::NotFound => "not_found",
        CoreErrorCode::InvalidInput => "invalid_input",
        CoreErrorCode::AdapterProtocol => "adapter_protocol",
        CoreErrorCode::AdapterExecution => "adapter_execution",
        CoreErrorCode::SnapshotNotCommitted => "snapshot_not_committed",
        CoreErrorCode::Conflict => "conflict",
        CoreErrorCode::Busy => "busy",
        CoreErrorCode::Cancelled => "cancelled",
        CoreErrorCode::DeadlineExceeded => "deadline_exceeded",
    }
}

async fn handle_request(root: &std::path::Path, req: JsonRpcRequest) -> Result<Value> {
    match req.method.as_str() {
        "initialize" => Ok(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "athanor",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "initialized" => Ok(serde_json::json!({})),
        "tools/list" => Ok(get_tools_list()),
        "tools/call" => {
            let params = req.params.context("missing params for tools/call")?;
            let tool_name = params
                .get("name")
                .and_then(|v| v.as_str())
                .context("missing tool name")?;
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let result = call_tool(root, tool_name, arguments).await;
            match result {
                Ok(content) => Ok(serde_json::json!({
                    "content": [
                        {
                            "type": "text",
                            "text": content
                        }
                    ]
                })),
                Err(err) => Ok(serde_json::json!({
                    "isError": true,
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Error calling tool: {:?}", err)
                        }
                    ]
                })),
            }
        }
        other => {
            bail!("Method not found: {}", other);
        }
    }
}

fn get_tools_list() -> Value {
    let mut response = serde_json::json!({
        "tools": [
            {
                "name": "index",
                "description": "Index project files and export canonical snapshot and read-models.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "validate_only": {
                            "type": "boolean",
                            "description": "Validate adapter contracts without writing snapshot files."
                        }
                    }
                }
            },
            {
                "name": "explain",
                "description": "Explain one canonical entity from the latest snapshot by stable key.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "stable_key": {
                            "type": "string",
                            "description": "Exact canonical stable key, e.g. api://POST:/login"
                        }
                    },
                    "required": ["stable_key"]
                }
            },
            {
                "name": "search",
                "description": "Search the project's knowledge base using BM25 lexical search.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query terms."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of search results (default 10)."
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "context",
                "description": "Build a task-focused context pack from the latest canonical snapshot.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Task description or question to seed selection."
                        },
                        "level": {
                            "type": "string",
                            "enum": ["summary", "normal", "deep", "full"],
                            "description": "Context detail level (default normal)."
                        },
                        "max_tokens": { "type": "integer" },
                        "max_files": { "type": "integer" },
                        "max_entities": { "type": "integer" },
                        "max_diagnostics": { "type": "integer" },
                        "max_depth": { "type": "integer" }
                    },
                    "required": ["task"]
                }
            },
            {
                "name": "impact",
                "description": "Calculate direct and transitive blast radius of changes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Target entity stable key or source file path."
                        },
                        "diff": {
                            "type": "boolean",
                            "description": "Analyze impact of all modified files in the working directory."
                        },
                        "max_depth": {
                            "type": "integer",
                            "description": "Maximum relation traversal depth (default 10)."
                        }
                    }
                }
            },
            {
                "name": "change_map",
                "description": "Build a bounded, evidence-backed map of likely change locations and relation chains.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task": { "type": "string" },
                        "target": { "type": "string" },
                        "diff": { "type": "boolean" },
                        "max_entities": { "type": "integer", "description": "Default 30." },
                        "max_files": { "type": "integer", "description": "Default 20." },
                        "max_diagnostics": { "type": "integer", "description": "Default 20." },
                        "max_depth": { "type": "integer", "description": "Default 3." }
                    }
                }
            },
            {
                "name": "rustok_architecture_context",
                "description": "Resolve a compact RusTok module ownership, public-contract, interaction, test, diagnostic, and evidence context for an implementation intent.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "intent": {
                            "type": "string",
                            "description": "Implementation intent or architecture question."
                        },
                        "module": {
                            "type": "string",
                            "description": "Optional explicit RusTok module anchor."
                        },
                        "max_modules": { "type": "integer", "description": "Default 6." },
                        "max_contracts": { "type": "integer", "description": "Default 16." },
                        "max_interactions": { "type": "integer", "description": "Default 16." },
                        "max_evidence": { "type": "integer", "description": "Default 20." }
                    },
                    "required": ["intent"]
                }
            },
            {
                "name": "check",
                "description": "Show open diagnostics from the latest canonical snapshot.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "scope": {
                            "type": "string",
                            "enum": ["api", "docs", "env", "scripts", "deployment"],
                            "description": "Diagnostic scope to check."
                        }
                    },
                    "required": ["scope"]
                }
            }
        ]
    });
    if let Some(tools) = response.get_mut("tools").and_then(Value::as_array_mut) {
        for tool in tools {
            if let Some(properties) = tool
                .get_mut("inputSchema")
                .and_then(|schema| schema.get_mut("properties"))
                .and_then(Value::as_object_mut)
            {
                properties.insert(
                    "deadline_unix_ms".to_string(),
                    serde_json::json!({
                        "type": "integer",
                        "minimum": 1,
                        "description": "Optional future Unix timestamp in milliseconds; Athanor stops awaiting this tool when it expires."
                    }),
                );
            }
        }
    }
    response
}

async fn call_tool(root: &std::path::Path, name: &str, args: Value) -> Result<String> {
    let deadline_unix_ms = args
        .get("deadline_unix_ms")
        .map(|value| {
            value
                .as_u64()
                .context("MCP tool deadline_unix_ms must be an unsigned integer")
        })
        .transpose()?;
    let Some(deadline_unix_ms) = deadline_unix_ms else {
        return call_tool_inner(root, name, args).await;
    };
    let now_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    if deadline_unix_ms <= now_unix_ms {
        bail!("MCP tool deadline_unix_ms must be in the future");
    }
    tokio::time::timeout(
        Duration::from_millis(deadline_unix_ms.saturating_sub(now_unix_ms)),
        call_tool_inner(root, name, args),
    )
    .await
    .map_err(|_| anyhow::anyhow!("MCP tool deadline exceeded"))?
}

async fn call_tool_inner(root: &std::path::Path, name: &str, args: Value) -> Result<String> {
    match name {
        "index" => {
            let validate_only = args
                .get("validate_only")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let report = athanor_app::index_project(athanor_app::IndexOptions {
                root: root.to_path_buf(),
                validation_report: None,
                validation_result: None,
                validate_only,
            })
            .await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "explain" => {
            let stable_key = args
                .get("stable_key")
                .and_then(|v| v.as_str())
                .context("missing stable_key")?
                .to_string();
            let report = athanor_app::explain_project(athanor_app::ExplainOptions {
                root: root.to_path_buf(),
                stable_key,
            })
            .await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "search" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .context("missing query")?
                .to_string();
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(10);
            let report = athanor_app::search_project(athanor_app::SearchOptions {
                root: root.to_path_buf(),
                query,
                limit,
            })
            .await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "context" => {
            let task = args
                .get("task")
                .and_then(|v| v.as_str())
                .context("missing task")?
                .to_string();
            let level_str = args
                .get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("normal");
            let level = match level_str {
                "summary" => athanor_domain::ContextLevel::Summary,
                "deep" => athanor_domain::ContextLevel::Deep,
                "full" => athanor_domain::ContextLevel::Full,
                _ => athanor_domain::ContextLevel::Normal,
            };
            let max_tokens = args
                .get("max_tokens")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let max_files = args
                .get("max_files")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let max_entities = args
                .get("max_entities")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let max_diagnostics = args
                .get("max_diagnostics")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let max_depth = args
                .get("max_depth")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            let report = athanor_app::context_project(athanor_app::ContextOptions {
                root: root.to_path_buf(),
                task,
                diff: false,
                level,
                limits: athanor_app::ContextLimitOverrides {
                    max_tokens,
                    max_files,
                    max_entities,
                    max_diagnostics,
                    max_depth,
                },
            })
            .await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "impact" => {
            let target = args
                .get("target")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let diff = args.get("diff").and_then(|v| v.as_bool()).unwrap_or(false);
            let max_depth = args
                .get("max_depth")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(10);

            let report = athanor_app::impact_project(athanor_app::ImpactOptions {
                root: root.to_path_buf(),
                target,
                diff,
                max_depth,
            })
            .await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "change_map" => {
            let usize_arg = |name: &str, default: usize| {
                args.get(name)
                    .and_then(|value| value.as_u64())
                    .map(|value| value as usize)
                    .unwrap_or(default)
            };
            let report = athanor_app::change_map_project(athanor_app::ChangeMapOptions {
                root: root.to_path_buf(),
                task: args
                    .get("task")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                target: args
                    .get("target")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                diff: args
                    .get("diff")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                max_entities: usize_arg("max_entities", 30),
                max_files: usize_arg("max_files", 20),
                max_diagnostics: usize_arg("max_diagnostics", 20),
                max_depth: usize_arg("max_depth", 3),
            })
            .await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "rustok_architecture_context" => {
            let intent = args
                .get("intent")
                .and_then(|value| value.as_str())
                .context("missing intent")?
                .to_string();
            let mut options = athanor_app::RustokArchitectureContextOptions::bounded(
                root.to_path_buf(),
                intent,
                args.get("module")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
            );
            options.max_modules = usize_arg(&args, "max_modules", 6);
            options.max_contracts = usize_arg(&args, "max_contracts", 16);
            options.max_interactions = usize_arg(&args, "max_interactions", 16);
            options.max_evidence = usize_arg(&args, "max_evidence", 20);
            let report = athanor_app::rustok_architecture_context(options).await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "check" => {
            let scope_str = args
                .get("scope")
                .and_then(|v| v.as_str())
                .context("missing scope")?;
            let scope = match scope_str {
                "docs" => athanor_app::DiagnosticScope::Docs,
                "env" => athanor_app::DiagnosticScope::Env,
                "scripts" => athanor_app::DiagnosticScope::Scripts,
                "deployment" => athanor_app::DiagnosticScope::Deployment,
                "runbooks" => athanor_app::DiagnosticScope::Runbooks,
                _ => athanor_app::DiagnosticScope::Api,
            };

            let report = athanor_app::check_project(athanor_app::DiagnosticCheckOptions {
                root: root.to_path_buf(),
                scope,
            })
            .await?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        other => bail!("Unknown tool name: {}", other),
    }
}

fn usize_arg(args: &Value, name: &str, default: usize) -> usize {
    args.get(name)
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_mcp_initialize() {
        let root = std::env::temp_dir();

        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }
        });

        let req_str = serde_json::to_string(&request).unwrap();
        let resp_str = handle_line(&root, &req_str).await.unwrap().unwrap();
        let resp: Value = serde_json::from_str(&resp_str).unwrap();

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(resp["result"]["serverInfo"]["name"], "athanor");
    }

    #[tokio::test]
    async fn test_mcp_tools_list() {
        let root = std::env::temp_dir();

        let request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let req_str = serde_json::to_string(&request).unwrap();
        let resp_str = handle_line(&root, &req_str).await.unwrap().unwrap();
        let resp: Value = serde_json::from_str(&resp_str).unwrap();

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 2);

        let tools = resp["result"]["tools"].as_array().unwrap();
        let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(tool_names.contains(&"search"));
        assert!(tool_names.contains(&"explain"));
        assert!(tool_names.contains(&"context"));
        assert!(tool_names.contains(&"impact"));
        assert!(tool_names.contains(&"change_map"));
        assert!(tool_names.contains(&"rustok_architecture_context"));
        assert!(tool_names.contains(&"check"));
        assert!(tool_names.contains(&"index"));
        assert!(tools.iter().all(|tool| {
            tool["inputSchema"]["properties"]
                .get("deadline_unix_ms")
                .is_some()
        }));
    }

    #[tokio::test]
    async fn expired_mcp_tool_deadline_is_rejected_before_execution() {
        let error = call_tool(
            std::path::Path::new("."),
            "search",
            json!({"query": "login", "deadline_unix_ms": 0}),
        )
        .await
        .expect_err("expired deadline must fail before the tool starts");

        assert!(error.to_string().contains("must be in the future"));
    }

    #[tokio::test]
    async fn malformed_mcp_tool_deadline_is_rejected_before_execution() {
        let error = call_tool(
            std::path::Path::new("."),
            "search",
            json!({"query": "login", "deadline_unix_ms": "tomorrow"}),
        )
        .await
        .expect_err("non-integer deadline must fail before the tool starts");

        assert!(error.to_string().contains("must be an unsigned integer"));
    }

    #[test]
    fn exposes_stable_core_error_details_in_json_rpc_data() {
        let error = anyhow::Error::new(CoreError::Busy("index is running".to_string()));
        let rpc = json_rpc_error_from_anyhow(&error);

        assert_eq!(rpc.code, -32603);
        assert_eq!(
            rpc.data.unwrap(),
            json!({"code": "busy", "retryable": true})
        );
    }

    #[test]
    fn does_not_expose_internal_error_text() {
        let error = anyhow::anyhow!("failed to read C:/private/token");
        let rpc = json_rpc_error_from_anyhow(&error);

        assert_eq!(rpc.message, "internal MCP tool error");
        assert_eq!(
            rpc.data.unwrap(),
            json!({"code": "internal", "retryable": false})
        );
    }

    #[tokio::test]
    async fn test_mcp_invalid_json() {
        let root = std::env::temp_dir();
        let resp_str = handle_line(&root, "invalid json").await.unwrap().unwrap();
        let resp: Value = serde_json::from_str(&resp_str).unwrap();

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], serde_json::Value::Null);
        assert_eq!(resp["error"]["code"], -32700);
    }
}
