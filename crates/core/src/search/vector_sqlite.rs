//! sqlite-vec vector store backend.
//!
//! Stores embeddings in `vec0` virtual tables alongside companion metadata
//! tables, per project, in memory. The sqlite-vec extension is registered once
//! at process startup via `sqlite3_auto_extension` (see `storage/sqlite.rs`).
//!
//! Each kind (text chunks, images) has its own pair of tables, created at the
//! width of the project's model on the first write (SPEC-0108 DEC-001). A write
//! at another width replaces the tables, since vectors of two widths cannot be
//! compared; a query at another width finds nothing and changes nothing.
//!
//! Each `index_chunks_checked` call embeds every chunk via the project's
//! embedding endpoint before storing the resulting float vector.
//! `query_vector` likewise embeds the query string before running the KNN
//! `MATCH` scan, so callers never need to produce their own vectors.

#![cfg(feature = "rag")]

use crate::errors::{Result, ToolError};
use crate::search::{
    ImageInput, IndexStats, IndexedChunk, Near, SearchBackend, SearchResult, StoredVector,
    VectorKind, VectorPage, embedding, similarity_of_distance,
};
use async_trait::async_trait;
use rusqlite::{Connection, params};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// The metadata table of a kind and the name of its `vec0` table at a width.
fn names(kind: VectorKind, dimension: u32) -> (&'static str, String) {
    match kind {
        VectorKind::Text => ("search_vec_meta", format!("search_vec_{dimension}")),
        VectorKind::Image => ("search_img_meta", format!("search_img_{dimension}")),
    }
}

fn meta_table(kind: VectorKind) -> &'static str {
    names(kind, 0).0
}

/// One open connection per volume, created on first use.
struct VolumeConn {
    conn: Connection,
    /// The widths each kind has a `vec0` table for, created by the first write at
    /// that width. Vectors of two widths cannot be compared, so each width has its
    /// own table and a query reads the one of the width in force.
    text_dims: BTreeSet<u32>,
    image_dims: BTreeSet<u32>,
}

impl VolumeConn {
    fn dims(&self, kind: VectorKind) -> &BTreeSet<u32> {
        match kind {
            VectorKind::Text => &self.text_dims,
            VectorKind::Image => &self.image_dims,
        }
    }

    /// Makes sure the `vec0` table of `kind` at `dimension` exists.
    fn ensure(&mut self, kind: VectorKind, dimension: u32) -> Result<()> {
        if self.dims(kind).contains(&dimension) {
            return Ok(());
        }
        let (_, vec) = names(kind, dimension);
        self.conn
            .execute_batch(&format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS {vec} USING vec0(embedding float[{dimension}]);"
            ))
            .map_err(|e| ToolError::internal(format!("sqlite-vec: create tables: {e}")))?;
        match kind {
            VectorKind::Text => self.text_dims.insert(dimension),
            VectorKind::Image => self.image_dims.insert(dimension),
        };
        Ok(())
    }

    /// Removes the rows of `path` (every row of the volume when `None`) from the
    /// tables of `kind` at every width; the number of metadata rows removed.
    fn purge(&self, kind: VectorKind, volume_id: &str, path: Option<&str>) -> Result<usize> {
        let meta = meta_table(kind);
        let (clause, p): (&str, Vec<&str>) = match path {
            Some(path) => ("volume_id=?1 AND path=?2", vec![volume_id, path]),
            None => ("volume_id=?1", vec![volume_id]),
        };
        let count: i64 = self
            .conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {meta} WHERE {clause}"),
                rusqlite::params_from_iter(&p),
                |row| row.get(0),
            )
            .map_err(|e| ToolError::internal(format!("sqlite-vec: count: {e}")))?;
        // The vec0 rows are keyed by the meta rowid, so they go first: once the
        // meta rows are gone the subquery can no longer find them.
        for dimension in self.dims(kind) {
            let (_, vec) = names(kind, *dimension);
            self.conn
                .execute(
                    &format!(
                        "DELETE FROM {vec} WHERE rowid IN \
                         (SELECT rowid FROM {meta} WHERE {clause} AND dim={dimension})"
                    ),
                    rusqlite::params_from_iter(&p),
                )
                .map_err(|e| ToolError::internal(format!("sqlite-vec: delete vec: {e}")))?;
        }
        self.conn
            .execute(&format!("DELETE FROM {meta} WHERE {clause}"), rusqlite::params_from_iter(&p))
            .map_err(|e| ToolError::internal(format!("sqlite-vec: delete meta: {e}")))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }
}

/// The little endian bytes of a vector, the `vec0` wire form.
fn to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_bytes(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

/// Vector search backend using sqlite-vec virtual tables.
pub struct SqliteVecBackend {
    /// `search.embedding.dimensions`: the width of a project that names no model.
    dimensions: u32,
    tantivy_dir: PathBuf,
    /// Shared HTTP client for embedding calls.
    client: Arc<reqwest::Client>,
    /// Where to embed, asked on every call (SPEC-0102 DEC-007).
    targets: Arc<dyn embedding::EmbedTargets>,
    /// Per-volume in-memory SQLite connections for the vector index.
    volumes: Arc<Mutex<HashMap<String, Arc<Mutex<VolumeConn>>>>>,
}

impl SqliteVecBackend {
    pub fn new(
        dimensions: u32,
        tantivy_dir: &str,
        client: Arc<reqwest::Client>,
        targets: Arc<dyn embedding::EmbedTargets>,
    ) -> Self {
        Self {
            dimensions,
            tantivy_dir: PathBuf::from(tantivy_dir),
            client,
            targets,
            volumes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The target and the width in force for `kind` at a project: the text model
    /// (platform width as the fallback) or the image model, which has no fallback.
    async fn target_of(
        &self,
        volume_id: &str,
        kind: VectorKind,
    ) -> Result<(embedding::ProjectTarget, u32)> {
        let project = match kind {
            VectorKind::Text => self.targets.target_for(volume_id).await?,
            VectorKind::Image => self
                .targets
                .image_target_for(volume_id)
                .await?
                .ok_or_else(|| ToolError::not_supported("the project has no image model"))?,
        };
        let dimension = project.dimension.unwrap_or(self.dimensions);
        Ok((project, dimension))
    }

    fn get_or_open(&self, volume_id: &str) -> Result<Arc<Mutex<VolumeConn>>> {
        let mut guard = self
            .volumes
            .lock()
            .map_err(|_| ToolError::internal("sqlite-vec volumes mutex poisoned"))?;
        if let Some(vc) = guard.get(volume_id) {
            return Ok(vc.clone());
        }
        let conn = Connection::open_in_memory()
            .map_err(|e| ToolError::internal(format!("sqlite-vec: open: {e}")))?;

        // Ensure the sqlite-vec extension symbol is resolved at compile time.
        #[cfg(feature = "rag")]
        {
            let _ = sqlite_vec::sqlite3_vec_init as *const ();
        }

        // Only the metadata exists up front: the `vec0` tables take their width
        // from the first write of the kind.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS search_vec_meta (
               rowid     INTEGER PRIMARY KEY AUTOINCREMENT,
               volume_id TEXT NOT NULL,
               path      TEXT NOT NULL,
               chunk_idx INTEGER NOT NULL,
               chunk_text TEXT NOT NULL,
               dim       INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS search_img_meta (
               rowid     INTEGER PRIMARY KEY AUTOINCREMENT,
               volume_id TEXT NOT NULL,
               path      TEXT NOT NULL,
               chunk_idx INTEGER NOT NULL,
               chunk_text TEXT NOT NULL,
               dim       INTEGER NOT NULL
             );",
        )
        .map_err(|e| ToolError::internal(format!("sqlite-vec: create tables: {e}")))?;

        let vc = Arc::new(Mutex::new(VolumeConn {
            conn,
            text_dims: BTreeSet::new(),
            image_dims: BTreeSet::new(),
        }));
        guard.insert(volume_id.to_string(), vc.clone());
        Ok(vc)
    }

    /// Replaces the rows of `path` in the tables of `kind` by `rows`, in one
    /// transaction (an error returns before the commit and the drop rolls back).
    async fn store(
        &self,
        kind: VectorKind,
        volume_id: &str,
        path: &str,
        dimension: Option<u32>,
        rows: Vec<(u32, String, Vec<f32>)>,
    ) -> Result<usize> {
        let vc = self.get_or_open(volume_id)?;
        let vol = volume_id.to_string();
        let path_owned = path.to_string();
        tokio::task::spawn_blocking(move || {
            let mut guard =
                vc.lock().map_err(|_| ToolError::internal("sqlite-vec conn mutex poisoned"))?;
            if let Some(d) = dimension {
                guard.ensure(kind, d)?;
            }
            let meta = meta_table(kind);
            // Idempotent: the rows of this path at every width go first, so a
            // model change leaves no vector of the old width (FR-NEW-004).
            let tx = guard
                .conn
                .unchecked_transaction()
                .map_err(|e| ToolError::internal(format!("sqlite-vec: begin: {e}")))?;
            for d in guard.dims(kind) {
                let (_, vec) = names(kind, *d);
                tx.execute(
                    &format!(
                        "DELETE FROM {vec} WHERE rowid IN \
                         (SELECT rowid FROM {meta} WHERE volume_id=?1 AND path=?2 AND dim={d})"
                    ),
                    params![vol, path_owned],
                )
                .map_err(|e| ToolError::internal(format!("sqlite-vec: delete vec: {e}")))?;
            }
            tx.execute(
                &format!("DELETE FROM {meta} WHERE volume_id=?1 AND path=?2"),
                params![vol, path_owned],
            )
            .map_err(|e| ToolError::internal(format!("sqlite-vec: delete meta: {e}")))?;
            for (number, text, vector) in &rows {
                let Some(d) = dimension else { break };
                let (_, vec) = names(kind, d);
                // Insert metadata row first; its auto-increment rowid is used
                // to link the vec0 table row.
                tx.execute(
                    &format!(
                        "INSERT INTO {meta} (volume_id, path, chunk_idx, chunk_text, dim) \
                         VALUES (?1, ?2, ?3, ?4, ?5)"
                    ),
                    params![vol, path_owned, i64::from(*number), text, i64::from(d)],
                )
                .map_err(|e| ToolError::internal(format!("sqlite-vec: insert meta: {e}")))?;
                let meta_rowid = tx.last_insert_rowid();
                tx.execute(
                    &format!("INSERT INTO {vec} (rowid, embedding) VALUES (?1, ?2)"),
                    params![meta_rowid, to_bytes(vector)],
                )
                .map_err(|e| ToolError::internal(format!("sqlite-vec: insert vec: {e}")))?;
            }
            tx.commit().map_err(|e| ToolError::internal(format!("sqlite-vec: commit: {e}")))?;
            Ok(rows.len())
        })
        .await
        .map_err(|e| ToolError::internal(format!("sqlite-vec task join: {e}")))?
    }
}

#[async_trait]
impl SearchBackend for SqliteVecBackend {
    async fn index_chunks_checked(
        &self,
        volume_id: &str,
        path: &str,
        chunks: &[IndexedChunk],
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        // Compute embeddings before entering the blocking closure: embedding
        // calls are async and must not be made from within spawn_blocking.
        let project = self.targets.target_for(volume_id).await?;
        let dimension = project.dimension.unwrap_or(self.dimensions);
        let mut rows = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let emb =
                embedding::embed_checked(&self.client, &project.target, &chunk.text, dimension)
                    .await?;
            rows.push((chunk.number, chunk.text.clone(), emb));
        }

        // After the embeddings, before the delete: a refusal keeps the old rows.
        if !still_current() {
            return Ok(None);
        }
        let n = self.store(VectorKind::Text, volume_id, path, Some(dimension), rows).await?;
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
        let mut rows = Vec::new();
        let mut dimension = None;
        if let Some(project) = project.filter(|_| !images.is_empty()) {
            let d = project.dimension.unwrap_or(self.dimensions);
            dimension = Some(d);
            for image in images {
                let v = embedding::embed_checked(&self.client, &project.target, &image.data_uri, d)
                    .await?;
                rows.push((image.number, image.text.clone(), v));
            }
        }
        if !still_current() {
            return Ok(None);
        }
        let n = self.store(VectorKind::Image, volume_id, path, dimension, rows).await?;
        Ok(Some(n))
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
        let vc = self.get_or_open(volume_id)?;
        let vol = volume_id.to_string();
        let path_owned = path.to_string();
        tokio::task::spawn_blocking(move || {
            let guard =
                vc.lock().map_err(|_| ToolError::internal("sqlite-vec conn mutex poisoned"))?;
            let empty = VectorPage { dimension: None, vectors: Vec::new(), next_after: None };
            let meta = meta_table(kind);
            // The width the path's rows were stored at (one: a write replaces all).
            let dimension: Option<i64> = guard
                .conn
                .query_row(
                    &format!("SELECT dim FROM {meta} WHERE volume_id=?1 AND path=?2 LIMIT 1"),
                    params![vol, path_owned],
                    |r| r.get(0),
                )
                .ok();
            let Some(dimension) = dimension.and_then(|d| u32::try_from(d).ok()) else {
                return Ok(empty);
            };
            let (_, vec) = names(kind, dimension);
            let mut stmt = guard
                .conn
                .prepare(&format!(
                    "SELECT m.chunk_idx, m.chunk_text, v.embedding FROM {meta} m \
                     JOIN {vec} v ON v.rowid = m.rowid \
                     WHERE m.volume_id=?1 AND m.path=?2 AND m.chunk_idx>?3 \
                     ORDER BY m.chunk_idx LIMIT ?4"
                ))
                .map_err(|e| ToolError::internal(format!("sqlite-vec: prepare: {e}")))?;
            let mut vectors: Vec<StoredVector> = stmt
                .query_map(
                    params![
                        vol,
                        path_owned,
                        i64::from(after),
                        i64::try_from(limit + 1).unwrap_or(i64::MAX)
                    ],
                    |row| {
                        Ok(StoredVector {
                            number: u32::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
                            text: row.get(1)?,
                            vector: from_bytes(&row.get::<_, Vec<u8>>(2)?),
                        })
                    },
                )
                .map_err(|e| ToolError::internal(format!("sqlite-vec: read vectors: {e}")))?
                .collect::<std::result::Result<_, _>>()
                .map_err(|e| ToolError::internal(format!("sqlite-vec: read vectors: {e}")))?;
            let more = vectors.len() > limit;
            vectors.truncate(limit);
            let next_after = if more { vectors.last().map(|v| v.number) } else { None };
            Ok(VectorPage { dimension: Some(dimension), vectors, next_after })
        })
        .await
        .map_err(|e| ToolError::internal(format!("sqlite-vec task join: {e}")))?
    }

    async fn embed_input(
        &self,
        volume_id: &str,
        kind: VectorKind,
        input: &str,
    ) -> Result<Vec<f32>> {
        let (project, dimension) = self.target_of(volume_id, kind).await?;
        embedding::embed_checked(&self.client, &project.target, input, dimension).await
    }

    async fn nearest(
        &self,
        volume_id: &str,
        kind: VectorKind,
        vector: &[f32],
        limit: usize,
    ) -> Result<Vec<Near>> {
        let vc = self.get_or_open(volume_id)?;
        let vol = volume_id.to_string();
        let blob = to_bytes(vector);
        let dimension = u32::try_from(vector.len()).unwrap_or(0);
        tokio::task::spawn_blocking(move || {
            let guard =
                vc.lock().map_err(|_| ToolError::internal("sqlite-vec conn mutex poisoned"))?;
            // No store of this width: nothing to find, and a read never creates one.
            if !guard.dims(kind).contains(&dimension) {
                return Ok(Vec::new());
            }
            let (meta, vec) = names(kind, dimension);
            // `vec_distance_cosine` is the exact cosine distance; the order is total
            // (distance, path, number), so a tie never depends on the scan order.
            let mut stmt = guard
                .conn
                .prepare(&format!(
                    "SELECT m.path, m.chunk_idx, m.chunk_text, \
                            vec_distance_cosine(v.embedding, ?2) AS d \
                     FROM {vec} v JOIN {meta} m ON m.rowid = v.rowid \
                     WHERE m.volume_id = ?1 \
                     ORDER BY d ASC, m.path ASC, m.chunk_idx ASC LIMIT ?3"
                ))
                .map_err(|e| ToolError::internal(format!("sqlite-vec: prepare: {e}")))?;
            let rows = stmt
                .query_map(
                    params![
                        vol,
                        rusqlite::types::Value::Blob(blob),
                        i64::try_from(limit).unwrap_or(i64::MAX)
                    ],
                    |row| {
                        Ok(Near {
                            path: row.get(0)?,
                            number: u32::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                            text: row.get(2)?,
                            similarity: similarity_of_distance(row.get::<_, f64>(3)?),
                        })
                    },
                )
                .map_err(|e| ToolError::internal(format!("sqlite-vec: nearest: {e}")))?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| ToolError::internal(format!("sqlite-vec: nearest: {e}")))?;
            Ok(rows)
        })
        .await
        .map_err(|e| ToolError::internal(format!("sqlite-vec task join: {e}")))?
    }

    async fn similarities(
        &self,
        volume_id: &str,
        kind: VectorKind,
        vector: &[f32],
        entries: &[(String, u32)],
    ) -> Result<Vec<Option<f32>>> {
        let vc = self.get_or_open(volume_id)?;
        let vol = volume_id.to_string();
        let blob = to_bytes(vector);
        let dimension = u32::try_from(vector.len()).unwrap_or(0);
        let entries = entries.to_vec();
        tokio::task::spawn_blocking(move || {
            let guard =
                vc.lock().map_err(|_| ToolError::internal("sqlite-vec conn mutex poisoned"))?;
            if !guard.dims(kind).contains(&dimension) {
                return Ok(vec![None; entries.len()]);
            }
            let (meta, vec) = names(kind, dimension);
            let mut stmt = guard
                .conn
                .prepare(&format!(
                    "SELECT vec_distance_cosine(v.embedding, ?4) \
                     FROM {meta} m JOIN {vec} v ON v.rowid = m.rowid \
                     WHERE m.volume_id = ?1 AND m.path = ?2 AND m.chunk_idx = ?3 AND m.dim = {dimension}"
                ))
                .map_err(|e| ToolError::internal(format!("sqlite-vec: prepare: {e}")))?;
            let mut out = Vec::with_capacity(entries.len());
            for (path, number) in &entries {
                let d: Option<f64> = rusqlite::OptionalExtension::optional(stmt.query_row(
                    params![vol, path, i64::from(*number), rusqlite::types::Value::Blob(blob.clone())],
                    |r| r.get(0),
                ))
                .map_err(|e| ToolError::internal(format!("sqlite-vec: similarity: {e}")))?;
                out.push(d.map(similarity_of_distance));
            }
            Ok(out)
        })
        .await
        .map_err(|e| ToolError::internal(format!("sqlite-vec task join: {e}")))?
    }

    async fn delete_path(&self, volume_id: &str, path: &str) -> Result<usize> {
        let vc = self.get_or_open(volume_id)?;
        let vol = volume_id.to_string();
        let path_owned = path.to_string();
        tokio::task::spawn_blocking(move || {
            let guard =
                vc.lock().map_err(|_| ToolError::internal("sqlite-vec conn mutex poisoned"))?;
            let n = guard.purge(VectorKind::Text, &vol, Some(&path_owned))?;
            guard.purge(VectorKind::Image, &vol, Some(&path_owned))?;
            Ok(n)
        })
        .await
        .map_err(|e| ToolError::internal(format!("sqlite-vec task join: {e}")))?
    }

    async fn delete_all(&self, volume_id: &str) -> Result<usize> {
        let vc = self.get_or_open(volume_id)?;
        let vol = volume_id.to_string();
        tokio::task::spawn_blocking(move || {
            let guard =
                vc.lock().map_err(|_| ToolError::internal("sqlite-vec conn mutex poisoned"))?;
            let n = guard.purge(VectorKind::Text, &vol, None)?;
            guard.purge(VectorKind::Image, &vol, None)?;
            Ok(n)
        })
        .await
        .map_err(|e| ToolError::internal(format!("sqlite-vec task join: {e}")))?
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
        // Embed the query text before entering the blocking section.
        let project = self.targets.target_for(volume_id).await?;
        let dimension = project.dimension.unwrap_or(self.dimensions);
        let embedding =
            embedding::embed_checked(&self.client, &project.target, query, dimension).await?;

        let vc = self.get_or_open(volume_id)?;
        let vol = volume_id.to_string();

        tokio::task::spawn_blocking(move || {
            let guard =
                vc.lock().map_err(|_| ToolError::internal("sqlite-vec conn mutex poisoned"))?;
            // No store of this width: nothing to find, and a read never rebuilds.
            if !guard.text_dims.contains(&dimension) {
                return Ok(Vec::new());
            }

            let mut stmt = guard
                .conn
                .prepare(&format!(
                    "SELECT m.path, m.chunk_text, v.distance, m.chunk_idx \
                     FROM search_vec_{dimension} v \
                     JOIN search_vec_meta m ON m.rowid = v.rowid \
                     WHERE m.volume_id = ?1 \
                       AND v.embedding MATCH ?2 \
                       AND k = ?3 \
                     ORDER BY v.distance"
                ))
                .map_err(|e| ToolError::internal(format!("sqlite-vec: prepare: {e}")))?;

            let rows: Vec<SearchResult> = stmt
                .query_map(
                    params![vol, rusqlite::types::Value::Blob(to_bytes(&embedding)), top_k as i64],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, f64>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .map_err(|e| ToolError::internal(format!("sqlite-vec: query: {e}")))?
                .enumerate()
                .filter_map(|(i, r)| {
                    r.ok().map(|(path, chunk, distance, idx)| SearchResult {
                        path,
                        // Convert cosine distance to a similarity score in [0,1].
                        score: 1.0 - distance as f32,
                        chunk,
                        rank: i + 1,
                        chunk_idx: u32::try_from(idx).unwrap_or(0),
                    })
                })
                .collect();

            Ok(rows)
        })
        .await
        .map_err(|e| ToolError::internal(format!("sqlite-vec task join: {e}")))?
    }

    async fn stats(&self, volume_id: &str) -> Result<IndexStats> {
        let vc = self.get_or_open(volume_id)?;
        let vol = volume_id.to_string();
        let tantivy_dir = self.tantivy_dir.join(volume_id);

        let count = tokio::task::spawn_blocking(move || {
            let guard =
                vc.lock().map_err(|_| ToolError::internal("sqlite-vec conn mutex poisoned"))?;
            let c: i64 = guard
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM search_vec_meta WHERE volume_id=?1",
                    params![vol],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            Ok::<i64, ToolError>(c)
        })
        .await
        .map_err(|e| ToolError::internal(format!("sqlite-vec task join: {e}")))??;

        let bm25_warm = tantivy_dir.exists()
            && tantivy_dir.read_dir().ok().map(|mut d| d.next().is_some()).unwrap_or(false);

        Ok(IndexStats {
            bm25_docs: 0,
            vector_chunks: count as usize,
            bm25_warm,
            mode: "rag".into(),
        })
    }

    fn supported_modes(&self) -> Vec<&'static str> {
        vec!["rag"]
    }
}

#[cfg(test)]
mod tests {
    // These tests use the real sqlite-vec vec0 MATCH operator. They need the
    // extension registered, which happens via the OnceLock in storage/sqlite.rs.
    // Since tests run in the same process and OnceLock fires once, we call the
    // registration function directly here.

    use super::*;
    use crate::config::EmbeddingConfig;
    use crate::errors::code;
    use crate::search::SearchBackend;
    use axum::routing::post;
    use axum::{Json, Router};

    fn register_extension() {
        crate::storage::sqlite::register_sqlite_vec();
    }

    /// Spawn a minimal axum server returning a fixed N-dim embedding.
    async fn spawn_fake_embedding(dims: usize) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = Router::new().route(
            "/v1/embeddings",
            post(move || async move {
                let vec: Vec<f32> = (0..dims).map(|i| (i + 1) as f32 / dims as f32).collect();
                Json(serde_json::json!({
                    "data": [{"embedding": vec, "index": 0}],
                    "model": "fake",
                    "usage": {"prompt_tokens": 1, "total_tokens": 1}
                }))
            }),
        );
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://127.0.0.1:{port}")
    }

    fn backend(endpoint: &str, dims: u32) -> (SqliteVecBackend, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let client = Arc::new(reqwest::Client::new());
        let cfg = EmbeddingConfig {
            endpoint: endpoint.to_string(),
            model: "fake".to_string(),
            api_key_env: String::new(),
            dimensions: dims,
        };
        let b = SqliteVecBackend::new(
            dims,
            dir.path().to_str().unwrap(),
            client,
            Arc::new(crate::search::embedding::StaticTargets(cfg)),
        );
        (b, dir)
    }

    #[tokio::test]
    async fn stats_zero_on_empty_volume() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        let s = b.stats("vol1").await.unwrap();
        assert_eq!(s.vector_chunks, 0);
    }

    #[tokio::test]
    async fn index_increments_chunk_count() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        crate::search::index_text(&b, "vol1", "/a.md", "hello world test").await.unwrap();
        let s = b.stats("vol1").await.unwrap();
        assert_eq!(s.vector_chunks, 1);
    }

    #[tokio::test]
    async fn delete_removes_chunks() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        crate::search::index_text(&b, "vol1", "/a.md", "some text to delete").await.unwrap();
        let deleted = b.delete_path("vol1", "/a.md").await.unwrap();
        assert!(deleted > 0, "delete must report at least one chunk removed");
        let s = b.stats("vol1").await.unwrap();
        assert_eq!(s.vector_chunks, 0);
    }

    #[tokio::test]
    async fn idempotent_reindex() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        crate::search::index_text(&b, "vol1", "/a.md", "first content").await.unwrap();
        crate::search::index_text(&b, "vol1", "/a.md", "second content").await.unwrap();
        let s = b.stats("vol1").await.unwrap();
        assert_eq!(s.vector_chunks, 1, "re-index must replace, not append");
    }

    #[tokio::test]
    async fn query_vector_returns_results() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        crate::search::index_text(&b, "vol1", "/doc.md", "rust systems programming").await.unwrap();
        // The fake server returns the same vector for every input, so cosine
        // distance between query and stored embedding is 0 (identical).
        let r = b.query_vector("vol1", "systems programming", 10).await.unwrap();
        assert!(!r.is_empty(), "expected at least one result");
        assert_eq!(r[0].path, "/doc.md");
        assert_eq!(r[0].rank, 1);
    }

    #[tokio::test]
    async fn volume_isolation() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        crate::search::index_text(&b, "vol_a", "/x.md", "content a").await.unwrap();
        crate::search::index_text(&b, "vol_b", "/x.md", "content b").await.unwrap();
        assert_eq!(b.stats("vol_a").await.unwrap().vector_chunks, 1);
        assert_eq!(b.stats("vol_b").await.unwrap().vector_chunks, 1);
    }

    #[tokio::test]
    async fn delete_all_removes_every_chunk_of_the_volume() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        crate::search::index_text(&b, "vol1", "/a.md", "alpha").await.unwrap();
        crate::search::index_text(&b, "vol1", "/b.md", "beta").await.unwrap();
        assert_eq!(b.stats("vol1").await.unwrap().vector_chunks, 2);

        assert_eq!(b.delete_all("vol1").await.unwrap(), 2);
        assert_eq!(b.stats("vol1").await.unwrap().vector_chunks, 0);
        assert!(b.query_vector("vol1", "alpha", 10).await.unwrap().is_empty());

        // Usable again straight after the wipe.
        crate::search::index_text(&b, "vol1", "/c.md", "gamma").await.unwrap();
        assert_eq!(b.stats("vol1").await.unwrap().vector_chunks, 1);
    }

    #[tokio::test]
    async fn delete_all_is_scoped_to_one_volume() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        crate::search::index_text(&b, "vol_a", "/x.md", "content a").await.unwrap();
        crate::search::index_text(&b, "vol_b", "/x.md", "content b").await.unwrap();
        b.delete_all("vol_a").await.unwrap();
        assert_eq!(b.stats("vol_a").await.unwrap().vector_chunks, 0);
        assert_eq!(b.stats("vol_b").await.unwrap().vector_chunks, 1);
    }

    #[tokio::test]
    async fn delete_all_on_an_empty_volume_is_zero() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        assert_eq!(b.delete_all("never-seen").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn query_bm25_returns_not_supported() {
        register_extension();
        let base = spawn_fake_embedding(3).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 3);
        let err = b.query_bm25("vol1", "anything", 10).await.unwrap_err();
        assert_eq!(err.code, code::NOT_SUPPORTED);
    }
    /// Embedding stub that answers `dims` floats, except the call numbered
    /// `short_call` (1 based, counted since the stub started) which answers 3
    /// floats, a dimension the vec0 table refuses.
    async fn spawn_short_on_call(dims: usize, short_call: usize) -> String {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = Router::new().route(
            "/v1/embeddings",
            post(move || {
                let calls = calls.clone();
                async move {
                    let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    let len = if n == short_call { 3 } else { dims };
                    let vec: Vec<f32> = (0..len).map(|i| (i + 1) as f32).collect();
                    Json(serde_json::json!({
                        "data": [{"embedding": vec, "index": 0}],
                        "model": "fake",
                        "usage": {"prompt_tokens": 1, "total_tokens": 1}
                    }))
                }
            }),
        );
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://127.0.0.1:{port}")
    }

    #[tokio::test]
    async fn one_transaction_sqlite_vec_rolls_back_a_half_replacement() {
        register_extension();
        // Calls 1 to 3 index "old"; calls 4 to 6 embed the new chunks and the
        // sixth is short; call 7 is the query.
        let base = spawn_short_on_call(4, 6).await;
        let (b, _d) = backend(&format!("{base}/v1/embeddings"), 4);
        let old = crate::search::entries(&["oldoldoldo"; 3]);
        assert_eq!(b.index_chunks_checked("vol1", "/a.md", &old, &|| true).await.unwrap(), Some(3));
        assert_eq!(b.stats("vol1").await.unwrap().vector_chunks, 3);

        let new = crate::search::entries(&["nnnnnnnnnn"; 3]);
        let res = b.index_chunks_checked("vol1", "/a.md", &new, &|| true).await;
        assert!(res.is_err(), "the short embedding must fail the replacement");

        assert_eq!(b.stats("vol1").await.unwrap().vector_chunks, 3, "old chunks kept");
        let r = b.query_vector("vol1", "old", 10).await.unwrap();
        assert_eq!(r[0].path, "/a.md");
        assert!(r.iter().all(|x| !x.chunk.contains('n')), "only old chunk text: {r:?}");
    }
}
