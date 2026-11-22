//! The chunk set tools (SPEC-0101 FR-NEW-013 to FR-NEW-018):
//! `fs.create_chunk_set`, `fs.list_chunk_sets`, `fs.list_chunks`,
//! `fs.get_chunk`, `fs.delete_chunk_set`.
//!
//! Typed functions only: the `#[tool]` methods in `mcp::server` call them and
//! each owns its refusal order (design 5.2): project, member, path absent,
//! folder, then the tool's own checks. MCP only, no REST twin (DEC-015).

use crate::chunking::model::decode_marker;
use crate::chunking::service::{self, Requested};
use crate::chunking::{ChunkStore, Set, model::encode_marker};
use crate::errors::{Result, ToolError};
use crate::state::AppState;
use crate::storage::volume::VolumeClient;
#[cfg(test)]
use crate::tools::registry_support::{ToolRegistry, ToolSchema, handler};
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// Must equal the `#[tool]` descriptions in `mcp::server`, literals there.
#[cfg(test)]
const LIST_SETS_DESCRIPTION: &str = "List the chunk sets of one file: id, strategy, settings, whether it is the default set, chunk and token counts, state (current or out_of_date) and build time.";
#[cfg(test)]
const LIST_CHUNKS_DESCRIPTION: &str = "List the chunks of one chunk set of a file by number, 100 per page, each with its unit, heading path, pages, token count, figures and a preview of its first 200 characters.";
#[cfg(test)]
const GET_CHUNK_DESCRIPTION: &str = "Read one chunk of a chunk set of a file in full, with up to 20 chunks before and after it, in order of number.";
#[cfg(test)]
const CREATE_SET_DESCRIPTION: &str = "Create a chunk set of one file with the strategy fixed or structural and optional settings, or return the equal set the file already holds; a file holds at most 5 chunk sets.";
#[cfg(test)]
const DELETE_SET_DESCRIPTION: &str =
    "Delete one chunk set of a file and its chunks; the default chunk set cannot be deleted.";
#[cfg(test)]
const SETTINGS_DESCRIPTION: &str = "Settings of the strategy: fixed takes size_tokens and overlap_tokens, structural takes heading_cut_depth, max_tokens and list_max_items; omitted settings take the server defaults.";

/// Chunks per `fs.list_chunks` answer (FR-NEW-015).
const PAGE: usize = 100;

/// Largest `before` or `after` of `fs.get_chunk` (FR-NEW-016).
const MAX_WINDOW: i64 = 20;

/// Test dispatch glue mirroring the `McpServer` methods, so the golden
/// contract regeneration sees the schemas the live surface serves.
#[cfg(test)]
pub(crate) fn register(reg: &mut ToolRegistry) {
    const MOUNT: &str = "Project/volume id the operation targets.";
    const PATH: &str = "Absolute POSIX path of the file.";
    const SET: &str = "Id of the chunk set, as fs.list_chunk_sets returns it.";
    reg.add(
        ToolSchema::new("fs.list_chunk_sets", LIST_SETS_DESCRIPTION)
            .req_str("mount_id", MOUNT)
            .req_str("path", PATH)
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            list_chunk_sets(&ctx.state, &ctx.person, &a.str("mount_id")?, &a.str("path")?).await
        }),
    );
    reg.add(
        ToolSchema::new("fs.list_chunks", LIST_CHUNKS_DESCRIPTION)
            .req_str("mount_id", MOUNT)
            .req_str("path", PATH)
            .req_str("set_id", SET)
            .opt_str("marker", "", "The marker of the previous answer, to read the next page.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            list_chunks(
                &ctx.state,
                &ctx.person,
                &a.str("mount_id")?,
                &a.str("path")?,
                &a.str("set_id")?,
                &a.str_or("marker", ""),
            )
            .await
        }),
    );
    reg.add(
        ToolSchema::new("fs.get_chunk", GET_CHUNK_DESCRIPTION)
            .req_str("mount_id", MOUNT)
            .req_str("path", PATH)
            .req_str("set_id", SET)
            .req_int("number", "Number of the chunk in the set, from 1.")
            .opt_int("before", 0, "Chunks to add before it, 0 to 20.")
            .opt_int("after", 0, "Chunks to add after it, 0 to 20.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            get_chunk(
                &ctx.state,
                &ctx.person,
                &a.str("mount_id")?,
                &a.str("path")?,
                &a.str("set_id")?,
                a.int("number")?,
                a.int_or("before", 0),
                a.int_or("after", 0),
            )
            .await
        }),
    );
    reg.add(
        ToolSchema::new("fs.create_chunk_set", CREATE_SET_DESCRIPTION)
            .req_str("mount_id", MOUNT)
            .req_str("path", PATH)
            .req_str("strategy", "Chunking strategy: 'fixed' or 'structural'.")
            .opt_obj("settings", SETTINGS_DESCRIPTION)
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            let settings = a.raw("settings").and_then(Value::as_object).cloned();
            create_chunk_set(
                &ctx.state,
                &ctx.person,
                &a.str("mount_id")?,
                &a.str("path")?,
                &a.str("strategy")?,
                settings.unwrap_or_default(),
            )
            .await
        }),
    );
    reg.add(
        ToolSchema::new("fs.delete_chunk_set", DELETE_SET_DESCRIPTION)
            .req_str("mount_id", MOUNT)
            .req_str("path", PATH)
            .req_str("set_id", SET)
            .read_only(false)
            .destructive(true)
            .idempotent(false)
            .open_world(false),
        handler(|ctx, a| async move {
            delete_chunk_set(
                &ctx.state,
                &ctx.person,
                &a.str("mount_id")?,
                &a.str("path")?,
                &a.str("set_id")?,
            )
            .await
        }),
    );
}

/// A file every chunk tool may read: its normalized path, its current sha
/// (empty when null) and the chunk store of its volume.
struct File {
    path: String,
    sha: String,
    store: Option<ChunkStore>,
    client: Arc<VolumeClient>,
}

/// The common checks, in the contract order: project absent or deleted, not a
/// member, path absent, path is a folder (FR-NEW-017). Two lookups: membership
/// and the node.
async fn file(state: &AppState, person: &str, mount_id: &str, path: &str) -> Result<File> {
    state.authorize(mount_id, person).await?;
    let path = state.safety.normalize_path(path)?;
    let client = state.stores.client(mount_id).await?;
    // The root has no node of its own, yet it is a folder.
    let node = if path == "/" { None } else { client.meta.get(&path).await? };
    if path != "/" && node.is_none() {
        return Err(ToolError::not_found(format!("file not found: {path}")));
    }
    let node = match node {
        Some(n) if n.kind != "dir" => n,
        _ => return Err(ToolError::invalid_argument("path must be a file")),
    };
    let store = client.meta.chunk_store();
    Ok(File { path, sha: node.sha256.unwrap_or_default(), store, client })
}

/// The set `set_id` of the file, refused as absent when it names a set of
/// another file: a set is reached only through its own file.
async fn set_of(f: &File, set_id: &str) -> Result<(ChunkStore, Set)> {
    let absent = || ToolError::not_found(format!("chunk set not found: {set_id}"));
    let store = f.store.clone().ok_or_else(absent)?;
    let set = store.get_set(set_id).await?.filter(|s| s.path == f.path).ok_or_else(absent)?;
    Ok((store, set))
}

/// `fs.list_chunk_sets` (FR-NEW-014): `{sets: [Set]}`, the default set first,
/// then `built_at` oldest first, then `set_id`; a file with no set gets an
/// empty list. Cost: one query of at most 5 rows.
pub(crate) async fn list_chunk_sets(
    state: &AppState,
    person: &str,
    mount_id: &str,
    path: &str,
) -> Result<Value> {
    let f = file(state, person, mount_id, path).await?;
    let sets = match &f.store {
        Some(store) => store.list_sets(&f.path).await?,
        None => Vec::new(),
    };
    let sets: Vec<Value> = sets.iter().map(|s| s.to_json(&f.sha)).collect();
    Ok(json!({"sets": sets}))
}

/// `fs.list_chunks` (FR-NEW-015): at most [`PAGE`] chunk summaries by number,
/// plus a marker when more follow. Order of the tool's checks: set, marker.
/// Cost: one set lookup, one keyset query of at most 101 rows on the primary key.
pub(crate) async fn list_chunks(
    state: &AppState,
    person: &str,
    mount_id: &str,
    path: &str,
    set_id: &str,
    marker: &str,
) -> Result<Value> {
    let f = file(state, person, mount_id, path).await?;
    let (store, set) = set_of(&f, set_id).await?;
    let after = if marker.is_empty() { 0 } else { decode_marker(marker, &set.set_id)? };
    let mut rows = store.list_chunks(&set.set_id, after, PAGE + 1).await?;
    let more = rows.len() > PAGE;
    rows.truncate(PAGE);
    let chunks: Vec<Value> = rows.iter().map(|c| c.to_summary_json()).collect();
    let mut v = json!({"chunks": chunks});
    if let Some(last) = rows.last().filter(|_| more) {
        v["marker"] = json!(encode_marker(&set.set_id, last.number));
    }
    Ok(v)
}

/// `fs.get_chunk` (FR-NEW-016): chunk `number` in full with `before` chunks
/// before it and `after` after it, by number; neighbours beyond the ends are
/// absent. Order of the tool's checks: set, number, window. Cost: one set
/// lookup, one range query of at most 41 rows.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn get_chunk(
    state: &AppState,
    person: &str,
    mount_id: &str,
    path: &str,
    set_id: &str,
    number: i64,
    before: i64,
    after: i64,
) -> Result<Value> {
    let f = file(state, person, mount_id, path).await?;
    let (store, set) = set_of(&f, set_id).await?;
    let absent = || ToolError::not_found(format!("chunk {number} not found in set {set_id}"));
    let n = u32::try_from(number).ok().filter(|n| *n >= 1).ok_or_else(absent)?;
    if store.get_chunk_window(&set.set_id, n, n).await?.is_empty() {
        return Err(absent());
    }
    if !(0..=MAX_WINDOW).contains(&before) || !(0..=MAX_WINDOW).contains(&after) {
        return Err(ToolError::invalid_argument("before and after must be between 0 and 20"));
    }
    // Both are 0 to 20 here, so the casts are exact.
    let from = n.saturating_sub(before as u32).max(1);
    let to = n.saturating_add(after as u32);
    let rows = store.get_chunk_window(&set.set_id, from, to).await?;
    let chunks: Vec<Value> = rows.iter().map(|c| c.to_json()).collect();
    Ok(json!({"chunks": chunks}))
}

/// `fs.create_chunk_set` (FR-NEW-013, FR-NEW-029): the set the file holds for
/// this strategy and these settings, built when it holds none. Order of the
/// tool's checks: strategy, settings, text, number of sets (FR-NEW-017).
/// Cost: one set lookup, one file read, one linear cut off the runtime, one
/// transaction of one row per chunk.
pub(crate) async fn create_chunk_set(
    state: &AppState,
    person: &str,
    mount_id: &str,
    path: &str,
    strategy: &str,
    settings: Map<String, Value>,
) -> Result<Value> {
    let f = file(state, person, mount_id, path).await?;
    let req = Requested::parse(strategy, &settings, &state.config.search.chunking())?;
    let store = f
        .store
        .clone()
        .ok_or_else(|| ToolError::internal("volume has no relational meta store attached"))?;
    let set = service::create_set(state, &f.client, &store, mount_id, &f.path, &f.sha, req).await?;
    Ok(set.to_json(&f.sha))
}

/// `fs.delete_chunk_set` (FR-NEW-018): removes an extra set and its chunks.
/// Order of the tool's checks: set, then the default set. Cost: one lookup,
/// one transaction of two deletes.
pub(crate) async fn delete_chunk_set(
    state: &AppState,
    person: &str,
    mount_id: &str,
    path: &str,
    set_id: &str,
) -> Result<Value> {
    let f = file(state, person, mount_id, path).await?;
    let (store, set) = set_of(&f, set_id).await?;
    if set.is_default {
        return Err(ToolError::invalid_argument("the default chunk set cannot be deleted"));
    }
    if !store.delete_set(&set.set_id).await? {
        // Deleted by someone else between the lookup and the delete.
        return Err(ToolError::not_found(format!("chunk set not found: {set_id}")));
    }
    Ok(json!({"deleted": true}))
}
