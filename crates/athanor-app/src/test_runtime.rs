use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Result, bail};
use athanor_checker_api::ApiConsistencyChecker;
use athanor_checker_markdown::MarkdownStructureChecker;
use athanor_core::{SearchDocument, SearchIndex};
use athanor_extractor_basic::FileExtractor;
use athanor_extractor_graphql::GraphQlExtractor;
use athanor_extractor_markdown::MarkdownExtractor;
use athanor_extractor_openapi::OpenApiExtractor;
use athanor_extractor_rust::RustExtractor;
use athanor_linker_api::ApiKnowledgeLinker;
use athanor_linker_markdown::MarkdownContainmentLinker;
use athanor_linker_rust::RustLinker;
use athanor_source_fs::LocalFileSystemSource;

use crate::{
    AdapterPluginKind, AdapterRegistry, AthanorStore, ProjectConfig, RuntimeComposition, StorageMode,
};

pub(crate) fn composition() -> RuntimeComposition {
    RuntimeComposition::new(
        default_adapter_registry,
        resolve_builtin_adapter,
        default_store,
        default_search_index,
        default_wiki_projector,
        default_html_projector,
    )
}

fn default_store<'a>(
    root: &'a Path,
    config: &'a ProjectConfig,
) -> Pin<Box<dyn Future<Output = Result<AthanorStore>> + Send + 'a>> {
    Box::pin(async move {
        if config.storage.mode != StorageMode::Jsonl {
            bail!("test runtime supports only JSONL storage");
        }
        Ok(AthanorStore::new(
            athanor_store_jsonl::JsonlKnowledgeStore::new(root.join(&config.storage.path)),
        ))
    })
}

fn default_search_index(
    index_dir: &Path,
    documents: Option<Vec<SearchDocument>>,
) -> Result<Arc<dyn SearchIndex>> {
    let index = if let Some(documents) = documents {
        athanor_search_tantivy::TantivySearchIndex::rebuild(index_dir, documents)?
    } else {
        athanor_search_tantivy::TantivySearchIndex::open_or_create(index_dir)?
    };
    Ok(Arc::new(index))
}

fn default_wiki_projector(
    target: &Path,
    snapshot: &str,
    payload: serde_json::Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    athanor_projector_wiki::project_wiki_payload_cancellable(
        target,
        snapshot,
        serde_json::from_value(payload)?,
        is_cancelled,
    )?;
    Ok(())
}

fn default_html_projector(
    target: &Path,
    snapshot: &str,
    payload: serde_json::Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    athanor_projector_html::project_html_report_payload_cancellable(
        target.to_path_buf(),
        snapshot,
        serde_json::from_value(payload)?,
        is_cancelled,
    )?;
    Ok(())
}

fn default_adapter_registry() -> AdapterRegistry {
    AdapterRegistry::empty()
        .register_source_id("builtin.source.local_filesystem", |root| {
            Box::new(LocalFileSystemSource::new(root))
        })
        .register_extractor_id("builtin.extractor.file", || Box::new(FileExtractor))
        .register_extractor_id("builtin.extractor.markdown", || Box::new(MarkdownExtractor))
        .register_extractor_id("builtin.extractor.openapi", || Box::new(OpenApiExtractor))
        .register_extractor_id("builtin.extractor.graphql", || Box::new(GraphQlExtractor))
        .register_extractor_id("builtin.extractor.rust", || Box::new(RustExtractor))
        .register_linker_id("builtin.linker.markdown_containment", || {
            Box::new(MarkdownContainmentLinker)
        })
        .register_linker_id("builtin.linker.api_knowledge", || {
            Box::new(ApiKnowledgeLinker)
        })
        .register_linker_id("builtin.linker.rust", || Box::new(RustLinker))
        .register_checker_id("builtin.checker.markdown_structure", || {
            Box::new(MarkdownStructureChecker)
        })
        .register_checker_id("builtin.checker.api_consistency", || {
            Box::new(ApiConsistencyChecker)
        })
}

fn resolve_builtin_adapter(
    registry: AdapterRegistry,
    kind: AdapterPluginKind,
    id: &str,
) -> Option<AdapterRegistry> {
    match (kind, id) {
        (AdapterPluginKind::Source, "builtin.source.local_filesystem") => Some(
            registry.register_source_id("builtin.source.local_filesystem", |root| {
                Box::new(LocalFileSystemSource::new(root))
            }),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.file") => Some(
            registry.register_extractor_id("builtin.extractor.file", || Box::new(FileExtractor)),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.graphql") => Some(
            registry
                .register_extractor_id("builtin.extractor.graphql", || Box::new(GraphQlExtractor)),
        ),
        _ => None,
    }
}
