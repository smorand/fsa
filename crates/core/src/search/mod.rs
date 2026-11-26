//! Search backend trait and shared types.
//!
//! The `SearchBackend` trait is implemented by:
//! - `bm25_sqlite` (Tantivy, always compiled, runs when `search.mode` is "bm25" or "both")
//! - `bm25_pg` (PostgreSQL tsvector, `#[cfg(feature = "postgres")]`)
//! - `vector_pg` (pgvector, `#[cfg(feature = "rag")]`)
//! - `vector_sqlite` (sqlite-vec, `#[cfg(feature = "rag")]`)
//!
//! The factory `build_backend` in this module picks the right combination from
//! the server config and the `infra.meta.backend` setting.

use crate::config::ServerConfig;
use crate::errors::{Result, ToolError};
use async_trait::async_trait;
use std::sync::Arc;

#[cfg(feature = "postgres")]
pub mod bm25_pg;
pub mod bm25_sqlite;
pub mod derived;
#[cfg(test)]
pub(crate) mod derived_e2e;
#[cfg(test)]
pub mod e2e;
pub mod embedding;
pub mod fusion;
pub mod indexer;
pub mod reconcile;
pub mod rerank;
#[cfg(feature = "rag")]
pub mod vector_pg;
#[cfg(feature = "rag")]
pub mod vector_sqlite;
#[cfg(all(test, feature = "rag"))]
mod vector_store_e2e;

/// One ranked result chunk returned by a search query.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
    pub chunk: String,
    pub rank: usize,
    /// The stored `chunk_idx` of the entry: the chunk number of the file's
    /// default set for an entry built from it (SPEC-0101 DEC-010).
    pub chunk_idx: u32,
}

/// One search entry to store: a chunk of the file's default set (SPEC-0101
/// DEC-009). Engines never cut text: they store what they are given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedChunk {
    /// The chunk number, 1 based, stored in `chunk_idx`.
    pub number: u32,
    /// The indexed text: the header line, a newline, then the chunk text.
    pub text: String,
}

impl IndexedChunk {
    /// The entry of one chunk (DEC-010): `<title>[ > <heading path titles
    /// joined by " > ">]` on its own line, then the chunk text, so a word of a
    /// heading or of the title finds the chunk.
    pub fn from_chunk(c: &crate::chunking::Chunk) -> Self {
        let mut header = c.document_title.clone();
        for title in &c.heading_path {
            header.push_str(" > ");
            header.push_str(title);
        }
        Self { number: c.number, text: format!("{header}\n{}", c.text) }
    }
}

/// The entries of `text` as the default structural settings cut it, for the
/// engine unit tests that used to hand a whole text to the old `index_path`.
#[cfg(test)]
pub(crate) fn test_chunks(path: &str, text: &str) -> Vec<IndexedChunk> {
    use crate::chunking::service::{Requested, Strategy, title};
    let req = Requested { strategy: Strategy::Structural, settings: Vec::new() };
    let d = crate::chunking::ChunkDefaults::default();
    crate::chunking::cut(text, &title(path), &req, &d)
        .chunks
        .iter()
        .map(IndexedChunk::from_chunk)
        .collect()
}

/// Entries numbered from 1 holding exactly `texts`, for the tests that need
/// fixed entries (a failing insert, a counted embedding call).
#[cfg(test)]
pub(crate) fn entries(texts: &[&str]) -> Vec<IndexedChunk> {
    texts
        .iter()
        .zip(1..)
        .map(|(t, number)| IndexedChunk { number, text: (*t).to_string() })
        .collect()
}

/// Indexes `text` cut by [`test_chunks`], the guard accepting: the test
/// stand in of the removed `index_path`. Returns the number of entries stored.
#[cfg(test)]
pub(crate) async fn index_text<B: SearchBackend + ?Sized>(
    b: &B,
    volume_id: &str,
    path: &str,
    text: &str,
) -> Result<usize> {
    let chunks = test_chunks(path, text);
    Ok(b.index_chunks_checked(volume_id, path, &chunks, &|| true).await?.unwrap_or(0))
}

/// What a stored vector was computed from (SPEC-0108): a chunk of the default
/// set, or an image kept from a converted document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorKind {
    Text,
    Image,
}

impl VectorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
        }
    }
}

/// One image to embed: its number (`image-<n>`), the text stored beside the
/// vector (caption and OCR) and the picture as a `data:` URI, the `input` of the
/// image model's `/embeddings` call (SPEC-0108 DEC-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInput {
    pub number: u32,
    pub text: String,
    pub data_uri: String,
}

/// One stored vector with the text it was computed from.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredVector {
    pub number: u32,
    pub text: String,
    pub vector: Vec<f32>,
}

/// One entry found by a vector read (SPEC-0109): where it is, the text stored
/// beside its vector and its similarity to the input.
#[derive(Debug, Clone, PartialEq)]
pub struct Near {
    pub path: String,
    pub number: u32,
    pub text: String,
    pub similarity: f32,
}

/// `1 - cosine distance`, clamped to 0 to 1: float noise can push a distance
/// slightly below 0 and an opposite vector is at 2, which both read as a bound.
pub fn similarity_of_distance(distance: f64) -> f32 {
    (1.0 - distance).clamp(0.0, 1.0) as f32
}

/// A page of the vectors of one file, in number order (FR-NEW-006).
#[derive(Debug, Clone, PartialEq)]
pub struct VectorPage {
    /// The width of the store that holds them; `None` when the file has none.
    pub dimension: Option<u32>,
    pub vectors: Vec<StoredVector>,
    /// The last number of the page when more follow.
    pub next_after: Option<u32>,
}

#[cfg(test)]
mod similarity_tests {
    use super::similarity_of_distance;

    /// SPEC-0109 FR-NEW-003: `1 - cosine distance`, clamped to 0 to 1.
    #[test]
    fn similarity_is_one_minus_the_distance_clamped() {
        assert!((similarity_of_distance(0.25) - 0.75).abs() < 1e-6);
        assert_eq!(similarity_of_distance(0.0), 1.0);
        assert_eq!(similarity_of_distance(-1e-7), 1.0, "float noise above one");
        assert_eq!(similarity_of_distance(1.0), 0.0);
        assert_eq!(similarity_of_distance(1.7), 0.0, "an opposite vector reads 0");
        assert_eq!(similarity_of_distance(2.0), 0.0);
    }
}

/// Per-volume index statistics, as reported by `search.status`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexStats {
    /// Number of indexed document chunks in the BM25 index.
    pub bm25_docs: usize,
    /// Number of stored vector chunks (0 when RAG is not enabled).
    pub vector_chunks: usize,
    /// True when the Tantivy index directory exists and is non-empty.
    pub bm25_warm: bool,
    /// The active mode ("bm25", "rag", or "both").
    pub mode: String,
}

/// The interface every search backend implements.
///
/// Each implementation handles one engine (Tantivy BM25 on SQLite, pgvector on
/// PostgreSQL, sqlite-vec on SQLite). A combined "both" backend is assembled by
/// the factory function, not by a wrapper type, so the trait stays flat.
#[async_trait]
pub trait SearchBackend: Send + Sync {
    /// Replaces the entries of `(volume_id, path)` by `chunks`, one entry per
    /// chunk: the text in the engine's text column, the number in `chunk_idx`.
    /// `still_current` runs after any embedding and right before the commit.
    /// When it returns false nothing is stored, the previous entries of the
    /// path stay as they were, and the result is `Ok(None)`. An error before
    /// the commit also leaves them untouched. Otherwise the result is the
    /// number of entries stored.
    async fn index_chunks_checked(
        &self,
        volume_id: &str,
        path: &str,
        chunks: &[IndexedChunk],
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>>;

    /// Remove all indexed chunks for `path` under `volume_id`.
    /// Returns the number of chunks deleted.
    async fn delete_path(&self, volume_id: &str, path: &str) -> Result<usize>;

    /// Remove every indexed chunk of `volume_id`, whatever the path.
    /// Returns the number of chunks deleted.
    ///
    /// This is the wipe half of a project index mode change, so it must be scoped
    /// to the one volume: another project's chunks live in the same tables.
    async fn delete_all(&self, volume_id: &str) -> Result<usize>;

    /// BM25 full-text query. Returns the top `top_k` results by relevance.
    async fn query_bm25(
        &self,
        volume_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>>;

    /// Vector KNN query. Returns the top `top_k` results by cosine similarity.
    ///
    /// The `query` text is embedded internally by the backend. Backends that do
    /// not support vector search return `ERR_NOT_SUPPORTED`.
    async fn query_vector(
        &self,
        volume_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>>;

    /// Return index statistics for the volume.
    async fn stats(&self, volume_id: &str) -> Result<IndexStats>;

    /// Replaces the image vectors of `(volume_id, path)` by one per image, with
    /// the project's image model; same guard contract as
    /// [`Self::index_chunks_checked`]. An empty list clears them. Engines without
    /// vectors refuse.
    async fn index_images_checked(
        &self,
        _volume_id: &str,
        _path: &str,
        _images: &[ImageInput],
        _still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        Err(ToolError::not_supported("the index mode of the project has no vectors"))
    }

    /// True when the project's profile names an image model, so a file's images
    /// are worth reading to embed.
    async fn embeds_images(&self, _volume_id: &str) -> Result<bool> {
        Ok(false)
    }

    /// One page of the vectors of a file (SPEC-0108 FR-NEW-006): at most `limit`
    /// entries numbered after `after`. Engines without vectors refuse.
    async fn vectors_of(
        &self,
        _volume_id: &str,
        _path: &str,
        _kind: VectorKind,
        _after: u32,
        _limit: usize,
    ) -> Result<VectorPage> {
        Err(ToolError::not_supported("the index mode of the project has no vectors"))
    }

    /// The vector of `input` at the width in force for the project, with the model
    /// of the kind (SPEC-0109): the text model for [`VectorKind::Text`], the image
    /// model for [`VectorKind::Image`], where `input` is a text or a `data:` URI.
    /// An unusable answer is refused as for a stored vector. Engines without
    /// vectors refuse.
    async fn embed_input(
        &self,
        _volume_id: &str,
        _kind: VectorKind,
        _input: &str,
    ) -> Result<Vec<f32>> {
        Err(ToolError::not_supported("the index mode of the project has no vectors"))
    }

    /// The `limit` entries of `kind` closest to `vector` in the store of its width,
    /// best first, ties by path then number. A width with no store finds nothing
    /// and creates nothing visible. Never writes a vector.
    async fn nearest(
        &self,
        _volume_id: &str,
        _kind: VectorKind,
        _vector: &[f32],
        _limit: usize,
    ) -> Result<Vec<Near>> {
        Err(ToolError::not_supported("the index mode of the project has no vectors"))
    }

    /// The similarity of `vector` to each `(path, number)` entry asked, in the
    /// order asked; `None` for an entry with no vector at that width.
    async fn similarities(
        &self,
        _volume_id: &str,
        _kind: VectorKind,
        _vector: &[f32],
        _entries: &[(String, u32)],
    ) -> Result<Vec<Option<f32>>> {
        Err(ToolError::not_supported("the index mode of the project has no vectors"))
    }

    /// Which query modes this backend can serve ("bm25", "rag", or both).
    fn supported_modes(&self) -> Vec<&'static str>;

    /// The rebuild scheduler the write hooks hand changes to (DEC-014). `None`
    /// keeps the immediate detached indexing, which only tests that build an
    /// `AppState` by hand still use.
    fn scheduler(&self) -> Option<Arc<derived::Scheduler>> {
        None
    }
}

/// The search backend the server serves: the engine plus the rebuild scheduler
/// (DEC-014). Every engine call is delegated unchanged, so `search.index`, the
/// queries and the move and mode change paths index at once as before; only the
/// write hooks, through [`SearchBackend::scheduler`], wait for the file to settle.
pub struct ScheduledBackend {
    inner: Arc<dyn SearchBackend>,
    scheduler: Arc<derived::Scheduler>,
}

impl ScheduledBackend {
    /// Wraps `inner` and starts the scheduler's dispatcher (needs a runtime).
    pub fn new(
        inner: Arc<dyn SearchBackend>,
        store: derived::JobStore,
        stores: Arc<crate::storage::StoreManager>,
        delay_secs: u64,
        defaults: crate::chunking::ChunkDefaults,
    ) -> Self {
        let scheduler = derived::Scheduler::new(inner.clone(), store, stores, delay_secs, defaults);
        Self { inner, scheduler }
    }
}

#[async_trait]
impl SearchBackend for ScheduledBackend {
    async fn index_chunks_checked(
        &self,
        volume_id: &str,
        path: &str,
        chunks: &[IndexedChunk],
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        self.inner.index_chunks_checked(volume_id, path, chunks, still_current).await
    }

    async fn delete_path(&self, volume_id: &str, path: &str) -> Result<usize> {
        self.inner.delete_path(volume_id, path).await
    }

    async fn delete_all(&self, volume_id: &str) -> Result<usize> {
        self.inner.delete_all(volume_id).await
    }

    async fn query_bm25(
        &self,
        volume_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        self.inner.query_bm25(volume_id, query, top_k).await
    }

    async fn query_vector(
        &self,
        volume_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        self.inner.query_vector(volume_id, query, top_k).await
    }

    async fn stats(&self, volume_id: &str) -> Result<IndexStats> {
        self.inner.stats(volume_id).await
    }

    async fn index_images_checked(
        &self,
        volume_id: &str,
        path: &str,
        images: &[ImageInput],
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        self.inner.index_images_checked(volume_id, path, images, still_current).await
    }

    async fn vectors_of(
        &self,
        volume_id: &str,
        path: &str,
        kind: VectorKind,
        after: u32,
        limit: usize,
    ) -> Result<VectorPage> {
        self.inner.vectors_of(volume_id, path, kind, after, limit).await
    }

    async fn embeds_images(&self, volume_id: &str) -> Result<bool> {
        self.inner.embeds_images(volume_id).await
    }

    async fn embed_input(
        &self,
        volume_id: &str,
        kind: VectorKind,
        input: &str,
    ) -> Result<Vec<f32>> {
        self.inner.embed_input(volume_id, kind, input).await
    }

    async fn nearest(
        &self,
        volume_id: &str,
        kind: VectorKind,
        vector: &[f32],
        limit: usize,
    ) -> Result<Vec<Near>> {
        self.inner.nearest(volume_id, kind, vector, limit).await
    }

    async fn similarities(
        &self,
        volume_id: &str,
        kind: VectorKind,
        vector: &[f32],
        entries: &[(String, u32)],
    ) -> Result<Vec<Option<f32>>> {
        self.inner.similarities(volume_id, kind, vector, entries).await
    }

    fn supported_modes(&self) -> Vec<&'static str> {
        self.inner.supported_modes()
    }

    fn scheduler(&self) -> Option<Arc<derived::Scheduler>> {
        Some(self.scheduler.clone())
    }
}

/// Build the search backend for the given config.
///
/// Returns `None` when `search.enabled` is false. The backend is shared across
/// all requests, so it is returned as an `Arc`.
///
/// `relational` is the process-wide pool cache. `http_client` is reused across
/// all embedding calls for the lifetime of the process.
/// `targets` says where the vector engines embed, asked on every call
/// (SPEC-0102 DEC-007); the YAML dimension still sets the table width.
#[allow(unused_variables)] // relational, http_client and targets are only used under feature flags
pub async fn build_backend(
    config: &ServerConfig,
    relational: &crate::storage::RelationalRegistry,
    http_client: Arc<reqwest::Client>,
    targets: Arc<dyn embedding::EmbedTargets>,
) -> Result<Option<Arc<dyn SearchBackend>>> {
    if !config.search.enabled {
        return Ok(None);
    }

    let mode = config.search.mode.as_str();
    let meta_backend = config.infra.meta.backend.as_str();

    // RAG and both modes require an embedding endpoint. Config validation
    // already checks this, but guard again here so the error message is clear
    // when called from tests that skip boot validation.
    if matches!(mode, "rag" | "both") && config.search.embedding.endpoint.is_empty() {
        return Err(ToolError::invalid_argument(
            "search.mode is 'rag' or 'both' but search.embedding.endpoint is empty",
        ));
    }

    let backend: Arc<dyn SearchBackend> = match (mode, meta_backend) {
        ("bm25", "postgres") => {
            #[cfg(feature = "postgres")]
            {
                let pool = get_pg_pool(config, relational).await?;
                Arc::new(bm25_pg::PostgresBm25Backend::new(pool).await?)
            }
            #[cfg(not(feature = "postgres"))]
            {
                return Err(ToolError::not_supported(
                    "search.mode=bm25 with backend=postgres requires the postgres feature",
                ));
            }
        }
        ("bm25", _) => Arc::new(bm25_sqlite::TantivyBm25Backend::new(&config.search.tantivy_dir)),
        #[cfg(feature = "rag")]
        ("rag", "postgres") => {
            let pool = get_pg_pool(config, relational).await?;
            Arc::new(
                vector_pg::PostgresVectorBackend::new(
                    pool,
                    config.search.embedding.dimensions,
                    http_client,
                    targets,
                )
                .await?,
            )
        }
        #[cfg(feature = "rag")]
        ("rag", _) => Arc::new(vector_sqlite::SqliteVecBackend::new(
            config.search.embedding.dimensions,
            &config.search.tantivy_dir,
            http_client,
            targets,
        )),
        #[cfg(feature = "rag")]
        ("both", "postgres") => {
            let pool = get_pg_pool(config, relational).await?;
            let bm25: Arc<dyn SearchBackend> =
                Arc::new(bm25_pg::PostgresBm25Backend::new(pool.clone()).await?);
            let vector: Arc<dyn SearchBackend> = Arc::new(
                vector_pg::PostgresVectorBackend::new(
                    pool,
                    config.search.embedding.dimensions,
                    http_client,
                    targets,
                )
                .await?,
            );
            Arc::new(CombinedBackend { bm25, vector, mode: "both".into() })
        }
        #[cfg(feature = "rag")]
        ("both", _) => {
            let bm25: Arc<dyn SearchBackend> =
                Arc::new(bm25_sqlite::TantivyBm25Backend::new(&config.search.tantivy_dir));
            let vector: Arc<dyn SearchBackend> = Arc::new(vector_sqlite::SqliteVecBackend::new(
                config.search.embedding.dimensions,
                &config.search.tantivy_dir,
                http_client,
                targets,
            ));
            Arc::new(CombinedBackend { bm25, vector, mode: "both".into() })
        }
        (other, _) => {
            return Err(ToolError::invalid_argument(format!(
                "unknown search.mode '{other}', expected bm25, rag, or both"
            )));
        }
    };

    Ok(Some(backend))
}

/// Open (or reuse) a `PgPool` rooted at the same server as `infra.meta`.
///
/// We cannot downcast `Arc<dyn RelationalDb>` to `PostgresRelationalDb` to
/// extract the pool, so a second pool object is created against the same DSN.
/// PostgreSQL handles multiple pool objects sharing one server without issue;
/// the pool is bounded by `pool.max_connections` so the connection count stays
/// within the operator's configured limit.
#[cfg(feature = "postgres")]
async fn get_pg_pool(
    config: &ServerConfig,
    _relational: &crate::storage::RelationalRegistry,
) -> Result<sqlx::PgPool> {
    use crate::storage::rel::{PoolSettings, PostgresRelationalDb};
    let m = &config.infra.meta;
    let pg = PostgresRelationalDb::connect(
        m.dsn.expose(),
        &m.schema,
        PoolSettings {
            max_connections: m.pool.max_connections,
            acquire_timeout: std::time::Duration::from_secs(m.pool.acquire_timeout_secs),
        },
    )
    .await?;
    Ok(pg.pool().clone())
}

/// A backend that runs both BM25 and vector search in parallel, merging with RRF.
/// Only compiled when the `rag` feature is on.
#[cfg(feature = "rag")]
struct CombinedBackend {
    bm25: Arc<dyn SearchBackend>,
    vector: Arc<dyn SearchBackend>,
    mode: String,
}

#[cfg(feature = "rag")]
#[async_trait]
impl SearchBackend for CombinedBackend {
    async fn index_chunks_checked(
        &self,
        volume_id: &str,
        path: &str,
        chunks: &[IndexedChunk],
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        // The vector side goes first because embedding is what can fail: an error
        // or a refusal there happens before BM25 is touched, so every engine keeps
        // the old entries. The bm25 entry count stays the authoritative one.
        let Some(_) =
            self.vector.index_chunks_checked(volume_id, path, chunks, still_current).await?
        else {
            return Ok(None);
        };
        self.bm25.index_chunks_checked(volume_id, path, chunks, still_current).await
    }

    async fn delete_path(&self, volume_id: &str, path: &str) -> Result<usize> {
        let n1 = self.bm25.delete_path(volume_id, path).await?;
        let _ = self.vector.delete_path(volume_id, path).await?;
        Ok(n1)
    }

    async fn delete_all(&self, volume_id: &str) -> Result<usize> {
        // Both halves are wiped even if the first reports nothing, otherwise a
        // mode change would leave the vector store holding stale chunks.
        let n1 = self.bm25.delete_all(volume_id).await?;
        let n2 = self.vector.delete_all(volume_id).await?;
        Ok(n1 + n2)
    }

    async fn query_bm25(
        &self,
        volume_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        self.bm25.query_bm25(volume_id, query, top_k).await
    }

    async fn query_vector(
        &self,
        volume_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        self.vector.query_vector(volume_id, query, top_k).await
    }

    async fn stats(&self, volume_id: &str) -> Result<IndexStats> {
        let s1 = self.bm25.stats(volume_id).await?;
        let s2 = self.vector.stats(volume_id).await?;
        Ok(IndexStats {
            bm25_docs: s1.bm25_docs,
            vector_chunks: s2.vector_chunks,
            bm25_warm: s1.bm25_warm,
            mode: self.mode.clone(),
        })
    }

    async fn index_images_checked(
        &self,
        volume_id: &str,
        path: &str,
        images: &[ImageInput],
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        self.vector.index_images_checked(volume_id, path, images, still_current).await
    }

    async fn vectors_of(
        &self,
        volume_id: &str,
        path: &str,
        kind: VectorKind,
        after: u32,
        limit: usize,
    ) -> Result<VectorPage> {
        self.vector.vectors_of(volume_id, path, kind, after, limit).await
    }

    async fn embeds_images(&self, volume_id: &str) -> Result<bool> {
        self.vector.embeds_images(volume_id).await
    }

    async fn embed_input(
        &self,
        volume_id: &str,
        kind: VectorKind,
        input: &str,
    ) -> Result<Vec<f32>> {
        self.vector.embed_input(volume_id, kind, input).await
    }

    async fn nearest(
        &self,
        volume_id: &str,
        kind: VectorKind,
        vector: &[f32],
        limit: usize,
    ) -> Result<Vec<Near>> {
        self.vector.nearest(volume_id, kind, vector, limit).await
    }

    async fn similarities(
        &self,
        volume_id: &str,
        kind: VectorKind,
        vector: &[f32],
        entries: &[(String, u32)],
    ) -> Result<Vec<Option<f32>>> {
        self.vector.similarities(volume_id, kind, vector, entries).await
    }

    fn supported_modes(&self) -> Vec<&'static str> {
        vec!["bm25", "rag", "both"]
    }
}

#[cfg(test)]
mod guarded_tests {
    use super::*;

    /// US-0264/T1: a refused guard stores nothing and the old entries stay.
    #[tokio::test]
    async fn guarded_refusal_keeps_old_entries_tantivy() {
        let dir = tempfile::tempdir().unwrap();
        let b = bm25_sqlite::TantivyBm25Backend::new(dir.path().to_str().unwrap());
        crate::search::index_text(&b, "v", "/a.md", "alpha beta").await.unwrap();

        let r = b
            .index_chunks_checked(
                "v",
                "/a.md",
                &crate::search::test_chunks("/a.md", "gamma delta"),
                &|| false,
            )
            .await;
        assert!(matches!(r, Ok(None)), "a refused guard must answer Ok(None), got {r:?}");
        let alpha = b.query_bm25("v", "alpha", 10).await.unwrap();
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].path, "/a.md");
        assert!(b.query_bm25("v", "gamma", 10).await.unwrap().is_empty());
    }

    /// An accepted guard replaces the entries and reports the chunk count.
    #[tokio::test]
    async fn guarded_acceptance_replaces_entries_tantivy() {
        let dir = tempfile::tempdir().unwrap();
        let b = bm25_sqlite::TantivyBm25Backend::new(dir.path().to_str().unwrap());
        crate::search::index_text(&b, "v", "/a.md", "alpha beta").await.unwrap();

        let r = b
            .index_chunks_checked(
                "v",
                "/a.md",
                &crate::search::test_chunks("/a.md", "gamma delta"),
                &|| true,
            )
            .await;
        assert!(matches!(r, Ok(Some(1))), "got {r:?}");
        assert!(b.query_bm25("v", "alpha", 10).await.unwrap().is_empty());
        assert_eq!(b.query_bm25("v", "gamma", 10).await.unwrap().len(), 1);
    }

    #[cfg(feature = "rag")]
    mod rag {
        use super::*;
        use crate::config::EmbeddingConfig;

        /// A fake endpoint that answers, or (`up == false`) a port nobody listens on.
        async fn endpoint(up: bool) -> String {
            use axum::{Json, Router, routing::post};
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            if up {
                let router = Router::new().route(
                    "/v1/embeddings",
                    post(|| async {
                        Json(serde_json::json!({
                            "data": [{"embedding": [0.1, 0.2, 0.3, 0.4], "index": 0}]
                        }))
                    }),
                );
                tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
            } else {
                drop(listener);
            }
            format!("http://127.0.0.1:{port}/v1/embeddings")
        }

        fn combined(dir: &std::path::Path, endpoint: String) -> CombinedBackend {
            crate::storage::sqlite::register_sqlite_vec();
            let client = Arc::new(reqwest::Client::new());
            let cfg = EmbeddingConfig {
                endpoint,
                model: "fake".into(),
                api_key_env: String::new(),
                dimensions: 4,
            };
            let dir = dir.to_str().unwrap();
            let bm25: Arc<dyn SearchBackend> = Arc::new(bm25_sqlite::TantivyBm25Backend::new(dir));
            let vector: Arc<dyn SearchBackend> = Arc::new(vector_sqlite::SqliteVecBackend::new(
                4,
                dir,
                client,
                Arc::new(embedding::StaticTargets(cfg)),
            ));
            CombinedBackend { bm25, vector, mode: "both".into() }
        }

        /// US-0264/T2: the vector side fails first, so BM25 keeps the old entries.
        #[tokio::test]
        async fn guarded_combined_keeps_bm25_when_the_vector_side_fails() {
            let dir = tempfile::tempdir().unwrap();
            let up = endpoint(true).await;
            let c = combined(dir.path(), up);
            crate::search::index_text(&c, "v", "/a.md", "alpha").await.unwrap();
            // The endpoint is stopped: rebuild the combined engine would not share
            // Tantivy handles, so swap only the vector half to a dead endpoint.
            let down = endpoint(false).await;
            let c2 = CombinedBackend {
                bm25: c.bm25.clone(),
                vector: combined(dir.path(), down).vector,
                mode: "both".into(),
            };
            let r = c2
                .index_chunks_checked(
                    "v",
                    "/a.md",
                    &crate::search::test_chunks("/a.md", "gamma"),
                    &|| true,
                )
                .await;
            assert!(r.is_err(), "an unreachable endpoint must fail the update");
            let alpha = c2.query_bm25("v", "alpha", 10).await.unwrap();
            assert_eq!(alpha.len(), 1);
            assert_eq!(alpha[0].path, "/a.md");
            assert!(c2.query_bm25("v", "gamma", 10).await.unwrap().is_empty());
        }

        /// A refused guard stores nothing on either side of the combined engine.
        #[tokio::test]
        async fn guarded_combined_refusal_stores_nothing() {
            let dir = tempfile::tempdir().unwrap();
            let up = endpoint(true).await;
            let c = combined(dir.path(), up);
            crate::search::index_text(&c, "v", "/a.md", "alpha").await.unwrap();

            let r = c
                .index_chunks_checked(
                    "v",
                    "/a.md",
                    &crate::search::test_chunks("/a.md", "gamma"),
                    &|| false,
                )
                .await;
            assert!(matches!(r, Ok(None)), "got {r:?}");
            assert_eq!(c.query_bm25("v", "alpha", 10).await.unwrap().len(), 1);
            assert!(c.query_bm25("v", "gamma", 10).await.unwrap().is_empty());
            let v = c.query_vector("v", "alpha", 10).await.unwrap();
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].chunk, "a\nalpha", "the vector side keeps the old entry");
        }
    }
}
