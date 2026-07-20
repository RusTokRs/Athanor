//! Default runtime composition for Athanor.

mod projector_operation;

use athanor_adapter_rustok_fba::{RustokFbaChecker, RustokFbaExtractor, RustokFbaLinker};
use athanor_adapter_rustok_ffa::{RustokFfaChecker, RustokFfaExtractor, RustokFfaLinker};
use athanor_adapter_rustok_page_builder::{
    RustokPageBuilderChecker, RustokPageBuilderExtractor, RustokPageBuilderLinker,
};
use athanor_app::{
    AdapterPluginKind, AdapterRegistry, AthanorStore, ProjectConfig, RuntimeComposition,
    StorageMode,
};
use athanor_checker_api::{
    ApiConsistencyChecker, DeploymentDocsChecker, EnvDocsChecker, RunbookConsistencyChecker,
    ScriptDocsChecker,
};
use athanor_checker_markdown::MarkdownStructureChecker;
use athanor_core::{OperationContext, SearchDocument, SearchIndex};
use athanor_extractor_basic::FileExtractor;
use athanor_extractor_graphql::GraphQlExtractor;
use athanor_extractor_js_ts::JsTsExtractor;
use athanor_extractor_markdown::MarkdownExtractor;
use athanor_extractor_openapi::OpenApiExtractor;
use athanor_extractor_operations::OperationsExtractor;
use athanor_extractor_rust::RustExtractor;
use athanor_linker_api::ApiKnowledgeLinker;
use athanor_linker_js_ts::JsTsImportLinker;
use athanor_linker_markdown::MarkdownContainmentLinker;
use athanor_linker_rust::RustLinker;
use athanor_source_fs::LocalFileSystemSource;

use crate::projector_operation::publish_projector_output_cancellable;

/// Builds the standard Athanor runtime without mutating process-global state.
pub fn production() -> RuntimeComposition {
    RuntimeComposition::new(
        default_adapter_registry,
        resolve_builtin_adapter,
        default_store,
        default_search_index,
        default_wiki_projector,
        default_html_projector,
    )
    .with_search_index_operation_factory(default_search_index_with_operation_context)
}

fn default_wiki_projector(
    target: &std::path::Path,
    snapshot: &str,
    payload: serde_json::Value,
    is_cancelled: &dyn Fn() -> bool,
) -> anyhow::Result<()> {
    let payload = serde_json::from_value(payload)?;
    publish_projector_output_cancellable(target, "wiki", is_cancelled, |staging| {
        athanor_projector_wiki::project_wiki_payload_cancellable(
            staging,
            snapshot,
            payload,
            is_cancelled,
        )?;
        Ok(())
    })
}

fn default_html_projector(
    target: &std::path::Path,
    snapshot: &str,
    payload: serde_json::Value,
    is_cancelled: &dyn Fn() -> bool,
) -> anyhow::Result<()> {
    let payload = serde_json::from_value(payload)?;
    publish_projector_output_cancellable(target, "HTML report", is_cancelled, |staging| {
        athanor_projector_html::project_html_report_payload_cancellable(
            staging.to_path_buf(),
            snapshot,
            payload,
            is_cancelled,
        )?;
        Ok(())
    })
}

fn default_search_index(
    index_dir: &std::path::Path,
    documents: Option<Vec<SearchDocument>>,
) -> anyhow::Result<std::sync::Arc<dyn SearchIndex>> {
    let index = if let Some(documents) = documents {
        athanor_search_tantivy::TantivySearchIndex::rebuild(index_dir, documents)?
    } else {
        athanor_search_tantivy::TantivySearchIndex::open_or_create(index_dir)?
    };
    Ok(std::sync::Arc::new(index))
}

fn default_search_index_with_operation_context(
    index_dir: &std::path::Path,
    documents: Option<Vec<SearchDocument>>,
    operation: &OperationContext,
) -> anyhow::Result<std::sync::Arc<dyn SearchIndex>> {
    let index = if let Some(documents) = documents {
        athanor_search_tantivy::TantivySearchIndex::rebuild_with_operation_context(
            index_dir, documents, operation,
        )?
    } else {
        athanor_search_tantivy::TantivySearchIndex::open_or_create(index_dir)?
    };
    Ok(std::sync::Arc::new(index))
}

fn default_store<'a>(
    root: &'a std::path::Path,
    config: &'a ProjectConfig,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<AthanorStore>> + Send + 'a>>
{
    Box::pin(async move {
        match config.storage.mode {
            StorageMode::Jsonl => {
                let path = root.join(&config.storage.path);
                Ok(AthanorStore::new_with_latest_pointer(
                    athanor_store_jsonl::JsonlKnowledgeStore::new(path),
                ))
            }
            StorageMode::SurrealEmbedded => {
                #[cfg(feature = "store-surreal")]
                {
                    let path = root.join(&config.storage.path);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let uri = format!("surrealkv://{}", path.to_string_lossy());
                    let store = athanor_store_surrealdb::SurrealKnowledgeStore::connect(&uri)
                        .await
                        .map_err(|e| anyhow::anyhow!("failed to connect to SurrealDB: {}", e))?;
                    Ok(AthanorStore::new_with_latest_pointer(store))
                }
                #[cfg(not(feature = "store-surreal"))]
                {
                    anyhow::bail!("SurrealDB support is not compiled in this build of Athanor")
                }
            }
            StorageMode::SurrealMemory => {
                #[cfg(feature = "store-surreal")]
                {
                    let store = athanor_store_surrealdb::SurrealKnowledgeStore::connect("mem://")
                        .await
                        .map_err(|e| {
                            anyhow::anyhow!("failed to connect to SurrealDB in-memory: {}", e)
                        })?;
                    Ok(AthanorStore::new_with_latest_pointer(store))
                }
                #[cfg(not(feature = "store-surreal"))]
                {
                    anyhow::bail!("SurrealDB support is not compiled in this build of Athanor")
                }
            }
        }
    })
}

pub fn default_adapter_registry() -> AdapterRegistry {
    AdapterRegistry::empty()
        .register_source_id("builtin.source.local_filesystem", |root| {
            Box::new(LocalFileSystemSource::new(root))
        })
        .register_extractor_id("builtin.extractor.file", || Box::new(FileExtractor))
        .register_extractor_id("builtin.extractor.markdown", || Box::new(MarkdownExtractor))
        .register_extractor_id("builtin.extractor.openapi", || Box::new(OpenApiExtractor))
        .register_extractor_id("builtin.extractor.graphql", || Box::new(GraphQlExtractor))
        .register_extractor_id("builtin.extractor.operations", || {
            Box::new(OperationsExtractor)
        })
        .register_extractor_id("builtin.extractor.js_ts", || Box::new(JsTsExtractor))
        .register_extractor_id("builtin.extractor.rust", || Box::new(RustExtractor))
        .register_linker_id("builtin.linker.markdown_containment", || {
            Box::new(MarkdownContainmentLinker)
        })
        .register_linker_id("builtin.linker.api_knowledge", || {
            Box::new(ApiKnowledgeLinker)
        })
        .register_linker_id("builtin.linker.js_ts_imports", || {
            Box::new(JsTsImportLinker)
        })
        .register_linker_id("builtin.linker.rust", || Box::new(RustLinker))
        .register_checker_id("builtin.checker.markdown_structure", || {
            Box::new(MarkdownStructureChecker)
        })
        .register_checker_id("builtin.checker.api_consistency", || {
            Box::new(ApiConsistencyChecker)
        })
        .register_checker_id("builtin.checker.env_docs", || Box::new(EnvDocsChecker))
        .register_checker_id("builtin.checker.script_docs", || {
            Box::new(ScriptDocsChecker)
        })
        .register_checker_id("builtin.checker.deployment_docs", || {
            Box::new(DeploymentDocsChecker)
        })
        .register_checker_id("builtin.checker.runbook_consistency", || {
            Box::new(RunbookConsistencyChecker)
        })
}

pub fn resolve_builtin_adapter(
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
        (AdapterPluginKind::Extractor, "builtin.extractor.markdown") => Some(
            registry.register_extractor_id("builtin.extractor.markdown", || {
                Box::new(MarkdownExtractor)
            }),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.openapi") => Some(
            registry
                .register_extractor_id("builtin.extractor.openapi", || Box::new(OpenApiExtractor)),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.graphql") => Some(
            registry
                .register_extractor_id("builtin.extractor.graphql", || Box::new(GraphQlExtractor)),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.operations") => Some(
            registry.register_extractor_id("builtin.extractor.operations", || {
                Box::new(OperationsExtractor)
            }),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.js_ts") => Some(
            registry.register_extractor_id("builtin.extractor.js_ts", || Box::new(JsTsExtractor)),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.rust") => Some(
            registry.register_extractor_id("builtin.extractor.rust", || Box::new(RustExtractor)),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.rustok_ffa") => Some(
            registry.register_extractor_id("builtin.extractor.rustok_ffa", || {
                Box::new(RustokFfaExtractor)
            }),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.rustok_fba") => Some(
            registry.register_extractor_id("builtin.extractor.rustok_fba", || {
                Box::new(RustokFbaExtractor)
            }),
        ),
        (AdapterPluginKind::Extractor, "builtin.extractor.rustok_page_builder") => Some(
            registry.register_extractor_id("builtin.extractor.rustok_page_builder", || {
                Box::new(RustokPageBuilderExtractor)
            }),
        ),
        (AdapterPluginKind::Linker, "builtin.linker.markdown_containment") => Some(
            registry.register_linker_id("builtin.linker.markdown_containment", || {
                Box::new(MarkdownContainmentLinker)
            }),
        ),
        (AdapterPluginKind::Linker, "builtin.linker.api_knowledge") => Some(
            registry.register_linker_id("builtin.linker.api_knowledge", || {
                Box::new(ApiKnowledgeLinker)
            }),
        ),
        (AdapterPluginKind::Linker, "builtin.linker.js_ts_imports") => Some(
            registry.register_linker_id("builtin.linker.js_ts_imports", || {
                Box::new(JsTsImportLinker)
            }),
        ),
        (AdapterPluginKind::Linker, "builtin.linker.rust") => {
            Some(registry.register_linker_id("builtin.linker.rust", || Box::new(RustLinker)))
        }
        (AdapterPluginKind::Linker, "builtin.linker.rustok_ffa") => Some(
            registry.register_linker_id("builtin.linker.rustok_ffa", || Box::new(RustokFfaLinker)),
        ),
        (AdapterPluginKind::Linker, "builtin.linker.rustok_fba") => Some(
            registry.register_linker_id("builtin.linker.rustok_fba", || Box::new(RustokFbaLinker)),
        ),
        (AdapterPluginKind::Linker, "builtin.linker.rustok_page_builder") => Some(
            registry.register_linker_id("builtin.linker.rustok_page_builder", || {
                Box::new(RustokPageBuilderLinker)
            }),
        ),
        (AdapterPluginKind::Checker, "builtin.checker.markdown_structure") => Some(
            registry.register_checker_id("builtin.checker.markdown_structure", || {
                Box::new(MarkdownStructureChecker)
            }),
        ),
        (AdapterPluginKind::Checker, "builtin.checker.api_consistency") => Some(
            registry.register_checker_id("builtin.checker.api_consistency", || {
                Box::new(ApiConsistencyChecker)
            }),
        ),
        (AdapterPluginKind::Checker, "builtin.checker.env_docs") => Some(
            registry.register_checker_id("builtin.checker.env_docs", || Box::new(EnvDocsChecker)),
        ),
        (AdapterPluginKind::Checker, "builtin.checker.script_docs") => Some(
            registry.register_checker_id("builtin.checker.script_docs", || {
                Box::new(ScriptDocsChecker)
            }),
        ),
        (AdapterPluginKind::Checker, "builtin.checker.deployment_docs") => Some(
            registry.register_checker_id("builtin.checker.deployment_docs", || {
                Box::new(DeploymentDocsChecker)
            }),
        ),
        (AdapterPluginKind::Checker, "builtin.checker.runbook_consistency") => Some(
            registry.register_checker_id("builtin.checker.runbook_consistency", || {
                Box::new(RunbookConsistencyChecker)
            }),
        ),
        (AdapterPluginKind::Checker, "builtin.checker.rustok_ffa") => Some(
            registry
                .register_checker_id("builtin.checker.rustok_ffa", || Box::new(RustokFfaChecker)),
        ),
        (AdapterPluginKind::Checker, "builtin.checker.rustok_fba") => Some(
            registry
                .register_checker_id("builtin.checker.rustok_fba", || Box::new(RustokFbaChecker)),
        ),
        (AdapterPluginKind::Checker, "builtin.checker.rustok_page_builder") => Some(
            registry.register_checker_id("builtin.checker.rustok_page_builder", || {
                Box::new(RustokPageBuilderChecker)
            }),
        ),
        _ => None,
    }
}
