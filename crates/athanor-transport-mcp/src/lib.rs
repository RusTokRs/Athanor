use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
                error: Some(JsonRpcError {
                    code: -32603,
                    message: err.to_string(),
                    data: None,
                }),
            };
            Ok(Some(serde_json::to_string(&resp)?))
        }
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
    serde_json::json!({
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
                "name": "check",
                "description": "Show open diagnostics from the latest canonical snapshot.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "scope": {
                            "type": "string",
                            "enum": ["api", "docs"],
                            "description": "Diagnostic scope to check."
                        }
                    },
                    "required": ["scope"]
                }
            }
        ]
    })
}

async fn call_tool(root: &std::path::Path, name: &str, args: Value) -> Result<String> {
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
        "check" => {
            let scope_str = args
                .get("scope")
                .and_then(|v| v.as_str())
                .context("missing scope")?;
            let scope = match scope_str {
                "docs" => athanor_app::DiagnosticScope::Docs,
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
        assert!(tool_names.contains(&"check"));
        assert!(tool_names.contains(&"index"));
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
