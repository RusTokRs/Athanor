//! Exercise freshness through one real, long-lived stdio MCP process.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[test]
fn long_lived_mcp_reads_follow_external_index_commits() {
    let fixture = Fixture::new("external");
    fixture.write_symbol("freshness_alpha");
    let first = fixture.index();
    let mut session = McpSession::start(&fixture.root);
    assert_eq!(
        session.tool("overview", json!({"top": 3}))["snapshot"],
        first
    );
    let first_search = session.tool("search", json!({"query": "freshness", "limit": 10}));
    let old_key = symbol_key(&first_search, "freshness_alpha");
    assert_eq!(
        session.tool("context", context_args())["payload"]["snapshot"],
        first
    );

    // A source edit alone is not an index publication. No automatic indexing is implied.
    fixture.write_symbol("freshness_beta");
    assert_eq!(
        session.tool("overview", json!({"top": 3}))["snapshot"],
        first
    );
    let updated = fixture.index();
    assert_ne!(first, updated);
    assert_eq!(
        session.tool("overview", json!({"top": 3}))["snapshot"],
        updated
    );
    for mode in ["lexical", "semantic", "hybrid"] {
        let report = session.tool(
            "search",
            json!({"query": "freshness", "limit": 10, "mode": mode}),
        );
        assert_eq!(report["schema"], "athanor.search.v1");
        assert_eq!(report["snapshot"], updated);
        assert_eq!(report["mode"], mode);
        assert!(
            !report["results"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["name"] == "freshness_alpha")
        );
    }
    let search = session.tool("search", json!({"query": "freshness", "limit": 10}));
    let new_key = symbol_key(&search, "freshness_beta");
    assert_eq!(
        session.tool("explain", json!({"stable_key": new_key}))["snapshot"],
        updated
    );
    assert_eq!(
        session.tool("context", context_args())["payload"]["snapshot"],
        updated
    );
    let explore = session.tool(
        "explore",
        json!({"target": "src/lib.rs", "entries": 4, "depth": 1}),
    );
    assert_eq!(explore["schema"], "athanor.explore.v1");
    assert_eq!(explore["snapshot"], updated);
    assert!(!explore["entries"].as_array().unwrap().is_empty());
    assert_eq!(
        session.call_tool("explain", json!({"stable_key": old_key}))["isError"],
        true
    );
}

#[test]
fn warmed_mcp_session_reports_unavailable_latest_without_stale_fallback() {
    let fixture = Fixture::new("unavailable");
    fixture.write_symbol("freshness_alpha");
    let first = fixture.index();
    let mut session = McpSession::start(&fixture.root);
    assert_eq!(
        session.tool("overview", json!({"top": 3}))["snapshot"],
        first
    );
    let pointer = fixture
        .root
        .join(".athanor/store/canonical/jsonl/latest.json");
    let valid = fs::read(&pointer).expect("published latest pointer");
    fs::remove_file(&pointer).unwrap();
    assert_eq!(
        session.call_tool("overview", json!({"top": 3}))["isError"],
        true
    );
    assert!(!pointer.exists(), "read must not recreate latest");
    fs::write(&pointer, b"{broken").unwrap();
    assert_eq!(
        session.call_tool("overview", json!({"top": 3}))["isError"],
        true
    );
    assert_eq!(fs::read(&pointer).unwrap(), b"{broken");
    fs::write(&pointer, &valid).unwrap();
    assert_eq!(
        session.tool("overview", json!({"top": 3}))["snapshot"],
        first
    );
    fixture.write_symbol("freshness_beta");
    let updated = fixture.index();
    assert_eq!(
        session.tool("overview", json!({"top": 3}))["snapshot"],
        updated
    );
}

fn context_args() -> Value {
    json!({"task": "freshness", "max_entities": 4, "max_files": 3, "max_tokens": 1000})
}

fn symbol_key(report: &Value, name: &str) -> String {
    report["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
        .unwrap_or_else(|| panic!("missing {name}: {report}"))
        .get("stable_key")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

struct McpSession {
    child: Child,
    stdin: Option<ChildStdin>,
    responses: Option<Receiver<Value>>,
    reader: Option<JoinHandle<()>>,
    next_id: u64,
}

impl McpSession {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ath"))
            .arg("mcp")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start MCP CLI");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, receiver) = mpsc::sync_channel(2);
        let reader = thread::spawn(move || {
            let mut stream = BufReader::new(stdout);
            loop {
                let mut line = Vec::new();
                let bytes = (&mut stream)
                    .take(MAX_RESPONSE_BYTES + 1)
                    .read_until(b'\n', &mut line)
                    .expect("read bounded MCP response");
                if bytes == 0 {
                    break;
                }
                assert!(
                    bytes as u64 <= MAX_RESPONSE_BYTES,
                    "MCP response exceeded fixture byte budget"
                );
                let response = serde_json::from_slice(&line).expect("one JSON document per line");
                if sender.send(response).is_err() {
                    break;
                }
            }
        });
        let mut session = Self {
            child,
            stdin: Some(stdin),
            responses: Some(receiver),
            reader: Some(reader),
            next_id: 1,
        };
        let initialized = session.request(
            "initialize",
            json!({
                "protocolVersion": athanor_transport_mcp::MCP_PROTOCOL_VERSION,
                "capabilities": {}, "clientInfo": {"name": "freshness-regression", "version": "1"}
            }),
        );
        assert_eq!(
            initialized["result"]["protocolVersion"],
            athanor_transport_mcp::MCP_PROTOCOL_VERSION
        );
        session.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        session
    }

    fn send(&mut self, request: Value) {
        let input = self.stdin.as_mut().expect("open MCP stdin");
        writeln!(input, "{request}").expect("write MCP request");
        input.flush().expect("flush MCP request");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let response = self
            .responses
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_secs(30))
            .expect("MCP response before fixture deadline");
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], id);
        assert!(response.get("error").is_none(), "{response}");
        response
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.request("tools/call", json!({"name": name, "arguments": arguments}));
        athanor_transport_mcp::validate_tool_call_result(&response["result"])
            .expect("MCP tool result contract");
        response["result"].clone()
    }

    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let result = self.call_tool(name, arguments);
        assert_ne!(result["isError"], true, "{result}");
        serde_json::from_str(
            result["content"][0]["text"]
                .as_str()
                .expect("text tool result"),
        )
        .expect("application JSON report")
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        // Disconnect the bounded reader channel before joining, including on assertion failures.
        self.responses.take();
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "athanor-mcp-freshness-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"mcp-freshness-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n").unwrap();
        Self { root }
    }

    fn write_symbol(&self, symbol: &str) {
        fs::write(
            self.root.join("src/lib.rs"),
            format!("pub fn {symbol}() -> u8 {{ 1 }}\n"),
        )
        .unwrap();
    }

    fn index(&self) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_ath"))
            .arg("index")
            .arg(&self.root)
            .arg("--json")
            .output()
            .expect("external index process");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).expect("index report");
        report["snapshot"]
            .as_str()
            .expect("committed snapshot id")
            .to_string()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
