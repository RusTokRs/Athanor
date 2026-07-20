use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, bail};
use athanor_app::RuntimeComposition;
use athanor_core::CancellationHandle;
use serde_json::{Map, Value, json};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinSet;

use crate::transport_contract::JSON_RPC_VERSION;

pub const DEFAULT_MAX_IN_FLIGHT_REQUESTS: usize = 32;
pub const DEFAULT_RESPONSE_QUEUE_CAPACITY: usize = 32;

pub(super) type ActiveReads = Arc<Mutex<HashMap<String, CancellationHandle>>>;
pub(super) type SessionState = Arc<Mutex<McpSessionPhase>>;
pub(super) type RequestTasks = JoinSet<Result<()>>;

#[derive(Clone)]
pub(super) struct RequestRuntime {
    pub(super) root: Arc<PathBuf>,
    pub(super) composition: Arc<RuntimeComposition>,
    pub(super) active_reads: ActiveReads,
    pub(super) session: SessionState,
    pub(super) responses_tx: mpsc::Sender<String>,
    pub(super) max_in_flight_requests: usize,
}

impl RequestRuntime {
    pub(super) fn new(
        root: Arc<PathBuf>,
        composition: Arc<RuntimeComposition>,
        active_reads: ActiveReads,
        session: SessionState,
        responses_tx: mpsc::Sender<String>,
        max_in_flight_requests: usize,
    ) -> Self {
        Self {
            root,
            composition,
            active_reads,
            session,
            responses_tx,
            max_in_flight_requests,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct McpServerLimits {
    pub(super) max_in_flight_requests: usize,
    pub(super) response_queue_capacity: usize,
}

impl Default for McpServerLimits {
    fn default() -> Self {
        Self {
            max_in_flight_requests: DEFAULT_MAX_IN_FLIGHT_REQUESTS,
            response_queue_capacity: DEFAULT_RESPONSE_QUEUE_CAPACITY,
        }
    }
}

impl McpServerLimits {
    pub(super) fn validate(self) -> Result<Self> {
        if self.max_in_flight_requests == 0 {
            bail!("MCP max in-flight request limit must be greater than zero");
        }
        if self.response_queue_capacity == 0 {
            bail!("MCP response queue capacity must be greater than zero");
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum McpSessionPhase {
    AwaitingInitialize,
    AwaitingInitialized,
    Ready,
}

#[derive(Debug)]
pub(super) enum RequestId {
    Omitted,
    Present(Value),
}

impl RequestId {
    pub(super) fn is_notification(&self) -> bool {
        matches!(self, Self::Omitted)
    }

    pub(super) fn into_value(self) -> Option<Value> {
        match self {
            Self::Omitted => None,
            Self::Present(value) => Some(value),
        }
    }

    pub(super) fn response_value(&self) -> Value {
        match self {
            Self::Omitted => Value::Null,
            Self::Present(value) => value.clone(),
        }
    }
}

#[derive(Debug)]
pub(super) struct JsonRpcRequest {
    pub(super) id: RequestId,
    pub(super) method: String,
    pub(super) params: Option<Value>,
}

#[derive(Debug, Clone)]
pub(super) struct RpcError {
    pub(super) code: i64,
    pub(super) message: String,
    pub(super) data: Option<Value>,
}

impl RpcError {
    pub(super) fn parse(error: &serde_json::Error) -> Self {
        Self {
            code: -32700,
            message: format!("Parse error: {error}"),
            data: None,
        }
    }

    pub(super) fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: message.into(),
            data: None,
        }
    }

    pub(super) fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }

    pub(super) fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    pub(super) fn server_not_initialized() -> Self {
        Self {
            code: -32002,
            message: "Server not initialized".to_string(),
            data: None,
        }
    }

    pub(super) fn server_busy(limit: usize) -> Self {
        Self {
            code: -32001,
            message: "Server busy".to_string(),
            data: Some(json!({
                "reason": "max_in_flight_requests",
                "limit": limit,
                "retryable": true
            })),
        }
    }

    pub(super) fn value(&self) -> Value {
        let mut error = Map::new();
        error.insert("code".to_string(), json!(self.code));
        error.insert("message".to_string(), Value::String(self.message.clone()));
        if let Some(data) = &self.data {
            error.insert("data".to_string(), data.clone());
        }
        Value::Object(error)
    }
}

#[derive(Debug)]
pub(super) struct ProtocolFailure {
    pub(super) id: Value,
    pub(super) error: Box<RpcError>,
}

impl ProtocolFailure {
    pub(super) fn parse(error: &serde_json::Error) -> Self {
        Self {
            id: Value::Null,
            error: Box::new(RpcError::parse(error)),
        }
    }

    pub(super) fn invalid_request(id: Value, message: impl Into<String>) -> Self {
        Self {
            id,
            error: Box::new(RpcError::invalid_request(message)),
        }
    }

    pub(super) fn response(&self) -> String {
        let response = json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": self.id.clone(),
            "error": self.error.value()
        });
        serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"internal MCP tool error"}}"#.to_string()
        })
    }
}

#[derive(Debug)]
pub(super) enum DispatchError {
    Protocol(RpcError),
    Application(anyhow::Error),
}

pub(super) type DispatchResult = std::result::Result<Value, DispatchError>;
