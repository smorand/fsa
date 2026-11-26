//! `fs.get_vectors` (SPEC-0108 FR-NEW-006, 007): the vectors stored for a file,
//! computed with the models of the project profile.
//!
//! Typed function only: the `#[tool]` method in `mcp::server` calls it. MCP only,
//! no REST twin (YAGNI, a vector is read by an agent, not downloaded).

use crate::errors::{Result, ToolError};
use crate::search::VectorKind;
use crate::state::AppState;
#[cfg(test)]
use crate::tools::registry_support::{ToolRegistry, ToolSchema, handler};
use serde_json::{Value, json};

/// Must equal the `#[tool]` description in `mcp::server`, a literal there.
#[cfg(test)]
pub(crate) const GET_VECTORS: &str = "Read the vectors stored for a file, with the text each was computed from: its chunks (kind 'text') or the images of a converted document (kind 'image'), in number order, 20 per page by default.";

/// The parameter descriptions, shared with the `#[tool]` argument struct.
#[cfg(test)]
pub(crate) const PARAMS: [(&str, &str); 3] = [
    (
        "kind",
        "What the vectors stand for: 'text' (chunks of the default set, the default) or 'image'.",
    ),
    ("after", "Number of the last entry of the previous page; 0 reads from the start."),
    ("limit", "Entries per page, 1 to 100."),
];

const MAX_LIMIT: i64 = 100;

#[cfg(test)]
pub(crate) fn register(reg: &mut ToolRegistry) {
    reg.add(
        ToolSchema::new("fs.get_vectors", GET_VECTORS)
            .req_str("mount_id", "Project/volume id the operation targets.")
            .req_str("path", "Absolute POSIX path of the file.")
            .opt_str("kind", "text", PARAMS[0].1)
            .opt_int("after", 0, PARAMS[1].1)
            .opt_int("limit", 20, PARAMS[2].1)
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            get_vectors(
                &ctx.state,
                &ctx.person,
                &a.str("mount_id")?,
                &a.str("path")?,
                &a.str_or("kind", "text"),
                a.int_or("after", 0),
                a.int_or("limit", 20),
            )
            .await
        }),
    );
}

/// Order of the refusals: membership, the shape of `kind`, `after` and `limit`,
/// search off, a mode without vectors, then a path that is not a file. Cost: one
/// file check and one page query of at most 101 rows.
pub(crate) async fn get_vectors(
    state: &AppState,
    person: &str,
    mount: &str,
    raw_path: &str,
    kind: &str,
    after: i64,
    limit: i64,
) -> Result<Value> {
    state.authorize(mount, person).await?;
    let path = state.safety.normalize_path(raw_path)?;
    let path = path.as_str();
    let kind = match kind {
        "" | "text" => VectorKind::Text,
        "image" => VectorKind::Image,
        _ => return Err(ToolError::invalid_argument("kind must be 'text' or 'image'")),
    };
    let after = u32::try_from(after)
        .map_err(|_| ToolError::invalid_argument("after must be 0 or a number from a page"))?;
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ToolError::invalid_argument("limit must be between 1 and 100"));
    }
    let backend = state.search.as_ref().ok_or_else(|| {
        ToolError::not_supported("search is not enabled; set search.enabled: true in config")
    })?;
    if !state.admin.get_index_mode(mount).await?.needs_embedding() {
        return Err(ToolError::not_supported("the index mode of the project has no vectors"));
    }
    let client = state.stores.client(mount).await?;
    if !client.is_file(path).await? {
        return Err(ToolError::not_found(format!("not a file: {path}")));
    }
    let page =
        backend.vectors_of(mount, path, kind, after, usize::try_from(limit).unwrap_or(20)).await?;
    let vectors: Vec<Value> = page
        .vectors
        .iter()
        .map(|v| json!({"number": v.number, "text": v.text, "vector": v.vector}))
        .collect();
    Ok(json!({
        "path": path,
        "kind": kind.as_str(),
        "dimension": page.dimension,
        "vectors": vectors,
        "next_after": page.next_after,
    }))
}

#[cfg(test)]
mod tests {
    /// The test registry and the live router describe the tool and its
    /// parameters alike, so the golden contract regenerated from the one is what
    /// the other serves.
    #[test]
    fn the_live_descriptions_equal_the_registry_ones() {
        let router = crate::mcp::server::McpServer::tool_router();
        let tools = router.list_all();
        let tool = tools.iter().find(|t| t.name == "fs.get_vectors").expect("fs.get_vectors");
        assert_eq!(tool.description.as_deref(), Some(super::GET_VECTORS));
        for (param, desc) in super::PARAMS {
            let live = &tool.input_schema["properties"][param]["description"];
            assert_eq!(live.as_str(), Some(desc), "{param}");
        }
    }
}
