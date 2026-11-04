//! Loopback-only HTTP reader for `ath docs serve`.
//!
//! Published documentation is immutable and checksum-checked before serving. Search and explore
//! delegate to existing application queries: they never index sources or publish canonical data,
//! but can refresh the same derived search cache as `ath search`. No provider or remote asset is
//! used. The HTTP parser, HTML presentation, filesystem confinement, and transport limits belong
//! here rather than in the domain or application crates.

mod catalog;
mod documents;
mod http;
mod markdown;
mod render;
mod request;
mod routes;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use athanor_app::{
    CancellationToken, DEFAULT_EXPLORE_DEPTH, DEFAULT_EXPLORE_ENTRIES, DEFAULT_EXPLORE_LINKS,
    DEFAULT_EXPLORE_PATHS, ExploreOptions, MAX_EXPLORE_DEPTH, MAX_EXPLORE_ENTRIES,
    RuntimeComposition, SearchOptions, explore_project_with_composition_and_operation_context,
    search_project_with_composition_and_operation_context,
};
use athanor_core::{OperationContext, OperationContextCancellation};
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use catalog::CatalogLimits;
use http::{HeadFailure, Response, read_request_head, write_response};
use routes::{ExploreQuery, Route, SearchQuery, ServeFailure, failure_from_service};

/// Stable identifier of the serve start-up report.
pub const DOCUMENTATION_SERVE_SCHEMA_V1: &str = "athanor.documentation_serve.v1";
/// Stable identifier of the serve error payload.
pub const DOCUMENTATION_SERVE_ERROR_SCHEMA_V1: &str = "athanor.documentation_serve_error.v1";
/// Maximum concurrent connections, including slow request heads and response bodies.
const MAX_CONNECTIONS: usize = 8;
/// Largest number of generations one catalog read may inspect.
const MAX_SERVED_GENERATIONS: usize = 1_000;
/// Smallest accepted per-document serve limit.
const MIN_DOCUMENT_BYTES: u64 = 1_024;
/// Largest accepted per-document serve limit.
const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
/// Largest accepted search limit per request.
const MAX_SEARCH_LIMIT: usize = 50;

/// Configuration of one documentation reader. Bounds are validated before binding a socket.
#[derive(Debug, Clone)]
pub struct DocumentationServeOptions {
    /// Existing project directory the served documentation belongs to.
    pub root: PathBuf,
    /// Numeric loopback address to bind; port `0` selects an ephemeral port.
    pub bind: SocketAddr,
    /// Default search limit; request overrides remain bounded to 1–50.
    pub search_limit: usize,
    /// Maximum number of explore entries returned per request.
    pub explore_entries: usize,
    /// Default explore hop bound; request overrides remain bounded to 1–4.
    pub explore_depth: usize,
    /// Number of newest generation directories to inspect, in descending numeric order.
    pub max_generations: usize,
    /// Largest raw document body read; larger files are refused, never truncated.
    pub max_document_bytes: u64,
    /// Separate timeout for a request head, an operation, and a response write, at most 120 seconds.
    pub request_timeout: Duration,
}

impl DocumentationServeOptions {
    /// Builds options with documented defaults, without touching the filesystem.
    pub fn new(root: impl Into<PathBuf>, bind: SocketAddr) -> Self {
        Self {
            root: root.into(),
            bind,
            search_limit: 10,
            explore_entries: DEFAULT_EXPLORE_ENTRIES,
            explore_depth: DEFAULT_EXPLORE_DEPTH,
            max_generations: 200,
            max_document_bytes: 4 * 1024 * 1024,
            request_timeout: Duration::from_secs(10),
        }
    }

    fn validate(&mut self) -> Result<()> {
        if !self.bind.ip().is_loopback() {
            bail!(
                "the documentation server may only bind to a loopback address; `{}` is not loopback",
                self.bind
            );
        }
        if !(1..=MAX_SERVED_GENERATIONS).contains(&self.max_generations) {
            bail!("max_generations must be between 1 and {MAX_SERVED_GENERATIONS}");
        }
        if !(MIN_DOCUMENT_BYTES..=MAX_DOCUMENT_BYTES).contains(&self.max_document_bytes) {
            bail!(
                "max_document_bytes must be between {MIN_DOCUMENT_BYTES} and {MAX_DOCUMENT_BYTES}"
            );
        }
        if !(1..=MAX_SEARCH_LIMIT).contains(&self.search_limit) {
            bail!("search_limit must be between 1 and {MAX_SEARCH_LIMIT}");
        }
        if !(1..=MAX_EXPLORE_ENTRIES).contains(&self.explore_entries) {
            bail!("explore_entries must be between 1 and {MAX_EXPLORE_ENTRIES}");
        }
        if !(1..=MAX_EXPLORE_DEPTH).contains(&self.explore_depth) {
            bail!("explore_depth must be between 1 and {MAX_EXPLORE_DEPTH}");
        }
        if self.request_timeout.is_zero() || self.request_timeout > Duration::from_secs(120) {
            bail!("request_timeout must be greater than zero and at most 120 seconds");
        }
        self.root = self.root.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize the documentation server root {}",
                self.root.display()
            )
        })?;
        if !self.root.is_dir() {
            bail!("the documentation server root must be a directory");
        }
        Ok(())
    }

    fn catalog_limits(&self) -> CatalogLimits {
        CatalogLimits {
            max_generations: self.max_generations,
            max_document_bytes: self.max_document_bytes,
        }
    }
}

/// One start-up report, emitted only after the socket is bound. It is not a live catalog.
#[derive(Debug, Clone, Serialize)]
pub struct DocumentationServeReport {
    /// Versioned JSON contract identifier.
    pub schema: String,
    /// Canonical served project directory.
    pub root: PathBuf,
    /// Actual bound socket address, resolving port `0`.
    pub addr: SocketAddr,
    /// Browser entry point for the bound address.
    pub url: String,
    /// Published documentation directory, which may not exist yet.
    pub documentation_root: PathBuf,
    /// Valid manifests loaded within the start-up catalog bound.
    pub generations: usize,
    /// Rejected generation directories within the start-up bound.
    pub invalid_generations: usize,
    /// Validated current pointer, when its generation is in the catalog.
    pub current: Option<String>,
    /// Hard concurrent connection bound.
    pub max_connections: usize,
}

/// A bound documentation reader with explicitly supplied application composition.
pub struct BoundDocumentationServer {
    listener: TcpListener,
    options: DocumentationServeOptions,
    composition: RuntimeComposition,
    report: DocumentationServeReport,
    requests: AtomicU64,
    // Existing search queries can rebuild a derived index. Serialize them within this reader so
    // concurrent browser requests never rebuild the same cache against one another.
    queries: Mutex<()>,
}

impl BoundDocumentationServer {
    /// Validates options and the bounded catalog before binding. No directories are created.
    pub async fn bind(
        mut options: DocumentationServeOptions,
        composition: RuntimeComposition,
    ) -> Result<Self> {
        options.validate()?;
        let catalog_options = options.clone();
        let catalog =
            tokio::task::spawn_blocking(move || documents::load_catalog(&catalog_options, None))
                .await
                .context("documentation catalog worker terminated unexpectedly")?
                .map_err(|error| anyhow::anyhow!(error.message().to_string()))?;
        let listener = TcpListener::bind(options.bind).await.with_context(|| {
            format!(
                "failed to bind the documentation server to {}",
                options.bind
            )
        })?;
        let addr = listener
            .local_addr()
            .context("failed to read the documentation server address")?;
        let report = DocumentationServeReport {
            schema: DOCUMENTATION_SERVE_SCHEMA_V1.to_string(),
            root: options.root.clone(),
            addr,
            url: format!("http://{addr}/"),
            documentation_root: catalog.documentation_root().to_path_buf(),
            generations: catalog.generations().len(),
            invalid_generations: catalog.invalid().len(),
            current: catalog.current().map(str::to_string),
            max_connections: MAX_CONNECTIONS,
        };
        Ok(Self {
            listener,
            options,
            composition,
            report,
            requests: AtomicU64::new(0),
            queries: Mutex::new(()),
        })
    }

    /// Actual loopback address this server listens on.
    pub fn addr(&self) -> SocketAddr {
        self.report.addr
    }

    /// Returns the immutable start-up report; `/api/generations` reads a fresh catalog.
    pub fn report(&self) -> DocumentationServeReport {
        self.report.clone()
    }

    /// Accepts at most eight connections concurrently, then drains active work on cancellation.
    pub async fn serve(self, cancellation: CancellationToken) -> Result<()> {
        let server = Arc::new(self);
        let mut connections = JoinSet::new();
        let mut result = loop {
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => break Ok(()),
                completed = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = completed {
                        break Err(anyhow::anyhow!("documentation connection worker terminated unexpectedly: {error}"));
                    }
                }
                accepted = server.listener.accept(), if connections.len() < MAX_CONNECTIONS => {
                    match accepted {
                        Ok((stream, peer)) if peer.ip().is_loopback() => {
                            let server = Arc::clone(&server);
                            let cancellation = cancellation.clone();
                            connections.spawn(async move { server.handle_connection(stream, &cancellation).await });
                        }
                        Ok(_) => {}
                        Err(error) => break Err(error).context("failed to accept a documentation connection"),
                    }
                }
            }
        };
        cancellation.cancel();
        while let Some(completed) = connections.join_next().await {
            if let Err(error) = completed
                && result.is_ok()
            {
                result =
                    Err(error).context("documentation connection worker terminated while draining");
            }
        }
        result
    }

    async fn handle_connection(&self, mut stream: TcpStream, cancellation: &CancellationToken) {
        let head = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return,
            head = read_request_head(&mut stream, self.options.request_timeout) => head,
        };
        let head = match head {
            Ok(head) => head,
            Err(failure @ (HeadFailure::Oversized | HeadFailure::Malformed)) => {
                let rejection = if matches!(failure, HeadFailure::Oversized) {
                    request::RequestRejection::Oversized
                } else {
                    request::RequestRejection::Malformed
                };
                let response = self.rejection_response(false, rejection);
                self.send_response(&mut stream, request::Method::Get, &response, cancellation)
                    .await;
                return;
            }
            Err(HeadFailure::Timeout | HeadFailure::Io) => return,
        };
        let method_hint = if head.starts_with("HEAD ") {
            request::Method::Head
        } else {
            request::Method::Get
        };
        let api_hint = head
            .split(' ')
            .nth(1)
            .is_some_and(|target| target.starts_with("/api/"));
        let request = match request::parse(&head) {
            Ok(request) => request,
            Err(rejection) => {
                let response = self.rejection_response(api_hint, rejection);
                self.send_response(&mut stream, method_hint, &response, cancellation)
                    .await;
                return;
            }
        };
        if request
            .host
            .as_deref()
            .is_some_and(|host| !request::host_is_loopback(host))
        {
            let response =
                self.rejection_response(api_hint, request::RequestRejection::UnexpectedHost);
            self.send_response(&mut stream, request.method, &response, cancellation)
                .await;
            return;
        }
        let response = self.respond(&request.target, cancellation).await;
        self.send_response(&mut stream, request.method, &response, cancellation)
            .await;
    }

    async fn send_response(
        &self,
        stream: &mut TcpStream,
        method: request::Method,
        response: &Response,
        cancellation: &CancellationToken,
    ) {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {}
            _ = tokio::time::timeout(self.options.request_timeout, write_response(stream, method, response)) => {}
        }
    }

    async fn respond(&self, raw_target: &str, cancellation: &CancellationToken) -> Response {
        let target = match request::parse_target(raw_target) {
            Ok(target) => target,
            Err(rejection) => {
                return self.rejection_response(raw_target.starts_with("/api/"), rejection);
            }
        };
        let api = target.segments.first().map(String::as_str) == Some("api");
        let route = match Route::resolve(&target, &self.options) {
            Ok(route) => route,
            Err(failure) => return self.failure_response(api, &failure),
        };
        let operation = self.request_operation();
        let request_cancellation = CancellationToken::new();
        if let Err(error) = request_cancellation.bind_operation(&operation) {
            return self.failure_response(api, &ServeFailure::Internal(error.to_string()));
        }
        let mut response = Box::pin(self.handle_route(route, &operation));
        let result = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                request_cancellation.cancel();
                response.await
            }
            _ = tokio::time::sleep(self.options.request_timeout) => {
                request_cancellation.cancel();
                let _ = response.await; // Drain operation-aware workers; do not detach them.
                Err(ServeFailure::Timeout)
            }
            result = &mut response => result,
        };
        result.unwrap_or_else(|failure| self.failure_response(api, &failure))
    }

    fn request_operation(&self) -> OperationContext {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let deadline = now.saturating_add(self.options.request_timeout).as_millis();
        OperationContext::new(format!(
            "docs-serve:{}:{}:{}",
            std::process::id(),
            self.addr(),
            self.requests.fetch_add(1, Ordering::Relaxed)
        ))
        .with_deadline_unix_ms(u64::try_from(deadline).unwrap_or(u64::MAX))
    }

    async fn handle_route(
        &self,
        route: Route,
        operation: &OperationContext,
    ) -> Result<Response, ServeFailure> {
        match route {
            Route::Search(query) => Ok(Response::html(
                200,
                render::search_page(&self.search(query, operation).await?),
            )),
            Route::ApiSearch(query) => Response::json(200, &self.search(query, operation).await?),
            Route::Explore(query) => Ok(Response::html(
                200,
                render::explore_page(&self.explore(query, operation).await?),
            )),
            Route::ApiExplore(query) => Response::json(200, &self.explore(query, operation).await?),
            document_route => {
                let options = self.options.clone();
                let operation = operation.clone();
                tokio::task::spawn_blocking(move || {
                    documents::respond(document_route, &options, &operation)
                })
                .await
                .map_err(|error| {
                    ServeFailure::Internal(format!(
                        "documentation reader worker terminated: {error}"
                    ))
                })?
            }
        }
    }

    async fn search(
        &self,
        query: SearchQuery,
        operation: &OperationContext,
    ) -> Result<athanor_app::SearchReport, ServeFailure> {
        let _guard = self.queries.lock().await;
        operation
            .check_active()
            .map_err(|error| failure_from_service(error.into()))?;
        search_project_with_composition_and_operation_context(
            SearchOptions {
                root: self.options.root.clone(),
                query: query.query,
                limit: query.limit,
                mode: query.mode,
            },
            &self.composition,
            operation,
        )
        .await
        .map_err(failure_from_service)
    }

    async fn explore(
        &self,
        query: ExploreQuery,
        operation: &OperationContext,
    ) -> Result<athanor_app::ExploreReport, ServeFailure> {
        let _guard = self.queries.lock().await;
        operation
            .check_active()
            .map_err(|error| failure_from_service(error.into()))?;
        let report = explore_project_with_composition_and_operation_context(
            ExploreOptions {
                root: self.options.root.clone(),
                query: query.query,
                target: query.target,
                entries: query.entries,
                links: DEFAULT_EXPLORE_LINKS,
                depth: query.depth,
                paths: DEFAULT_EXPLORE_PATHS,
                mode: query.mode,
            },
            &self.composition,
            operation,
        )
        .await
        .map_err(failure_from_service)?;
        operation
            .check_active()
            .map_err(|error| failure_from_service(error.into()))?;
        Ok(report)
    }

    fn rejection_response(&self, api: bool, rejection: request::RequestRejection) -> Response {
        let mut response = self.failure_response(api, &ServeFailure::Rejected(rejection));
        if rejection == request::RequestRejection::MethodNotAllowed {
            response.allow = vec!["GET", "HEAD"];
        }
        response
    }

    fn failure_response(&self, api: bool, failure: &ServeFailure) -> Response {
        if api {
            return Response {
                status: failure.status(),
                content_type: "application/json; charset=utf-8".to_string(),
                body: serde_json::json!({
                    "schema": DOCUMENTATION_SERVE_ERROR_SCHEMA_V1,
                    "code": failure.code(), "status": failure.status(),
                    "message": failure.message(), "hint": failure.hint(),
                })
                .to_string()
                .into_bytes(),
                allow: Vec::new(),
            };
        }
        Response::html(
            failure.status(),
            render::error_page(
                failure.status(),
                failure.title(),
                failure.message(),
                failure.hint(),
            ),
        )
    }
}

#[cfg(test)]
mod tests;
