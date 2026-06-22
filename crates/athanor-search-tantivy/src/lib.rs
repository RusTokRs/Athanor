#![allow(clippy::collapsible_if)]

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, SearchDocument, SearchIndex, SearchQuery, SearchResult};
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tantivy::{
    Index, IndexReader, IndexWriter, TantivyDocument,
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{
        Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions,
        Value as TantivyValue,
    },
};

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
        let mut schema_builder = Schema::builder();

        // Versioned tokenizer setting ("athanor_en_v1")
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

        let id_field = schema_builder.add_text_field("id", STRING | STORED);
        let title_field = schema_builder.add_text_field("title", text_options);
        let body_field = schema_builder.add_text_field("body", body_options);
        let payload_field = schema_builder.add_text_field("payload", STORED);
        let schema = schema_builder.build();

        let index = match Index::open_in_dir(path) {
            Ok(idx) => idx,
            Err(_) => {
                let _ = std::fs::remove_dir_all(path);
                std::fs::create_dir_all(path)?;
                Index::create_in_dir(path, schema.clone())?
            }
        };

        // Register custom versioned tokenizer
        let tokenizer = tantivy::tokenizer::TextAnalyzer::builder(
            tantivy::tokenizer::SimpleTokenizer::default(),
        )
        .filter(tantivy::tokenizer::LowerCaser)
        .filter(tantivy::tokenizer::Stemmer::new(
            tantivy::tokenizer::Language::English,
        ))
        .build();

        index.tokenizers().register("athanor_en_v1", tokenizer);

        let writer = index.writer(50_000_000)?;
        let reader = index.reader()?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            id_field,
            title_field,
            body_field,
            payload_field,
        })
    }
}

#[async_trait]
impl SearchIndex for TantivySearchIndex {
    async fn index_document(&self, doc: SearchDocument) -> CoreResult<()> {
        let payload_str = serde_json::to_string(&doc.payload)
            .map_err(|e| CoreError::Adapter(format!("Failed to serialize payload: {e}")))?;

        let mut writer = self.writer.lock().map_err(|e| {
            CoreError::Adapter(format!("Failed to acquire Tantivy writer lock: {e}"))
        })?;

        // Remove old document with same ID first
        let term = tantivy::Term::from_field_text(self.id_field, &doc.id);
        writer.delete_term(term);

        writer
            .add_document(doc!(
                self.id_field => doc.id,
                self.title_field => doc.title,
                self.body_field => doc.body,
                self.payload_field => payload_str,
            ))
            .map_err(|e| CoreError::Adapter(format!("Tantivy index error: {e}")))?;

        writer
            .commit()
            .map_err(|e| CoreError::Adapter(format!("Tantivy writer commit error: {e}")))?;

        self.reader
            .reload()
            .map_err(|e| CoreError::Adapter(format!("Tantivy reader reload error: {e}")))?;

        Ok(())
    }

    async fn remove_document(&self, id: &str) -> CoreResult<()> {
        let mut writer = self.writer.lock().map_err(|e| {
            CoreError::Adapter(format!("Failed to acquire Tantivy writer lock: {e}"))
        })?;

        let term = tantivy::Term::from_field_text(self.id_field, id);
        writer.delete_term(term);

        writer
            .commit()
            .map_err(|e| CoreError::Adapter(format!("Tantivy writer commit error: {e}")))?;

        self.reader
            .reload()
            .map_err(|e| CoreError::Adapter(format!("Tantivy reader reload error: {e}")))?;

        Ok(())
    }

    async fn search(&self, query: SearchQuery) -> CoreResult<Vec<SearchResult>> {
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.body_field]);
        let parsed_query = query_parser
            .parse_query(&query.query)
            .map_err(|e| CoreError::Adapter(format!("Tantivy query parse error: {e}")))?;

        let searcher = self.reader.searcher();
        let top_docs = searcher
            .search(
                &parsed_query,
                &TopDocs::with_limit(query.limit).order_by_score(),
            )
            .map_err(|e| CoreError::Adapter(format!("Tantivy search error: {e}")))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| CoreError::Adapter(format!("Tantivy doc retrieval error: {e}")))?;

            let id = retrieved_doc
                .get_first(self.id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let payload_str = retrieved_doc
                .get_first(self.payload_field)
                .and_then(|v| v.as_str())
                .unwrap_or("{}");

            let payload: Value = serde_json::from_str(payload_str)
                .map_err(|e| CoreError::Adapter(format!("Tantivy payload parse error: {e}")))?;

            results.push(SearchResult { id, score, payload });
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_tantivy_search_index() {
        let temp_dir = std::env::temp_dir().join(format!(
            "athanor-tantivy-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let index = TantivySearchIndex::open_or_create(&temp_dir).unwrap();

        let doc1 = SearchDocument {
            id: "doc1".to_string(),
            title: "Authentication Module".to_string(),
            body: "This module handles login and user authentication with password hashes."
                .to_string(),
            payload: json!({"key": "auth"}),
        };

        let doc2 = SearchDocument {
            id: "doc2".to_string(),
            title: "User Profile Settings".to_string(),
            body: "Allows updating profile name, email, and user preferences.".to_string(),
            payload: json!({"key": "profile"}),
        };

        index.index_document(doc1).await.unwrap();
        index.index_document(doc2).await.unwrap();

        // Test search
        let query = SearchQuery {
            query: "login".to_string(),
            limit: 5,
        };
        let results = index.search(query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
        assert_eq!(results[0].payload["key"], "auth");

        // Test update/re-index
        let doc1_updated = SearchDocument {
            id: "doc1".to_string(),
            title: "Authentication API".to_string(),
            body: "Login, logout, and token session management.".to_string(),
            payload: json!({"key": "auth_new"}),
        };
        index.index_document(doc1_updated).await.unwrap();

        let results = index
            .search(SearchQuery {
                query: "session".to_string(),
                limit: 5,
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
        assert_eq!(results[0].payload["key"], "auth_new");

        // Test remove
        index.remove_document("doc1").await.unwrap();
        let results = index
            .search(SearchQuery {
                query: "session".to_string(),
                limit: 5,
            })
            .await
            .unwrap();
        assert!(results.is_empty());

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
