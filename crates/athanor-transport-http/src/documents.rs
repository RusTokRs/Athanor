//! Blocking, bounded publication reads for the documentation routes.

use athanor_core::{OperationContext, OperationContextCancellation};

use super::DocumentationServeOptions;
use super::catalog::Catalog;
use super::http::Response;
use super::render::{self, DocumentView};
use super::routes::{Route, ServeFailure, failure_from_catalog_error, failure_from_service};

/// Aggregate Markdown source budget for one generation page, independent of the raw file limit.
const MAX_GENERATION_SOURCE_BYTES: u64 = 8 * 1024 * 1024;

pub(super) fn load_catalog(
    options: &DocumentationServeOptions,
    operation: Option<&OperationContext>,
) -> Result<Catalog, ServeFailure> {
    let catalog = match operation {
        Some(operation) => {
            Catalog::load_for_operation(&options.root, options.catalog_limits(), operation)
        }
        None => Catalog::load(&options.root, options.catalog_limits()),
    };
    catalog.map_err(|error| {
        failure_from_service(error.context("failed to read the documentation catalog"))
    })
}

pub(super) fn respond(
    route: Route,
    options: &DocumentationServeOptions,
    operation: &OperationContext,
) -> Result<Response, ServeFailure> {
    check_active(operation)?;
    let response = match route {
        Route::Health => Response::text(200, "ok"),
        Route::Catalog => {
            let catalog = load_catalog(options, Some(operation))?;
            Response::html(
                200,
                render::index_page(
                    &catalog.report(),
                    options.search_limit,
                    options.explore_depth,
                ),
            )
        }
        Route::ApiCatalog => {
            Response::json(200, &load_catalog(options, Some(operation))?.report())?
        }
        Route::Generation(id) => generation_page(&id, options, operation)?,
        Route::Manifest(id) => {
            let body = load_catalog(options, Some(operation))?
                .manifest_json(&id)
                .map_err(failure_from_catalog_error)?;
            Response {
                status: 200,
                content_type: "application/json; charset=utf-8".to_string(),
                body: body.into_bytes(),
                allow: Vec::new(),
            }
        }
        Route::Raw(id, path) => {
            let served = load_catalog(options, Some(operation))?
                .read_document(&id, &path, options.max_document_bytes)
                .map_err(failure_from_catalog_error)?;
            Response {
                status: 200,
                content_type: served.media_type,
                body: served.bytes,
                allow: Vec::new(),
            }
        }
        _ => {
            return Err(ServeFailure::Internal(
                "a query route was passed to the document reader".to_string(),
            ));
        }
    };
    check_active(operation)?;
    Ok(response)
}

fn generation_page(
    id: &str,
    options: &DocumentationServeOptions,
    operation: &OperationContext,
) -> Result<Response, ServeFailure> {
    let catalog = load_catalog(options, Some(operation))?;
    let generation = catalog.generation(id).ok_or_else(|| {
        ServeFailure::NotFound(format!("unknown documentation generation `{id}`"))
    })?;
    let mut views = Vec::with_capacity(generation.documents.len());
    let mut remaining = MAX_GENERATION_SOURCE_BYTES;
    for document in &generation.documents {
        check_active(operation)?;
        let mut view = DocumentView {
            report: document,
            html: None,
            error: None,
        };
        if document.markdown {
            if remaining == 0 {
                view.error = Some(
                    "the generation page's 8 MiB source budget is exhausted; use the raw link"
                        .to_string(),
                );
            } else {
                match catalog.read_document(
                    id,
                    &document.path,
                    options.max_document_bytes.min(remaining),
                ) {
                    Ok(served) => {
                        remaining = remaining.saturating_sub(served.bytes.len() as u64);
                        match String::from_utf8(served.bytes) {
                            Ok(source) => view.html = Some(render::markdown_to_html(&source)),
                            Err(_) => {
                                view.error =
                                    Some("the document body is not valid UTF-8".to_string())
                            }
                        }
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
            }
        }
        views.push(view);
    }
    check_active(operation)?;
    Ok(Response::html(
        200,
        render::generation_page(&catalog, generation, &views),
    ))
}

fn check_active(operation: &OperationContext) -> Result<(), ServeFailure> {
    operation
        .check_active()
        .map_err(|error| failure_from_service(error.into()))
}
