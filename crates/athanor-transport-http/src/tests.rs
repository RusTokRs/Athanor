use super::*;
use crate::http::{head_end, reason_phrase};
use athanor_app::{DocumentationProfile, SearchMode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn target(path: &str) -> request::Target {
    request::parse_target(path).expect("valid target")
}

fn resolve(target: &request::Target) -> Result<Route, ServeFailure> {
    Route::resolve(
        target,
        &DocumentationServeOptions::new(".", "127.0.0.1:0".parse().expect("address")),
    )
}

#[test]
fn resolves_every_documented_route() {
    assert!(matches!(resolve(&target("/")), Ok(Route::Catalog)));
    assert!(matches!(resolve(&target("/healthz")), Ok(Route::Health)));
    assert!(matches!(
        resolve(&target("/api/generations")),
        Ok(Route::ApiCatalog)
    ));
    assert!(matches!(
        resolve(&target("/g/gen-000001/")),
        Ok(Route::Generation(id)) if id == "gen-000001"
    ));
    assert!(matches!(
        resolve(&target("/g/gen-000001/manifest.json")),
        Ok(Route::Manifest(id)) if id == "gen-000001"
    ));
    assert!(matches!(
        resolve(&target("/g/gen-000001/raw/architecture/index.md")),
        Ok(Route::Raw(id, path)) if id == "gen-000001" && path == "architecture/index.md"
    ));
    assert!(matches!(
        resolve(&target("/search?q=store")),
        Ok(Route::Search(query)) if query.query == "store" && query.mode == SearchMode::Lexical
    ));
    assert!(matches!(
        resolve(&target("/api/explore?target=src/lib.rs&depth=3")),
        Ok(Route::ApiExplore(query)) if query.target.as_deref() == Some("src/lib.rs") && query.depth == 3
    ));
    assert!(matches!(
        resolve(&target("/nope")),
        Err(ServeFailure::NotFound(_))
    ));
}

#[test]
fn rejects_search_and_explore_without_input() {
    assert!(matches!(
        resolve(&target("/search?q=%20")),
        Err(ServeFailure::BadRequest(_))
    ));
    assert!(matches!(
        resolve(&target("/api/explore")),
        Err(ServeFailure::BadRequest(_))
    ));
    assert!(matches!(
        resolve(&target("/search?q=x&mode=magic")),
        Err(ServeFailure::BadRequest(_))
    ));
    assert!(matches!(
        resolve(&target("/search?q=x&limit=51")),
        Err(ServeFailure::BadRequest(_))
    ));
    assert!(matches!(
        resolve(&target("/explore?q=x&depth=5")),
        Err(ServeFailure::BadRequest(_))
    ));
}

fn new_options(root: PathBuf, bind: SocketAddr) -> DocumentationServeOptions {
    DocumentationServeOptions::new(root, bind)
}

#[test]
fn refuses_a_non_loopback_bind() {
    let mut options = new_options(
        std::env::temp_dir(),
        "0.0.0.0:8080".parse().expect("address"),
    );
    let error = options.validate().expect_err("non-loopback bind must fail");
    assert!(error.to_string().contains("loopback"));
}

#[test]
fn validates_every_bound() {
    let mut options = new_options(
        std::env::temp_dir(),
        "127.0.0.1:0".parse().expect("address"),
    );
    options.search_limit = 0;
    assert!(options.validate().is_err());

    let mut options = new_options(
        std::env::temp_dir(),
        "127.0.0.1:0".parse().expect("address"),
    );
    options.max_document_bytes = 10;
    assert!(options.validate().is_err());

    let mut options = new_options(
        std::env::temp_dir(),
        "127.0.0.1:0".parse().expect("address"),
    );
    options.max_generations = 0;
    assert!(options.validate().is_err());
}

#[test]
fn finds_the_end_of_a_request_head() {
    assert_eq!(head_end(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"), Some(23));
    assert_eq!(head_end(b"GET / HTTP/1.0\n\n"), Some(14));
    assert_eq!(head_end(b"GET / HTTP/1.1\r\nHost: x"), None);
}

#[test]
fn maps_failures_to_status_and_reason() {
    let failures = [
        (ServeFailure::BadRequest(String::new()), 400, "Bad Request"),
        (ServeFailure::NotFound(String::new()), 404, "Not Found"),
        (
            ServeFailure::TooLarge(String::new()),
            413,
            "Payload Too Large",
        ),
        (
            ServeFailure::Unavailable(String::new()),
            503,
            "Service Unavailable",
        ),
        (
            ServeFailure::Integrity(String::new()),
            500,
            "Internal Server Error",
        ),
        (ServeFailure::Timeout, 504, "Gateway Timeout"),
        (
            ServeFailure::Internal(String::new()),
            500,
            "Internal Server Error",
        ),
    ];
    for (failure, status, reason) in failures {
        assert_eq!(failure.status(), status);
        assert_eq!(reason_phrase(status), reason);
        assert!(!failure.code().is_empty());
    }
    assert!(ServeFailure::Unavailable(String::new()).hint().is_some());
    assert!(ServeFailure::BadRequest(String::new()).hint().is_none());
}

#[tokio::test]
async fn serves_the_catalog_and_refuses_a_foreign_host() {
    let root = std::env::temp_dir().join(format!(
        "athanor-docs-serve-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create fixture root");
    let server = BoundDocumentationServer::bind(
        DocumentationServeOptions::new(&root, "127.0.0.1:0".parse().expect("address")),
        athanor_runtime_defaults::production(),
    )
    .await
    .expect("bind the server");
    let addr = server.addr();
    assert_eq!(server.report().generations, 0);
    let cancellation = CancellationToken::new();
    let serve = tokio::spawn({
        let cancellation = cancellation.clone();
        async move { server.serve(cancellation).await }
    });

    let body = get(addr, "GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(body.starts_with("HTTP/1.1 200 OK"), "{body}");
    assert!(body.contains("Content-Security-Policy: default-src 'none'"));
    assert!(body.ends_with("ok\n"));

    let body = get(addr, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
    assert!(body.starts_with("HTTP/1.1 200 OK"), "{body}");
    assert!(body.contains("No published generations yet"));

    let body = get(
        addr,
        "GET /api/generations HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    assert!(body.starts_with("HTTP/1.1 200 OK"), "{body}");
    assert!(body.contains("athanor.documentation_serve_catalog.v1"));

    let body = get(addr, "GET / HTTP/1.1\r\nHost: evil.example\r\n\r\n").await;
    assert!(body.starts_with("HTTP/1.1 421 "), "{body}");

    let body = get(addr, "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(body.starts_with("HTTP/1.1 405 "), "{body}");
    assert!(body.contains("Allow: GET, HEAD"));

    let body = get(
        addr,
        "GET /g/gen-000404/ HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    assert!(body.starts_with("HTTP/1.1 404 "), "{body}");

    let body = get(
        addr,
        "GET /api/explore?q=x HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    assert!(body.starts_with("HTTP/1.1 503 "), "{body}");
    assert!(body.contains("knowledge_unavailable"));

    cancellation.cancel();
    serve.await.expect("serve task").expect("serve result");
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn answers_head_without_a_body() {
    let root = std::env::temp_dir().join(format!(
        "athanor-docs-serve-head-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create fixture root");
    let server = BoundDocumentationServer::bind(
        DocumentationServeOptions::new(&root, "127.0.0.1:0".parse().expect("address")),
        athanor_runtime_defaults::production(),
    )
    .await
    .expect("bind the server");
    let addr = server.addr();
    let cancellation = CancellationToken::new();
    let serve = tokio::spawn({
        let cancellation = cancellation.clone();
        async move { server.serve(cancellation).await }
    });

    let body = get(addr, "HEAD /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(body.starts_with("HTTP/1.1 200 OK"), "{body}");
    assert!(body.contains("Content-Length: 3\r\n"));
    assert!(body.ends_with("\r\n\r\n"));

    cancellation.cancel();
    serve.await.expect("serve task").expect("serve result");
    std::fs::remove_dir_all(&root).ok();
}

async fn get(addr: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(addr)
        .await
        .expect("connect to the server");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("write the request");
    stream.flush().await.expect("flush the request");
    let mut response = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        stream.take(16 * 1024 * 1024).read_to_end(&mut response),
    )
    .await
    .expect("bounded response deadline")
    .expect("read the response");
    String::from_utf8_lossy(&response).into_owned()
}

#[test]
fn keeps_the_serve_schema_ids_stable() {
    assert_eq!(
        DOCUMENTATION_SERVE_SCHEMA_V1,
        "athanor.documentation_serve.v1"
    );
    assert_eq!(
        DOCUMENTATION_SERVE_ERROR_SCHEMA_V1,
        "athanor.documentation_serve_error.v1"
    );
    assert_eq!(
        catalog::SERVE_CATALOG_SCHEMA_V1,
        "athanor.documentation_serve_catalog.v1"
    );
}

#[test]
fn options_carry_documented_defaults() {
    let options = DocumentationServeOptions::new(".", "127.0.0.1:0".parse().expect("address"));
    assert_eq!(options.search_limit, 10);
    assert_eq!(options.explore_entries, DEFAULT_EXPLORE_ENTRIES);
    assert_eq!(options.explore_depth, DEFAULT_EXPLORE_DEPTH);
    assert_eq!(options.max_generations, 200);
    assert_eq!(options.max_document_bytes, 4 * 1024 * 1024);
}

#[test]
fn documentation_profile_labels_stay_stable() {
    assert_eq!(
        catalog::profile_label(DocumentationProfile::Architecture),
        "architecture"
    );
    assert_eq!(
        catalog::profile_label(DocumentationProfile::Onboarding),
        "onboarding"
    );
}

#[test]
fn configured_query_defaults_are_applied_without_overriding_explicit_limits() {
    let mut options = DocumentationServeOptions::new(".", "127.0.0.1:0".parse().expect("address"));
    options.search_limit = 3;
    options.explore_entries = 4;
    options.explore_depth = 3;
    assert!(
        matches!(Route::resolve(&target("/search?q=x"), &options), Ok(Route::Search(query)) if query.limit == 3)
    );
    assert!(
        matches!(Route::resolve(&target("/search?q=x&limit=2"), &options), Ok(Route::Search(query)) if query.limit == 2)
    );
    assert!(
        matches!(Route::resolve(&target("/explore?q=x"), &options), Ok(Route::Explore(query)) if query.entries == 4 && query.depth == 3)
    );
}

#[tokio::test]
async fn rejects_oversized_heads_even_when_the_final_chunk_has_a_terminator() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let mut client = TcpStream::connect(listener.local_addr().expect("address"))
        .await
        .expect("test client");
    let (mut stream, _) = listener.accept().await.expect("accept test client");
    let head = format!(
        "GET / HTTP/1.1\r\nHost: localhost\r\nX: {}\r\n\r\n",
        "a".repeat(request::MAX_REQUEST_HEAD_BYTES)
    );
    client
        .write_all(head.as_bytes())
        .await
        .expect("write oversized head");
    assert!(matches!(
        read_request_head(&mut stream, Duration::from_secs(1)).await,
        Err(HeadFailure::Oversized)
    ));
}

#[tokio::test]
async fn slow_clients_do_not_block_health_and_cancellation_drains_idle_connections() {
    let root = std::env::temp_dir().join(format!(
        "athanor-docs-idle-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create fixture root");
    let server = BoundDocumentationServer::bind(
        DocumentationServeOptions::new(&root, "127.0.0.1:0".parse().expect("address")),
        athanor_runtime_defaults::production(),
    )
    .await
    .expect("bind reader");
    let addr = server.addr();
    let cancellation = CancellationToken::new();
    let serve = tokio::spawn({
        let cancellation = cancellation.clone();
        async move { server.serve(cancellation).await }
    });
    let mut idle = TcpStream::connect(addr).await.expect("connect idle client");
    idle.write_all(b"GET / HTTP/1.1\r\n")
        .await
        .expect("write incomplete head");
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        get(addr, "GET /healthz HTTP/1.1\r\nHost: localhost\r\n\r\n"),
    )
    .await
    .expect("health is not blocked by the idle client");
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    cancellation.cancel();
    tokio::time::timeout(Duration::from_secs(2), serve)
        .await
        .expect("bounded cancellation")
        .expect("server task")
        .expect("server result");
    assert!(!root.join(".athanor").exists());
    std::fs::remove_dir_all(root).ok();
}
