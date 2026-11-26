//! pgvector vector-search backend.
//!
//! Stores chunk and image embeddings in `VECTOR(N)` columns backed by the
//! `vector` extension, one table per kind and dimension (SPEC-0108 DEC-001):
//! `search_chunks` for the text vectors of the platform width (the table that
//! existed before projects chose their model, so stored vectors stay valid),
//! `search_chunks_<dim>` for any other text width, `search_images_<dim>` for
//! images. A table is created on first use; its name is built from an integer
//! only. The extension is enabled per connection in `storage/rel/postgres.rs`
//! (`CREATE EXTENSION IF NOT EXISTS vector`).
//!
//! Both `index_chunks_checked` and `query_vector` call the embedding endpoint of
//! the project (its profile model, else the platform default) to produce float
//! vectors; callers never need to pre-compute embeddings.
//!
//! Raw `sqlx` is used because the `VECTOR` type and its operators (`<=>`) are
//! not representable through `SchemaSet` / `RelationalDb`.

#![cfg(feature = "rag")]

use crate::errors::{Result, ToolError};
use crate::search::{
    ImageInput, IndexStats, IndexedChunk, SearchBackend, SearchResult, StoredVector, VectorKind,
    VectorPage, embedding,
};
use async_trait::async_trait;
use sqlx::AssertSqlSafe;
use sqlx::Row;
use std::collections::HashSet;
use std::sync::Arc;

/// pgvector's `ivfflat` and `hnsw` indexes hold at most this many dimensions; a
/// wider table is scanned exactly, without an index.
const MAX_INDEXED_DIMENSION: u32 = 2000;

/// The table of one kind and width. `platform` is the width of the legacy table.
pub(crate) fn table_name(kind: VectorKind, dimension: u32, platform: u32) -> String {
    match kind {
        VectorKind::Text if dimension == platform => "search_chunks".to_string(),
        VectorKind::Text => format!("search_chunks_{dimension}"),
        VectorKind::Image => format!("search_images_{dimension}"),
    }
}

/// The kind and width a table name stands for; `None` for the legacy table's
/// width (the platform's) and for a name that is not a vector table.
fn parse_table(name: &str) -> Option<(VectorKind, Option<u32>)> {
    if name == "search_chunks" {
        return Some((VectorKind::Text, None));
    }
    let (kind, rest) = if let Some(r) = name.strip_prefix("search_chunks_") {
        (VectorKind::Text, r)
    } else {
        (VectorKind::Image, name.strip_prefix("search_images_")?)
    };
    rest.parse::<u32>().ok().map(|d| (kind, Some(d)))
}

/// pgvector-backed vector search backend.
pub struct PostgresVectorBackend {
    pool: sqlx::PgPool,
    /// `search.embedding.dimensions`: the width of the legacy table and of a
    /// project that names no model.
    dimensions: u32,
    client: Arc<reqwest::Client>,
    /// Where to embed, asked on every call (SPEC-0102 DEC-007).
    targets: Arc<dyn embedding::EmbedTargets>,
    /// Tables this process has created or seen, so DDL runs once per table.
    ready: tokio::sync::Mutex<HashSet<String>>,
}

impl PostgresVectorBackend {
    /// Create the backend and ensure the table of the platform width exists.
    ///
    /// The `CREATE EXTENSION IF NOT EXISTS vector` statement is executed by the
    /// pool's `after_connect` hook in `storage/rel/postgres.rs`, so it is
    /// already present when we arrive here.
    pub async fn new(
        pool: sqlx::PgPool,
        dimensions: u32,
        client: Arc<reqwest::Client>,
        targets: Arc<dyn embedding::EmbedTargets>,
    ) -> Result<Self> {
        let b = Self { pool, dimensions, client, targets, ready: Default::default() };
        b.ensure(VectorKind::Text, dimensions).await?;
        Ok(b)
    }

    /// The table of `kind` at `dimension`, created when missing. One lock for the
    /// whole DDL: two first uses of a width in this process never race.
    async fn ensure(&self, kind: VectorKind, dimension: u32) -> Result<String> {
        let name = table_name(kind, dimension, self.dimensions);
        let mut ready = self.ready.lock().await;
        if ready.contains(&name) {
            return Ok(name);
        }
        // The VECTOR(N) column type requires the dimension to be part of the
        // DDL string; it cannot be passed as a bind parameter.
        let create = format!(
            "CREATE TABLE IF NOT EXISTS {name} (\
                volume_id  TEXT     NOT NULL,\
                path       TEXT     NOT NULL,\
                chunk_idx  INTEGER  NOT NULL,\
                chunk_text TEXT     NOT NULL,\
                embedding  VECTOR({dimension}) NOT NULL,\
                PRIMARY KEY (volume_id, path, chunk_idx)\
            )"
        );
        sqlx::query(AssertSqlSafe(create))
            .execute(&self.pool)
            .await
            .map_err(|e| ToolError::internal(format!("vector_pg: create table: {e}")))?;
        if dimension <= MAX_INDEXED_DIMENSION {
            // The table of the platform width keeps the index it was created with;
            // a table created empty gets `hnsw`, which needs no training data (an
            // `ivfflat` index built on no row has no useful lists).
            let method = if name == "search_chunks" { "ivfflat" } else { "hnsw" };
            let index = format!(
                "CREATE INDEX IF NOT EXISTS idx_{name}_emb \
                 ON {name} USING {method} (embedding vector_cosine_ops)"
            );
            sqlx::query(AssertSqlSafe(index))
                .execute(&self.pool)
                .await
                .map_err(|e| ToolError::internal(format!("vector_pg: create index: {e}")))?;
        }
        ready.insert(name.clone());
        Ok(name)
    }

    /// Every existing table of `kind`, with its width (the platform's for the
    /// legacy one).
    async fn tables(&self, kind: VectorKind) -> Result<Vec<(String, u32)>> {
        let rows = sqlx::query(AssertSqlSafe(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = current_schema() \
               AND (table_name LIKE 'search\\_chunks%' OR table_name LIKE 'search\\_images\\_%') \
             ORDER BY table_name",
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ToolError::internal(format!("vector_pg: list tables: {e}")))?;
        let mut out = Vec::new();
        for row in rows {
            let name: String = row
                .try_get("table_name")
                .map_err(|e| ToolError::internal(format!("vector_pg: read table: {e}")))?;
            if let Some((k, d)) = parse_table(&name)
                && k == kind
            {
                out.push((name, d.unwrap_or(self.dimensions)));
            }
        }
        Ok(out)
    }

    /// Deletes the rows of a path (or of the whole volume when `path` is `None`)
    /// in every table of `kind`; the number of rows removed.
    async fn purge(&self, kind: VectorKind, volume_id: &str, path: Option<&str>) -> Result<usize> {
        let mut total = 0usize;
        for (table, _) in self.tables(kind).await? {
            total += delete_in(&self.pool, &table, volume_id, path).await?;
        }
        Ok(total)
    }
}

/// `DELETE` on one table, answering the rows removed.
async fn delete_in<'e, E>(
    exec: E,
    table: &str,
    volume_id: &str,
    path: Option<&str>,
) -> Result<usize>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let sql = match path {
        Some(_) => format!(
            "WITH deleted AS (DELETE FROM {table} WHERE volume_id = $1 AND path = $2 \
             RETURNING 1) SELECT COUNT(*) AS cnt FROM deleted"
        ),
        None => format!(
            "WITH deleted AS (DELETE FROM {table} WHERE volume_id = $1 RETURNING 1) \
             SELECT COUNT(*) AS cnt FROM deleted"
        ),
    };
    let mut q = sqlx::query(AssertSqlSafe(sql)).bind(volume_id);
    if let Some(p) = path {
        q = q.bind(p);
    }
    let row = q
        .fetch_one(exec)
        .await
        .map_err(|e| ToolError::internal(format!("vector_pg: delete: {e}")))?;
    let cnt: i64 = row
        .try_get("cnt")
        .map_err(|e| ToolError::internal(format!("vector_pg: read count: {e}")))?;
    Ok(usize::try_from(cnt).unwrap_or(0))
}

/// Format a `Vec<f32>` as a pgvector literal string, e.g. `"[0.1,0.2,0.3]"`.
fn vec_to_pg(v: &[f32]) -> String {
    let inner = v.iter().map(f32::to_string).collect::<Vec<_>>().join(",");
    format!("[{inner}]")
}

/// The values of a pgvector text literal.
fn vec_from_pg(text: &str) -> Result<Vec<f32>> {
    text.trim_matches(['[', ']'])
        .split(',')
        .map(|x| {
            x.trim()
                .parse::<f32>()
                .map_err(|e| ToolError::internal(format!("vector_pg: read vector: {e}")))
        })
        .collect()
}

#[async_trait]
impl SearchBackend for PostgresVectorBackend {
    async fn index_chunks_checked(
        &self,
        volume_id: &str,
        path: &str,
        chunks: &[IndexedChunk],
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        let chunks = chunks.to_vec();
        let n = chunks.len();

        // Compute all embeddings before touching the database. Embedding calls
        // are async and must not run inside a database transaction.
        let project = self.targets.target_for(volume_id).await?;
        let dimension = project.dimension.unwrap_or(self.dimensions);
        let mut embeddings: Vec<String> = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            let emb =
                embedding::embed_checked(&self.client, &project.target, &chunk.text, dimension)
                    .await?;
            embeddings.push(vec_to_pg(&emb));
        }

        // After the embeddings, before the delete: a refusal keeps the old rows.
        if !still_current() {
            return Ok(None);
        }
        let table = self.ensure(VectorKind::Text, dimension).await?;
        let stale_tables = self.tables(VectorKind::Text).await?;

        // One transaction: a reader sees all the old rows or all the new ones, and
        // an error returns before commit, so the drop rolls the delete back.
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ToolError::internal(format!("vector_pg: begin: {e}")))?;

        // Idempotent: remove existing rows of this (volume, path) pair at every
        // width, so a model change leaves no vector of the old width (FR-NEW-004).
        for (t, _) in &stale_tables {
            delete_in(&mut *tx, t, volume_id, Some(path)).await?;
        }

        let insert = format!(
            "INSERT INTO {table} (volume_id, path, chunk_idx, chunk_text, embedding) \
             VALUES ($1, $2, $3, $4, $5::vector)"
        );
        for (chunk, vec_str) in chunks.iter().zip(embeddings.iter()) {
            // The embedding is passed as a TEXT parameter and cast to VECTOR with
            // the `::vector` suffix, which is the recommended approach for dynamic
            // vector values in sqlx where no native pgvector type binding exists.
            sqlx::query(AssertSqlSafe(insert.as_str()))
                .bind(volume_id)
                .bind(path)
                .bind(chunk.number as i32)
                .bind(chunk.text.as_str())
                .bind(vec_str.as_str())
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    ToolError::internal(format!("vector_pg: insert chunk {}: {e}", chunk.number))
                })?;
        }

        tx.commit().await.map_err(|e| ToolError::internal(format!("vector_pg: commit: {e}")))?;

        Ok(Some(n))
    }

    async fn index_images_checked(
        &self,
        volume_id: &str,
        path: &str,
        images: &[ImageInput],
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        let project = self.targets.image_target_for(volume_id).await?;
        let mut rows: Vec<(u32, String, String)> = Vec::new();
        let mut dimension = None;
        if let Some(project) = project.filter(|_| !images.is_empty()) {
            let d = project.dimension.unwrap_or(self.dimensions);
            dimension = Some(d);
            for image in images {
                let v = embedding::embed_checked(&self.client, &project.target, &image.data_uri, d)
                    .await?;
                rows.push((image.number, image.text.clone(), vec_to_pg(&v)));
            }
        }
        if !still_current() {
            return Ok(None);
        }
        let table = match dimension {
            Some(d) => Some(self.ensure(VectorKind::Image, d).await?),
            None => None,
        };
        let old = self.tables(VectorKind::Image).await?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ToolError::internal(format!("vector_pg: begin: {e}")))?;
        for (t, _) in &old {
            delete_in(&mut *tx, t, volume_id, Some(path)).await?;
        }
        if let Some(table) = &table {
            let insert = format!(
                "INSERT INTO {table} (volume_id, path, chunk_idx, chunk_text, embedding) \
                 VALUES ($1, $2, $3, $4, $5::vector)"
            );
            for (number, text, vec_str) in &rows {
                sqlx::query(AssertSqlSafe(insert.as_str()))
                    .bind(volume_id)
                    .bind(path)
                    .bind(*number as i32)
                    .bind(text.as_str())
                    .bind(vec_str.as_str())
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| ToolError::internal(format!("vector_pg: insert image: {e}")))?;
            }
        }
        tx.commit().await.map_err(|e| ToolError::internal(format!("vector_pg: commit: {e}")))?;
        Ok(Some(rows.len()))
    }

    async fn embeds_images(&self, volume_id: &str) -> Result<bool> {
        Ok(self.targets.image_target_for(volume_id).await?.is_some())
    }

    async fn vectors_of(
        &self,
        volume_id: &str,
        path: &str,
        kind: VectorKind,
        after: u32,
        limit: usize,
    ) -> Result<VectorPage> {
        for (table, dimension) in self.tables(kind).await? {
            let any = sqlx::query(AssertSqlSafe(format!(
                "SELECT 1 FROM {table} WHERE volume_id = $1 AND path = $2 LIMIT 1"
            )))
            .bind(volume_id)
            .bind(path)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ToolError::internal(format!("vector_pg: read vectors: {e}")))?;
            if any.is_none() {
                continue;
            }
            let rows = sqlx::query(AssertSqlSafe(format!(
                "SELECT chunk_idx, chunk_text, embedding::text AS emb FROM {table} \
                 WHERE volume_id = $1 AND path = $2 AND chunk_idx > $3 \
                 ORDER BY chunk_idx LIMIT $4"
            )))
            .bind(volume_id)
            .bind(path)
            .bind(i32::try_from(after).unwrap_or(i32::MAX))
            .bind(i64::try_from(limit + 1).unwrap_or(i64::MAX))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ToolError::internal(format!("vector_pg: read vectors: {e}")))?;
            let mut vectors = Vec::with_capacity(rows.len());
            for row in &rows {
                let idx: i32 = row
                    .try_get("chunk_idx")
                    .map_err(|e| ToolError::internal(format!("vector_pg: read chunk_idx: {e}")))?;
                let text: String = row
                    .try_get("chunk_text")
                    .map_err(|e| ToolError::internal(format!("vector_pg: read chunk_text: {e}")))?;
                let emb: String = row
                    .try_get("emb")
                    .map_err(|e| ToolError::internal(format!("vector_pg: read embedding: {e}")))?;
                vectors.push(StoredVector {
                    number: u32::try_from(idx).unwrap_or(0),
                    text,
                    vector: vec_from_pg(&emb)?,
                });
            }
            let more = vectors.len() > limit;
            vectors.truncate(limit);
            let next_after = if more { vectors.last().map(|v| v.number) } else { None };
            return Ok(VectorPage { dimension: Some(dimension), vectors, next_after });
        }
        Ok(VectorPage { dimension: None, vectors: Vec::new(), next_after: None })
    }

    async fn delete_path(&self, volume_id: &str, path: &str) -> Result<usize> {
        let n = self.purge(VectorKind::Text, volume_id, Some(path)).await?;
        self.purge(VectorKind::Image, volume_id, Some(path)).await?;
        Ok(n)
    }

    async fn delete_all(&self, volume_id: &str) -> Result<usize> {
        let n = self.purge(VectorKind::Text, volume_id, None).await?;
        self.purge(VectorKind::Image, volume_id, None).await?;
        Ok(n)
    }

    async fn query_bm25(
        &self,
        _volume_id: &str,
        _query: &str,
        _top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        Err(ToolError::not_supported(
            "BM25 search is not available in rag mode; use mode=bm25 or mode=both",
        ))
    }

    async fn query_vector(
        &self,
        volume_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        let project = self.targets.target_for(volume_id).await?;
        let dimension = project.dimension.unwrap_or(self.dimensions);
        let emb = embedding::embed_checked(&self.client, &project.target, query, dimension).await?;
        let vec_str = vec_to_pg(&emb);
        let table = self.ensure(VectorKind::Text, dimension).await?;

        // `<=>` is the cosine distance operator provided by pgvector.
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT path, chunk_text, chunk_idx, \
                    (embedding <=> $1::vector) AS distance \
             FROM {table} \
             WHERE volume_id = $2 \
             ORDER BY distance ASC \
             LIMIT $3"
        )))
        .bind(vec_str.as_str())
        .bind(volume_id)
        .bind(top_k as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ToolError::internal(format!("vector_pg: query: {e}")))?;

        let mut results = Vec::with_capacity(rows.len());
        for (i, row) in rows.iter().enumerate() {
            let path: String = row
                .try_get("path")
                .map_err(|e| ToolError::internal(format!("vector_pg: read path: {e}")))?;
            let chunk: String = row
                .try_get("chunk_text")
                .map_err(|e| ToolError::internal(format!("vector_pg: read chunk_text: {e}")))?;
            let distance: f64 = row
                .try_get("distance")
                .map_err(|e| ToolError::internal(format!("vector_pg: read distance: {e}")))?;
            // Convert cosine distance [0,2] to a similarity score in [0,1].
            let score = 1.0 - (distance as f32 / 2.0);
            let idx: i32 = row
                .try_get("chunk_idx")
                .map_err(|e| ToolError::internal(format!("vector_pg: read chunk_idx: {e}")))?;
            let chunk_idx = u32::try_from(idx).unwrap_or(0);
            results.push(SearchResult { path, score, chunk, rank: i + 1, chunk_idx });
        }
        Ok(results)
    }

    async fn stats(&self, volume_id: &str) -> Result<IndexStats> {
        let mut cnt = 0i64;
        for (table, _) in self.tables(VectorKind::Text).await? {
            let row = sqlx::query(AssertSqlSafe(format!(
                "SELECT COUNT(*) AS cnt FROM {table} WHERE volume_id = $1"
            )))
            .bind(volume_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ToolError::internal(format!("vector_pg: stats: {e}")))?;
            let n: i64 = row
                .try_get("cnt")
                .map_err(|e| ToolError::internal(format!("vector_pg: read stats count: {e}")))?;
            cnt += n;
        }

        Ok(IndexStats {
            bm25_docs: 0,
            vector_chunks: cnt as usize,
            bm25_warm: false,
            mode: "rag".into(),
        })
    }

    fn supported_modes(&self) -> Vec<&'static str> {
        vec!["rag"]
    }
}

#[cfg(test)]
mod tests {
    // Live tests require MCPFS_TEST_PG_DSN and a Postgres instance with the
    // `vector` extension. They skip automatically when the variable is unset.
    //
    // Tests that call `index_chunks_checked` or `query_vector` need an embedding endpoint.
    // Rather than require a live LLM, an inline axum stub is spawned that returns
    // a fixed 3-dimensional vector for any input. This keeps the tests self-
    // contained and deterministic.

    use super::*;
    use crate::config::EmbeddingConfig;
    use crate::errors::code;
    use crate::search::SearchBackend;
    use axum::routing::post;
    use axum::{Json, Router};

    // ── helpers ──────────────────────────────────────────────────────────────

    async fn live_backend(
        schema: &str,
        dims: u32,
        endpoint: &str,
    ) -> Option<PostgresVectorBackend> {
        let dsn = std::env::var("MCPFS_TEST_PG_DSN").ok().filter(|v| !v.trim().is_empty())?;
        use crate::storage::rel::{PoolSettings, PostgresRelationalDb};
        let db = PostgresRelationalDb::connect(&dsn, schema, PoolSettings::default())
            .await
            .expect("MCPFS_TEST_PG_DSN is set but connection failed");
        let client = Arc::new(reqwest::Client::new());
        let cfg = EmbeddingConfig {
            endpoint: endpoint.to_string(),
            model: "fake".to_string(),
            api_key_env: String::new(),
            dimensions: dims,
        };
        Some(
            PostgresVectorBackend::new(
                db.pool().clone(),
                dims,
                client,
                Arc::new(crate::search::embedding::StaticTargets(cfg)),
            )
            .await
            .unwrap(),
        )
    }

    /// Spawn a minimal axum server that returns a fixed 3-dim embedding for any
    /// POST to `/v1/embeddings`. Returns the base URL, e.g. `http://127.0.0.1:PORT`.
    async fn spawn_fake_embedding_server(dims: usize) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = Router::new().route(
            "/v1/embeddings",
            post(move || async move {
                // Return a fixed unit vector of the requested dimension.
                let vec: Vec<f32> = (0..dims).map(|i| (i + 1) as f32 / dims as f32).collect();
                Json(serde_json::json!({
                    "data": [{"embedding": vec, "index": 0}],
                    "model": "fake",
                    "usage": {"prompt_tokens": 1, "total_tokens": 1}
                }))
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        format!("http://127.0.0.1:{port}")
    }

    // ── tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn live_stats_zero_on_empty_volume() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) = live_backend("fsa_vecpg_stats0", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        let s = b.stats("empty_vol").await.unwrap();
        assert_eq!(s.vector_chunks, 0);
    }

    #[tokio::test]
    async fn live_stats_counts_chunks_after_index() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) = live_backend("fsa_vecpg_stats2", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        crate::search::index_text(&b, "v1", "/a.md", "alpha beta gamma").await.unwrap();
        crate::search::index_text(&b, "v1", "/b.md", "delta epsilon zeta").await.unwrap();
        let s = b.stats("v1").await.unwrap();
        assert_eq!(s.vector_chunks, 2, "expected 2 chunks after indexing 2 files");
    }

    #[tokio::test]
    async fn live_index_and_query() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) = live_backend("fsa_vecpg_query", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        crate::search::index_text(&b, "v1", "/doc.md", "rust is a systems language").await.unwrap();
        let r = b.query_vector("v1", "systems programming", 10).await.unwrap();
        // The fake embedding server returns the same vector for every input, so
        // the query finds the document (cosine distance 0 = identical vectors).
        assert!(!r.is_empty(), "expected at least one result");
        assert_eq!(r[0].path, "/doc.md");
        assert!(r[0].score > 0.0, "score must be positive");
        assert_eq!(r[0].rank, 1);
    }

    #[tokio::test]
    async fn live_delete_removes_chunks() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) = live_backend("fsa_vecpg_del", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        crate::search::index_text(&b, "v1", "/c.md", "content to delete").await.unwrap();
        let deleted = b.delete_path("v1", "/c.md").await.unwrap();
        assert!(deleted > 0, "delete must report at least one chunk removed");
        let s = b.stats("v1").await.unwrap();
        assert_eq!(s.vector_chunks, 0, "stats must reflect deletion");
    }

    #[tokio::test]
    async fn live_idempotent_reindex() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) = live_backend("fsa_vecpg_idem", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        crate::search::index_text(&b, "v1", "/d.md", "first content").await.unwrap();
        // Re-index must replace, not duplicate.
        crate::search::index_text(&b, "v1", "/d.md", "second content").await.unwrap();
        let s = b.stats("v1").await.unwrap();
        assert_eq!(s.vector_chunks, 1, "re-index must replace, not append");
    }

    #[tokio::test]
    async fn live_volume_isolation() {
        let base = spawn_fake_embedding_server(3).await;
        let ep = format!("{base}/v1/embeddings");
        let Some(b) = live_backend("fsa_vecpg_iso", 3, &ep).await else { return };
        crate::search::index_text(&b, "vol_a", "/shared.md", "content in a").await.unwrap();
        crate::search::index_text(&b, "vol_b", "/shared.md", "content in b").await.unwrap();
        assert_eq!(b.stats("vol_a").await.unwrap().vector_chunks, 1);
        assert_eq!(b.stats("vol_b").await.unwrap().vector_chunks, 1);
        // query_vector must only return rows from the requested volume.
        let ra = b.query_vector("vol_a", "content", 10).await.unwrap();
        assert!(ra.iter().all(|r| r.path == "/shared.md"));
    }

    #[tokio::test]
    async fn live_delete_all_removes_every_chunk_of_the_volume() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) = live_backend("fsa_vecpg_delall", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        crate::search::index_text(&b, "v1", "/a.md", "alpha content").await.unwrap();
        crate::search::index_text(&b, "v1", "/b.md", "beta content").await.unwrap();
        assert_eq!(b.stats("v1").await.unwrap().vector_chunks, 2);

        assert_eq!(b.delete_all("v1").await.unwrap(), 2, "delete_all reports what it removed");
        assert_eq!(b.stats("v1").await.unwrap().vector_chunks, 0);
        assert!(b.query_vector("v1", "content", 10).await.unwrap().is_empty());
    }

    /// One database holds every volume, so a wipe without the volume_id filter
    /// would empty another project's index.
    #[tokio::test]
    async fn live_delete_all_is_scoped_to_one_volume() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) =
            live_backend("fsa_vecpg_delall_iso", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        crate::search::index_text(&b, "vol_a", "/x.md", "content a").await.unwrap();
        crate::search::index_text(&b, "vol_b", "/x.md", "content b").await.unwrap();
        b.delete_all("vol_a").await.unwrap();
        assert_eq!(b.stats("vol_a").await.unwrap().vector_chunks, 0);
        assert_eq!(b.stats("vol_b").await.unwrap().vector_chunks, 1, "the other volume survives");
    }

    #[tokio::test]
    async fn live_delete_all_on_an_empty_volume_is_zero() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) =
            live_backend("fsa_vecpg_delall_empty", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        assert_eq!(b.delete_all("never-seen").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn live_query_bm25_returns_not_supported() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) = live_backend("fsa_vecpg_norag", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        let err = b.query_bm25("v1", "anything", 10).await.unwrap_err();
        assert_eq!(err.code, code::NOT_SUPPORTED);
    }
    #[tokio::test]
    async fn one_transaction_vector_pg_rolls_back_a_half_replacement() {
        let base = spawn_fake_embedding_server(3).await;
        let Some(b) = live_backend("fsa_vecpg_onetx", 3, &format!("{base}/v1/embeddings")).await
        else {
            return;
        };
        let vol = "onetx_vec";
        b.delete_all(vol).await.unwrap();
        let old = crate::search::entries(&["oldoldoldo"; 3]);
        assert_eq!(b.index_chunks_checked(vol, "/a.md", &old, &|| true).await.unwrap(), Some(3));
        assert_eq!(b.stats(vol).await.unwrap().vector_chunks, 3);

        // A NUL byte in the last chunk makes the third insert fail.
        let bad = crate::search::entries(&["nnnnnnnnnn", "nnnnnnnnnn", "\0nnnnnnnn"]);
        assert!(b.index_chunks_checked(vol, "/a.md", &bad, &|| true).await.is_err());

        assert_eq!(b.stats(vol).await.unwrap().vector_chunks, 3, "old rows kept");
        let r = b.query_vector(vol, "old", 10).await.unwrap();
        assert!(!r.is_empty(), "old rows still returned");
        assert!(r.iter().all(|x| !x.chunk.contains('n')), "only old chunk text: {r:?}");
    }
}
