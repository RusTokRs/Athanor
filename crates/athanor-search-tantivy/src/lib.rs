#![allow(clippy::collapsible_if)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use athanor_core::{
    CoreError, CoreResult, OperationContext, OperationContextCancellation, SearchDocument,
    SearchIndex, SearchQuery, SearchResult,
};
use serde_json::Value;
use tantivy::{
    Index, IndexReader, IndexWriter, TantivyDocument, TantivyError,
    collector::TopDocs,
    doc,
    indexer::NoMergePolicy,
    query::QueryParser,
    schema::{
        Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions,
        Value as TantivyValue,
    },
};

const COMMIT_PERMISSION_RETRIES: usize = 3;
const REBUILD_POLL_DOCUMENTS: usize = 256;

pub struct TantivySearchIndex {
    index: Index,
    reader: IndexReader,
    writer: Arc<Mutex<IndexWriter>>,
    id_field: Field,
    title_field: Field,
    body_field: Field,
    payload_field: Field,
}

impl TantivySearchIndex {
    pub fn open_or_create(path: &Path) -> anyhow::Result<Self> {
        let (schema, fields) = search_schema();
        let index = match Index::open_in_dir(path) {
            Ok(index) => index,
            Err(_) => {
                let _ = std::fs::remove_dir_all(path);
                std::fs::create_dir_all(path)?;
                Index::create_in_dir(path, schema.clone())?
            }
        };
        register_tokenizer(&index);
        let writer = index.writer(50_000_000)?;
        writer.set_merge_policy(Box::new(NoMergePolicy));
        let reader = index.reader()?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            id_field: fields.id,
            title_field: fields.title,
            body_field: fields.body,
            payload_field: fields.payload,
        })
    }

    pub fn rebuild(path: &Path, documents: Vec<SearchDocument>) -> anyhow::Result<Self> {
        rebuild_with_checkpoint(path, documents, || Ok(()))
    }

    pub fn rebuild_with_operation_context(
        path: &Path,
        documents: Vec<SearchDocument>,
        operation: &OperationContext,
    ) -> anyhow::Result<Self> {
        operation.check_active().map_err(anyhow::Error::new)?;
        rebuild_with_checkpoint(path, documents, || {
            operation.check_active().map_err(anyhow::Error::new)
        })
    }
}

fn rebuild_with_checkpoint(
    path: &Path,
    documents: Vec<SearchDocument>,
    mut checkpoint: impl FnMut() -> anyhow::Result<()>,
) -> anyhow::Result<TantivySearchIndex> {
    checkpoint()?;
    let staging = staging_path(path);
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;

    let result = (|| {
        let (schema, fields) = search_schema();
        let index = Index::create_in_dir(&staging, schema)?;
        register_tokenizer(&index);
        let mut writer = index.writer(50_000_000)?;

        for (position, document) in documents.into_iter().enumerate() {
            if position % REBUILD_POLL_DOCUMENTS == 0 {
                checkpoint()?;
            }
            let payload = serde_json::to_string(&document.payload)?;
            writer.add_document(doc!(
                fields.id => document.id,
                fields.title => document.title,
                fields.body => document.body,
                fields.payload => payload,
            ))?;
        }
        checkpoint()?;
        commit_writer_with_checkpoint(&mut writer, &mut checkpoint)?;
        drop(writer);
        checkpoint()?;

        replace_directory(path, &staging)?;
        TantivySearchIndex::open_or_create(path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

fn replace_directory(path: &Path, staging: &Path) -> anyhow::Result<()> {
    let backup = backup_path(path);
    let _ = std::fs::remove_dir_all(&backup);
    let had_existing = path.exists();
    if had_existing {
        std::fs::rename(path, &backup)?;
    }
    if let Err(error) = std::fs::rename(staging, path) {
        if had_existing {
            let _ = std::fs::rename(&backup, path);
        }
        return Err(error.into());
    }
    if had_existing {
        let _ = std::fs::remove_dir_all(backup);
    }
    Ok(())
}

fn staging_path(path: &Path) -> PathBuf {
    sibling_path(path, "rebuild")
}

fn backup_path(path: &Path) -> PathBuf {
    sibling_path(path, "backup")
}

fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("search");
    path.with_file_name(format!(".{name}.{suffix}-{}", std::process::id()))
}

fn commit_writer(writer: &mut IndexWriter) -> tantivy::Result<()> {
    commit_writer_with_checkpoint(writer, &mut || Ok(())).map_err(|error| {
        error
            .downcast::<TantivyError>()
            .unwrap_or_else(|error| TantivyError::InvalidArgument(error.to_string()))
    })
}

fn commit_writer_with_checkpoint(
    writer: &mut IndexWriter,
    checkpoint: &mut impl FnMut() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    for attempt in 0..=COMMIT_PERMISSION_RETRIES {
        checkpoint()?;
        match writer.commit() {
            Ok(_) => return Ok(()),
            Err(error)
                if attempt < COMMIT_PERMISSION_RETRIES && is_transient_permission_error(&error) =>
            {
                std::thread::sleep(std::time::Duration::from_millis(10 * (attempt as u64 + 1)));
            }
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!("bounded commit retry loop always returns")
}

fn is_transient_permission_error(error: &TantivyError) -> bool {
    matches!(error, TantivyError::IoError(error) if error.kind() == std::io::ErrorKind::PermissionDenied)
}

struct SearchFields {
    id: Field,
    title: Field,
    body: Field,
    payload: Field,
}

fn search_schema() -> (Schema, SearchFields) {
    let mut schema_builder = Schema::builder();
    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("athanor_en_v1")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    let body_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("athanor_en_v1")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    let fields = SearchFields {
        id: schema_builder.add_text_field("id", STRING | STORED),
        title: schema_builder.add_text_field("title", text_options),
        body: schema_builder.add_text_field("body", body_options),
        payload: schema_builder.add_text_field("payload", STORED),
    };
    (schema_builder.build(), fields)
}

fn register_tokenizer(index: &Index) {
    let tokenizer =
        tantivy::tokenizer::TextAnalyzer::builder(tantivy::tokenizer::SimpleTokenizer::default())
            .filter(tantivy::tokenizer::LowerCaser)
            .filter(tantivy::tokenizer::Stemmer::new(
                tantivy::tokenizer::Language::English,
            ))
            .build();
    index.tokenizers().register("athanor_en_v1", tokenizer);
}

#[async_trait]
impl SearchIndex for TantivySearchIndex {
    async fn index_document(&self, document: SearchDocument) -> CoreResult<()> {
        let payload = serde_json::to_string(&document.payload)
            .map_err(|error| CoreError::Adapter(format!("Failed to serialize payload: {error}")))?;
        let mut writer = self.writer.lock().map_err(|error| {
            CoreError::Adapter(format!("Failed to acquire Tantivy writer lock: {error}"))
        })?;
        writer.delete_term(tantivy::Term::from_field_text(self.id_field, &document.id));
        writer
            .add_document(doc!(
                self.id_field => document.id,
                self.title_field => document.title,
                self.body_field => document.body,
                self.payload_field => payload,
            ))
            .map_err(|error| CoreError::Adapter(format!("Tantivy index error: {error}")))?;
        commit_writer(&mut writer)
            .map_err(|error| CoreError::Adapter(format!("Tantivy writer commit error: {error}")))?;
        self.reader
            .reload()
            .map_err(|error| CoreError::Adapter(format!("Tantivy reader reload error: {error}")))?;
        Ok(())
    }

    async fn remove_document(&self, id: &str) -> CoreResult<()> {
        let mut writer = self.writer.lock().map_err(|error| {
            CoreError::Adapter(format!("Failed to acquire Tantivy writer lock: {error}"))
        })?;
        writer.delete_term(tantivy::Term::from_field_text(self.id_field, id));
        commit_writer(&mut writer)
            .map_err(|error| CoreError::Adapter(format!("Tantivy writer commit error: {error}")))?;
        self.reader
            .reload()
            .map_err(|error| CoreError::Adapter(format!("Tantivy reader reload error: {error}")))?;
        Ok(())
    }

    async fn search(&self, query: SearchQuery) -> CoreResult<Vec<SearchResult>> {
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.body_field]);
        let parsed_query = query_parser
            .parse_query(&query.query)
            .map_err(|error| CoreError::Adapter(format!("Tantivy query parse error: {error}")))?;
        let searcher = self.reader.searcher();
        let top_docs = searcher
            .search(
                &parsed_query,
                &TopDocs::with_limit(query.limit).order_by_score(),
            )
            .map_err(|error| CoreError::Adapter(format!("Tantivy search error: {error}")))?;
        let mut results = Vec::new();
        for (score, address) in top_docs {
            let document: TantivyDocument = searcher
                .doc(address)
                .map_err(|error| CoreError::Adapter(format!("Tantivy doc retrieval error: {error}")))?;
            let id = document
                .get_first(self.id_field)
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            let payload = document
                .get_first(self.payload_field)
                .and_then(|value| value.as_str())
                .unwrap_or("{}");
            let payload: Value = serde_json::from_str(payload)
                .map_err(|error| CoreError::Adapter(format!("Tantivy payload parse error: {error}")))?;
            results.push(SearchResult { id, score, payload });
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn retries_only_permission_denied_io_errors() {
        let denied = TantivyError::IoError(Arc::new(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        )));
        let missing =
            TantivyError::IoError(Arc::new(std::io::Error::from(std::io::ErrorKind::NotFound)));
        assert!(is_transient_permission_error(&denied));
        assert!(!is_transient_permission_error(&missing));
    }

    #[tokio::test]
    async fn test_tantivy_search_index() {
        let root = test_root("basic");
        std::fs::create_dir_all(&root).unwrap();
        let index = TantivySearchIndex::open_or_create(&root).unwrap();
        index.index_document(document("doc1", "Authentication Module", "login authentication", "auth")).await.unwrap();
        index.index_document(document("doc2", "User Profile", "profile settings", "profile")).await.unwrap();
        let results = index.search(SearchQuery { query: "login".to_string(), limit: 5 }).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
        index.remove_document("doc1").await.unwrap();
        let results = index.search(SearchQuery { query: "login".to_string(), limit: 5 }).await.unwrap();
        assert!(results.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rebuild_indexes_documents_with_one_commit() {
        let root = test_root("rebuild");
        let documents = (0..2_500)
            .map(|index| document(&format!("doc-{index}"), &format!("API retention {index}"), "snapshot cleanup retention", "retention"))
            .collect();
        let index = TantivySearchIndex::rebuild(&root, documents).unwrap();
        let results = index.search(SearchQuery { query: "retention".to_string(), limit: 3 }).await.unwrap();
        assert_eq!(results.len(), 3);
        drop(index);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn cancelled_staged_rebuild_preserves_current_index() {
        let root = test_root("cancelled-rebuild");
        let current = TantivySearchIndex::rebuild(
            &root,
            vec![document("current", "Current index", "stable marker", "current")],
        )
        .unwrap();
        drop(current);
        let operation = OperationContext::new("tantivy-rebuild-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        cancellation.cancel();

        let error = TantivySearchIndex::rebuild_with_operation_context(
            &root,
            vec![document("replacement", "Replacement", "new marker", "replacement")],
            &operation,
        )
        .expect_err("cancelled rebuild must fail before replacing current index");
        assert!(error.chain().any(|cause| matches!(cause.downcast_ref::<CoreError>(), Some(CoreError::Cancelled(_)))));

        let index = TantivySearchIndex::open_or_create(&root).unwrap();
        let results = index.search(SearchQuery { query: "stable".to_string(), limit: 5 }).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "current");
        drop(index);
        let _ = std::fs::remove_dir_all(root);
    }

    fn document(id: &str, title: &str, body: &str, key: &str) -> SearchDocument {
        SearchDocument {
            id: id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            payload: json!({"key": key}),
        }
    }

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "athanor-tantivy-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
