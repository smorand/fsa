//! Semantic search tool family: `search.index`, `search.query`, `search.delete`,
//! `search.status`.
//!
//! These tools are optional: they are only registered when `search.enabled` is
//! true in the server config. `index` and `query` are the engines both the MCP
//! surface and the test registry call. Each tool authorizes via `state.authorize` before
//! any storage access, exactly like the `fs.*` family.
//!
//! Mode dispatch:
//! - `bm25`  : Tantivy on SQLite (always available)
//! - `rag`   : vector KNN, requires `rag` feature and embedding config
//! - `both`  : BM25 + vector, RRF merge, optional reranking

#[cfg(test)]
use crate::config::SearchConfig;
use crate::errors::{Result, ToolError};
use crate::search::fusion::rrf_merge;
use crate::search::{SearchBackend, SearchResult};
use crate::state::AppState;
use crate::storage::VolumeClient;
#[cfg(test)]
use crate::tools::registry_support::{ToolRegistry, ToolSchema, handler};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

/// The search backend, or the refusal of a server whose search is off.
fn backend(state: &AppState) -> Result<&Arc<dyn SearchBackend>> {
    state.search.as_ref().ok_or_else(|| {
        ToolError::not_supported("search is not enabled; set search.enabled: true in config")
    })
}

/// `search.index` (SPEC-0101 FR-NEW-024, DEC-011): each text file gets its
/// default set, built first when it has none or it is `out_of_date`, and its
/// entries are that set's chunks, through the same code as the rebuild.
/// `sizes_passed` is true when the caller sent `chunk_size` or
/// `chunk_overlap`, which are refused. A single path that holds no text is
/// refused; inside a folder such a file is skipped and counted. Answers the
/// number of chunks indexed. Cost: per file, one read, the set (one read, or
/// one linear cut and one transaction) and the engine's replacement.
pub(crate) async fn index(
    state: &AppState,
    person: &str,
    mount: &str,
    raw_path: &str,
    recursive: bool,
    sizes_passed: bool,
) -> Result<Value> {
    state.authorize(mount, person).await?;
    if sizes_passed {
        return Err(ToolError::invalid_argument(
            "chunk_size and chunk_overlap are no longer accepted",
        ));
    }
    let path = state.safety.normalize_path(raw_path)?;
    let backend = backend(state)?;
    let client = state.stores.client(mount).await?;

    let paths: Vec<String> = if recursive {
        let trash_dir = &state.safety.config().trash_dir;
        crate::core::fs_ops::iter_files(&client, &path, &[])
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(p, _mtime)| p)
            .filter(|p| !crate::search::indexer::is_trash_path(p, trash_dir))
            .collect()
    } else {
        vec![path.clone()]
    };

    let mut indexed = 0usize;
    let mut skipped = 0usize;
    for file in &paths {
        match index_one(state, &client, backend, mount, file).await {
            Ok(n) => indexed += n,
            Err(_) if recursive => skipped += 1,
            Err(e) => return Err(unreachable_text(e)),
        }
    }
    Ok(json!({ "indexed": indexed, "skipped": skipped, "path": path }))
}

/// Indexes one file through its default set; the number of chunks indexed.
async fn index_one(
    state: &AppState,
    client: &VolumeClient,
    backend: &Arc<dyn SearchBackend>,
    mount: &str,
    path: &str,
) -> Result<usize> {
    let text = crate::chunking::service::source_text(state, client, mount, path).await?;
    let d = state.config.search.chunking();
    let n = crate::search::indexer::index_default_set(
        &**backend,
        &*client.meta,
        mount,
        path,
        text,
        d,
        false,
        &|| true,
    )
    .await?;
    Ok(n.unwrap_or(0))
}

/// The embedding failure texts read as the service being unreachable, the
/// reason a rebuild job records (SPEC-0100 DEC-019); any other error is kept.
fn unreachable_text(e: ToolError) -> ToolError {
    let reason = crate::search::derived::scheduler::failure_reason(&e);
    if reason == "embedding service unreachable" { ToolError::internal(reason) } else { e }
}

/// `search.query`: the engine results, each with its chunk number and the
/// heading path of that chunk (SPEC-0101 DEC-010). Fusion, ranking and the
/// number of results are unchanged.
pub(crate) async fn query(
    state: &AppState,
    person: &str,
    mount: &str,
    query: &str,
    mode: Option<String>,
    top_k: usize,
) -> Result<Value> {
    state.authorize(mount, person).await?;
    let mode = mode.unwrap_or_else(|| state.config.search.mode.clone());
    let backend = backend(state)?;

    if matches!(mode.as_str(), "rag" | "both") {
        if state.config.search.embedding.endpoint.is_empty() {
            return Err(ToolError::not_supported(
                "mode rag/both requires search.embedding.endpoint to be configured",
            ));
        }
        if !backend.supported_modes().contains(&mode.as_str()) {
            return Err(ToolError::not_supported(format!(
                "mode '{mode}' is not supported by the current search backend"
            )));
        }
    }

    let (results, mode_used) = match mode.as_str() {
        "bm25" => (backend.query_bm25(mount, query, top_k).await?, "bm25"),
        "rag" => (backend.query_vector(mount, query, top_k).await?, "rag"),
        "both" => {
            let bm25 = backend.query_bm25(mount, query, top_k).await.unwrap_or_default();
            let vec = backend.query_vector(mount, query, top_k).await.unwrap_or_default();
            (rrf_merge(&bm25, &vec), "both")
        }
        other => {
            return Err(ToolError::invalid_argument(format!(
                "unknown mode '{other}', expected bm25, rag, or both"
            )));
        }
    };

    let warning: Option<&str> = if matches!(mode_used, "bm25" | "both") {
        match backend.stats(mount).await {
            Ok(s) if !s.bm25_warm => {
                Some("BM25 index is not warm; run search.index to populate it")
            }
            _ => None,
        }
    } else {
        None
    };

    let client = state.stores.client(mount).await?;
    Ok(json!({
        "results": result_values(&client, &results).await?,
        "mode_used": mode_used,
        "warning": warning,
    }))
}

/// The results as `search.query` shows them (DEC-010). An entry of a file
/// with a default set shows the text after its header line, its chunk number
/// and that chunk's heading path; an entry of a file with none, built before
/// the upgrade, shows its text unchanged, chunk number 0 and no heading path
/// (FR-NEW-026). Cost: one set lookup per distinct path, one chunk lookup per
/// result of such a file, on primary keys.
async fn result_values(client: &VolumeClient, results: &[SearchResult]) -> Result<Vec<Value>> {
    let store = client.meta.chunk_store();
    let mut sets: HashMap<&str, Option<String>> = HashMap::new();
    let mut out = Vec::with_capacity(results.len());
    for r in results {
        let set_id = match sets.get(r.path.as_str()) {
            Some(id) => id.clone(),
            None => {
                let id = match &store {
                    Some(store) => store.default_set(&r.path).await?.map(|s| s.set_id),
                    None => None,
                };
                sets.insert(&r.path, id.clone());
                id
            }
        };
        let (chunk, number, heading_path) = match (&store, set_id) {
            (Some(store), Some(id)) => {
                let heading = store
                    .get_chunk_window(&id, r.chunk_idx, r.chunk_idx)
                    .await?
                    .into_iter()
                    .next()
                    .map(|c| c.heading_path)
                    .unwrap_or_default();
                let text = r.chunk.split_once('\n').map_or(r.chunk.as_str(), |(_, t)| t);
                (text, r.chunk_idx, heading)
            }
            _ => (r.chunk.as_str(), 0, Vec::new()),
        };
        out.push(json!({
            "path": r.path,
            "score": r.score,
            "chunk": chunk,
            "rank": r.rank,
            "chunk_number": number,
            "heading_path": heading_path,
        }));
    }
    Ok(out)
}

/// Register the four `search.*` tools.
#[cfg(test)]
pub(crate) fn register(reg: &mut ToolRegistry, _config: &SearchConfig) {
    reg.add(
        ToolSchema::new(
            "search.index",
            "Index a file or directory into the search engine for this volume.",
        )
        .req_str("mount_id", "Project/volume id the operation targets.")
        .req_str("path", "Absolute POSIX path to index (file or directory).")
        .opt_bool("recursive", false, "Recurse into subdirectories when path is a directory.")
        .read_only(false)
        .destructive(false)
        .idempotent(true)
        .open_world(false),
        handler(|ctx, a| async move {
            let sizes = a.raw("chunk_size").is_some() || a.raw("chunk_overlap").is_some();
            let mount = a.str("mount_id")?;
            let path = a.str("path")?;
            let recursive = a.bool_or("recursive", false);
            index(&ctx.state, &ctx.person, &mount, &path, recursive, sizes).await
        }),
    );

    reg.add(
        ToolSchema::new(
            "search.query",
            "Search indexed content. mode overrides the server default (bm25, rag, both).",
        )
        .req_str("mount_id", "Project/volume id the operation targets.")
        .req_str("query", "Search query text.")
        .opt_str_null("mode", "Query mode: bm25, rag, or both. Defaults to server config.")
        .opt_int("top_k", 10, "Maximum number of results to return.")
        .opt_bool("rerank", true, "Apply reranking when configured and available.")
        .read_only(true)
        .idempotent(true)
        .open_world(false),
        handler(|ctx, a| async move {
            let mount = a.str("mount_id")?;
            let q = a.str("query")?;
            let top_k = a.int_or("top_k", 10) as usize;
            let _rerank = a.bool_or("rerank", true);
            query(&ctx.state, &ctx.person, &mount, &q, a.opt_str("mode"), top_k).await
        }),
    );

    reg.add(
        ToolSchema::new("search.delete", "Remove a path from the search index.")
            .req_str("mount_id", "Project/volume id the operation targets.")
            .req_str("path", "Absolute POSIX path to remove from the index.")
            .opt_bool("recursive", false, "Recurse into subdirectories when path is a directory.")
            .read_only(false)
            .destructive(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            let mount = a.str("mount_id")?;
            ctx.state.authorize(&mount, &ctx.person).await?;

            let path = ctx.state.safety.normalize_path(&a.str("path")?)?;
            let _recursive = a.bool_or("recursive", false);

            let backend = ctx.state.search.as_ref().ok_or_else(|| {
                ToolError::not_supported(
                    "search is not enabled; set search.enabled: true in config",
                )
            })?;

            let deleted = backend.delete_path(&mount, &path).await?;

            Ok(json!({ "deleted": deleted }))
        }),
    );

    reg.add(
        ToolSchema::new("search.status", "Report index statistics for this volume.")
            .req_str("mount_id", "Project/volume id the operation targets.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            let mount = a.str("mount_id")?;
            ctx.state.authorize(&mount, &ctx.person).await?;

            let backend = ctx.state.search.as_ref().ok_or_else(|| {
                ToolError::not_supported(
                    "search is not enabled; set search.enabled: true in config",
                )
            })?;

            let stats = backend.stats(&mount).await?;

            Ok(json!({
                "bm25_docs": stats.bm25_docs,
                "vector_chunks": stats.vector_chunks,
                "mode": stats.mode,
                "bm25_warm": stats.bm25_warm,
            }))
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::bm25_sqlite::TantivyBm25Backend;
    use crate::tools::testkit::MOUNT;

    fn reg() -> ToolRegistry {
        let mut r = ToolRegistry::new();
        register(&mut r, &SearchConfig::default());
        r
    }

    #[test]
    fn search_tools_register_four_tools() {
        assert_eq!(
            reg().names(),
            ["search.index", "search.query", "search.delete", "search.status"]
        );
    }

    /// Build a harness with a real Tantivy BM25 backend injected.
    async fn search_harness() -> (crate::tools::testkit::Harness, tempfile::TempDir) {
        let search_dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(TantivyBm25Backend::new(search_dir.path().to_str().unwrap()));
        let h = crate::tools::testkit::harness_with_search(
            |c| {
                c.search.enabled = true;
            },
            Some(backend as Arc<dyn crate::search::SearchBackend>),
        )
        .await;
        (h, search_dir)
    }

    #[tokio::test]
    async fn bm25_index_and_query_sqlite() {
        let (h, _dir) = search_harness().await;
        // Seed a file in the volume.
        h.seed("/doc.md", "the quick brown fox jumps over the lazy dog").await;

        // Index through the backend directly.
        let backend = h.state.search.as_ref().unwrap();
        let text = "the quick brown fox jumps over the lazy dog";
        let n = crate::search::index_text(&**backend, MOUNT, "/doc.md", text).await.unwrap();
        assert!(n > 0, "expected at least one chunk indexed");

        let results = h.state.search.as_ref().unwrap().query_bm25(MOUNT, "fox", 10).await.unwrap();
        assert!(!results.is_empty(), "expected at least one result");
        assert_eq!(results[0].path, "/doc.md");
    }

    #[tokio::test]
    async fn bm25_delete_removes_doc() {
        let (h, _dir) = search_harness().await;
        let backend = h.state.search.as_ref().unwrap();
        crate::search::index_text(&**backend, MOUNT, "/a.md", "hello world search test")
            .await
            .unwrap();
        h.state.search.as_ref().unwrap().delete_path(MOUNT, "/a.md").await.unwrap();
        let results =
            h.state.search.as_ref().unwrap().query_bm25(MOUNT, "hello", 10).await.unwrap();
        assert!(results.is_empty(), "deleted doc must not appear in results");
    }

    #[tokio::test]
    async fn status_zero_on_empty_volume() {
        let (h, _dir) = search_harness().await;
        let stats = h.state.search.as_ref().unwrap().stats(MOUNT).await.unwrap();
        assert_eq!(stats.bm25_docs, 0);
        assert!(!stats.bm25_warm);
    }

    #[tokio::test]
    async fn rag_mode_without_embedding_config_returns_not_supported() {
        let (h, _dir) = search_harness().await;
        // BM25 backend does not support vector queries.
        let backend = h.state.search.as_ref().unwrap();
        let err = backend.query_vector(MOUNT, "fox", 10).await.unwrap_err();
        assert_eq!(err.code, crate::errors::code::NOT_SUPPORTED);
    }

    #[test]
    fn search_schema_matches_contract_index() {
        let r = reg();
        let t = r.resolve("search.index").unwrap();
        let schema = t.schema.input_schema();
        assert_eq!(schema["type"], "object");
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required[0], "mount_id");
        assert_eq!(required[1], "path");
    }

    #[test]
    fn search_schema_matches_contract_query() {
        let r = reg();
        let t = r.resolve("search.query").unwrap();
        let schema = t.schema.input_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required[0], "mount_id");
        assert_eq!(required[1], "query");
    }

    #[test]
    fn search_schema_matches_contract_delete() {
        let r = reg();
        let t = r.resolve("search.delete").unwrap();
        let schema = t.schema.input_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required[0], "mount_id");
        assert_eq!(required[1], "path");
    }

    #[test]
    fn search_schema_matches_contract_status() {
        let r = reg();
        let t = r.resolve("search.status").unwrap();
        let schema = t.schema.input_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required[0], "mount_id");
    }
}
