use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use athanor_app::{
    AdapterPluginKind, AdapterRegistry, AthanorStore, ProjectConfig, RuntimeComposition,
};
use athanor_core::{CoreResult, SearchDocument, SearchIndex, SearchQuery, SearchResult};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde_json::{Value, json};

const ITERATIONS: usize = 24;

static STORE_A_CALLS: AtomicUsize = AtomicUsize::new(0);
static STORE_B_CALLS: AtomicUsize = AtomicUsize::new(0);
static SEARCH_A_CALLS: AtomicUsize = AtomicUsize::new(0);
static SEARCH_B_CALLS: AtomicUsize = AtomicUsize::new(0);
static WIKI_A_CALLS: AtomicUsize = AtomicUsize::new(0);
static WIKI_B_CALLS: AtomicUsize = AtomicUsize::new(0);
static HTML_A_CALLS: AtomicUsize = AtomicUsize::new(0);
static HTML_B_CALLS: AtomicUsize = AtomicUsize::new(0);

fn empty_registry() -> AdapterRegistry {
    AdapterRegistry::empty()
}

fn no_builtin_adapter(
    _registry: AdapterRegistry,
    _kind: AdapterPluginKind,
    _id: &str,
) -> Option<AdapterRegistry> {
    None
}

fn store_a<'a>(
    root: &'a Path,
    _config: &'a ProjectConfig,
) -> Pin<Box<dyn Future<Output = Result<AthanorStore>> + Send + 'a>> {
    STORE_A_CALLS.fetch_add(1, Ordering::SeqCst);
    Box::pin(async move {
        Ok(AthanorStore::new_with_latest_pointer(
            JsonlKnowledgeStore::new(root.join("store-a")),
        ))
    })
}

fn store_b<'a>(
    root: &'a Path,
    _config: &'a ProjectConfig,
) -> Pin<Box<dyn Future<Output = Result<AthanorStore>> + Send + 'a>> {
    STORE_B_CALLS.fetch_add(1, Ordering::SeqCst);
    Box::pin(async move {
        Ok(AthanorStore::new_with_latest_pointer(
            JsonlKnowledgeStore::new(root.join("store-b")),
        ))
    })
}

struct MarkerIndex(&'static str);

#[async_trait::async_trait]
impl SearchIndex for MarkerIndex {
    async fn index_document(&self, _doc: SearchDocument) -> CoreResult<()> {
        Ok(())
    }

    async fn remove_document(&self, _id: &str) -> CoreResult<()> {
        Ok(())
    }

    async fn search(&self, _query: SearchQuery) -> CoreResult<Vec<SearchResult>> {
        Ok(vec![SearchResult {
            id: self.0.to_string(),
            score: 1.0,
            payload: json!({ "owner": self.0 }),
        }])
    }
}

fn search_a(
    _index_dir: &Path,
    _documents: Option<Vec<SearchDocument>>,
) -> Result<Arc<dyn SearchIndex>> {
    SEARCH_A_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Arc::new(MarkerIndex("a")))
}

fn search_b(
    _index_dir: &Path,
    _documents: Option<Vec<SearchDocument>>,
) -> Result<Arc<dyn SearchIndex>> {
    SEARCH_B_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Arc::new(MarkerIndex("b")))
}

fn write_marker(target: &Path, owner: &str, payload: &Value) -> Result<()> {
    std::fs::create_dir_all(target)?;
    std::fs::write(target.join("owner.txt"), owner)?;
    std::fs::write(target.join("payload.json"), serde_json::to_vec(payload)?)?;
    Ok(())
}

fn wiki_a(
    target: &Path,
    _snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    assert!(!is_cancelled());
    WIKI_A_CALLS.fetch_add(1, Ordering::SeqCst);
    write_marker(target, "a", &payload)
}

fn wiki_b(
    target: &Path,
    _snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    assert!(!is_cancelled());
    WIKI_B_CALLS.fetch_add(1, Ordering::SeqCst);
    write_marker(target, "b", &payload)
}

fn html_a(
    target: &Path,
    _snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    assert!(!is_cancelled());
    HTML_A_CALLS.fetch_add(1, Ordering::SeqCst);
    write_marker(target, "a", &payload)
}

fn html_b(
    target: &Path,
    _snapshot: &str,
    payload: Value,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    assert!(!is_cancelled());
    HTML_B_CALLS.fetch_add(1, Ordering::SeqCst);
    write_marker(target, "b", &payload)
}

fn composition_a() -> RuntimeComposition {
    RuntimeComposition::new(
        empty_registry,
        no_builtin_adapter,
        store_a,
        search_a,
        wiki_a,
        html_a,
    )
}

fn composition_b() -> RuntimeComposition {
    RuntimeComposition::new(
        empty_registry,
        no_builtin_adapter,
        store_b,
        search_b,
        wiki_b,
        html_b,
    )
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-composition-isolation-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

async fn exercise(composition: RuntimeComposition, root: PathBuf, owner: &'static str) {
    let config = ProjectConfig::default();
    std::fs::create_dir_all(&root).unwrap();

    for iteration in 0..ITERATIONS {
        let _store = composition.init_store(&root, &config).await.unwrap();
        let index = composition
            .build_search_index(&root.join(format!("search-{iteration}")), None)
            .unwrap();
        let results = index
            .search(SearchQuery {
                query: "owner".to_string(),
                limit: 1,
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, owner);
        assert_eq!(results[0].payload["owner"], owner);

        let wiki = root.join(format!("wiki-{iteration}"));
        let html = root.join(format!("html-{iteration}"));
        let payload = json!({ "iteration": iteration, "owner": owner });
        composition
            .project_wiki(&wiki, "snapshot", payload.clone(), &|| false)
            .unwrap();
        composition
            .project_html(&html, "snapshot", payload, &|| false)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(wiki.join("owner.txt")).unwrap(),
            owner
        );
        assert_eq!(
            std::fs::read_to_string(html.join("owner.txt")).unwrap(),
            owner
        );

        tokio::task::yield_now().await;
    }

    std::fs::remove_dir_all(root).unwrap();
}

fn reset_counters() {
    for counter in [
        &STORE_A_CALLS,
        &STORE_B_CALLS,
        &SEARCH_A_CALLS,
        &SEARCH_B_CALLS,
        &WIKI_A_CALLS,
        &WIKI_B_CALLS,
        &HTML_A_CALLS,
        &HTML_B_CALLS,
    ] {
        counter.store(0, Ordering::SeqCst);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_compositions_do_not_cross_store_search_or_projector_factories() {
    reset_counters();

    let task_a = tokio::spawn(exercise(composition_a(), temp_root("a"), "a"));
    let task_b = tokio::spawn(exercise(composition_b(), temp_root("b"), "b"));
    task_a.await.unwrap();
    task_b.await.unwrap();

    for counter in [
        &STORE_A_CALLS,
        &STORE_B_CALLS,
        &SEARCH_A_CALLS,
        &SEARCH_B_CALLS,
        &WIKI_A_CALLS,
        &WIKI_B_CALLS,
        &HTML_A_CALLS,
        &HTML_B_CALLS,
    ] {
        assert_eq!(counter.load(Ordering::SeqCst), ITERATIONS);
    }
}
