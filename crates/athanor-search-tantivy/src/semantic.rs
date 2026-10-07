//! Local semantic retrieval: hashing-trick embeddings (character trigrams plus
//! word n-grams hashed with FNV-1a into signed buckets) and a JSON sidecar vector
//! store. Zero new dependencies — no model weights, no external embedding service,
//! suitable for offline CPU-only use.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;
use athanor_core::{
    CoreError, CoreResult, EmbeddingInput, EmbeddingProvider, VectorIndex, VectorItem, VectorQuery,
    VectorSearchResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Dimensionality of the hashing embeddings.
pub const SEMANTIC_EMBEDDING_DIM: usize = 384;
/// Sidecar file name inside the search index directory.
pub const SEMANTIC_VECTOR_FILE: &str = "semantic-vectors.json";
/// Bump when the embedding features or the sidecar layout change; stale sidecars
/// are ignored on load.
pub const SEMANTIC_VECTOR_STORE_VERSION: u32 = 1;

/// Embed text into a fixed-size, L2-normalized hashing vector.
///
/// Features are word unigrams, word bigrams and space-padded character trigrams,
/// hashed with FNV-1a into `dim` signed buckets (the hashing trick). Character
/// n-grams provide sub-word overlap, so related spellings ("auth" vs
/// "authentication") share features without a stemming dependency.
pub fn embed_text(text: &str) -> Vec<f32> {
    embed_with_dim(text, SEMANTIC_EMBEDDING_DIM)
}

/// Embed text into a vector of the given dimension.
pub fn embed_with_dim(text: &str, dim: usize) -> Vec<f32> {
    let dim = dim.max(1);
    let mut vector = vec![0.0f32; dim];
    for feature in features(tokenize(text)) {
        let hash = fnv1a_64(feature.as_bytes());
        let bucket = (hash % dim as u64) as usize;
        let sign = if hash & (1u64 << 63) == 0 { 1.0 } else { -1.0 };
        vector[bucket] += sign;
    }
    normalize(&mut vector);
    vector
}

/// Cosine similarity between two vectors; returns 0.0 when either norm is zero.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Zero-dependency local embedding provider backed by the hashing trick.
#[derive(Debug, Default, Clone, Copy)]
pub struct HashingEmbeddingProvider;

impl HashingEmbeddingProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EmbeddingProvider for HashingEmbeddingProvider {
    async fn embed(&self, input: EmbeddingInput) -> CoreResult<Vec<f32>> {
        Ok(embed_text(&input.text))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredVector {
    id: String,
    vector: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    payload: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorStoreSnapshot {
    version: u32,
    dim: usize,
    vectors: Vec<StoredVector>,
}

/// In-memory semantic vector store with JSON sidecar persistence.
///
/// Vectors are keyed by document id; payloads are optional so the store can back
/// both the Tantivy semantic index (payloads stay in Tantivy) and the standalone
/// `VectorIndex` port (payloads travel with the sidecar).
pub struct SemanticVectorStore {
    dim: usize,
    vectors: Mutex<HashMap<String, (Vec<f32>, Option<Value>)>>,
}

impl SemanticVectorStore {
    pub fn new(dim: usize) -> Self {
        Self {
            dim: dim.max(1),
            vectors: Mutex::new(HashMap::new()),
        }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn len(&self) -> usize {
        self.vectors.lock().map(|vectors| vectors.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Build a store from `(id, text)` pairs, embedding each text.
    pub fn from_documents(dim: usize, documents: impl IntoIterator<Item = (String, String)>) -> Self {
        let store = Self::new(dim);
        for (id, text) in documents {
            let vector = embed_with_dim(&text, store.dim);
            store.upsert(&id, vector, None);
        }
        store
    }

    /// Load a sidecar; returns `None` when missing, stale (version/dim mismatch)
    /// or corrupt.
    pub fn load(path: &Path) -> Option<Self> {
        let contents = std::fs::read(path).ok()?;
        let snapshot: VectorStoreSnapshot = serde_json::from_slice(&contents).ok()?;
        if snapshot.version != SEMANTIC_VECTOR_STORE_VERSION || snapshot.dim == 0 {
            return None;
        }
        let store = Self::new(snapshot.dim);
        let mut vectors = store.vectors.lock().ok()?;
        for stored in snapshot.vectors {
            if stored.vector.len() == snapshot.dim {
                vectors.insert(stored.id, (stored.vector, stored.payload));
            }
        }
        Some(store)
    }

    /// Atomically persist the store to its sidecar path.
    pub fn save(&self, path: &Path) -> Result<()> {
        let vectors = self
            .vectors
            .lock()
            .map_err(|error| anyhow::anyhow!("semantic vector store lock poisoned: {error}"))?;
        let mut stored: Vec<StoredVector> = vectors
            .iter()
            .map(|(id, (vector, payload))| StoredVector {
                id: id.clone(),
                vector: vector.clone(),
                payload: payload.clone(),
            })
            .collect();
        stored.sort_by(|a, b| a.id.cmp(&b.id));
        let snapshot = VectorStoreSnapshot {
            version: SEMANTIC_VECTOR_STORE_VERSION,
            dim: self.dim,
            vectors: stored,
        };
        let contents = serde_json::to_vec_pretty(&snapshot)?;
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, contents)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }

    pub fn upsert(&self, id: &str, vector: Vec<f32>, payload: Option<Value>) {
        if let Ok(mut vectors) = self.vectors.lock() {
            vectors.insert(id.to_string(), (vector, payload));
        }
    }

    pub fn remove(&self, id: &str) {
        if let Ok(mut vectors) = self.vectors.lock() {
            vectors.remove(id);
        }
    }

    pub fn payload(&self, id: &str) -> Option<Value> {
        self.vectors
            .lock()
            .ok()?
            .get(id)
            .and_then(|(_, payload)| payload.clone())
    }

    /// Top-`limit` cosine matches, best first; ties break by id for determinism.
    /// Non-positive similarities are treated as unrelated and filtered out.
    pub fn search(&self, query: &[f32], limit: usize) -> Vec<(String, f32)> {
        if limit == 0 {
            return Vec::new();
        }
        let Ok(vectors) = self.vectors.lock() else {
            return Vec::new();
        };
        let mut scored: Vec<(String, f32)> = vectors
            .iter()
            .map(|(id, (vector, _))| (id.clone(), cosine_similarity(query, vector)))
            .filter(|(_, score)| *score > 0.0)
            .collect();
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        scored.truncate(limit);
        scored
    }
}

#[async_trait]
impl VectorIndex for SemanticVectorStore {
    async fn upsert(&self, item: VectorItem) -> CoreResult<()> {
        if item.vector.len() != self.dim {
            return Err(CoreError::Adapter(format!(
                "semantic vector dimension mismatch: expected {}, got {}",
                self.dim,
                item.vector.len()
            )));
        }
        SemanticVectorStore::upsert(self, &item.id, item.vector, Some(item.payload));
        Ok(())
    }

    async fn search(&self, query: VectorQuery) -> CoreResult<Vec<VectorSearchResult>> {
        if query.vector.len() != self.dim {
            return Err(CoreError::Adapter(format!(
                "semantic vector dimension mismatch: expected {}, got {}",
                self.dim,
                query.vector.len()
            )));
        }
        Ok(self
            .search(&query.vector, query.limit)
            .into_iter()
            .map(|(id, score)| VectorSearchResult {
                payload: self.payload(&id).unwrap_or(Value::Null),
                id,
                score,
            })
            .collect())
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split(' ')
        .filter(|token| token.chars().count() >= 2)
        .map(str::to_string)
        .collect()
}

fn features(tokens: Vec<String>) -> Vec<String> {
    let mut features = Vec::new();
    for token in &tokens {
        features.push(token.clone());
        let padded = format!(" {token} ");
        let chars: Vec<char> = padded.chars().collect();
        for window in chars.windows(3) {
            features.push(window.iter().collect());
        }
    }
    for pair in tokens.windows(2) {
        features.push(format!("{} {}", pair[0], pair[1]));
    }
    features
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn embed_text_is_deterministic_and_normalized() {
        let a = embed_text("authentication login module");
        let b = embed_text("authentication login module");
        assert_eq!(a.len(), SEMANTIC_EMBEDDING_DIM);
        assert_eq!(a, b);
        let norm = a.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn embed_empty_text_is_zero_vector() {
        let vector = embed_text("   ");
        assert!(vector.iter().all(|value| *value == 0.0));
        assert_eq!(cosine_similarity(&vector, &embed_text("anything")), 0.0);
    }

    #[test]
    fn cosine_similarity_bounds() {
        let a = embed_text("alpha beta gamma");
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-5);
        assert!(cosine_similarity(&a, &embed_text("zzq qzx jvq")) < 0.5);
    }

    #[test]
    fn related_texts_score_higher_than_unrelated() {
        let target = embed_text("authentication login module");
        let related = cosine_similarity(&target, &embed_text("auth sign in attempt"));
        let unrelated = cosine_similarity(&target, &embed_text("kubernetes deployment rollback"));
        assert!(
            related > unrelated,
            "related {related} should beat unrelated {unrelated}"
        );
    }

    #[test]
    fn store_roundtrips_through_sidecar() {
        let dir = std::env::temp_dir().join(format!(
            "athanor-semantic-store-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SEMANTIC_VECTOR_FILE);
        let store = SemanticVectorStore::from_documents(
            SEMANTIC_EMBEDDING_DIM,
            vec![
                ("a".to_string(), "authentication login".to_string()),
                ("b".to_string(), "database migration".to_string()),
            ],
        );
        store.upsert("c", embed_text("payment gateway"), Some(json!({"k": "c"})));
        store.save(&path).unwrap();

        let loaded = SemanticVectorStore::load(&path).expect("sidecar loads");
        assert_eq!(loaded.dim(), SEMANTIC_EMBEDDING_DIM);
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.payload("c"), Some(json!({"k": "c"})));

        let hits = loaded.search(&embed_text("login auth"), 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "a");

        loaded.remove("a");
        assert!(loaded.search(&embed_text("login auth"), 5).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn store_rejects_stale_sidecar_versions() {
        let dir = std::env::temp_dir().join(format!(
            "athanor-semantic-stale-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SEMANTIC_VECTOR_FILE);
        std::fs::write(
            &path,
            serde_json::to_vec(&VectorStoreSnapshot {
                version: SEMANTIC_VECTOR_STORE_VERSION + 1,
                dim: SEMANTIC_EMBEDDING_DIM,
                vectors: vec![],
            })
            .unwrap(),
        )
        .unwrap();
        assert!(SemanticVectorStore::load(&path).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn vector_index_port_enforces_dimension() {
        let store = SemanticVectorStore::new(SEMANTIC_EMBEDDING_DIM);
        let error = VectorIndex::upsert(
            &store,
            VectorItem {
                id: "x".to_string(),
                vector: vec![0.0; 3],
                payload: Value::Null,
            },
        )
        .await
        .expect_err("dimension mismatch must fail");
        assert!(matches!(error, CoreError::Adapter(_)));
        VectorIndex::upsert(
            &store,
            VectorItem {
                id: "x".to_string(),
                vector: embed_text("hello world"),
                payload: json!({"k": "x"}),
            },
        )
        .await
        .unwrap();
        let results = VectorIndex::search(
            &store,
            VectorQuery {
                vector: embed_text("hello there world"),
                limit: 5,
            },
        )
        .await
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "x");
        assert_eq!(results[0].payload, json!({"k": "x"}));
    }

    #[tokio::test]
    async fn hashing_provider_matches_embed_text() {
        let provider = HashingEmbeddingProvider::new();
        let vector = EmbeddingProvider::embed(&provider, EmbeddingInput {
            text: "authentication login".to_string(),
        })
        .await
        .unwrap();
        assert_eq!(vector, embed_text("authentication login"));
    }
}
