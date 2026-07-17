use serde_json::{Value, json};

pub(super) fn list() -> Value {
    let mut response = json!({
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
                        "query": { "type": "string" },
                        "limit": { "type": "integer", "description": "Default 10." }
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
                        "task": { "type": "string" },
                        "level": {
                            "type": "string",
                            "enum": ["summary", "normal", "deep", "full"]
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
                        "target": { "type": "string" },
                        "diff": { "type": "boolean" },
                        "max_depth": { "type": "integer", "description": "Default 10." }
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
                "description": "Resolve compact RusTok architecture context for an implementation intent.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "intent": { "type": "string" },
                        "module": { "type": "string" },
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
                            "enum": ["api", "docs", "env", "scripts", "deployment", "runbooks"]
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
                    json!({
                        "type": "integer",
                        "minimum": 1,
                        "description": "Optional future Unix timestamp in milliseconds."
                    }),
                );
            }
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_list_has_deadline_on_every_tool() {
        let tools = list()["tools"].as_array().unwrap().clone();
        assert_eq!(tools.len(), 8);
        assert!(tools.iter().all(|tool| {
            tool["inputSchema"]["properties"]
                .get("deadline_unix_ms")
                .is_some()
        }));
    }
}
