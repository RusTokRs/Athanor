use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn serve_help_exposes_loopback_and_resource_bounds() {
    let output = run(&["docs", "serve", "--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = String::from_utf8_lossy(&output.stdout);
    for flag in [
        "--bind",
        "--json",
        "--search-limit",
        "--explore-entries",
        "--explore-depth",
        "--max-generations",
        "--max-document-bytes",
        "--request-timeout-secs",
    ] {
        assert!(text.contains(flag), "missing {flag}: {text}");
    }
}

#[test]
fn refuses_non_loopback_and_invalid_bounds_before_creating_runtime_state() {
    let fixture = Fixture::new("invalid");
    for (flag, value, expected) in [
        ("--bind", "0.0.0.0:0", "loopback"),
        ("--search-limit", "0", "search_limit"),
        ("--max-generations", "0", "max_generations"),
        ("--max-document-bytes", "1", "max_document_bytes"),
        ("--request-timeout-secs", "0", "request_timeout"),
    ] {
        let output = run(&["docs", "serve", fixture.arg(), flag, value, "--json"]);
        assert!(!output.status.success(), "{flag} {value} was accepted");
        assert!(stderr(&output).contains(expected), "{}", stderr(&output));
        assert!(!fixture.root.join(".athanor").exists());
    }
}

#[test]
fn empty_projects_serve_without_initialization_and_report_missing_knowledge() {
    let fixture = Fixture::new("empty");
    let server = Server::start(&fixture.root, &[]);
    assert_eq!(server.report["schema"], "athanor.documentation_serve.v1");
    assert_eq!(server.report["generations"], 0);
    assert_eq!(server.report["max_connections"], 8);
    let index = server.get("/");
    assert_eq!(index.status, 200);
    assert!(index.body.contains("No published generations yet"));
    let catalog = server.json("/api/generations");
    assert_eq!(catalog["schema"], "athanor.documentation_serve_catalog.v1");
    assert_eq!(catalog["returned"], 0);
    assert_eq!(catalog["limits"]["max_generations"], 200);
    for path in ["/api/search?q=execute", "/api/explore?q=execute"] {
        let response = server.get(path);
        assert_eq!(response.status, 503, "{}", response.body);
        let error: Value = serde_json::from_str(&response.body).expect("error JSON");
        assert_eq!(error["schema"], "athanor.documentation_serve_error.v1");
        assert_eq!(error["code"], "knowledge_unavailable");
        assert!(error["hint"].as_str().expect("hint").contains("ath index"));
    }
    // Even failed queries must not initialize a canonical store or documentation publication.
    assert!(!fixture.root.join(".athanor").exists());
}

#[test]
fn published_profiles_refresh_without_restart_and_queries_do_not_mutate_canonical_data() {
    let fixture = Fixture::new("published");
    let indexed = json_command(&["index", fixture.arg(), "--json"]);
    let snapshot = indexed["snapshot"].as_str().expect("snapshot id");
    let architecture = generate(&fixture, "architecture", snapshot);
    let architecture_id = architecture["generation"].as_str().expect("generation id");
    let server = Server::start(
        &fixture.root,
        &[
            "--search-limit",
            "3",
            "--explore-entries",
            "4",
            "--explore-depth",
            "3",
        ],
    );
    assert_eq!(server.report["generations"], 1);
    assert_eq!(server.json("/api/generations")["current"], architecture_id);

    let generation_path = format!("/g/{architecture_id}/");
    let generation = server.get(&generation_path);
    assert_eq!(generation.status, 200, "{}", generation.body);
    assert!(generation.body.contains("architecture documentation"));
    assert!(generation.body.contains("Evidence and citations"));
    assert!(generation.body.contains(snapshot));
    assert!(!generation.body.contains("<script"));
    assert!(
        generation
            .head
            .contains("Content-Security-Policy: default-src 'none'")
    );
    assert!(generation.head.contains("X-Content-Type-Options: nosniff"));
    let manifest = server.json(&format!("/g/{architecture_id}/manifest.json"));
    assert_eq!(manifest["snapshot"], snapshot);
    let raw = server.get(&format!("/g/{architecture_id}/raw/architecture/index.md"));
    assert_eq!(raw.status, 200);
    assert!(
        raw.head
            .contains("Content-Type: text/markdown; charset=utf-8")
    );
    assert_eq!(
        raw.body,
        fs::read_to_string(architecture["document"].as_str().expect("document path"))
            .expect("published Markdown")
    );

    // All profiles share one current.json. The reader must retain earlier profiles, not only the
    // last pointer, and observe external atomic publications without a server restart.
    for profile in ["module", "api", "operations", "onboarding"] {
        generate(&fixture, profile, snapshot);
    }
    let catalog = server.json("/api/generations");
    assert_eq!(catalog["returned"], 5);
    let generations = catalog["generations"].as_array().expect("generations");
    assert_eq!(generations[0]["profile"], "onboarding");
    assert_eq!(
        generations
            .iter()
            .filter(|generation| generation["current"].as_bool() == Some(true))
            .count(),
        1
    );
    for profile in ["architecture", "module", "api", "operations", "onboarding"] {
        let generation = generations
            .iter()
            .find(|generation| generation["profile"] == profile)
            .expect("profile remains visible");
        assert_eq!(generation["validation"]["status"], "valid");
    }

    assert_live_queries_read_only(&server, &fixture, snapshot);

    let head = server.request("HEAD", "/healthz", "localhost", "");
    assert_eq!(head.status, 200);
    assert!(head.head.contains("Content-Length: 3"));
    assert!(head.body.is_empty());
    let missing = server.get(&format!("/g/{architecture_id}/raw/not-declared.txt"));
    assert_eq!(missing.status, 404);
    let traversal = server.get(&format!("/g/{architecture_id}/raw/%2e%2e/current.json"));
    assert_eq!(traversal.status, 400);

    // An external edit is not trusted merely because the manifest was previously loadable.
    fs::write(
        architecture["document"].as_str().expect("document path"),
        "<script>untrusted-body</script>\n",
    )
    .expect("tamper with document");
    let tampered = server.get(&format!("/g/{architecture_id}/raw/architecture/index.md"));
    assert_eq!(tampered.status, 500, "{}", tampered.body);
    assert!(tampered.body.contains("checksum"));
    assert!(!tampered.body.contains("untrusted-body"));
    let page = server.get(&generation_path);
    assert!(page.body.contains("could not be served"));
    assert!(!page.body.contains("untrusted-body"));
}

#[test]
fn rejects_cross_host_mutations_and_bad_query_inputs() {
    let fixture = Fixture::new("http-errors");
    let server = Server::start(&fixture.root, &[]);
    assert_eq!(server.request("GET", "/", "evil.example", "").status, 421);
    assert_eq!(
        server
            .request("GET", "/", "localhost", "Host: evil.example\r\n")
            .status,
        400
    );
    let post = server.request("POST", "/api/generations", "localhost", "");
    assert_eq!(post.status, 405);
    assert!(post.head.contains("Allow: GET, HEAD"));
    let error: Value = serde_json::from_str(&post.body).expect("method error JSON");
    assert_eq!(error["code"], "method_not_allowed");
    for path in [
        "/api/search?q=",
        "/api/search?q=x&mode=remote",
        "/api/search?q=x&limit=51",
        "/api/explore?target=x&depth=5",
    ] {
        let response = server.get(path);
        assert_eq!(response.status, 400, "{path}: {}", response.body);
        let error: Value = serde_json::from_str(&response.body).expect("query error JSON");
        assert_eq!(error["schema"], "athanor.documentation_serve_error.v1");
    }
    assert!(!fixture.root.join(".athanor").exists());
}

fn assert_live_queries_read_only(server: &Server, fixture: &Fixture, snapshot: &str) {
    let canonical = fixture.root.join(".athanor/store/canonical");
    let documentation = fixture.root.join(".athanor/generated/documentation");
    let canonical_before = tree_bytes(&canonical);
    let documentation_before = tree_bytes(&documentation);
    for mode in ["lexical", "semantic", "hybrid"] {
        let search = server.json(&format!("/api/search?q=execute&mode={mode}"));
        assert_eq!(search["schema"], "athanor.search.v1");
        assert_eq!(search["snapshot"], snapshot);
        assert_eq!(search["mode"], mode);
        assert_eq!(search["limit"], 3);
        assert!(search["returned"].as_u64().expect("returned") <= 3);
        assert_eq!(
            server.get(&format!("/search?q=execute&mode={mode}")).status,
            200
        );
    }
    let explore = server.json("/api/explore?target=src%2Flib.rs");
    assert_eq!(explore["schema"], "athanor.explore.v1");
    assert_eq!(explore["snapshot"], snapshot);
    assert_eq!(explore["limits"]["entries"], 4);
    assert_eq!(explore["limits"]["depth"], 3);
    let entries = explore["entries"].as_array().expect("entries");
    assert!(!entries.is_empty());
    assert!(
        entries.iter().any(|entry| entry["excerpt"].is_object()),
        "{explore}"
    );
    assert!(explore["paths"].is_array());
    assert!(explore["blast"].is_object());
    let page = server.get("/explore?target=src%2Flib.rs");
    assert_eq!(page.status, 200, "{}", page.body);
    assert!(page.body.contains("Blast radius"));
    assert!(
        page.body
            .contains("/api/explore?q=&amp;target=src%2Flib.rs")
    );
    let retrieved = server.json("/api/explore?q=execute&mode=hybrid&depth=2");
    assert_eq!(retrieved["limits"]["depth"], 2);
    assert_eq!(retrieved["mode"], "hybrid");
    assert_eq!(
        tree_bytes(&canonical),
        canonical_before,
        "HTTP queries changed canonical state"
    );
    assert_eq!(
        tree_bytes(&documentation),
        documentation_before,
        "HTTP queries changed published documents"
    );
}

struct Server {
    child: Child,
    addr: SocketAddr,
    report: Value,
}

impl Server {
    fn start(root: &Path, extra: &[&str]) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_ath"))
            .args([
                "docs",
                "serve",
                root.to_str().expect("UTF-8 fixture root"),
                "--bind",
                "127.0.0.1:0",
                "--json",
            ])
            .args(extra)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start documentation server");
        let mut server = Self {
            child,
            addr: "127.0.0.1:0".parse().expect("placeholder bind"),
            report: Value::Null,
        };
        let stdout = server.child.stdout.take().expect("server stdout");
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
            let _ = send.send(result);
        });
        let line = receive
            .recv_timeout(Duration::from_secs(30))
            .expect("bounded server start-up")
            .expect("read start-up report");
        server.report = serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("invalid start-up JSON: {error}: {line}"));
        server.addr = server.report["addr"]
            .as_str()
            .expect("bound address")
            .parse()
            .expect("socket address");
        assert!(server.addr.ip().is_loopback());
        assert_ne!(server.addr.port(), 0);
        assert_eq!(server.report["url"], format!("http://{}/", server.addr));
        server
    }

    fn request(&self, method: &str, path: &str, host: &str, extra_headers: &str) -> HttpResponse {
        let mut stream = TcpStream::connect_timeout(&self.addr, Duration::from_secs(5))
            .expect("connect to reader");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("set read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("set write timeout");
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: {host}\r\n{extra_headers}\r\n"
        )
        .expect("send request");
        let mut bytes = Vec::new();
        stream
            .take(16 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .expect("bounded HTTP response");
        let text = String::from_utf8(bytes).expect("UTF-8 HTTP response");
        let (head, body) = text.split_once("\r\n\r\n").expect("HTTP response framing");
        let status = head
            .split_whitespace()
            .nth(1)
            .expect("HTTP status")
            .parse()
            .expect("numeric status");
        HttpResponse {
            status,
            head: head.to_string(),
            body: body.to_string(),
        }
    }

    fn get(&self, path: &str) -> HttpResponse {
        self.request("GET", path, "localhost", "")
    }

    fn json(&self, path: &str) -> Value {
        let response = self.get(path);
        assert_eq!(response.status, 200, "{path}: {}", response.body);
        serde_json::from_str(&response.body).expect("HTTP JSON body")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct HttpResponse {
    status: u16,
    head: String,
    body: String,
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "athanor-docs-serve-cli-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("src")).expect("create fixture source");
        fs::write(root.join("src/lib.rs"), "pub fn execute() -> u8 { helper() }\nfn helper() -> u8 { value() }\nfn value() -> u8 { 1 }\n").expect("write Rust fixture");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"serve-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("write package fixture");
        fs::write(root.join("README.md"), "---\nid: doc://README.md\nkind: project_overview\nlanguage: en\nsource_language: en\nstatus: active\n---\n# Fixture service\n\nThe service exposes `execute`.\n").expect("write docs fixture");
        fs::write(root.join("Makefile"), "check:\n\tcargo test\n")
            .expect("write operations fixture");
        Self { root }
    }
    fn arg(&self) -> &str {
        self.root.to_str().expect("UTF-8 fixture path")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn generate(fixture: &Fixture, profile: &str, snapshot: &str) -> Value {
    json_command(&[
        "docs",
        &format!("generate-{profile}"),
        fixture.arg(),
        "--snapshot",
        snapshot,
        "--json",
    ])
}

fn tree_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).expect("read fixture tree") {
            let path = entry.expect("fixture entry").path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root)
                        .expect("relative path")
                        .to_path_buf(),
                    fs::read(path).expect("read fixture artifact"),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn json_command(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{}: {}",
        args.join(" "),
        stderr(&output)
    );
    serde_json::from_slice(&output.stdout).expect("CLI JSON")
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
