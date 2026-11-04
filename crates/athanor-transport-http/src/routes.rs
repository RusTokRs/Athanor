//! Route resolution, bounded query inputs, and stable error categories.

use athanor_app::{MAX_EXPLORE_DEPTH, SearchMode};
use athanor_core::CoreError;

use super::{DocumentationServeOptions, MAX_SEARCH_LIMIT, catalog::CatalogError, request};

pub(super) fn failure_from_catalog_error(error: CatalogError) -> ServeFailure {
    match error {
        CatalogError::NotFound(message) => ServeFailure::NotFound(message),
        CatalogError::TooLarge(message) => ServeFailure::TooLarge(message),
        CatalogError::Unreadable(message) => ServeFailure::Internal(message),
        CatalogError::ChecksumMismatch(message) => ServeFailure::Integrity(message),
    }
}

pub(super) fn failure_from_service(error: anyhow::Error) -> ServeFailure {
    if error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::DeadlineExceeded(_))
        )
    }) {
        return ServeFailure::Timeout;
    }
    let message = format!("{error:#}");
    if message.contains("no canonical snapshot") || message.contains("run `ath index`") {
        return ServeFailure::Unavailable(
            "this project has no canonical snapshot the server can answer from".to_string(),
        );
    }
    if message.contains("could not resolve target") {
        return ServeFailure::NotFound(message);
    }
    if message.contains("must not be empty") {
        return ServeFailure::BadRequest(message);
    }
    ServeFailure::Internal(message)
}

/// One bounded search request.
#[derive(Debug, Clone)]
pub(super) struct SearchQuery {
    pub(super) query: String,
    pub(super) mode: SearchMode,
    pub(super) limit: usize,
}

/// One bounded explore request.
#[derive(Debug, Clone)]
pub(super) struct ExploreQuery {
    pub(super) query: String,
    pub(super) target: Option<String>,
    pub(super) mode: SearchMode,
    pub(super) depth: usize,
    pub(super) entries: usize,
}

/// Resolved route of one accepted request.
#[derive(Debug, Clone)]
pub(super) enum Route {
    Health,
    Catalog,
    ApiCatalog,
    Generation(String),
    Manifest(String),
    Raw(String, String),
    Search(SearchQuery),
    ApiSearch(SearchQuery),
    Explore(ExploreQuery),
    ApiExplore(ExploreQuery),
}

impl Route {
    pub(super) fn resolve(
        target: &request::Target,
        defaults: &DocumentationServeOptions,
    ) -> Result<Self, ServeFailure> {
        let segments = target.segments.as_slice();
        match segments {
            [] => Ok(Route::Catalog),
            [health] if health.as_str() == "healthz" => Ok(Route::Health),
            [api, catalog] if api.as_str() == "api" && catalog.as_str() == "generations" => {
                Ok(Route::ApiCatalog)
            }
            [api, search] if api.as_str() == "api" && search.as_str() == "search" => {
                Ok(Route::ApiSearch(search_query(&target.query, defaults)?))
            }
            [api, explore] if api.as_str() == "api" && explore.as_str() == "explore" => {
                Ok(Route::ApiExplore(explore_query(&target.query, defaults)?))
            }
            [search] if search.as_str() == "search" => {
                Ok(Route::Search(search_query(&target.query, defaults)?))
            }
            [explore] if explore.as_str() == "explore" => {
                Ok(Route::Explore(explore_query(&target.query, defaults)?))
            }
            [generations, id] if generations.as_str() == "g" => Ok(Route::Generation(id.clone())),
            [generations, id, manifest]
                if generations.as_str() == "g" && manifest.as_str() == "manifest.json" =>
            {
                Ok(Route::Manifest(id.clone()))
            }
            [generations, id, raw, path @ ..]
                if generations.as_str() == "g" && raw.as_str() == "raw" && !path.is_empty() =>
            {
                Ok(Route::Raw(id.clone(), path.join("/")))
            }
            _ => Err(ServeFailure::NotFound(
                "no documentation route matches this path".to_string(),
            )),
        }
    }
}

fn search_query(
    query: &request::Query,
    defaults: &DocumentationServeOptions,
) -> Result<SearchQuery, ServeFailure> {
    let text = query.get("q").unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err(ServeFailure::BadRequest(
            "a search request needs a non-empty `q` parameter".to_string(),
        ));
    }
    let mode = parse_mode(query)?;
    let limit = match query.get("limit") {
        Some(value) => parse_bounded(value, 1, MAX_SEARCH_LIMIT, "limit")?,
        None => defaults.search_limit,
    };
    Ok(SearchQuery {
        query: text,
        mode,
        limit,
    })
}

fn explore_query(
    query: &request::Query,
    defaults: &DocumentationServeOptions,
) -> Result<ExploreQuery, ServeFailure> {
    let text = query.get("q").unwrap_or_default().trim().to_string();
    let target = query
        .get("target")
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(str::to_string);
    if text.is_empty() && target.is_none() {
        return Err(ServeFailure::BadRequest(
            "an explore request needs a non-empty `q` or `target` parameter".to_string(),
        ));
    }
    let mode = parse_mode(query)?;
    let depth = match query.get("depth") {
        Some(value) => parse_bounded(value, 1, MAX_EXPLORE_DEPTH, "depth")?,
        None => defaults.explore_depth,
    };
    Ok(ExploreQuery {
        query: text,
        target,
        mode,
        depth,
        entries: defaults.explore_entries,
    })
}

fn parse_mode(query: &request::Query) -> Result<SearchMode, ServeFailure> {
    match query.get("mode").unwrap_or("lexical") {
        "lexical" => Ok(SearchMode::Lexical),
        "semantic" => Ok(SearchMode::Semantic),
        "hybrid" => Ok(SearchMode::Hybrid),
        other => Err(ServeFailure::BadRequest(format!(
            "unsupported search mode `{other}`; use lexical, semantic, or hybrid"
        ))),
    }
}

fn parse_bounded(
    value: &str,
    minimum: usize,
    maximum: usize,
    name: &str,
) -> Result<usize, ServeFailure> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| ServeFailure::BadRequest(format!("`{name}` must be an unsigned integer")))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(ServeFailure::BadRequest(format!(
            "`{name}` must be between {minimum} and {maximum}"
        )));
    }
    Ok(parsed)
}

/// Why one request could not be answered.
#[derive(Debug, Clone)]
pub(super) enum ServeFailure {
    Rejected(request::RequestRejection),
    BadRequest(String),
    NotFound(String),
    TooLarge(String),
    Unavailable(String),
    Timeout,
    Integrity(String),
    Internal(String),
}

impl ServeFailure {
    pub(super) fn status(&self) -> u16 {
        match self {
            ServeFailure::Rejected(rejection) => rejection.status(),
            ServeFailure::BadRequest(_) => 400,
            ServeFailure::NotFound(_) => 404,
            ServeFailure::TooLarge(_) => 413,
            ServeFailure::Unavailable(_) => 503,
            ServeFailure::Timeout => 504,
            ServeFailure::Integrity(_) => 500,
            ServeFailure::Internal(_) => 500,
        }
    }

    pub(super) fn code(&self) -> &'static str {
        match self {
            ServeFailure::Rejected(rejection) => rejection.code(),
            ServeFailure::BadRequest(_) => "bad_request",
            ServeFailure::NotFound(_) => "not_found",
            ServeFailure::TooLarge(_) => "document_too_large",
            ServeFailure::Unavailable(_) => "knowledge_unavailable",
            ServeFailure::Timeout => "request_timeout",
            ServeFailure::Integrity(_) => "document_integrity_error",
            ServeFailure::Internal(_) => "internal_error",
        }
    }

    pub(super) fn title(&self) -> &'static str {
        match self {
            ServeFailure::Rejected(rejection) => rejection.message(),
            ServeFailure::BadRequest(_) => "Bad request",
            ServeFailure::NotFound(_) => "Not found",
            ServeFailure::TooLarge(_) => "Document too large",
            ServeFailure::Unavailable(_) => "Knowledge unavailable",
            ServeFailure::Timeout => "Request timed out",
            ServeFailure::Integrity(_) => "Document integrity error",
            ServeFailure::Internal(_) => "Internal error",
        }
    }

    pub(super) fn message(&self) -> &str {
        match self {
            ServeFailure::Rejected(rejection) => rejection.message(),
            ServeFailure::Timeout => "the request exceeded the configured operation timeout",
            ServeFailure::BadRequest(message)
            | ServeFailure::NotFound(message)
            | ServeFailure::TooLarge(message)
            | ServeFailure::Unavailable(message)
            | ServeFailure::Integrity(message)
            | ServeFailure::Internal(message) => message,
        }
    }

    pub(super) fn hint(&self) -> Option<&'static str> {
        match self {
            ServeFailure::Unavailable(_) => Some(
                "Run `ath index .` in the served project, then publish a documentation generation.",
            ),
            ServeFailure::Integrity(_) => Some(
                "Republish the generation with `ath docs generate-<profile>`; the server never serves a document whose checksum does not match its manifest.",
            ),
            ServeFailure::Rejected(request::RequestRejection::UnexpectedHost) => Some(
                "Reach the server through http://127.0.0.1:<port>/ or http://localhost:<port>/.",
            ),
            _ => None,
        }
    }
}
