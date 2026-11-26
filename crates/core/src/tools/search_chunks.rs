//! `search.chunks` (SPEC-0109): the chunks closest to one or several labeled
//! inputs, texts and images, as lines (input, document, chunk, similarity) with
//! optional chunks around each line marked as context.
//!
//! Typed functions only: the `#[tool]` methods in `mcp::server` call them. MCP
//! only, no REST twin (YAGNI). Nothing is ever written: a call embeds and
//! reads. The ordering is the one of paragraph-v3a section 7: a dense list and a
//! keyword list of 50 candidates each, fused by `1/(60 + rank)`, ties by best
//! rank then stable chunk id, one line per chunk (`search::fusion::fuse_units`).

use crate::errors::{Result, ToolError};
use crate::search::fusion::fuse_units;
use crate::search::{SearchBackend, VectorKind};
use crate::state::AppState;
use crate::storage::VolumeClient;
use crate::storage::traits::IndexMode;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const MAX_INPUTS: usize = 10;
const MAX_TEXT_CHARS: usize = 8000;
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_TOP_K: i64 = 50;
const MAX_CONTEXT: i64 = 5;
/// Candidates of each list (paragraph-v3a section 7).
const CANDIDATES: usize = 50;
/// Distinct words of an input the keyword list searches for.
const MAX_KEYWORDS: usize = 32;
const IMAGE_EXTENSIONS: [&str; 6] = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"];

/// One input as the caller sent it: exactly one of the two is set.
#[derive(Debug, Clone, Default)]
pub(crate) struct InputArg {
    pub text: Option<String>,
    pub image: Option<String>,
}

/// An input after the shape checks and, for an image, the read of its file.
enum Prepared {
    Text(String),
    Image { path: String, bytes: Vec<u8> },
}

struct Group {
    label: String,
    /// 1 based position in the call, for the refusal texts.
    position: usize,
    input: Prepared,
}

/// The words of `text` as a keyword query: letters and digits, lower case, so
/// converted Markdown never reaches the full text parser as syntax (DEC-003).
pub(crate) fn keyword_query(text: &str) -> String {
    let mut seen = HashSet::new();
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .filter(|w| seen.insert(w.clone()))
        .take(MAX_KEYWORDS)
        .collect::<Vec<_>>()
        .join(" ")
}

/// A similarity as the answer shows it: six decimals, so a repeat prints alike.
fn round6(s: f32) -> f64 {
    (f64::from(s) * 1e6).round() / 1e6
}

fn similarity_json(s: Option<f32>) -> Value {
    s.map_or(Value::Null, |s| json!(round6(s)))
}

struct Prepare<'a> {
    backend: &'a Arc<dyn SearchBackend>,
    client: Arc<VolumeClient>,
    groups: Vec<Group>,
    mode: IndexMode,
}

/// The checks both tools share, in their order: membership, the shape of the
/// inputs and of the counts, search on, a mode with vectors, then the image
/// files (read now, so a missing one is found before any call is made).
async fn prepare<'a>(
    state: &'a AppState,
    person: &str,
    mount: &str,
    inputs: &[InputArg],
    shape_ok: impl FnOnce() -> Result<()>,
) -> Result<Prepare<'a>> {
    state.authorize(mount, person).await?;
    if inputs.is_empty() || inputs.len() > MAX_INPUTS {
        return Err(ToolError::invalid_argument(format!(
            "inputs must hold 1 to {MAX_INPUTS} entries"
        )));
    }
    for (i, input) in inputs.iter().enumerate() {
        let n = i + 1;
        match (&input.text, &input.image) {
            (Some(text), None) => {
                if text.trim().is_empty() {
                    return Err(ToolError::invalid_argument(format!("input {n} text is empty")));
                }
                if text.trim().chars().count() > MAX_TEXT_CHARS {
                    return Err(ToolError::invalid_argument(format!(
                        "input {n} text is over {MAX_TEXT_CHARS} characters"
                    )));
                }
            }
            (None, Some(_)) => {}
            _ => {
                return Err(ToolError::invalid_argument(format!(
                    "input {n} must hold exactly one of text and image"
                )));
            }
        }
    }
    shape_ok()?;
    let backend = state.search.as_ref().ok_or_else(|| {
        ToolError::not_supported("search is not enabled; set search.enabled: true in config")
    })?;
    let mode = state.admin.get_index_mode(mount).await?;
    if !mode.needs_embedding() {
        return Err(ToolError::not_supported("the index mode of the project has no vectors"));
    }
    let client = state.stores.client(mount).await?;
    let (mut texts, mut images) = (0, 0);
    let mut groups = Vec::with_capacity(inputs.len());
    for (i, input) in inputs.iter().enumerate() {
        let position = i + 1;
        let (label, prepared) = match (&input.text, &input.image) {
            (Some(text), _) => {
                texts += 1;
                (format!("S{texts}"), Prepared::Text(text.trim().to_string()))
            }
            (_, Some(raw)) => {
                images += 1;
                let path = state.safety.normalize_path(raw)?;
                let lower = path.to_ascii_lowercase();
                if !IMAGE_EXTENSIONS.iter().any(|e| lower.ends_with(e)) {
                    return Err(ToolError::invalid_argument(format!(
                        "input {position} image must be a png, jpg, jpeg, gif, webp or bmp file"
                    )));
                }
                if !client.is_file(&path).await? {
                    return Err(ToolError::not_found(format!("not a file: {path}")));
                }
                // The declared size first: a huge file is never read into memory.
                if usize::try_from(client.stat(&path).await?.size).unwrap_or(usize::MAX)
                    > MAX_IMAGE_BYTES
                {
                    return Err(ToolError::invalid_argument(format!(
                        "input {position} image is over {MAX_IMAGE_BYTES} bytes"
                    )));
                }
                let bytes = client.read_bytes(&path).await?;
                if bytes.len() > MAX_IMAGE_BYTES {
                    return Err(ToolError::invalid_argument(format!(
                        "input {position} image is over {MAX_IMAGE_BYTES} bytes"
                    )));
                }
                (format!("I{images}"), Prepared::Image { path, bytes })
            }
            _ => unreachable!("the shape check above"),
        };
        groups.push(Group { label, position, input: prepared });
    }
    Ok(Prepare { backend, client, groups, mode })
}

fn top_k_of(top_k: i64) -> Result<usize> {
    if !(1..=MAX_TOP_K).contains(&top_k) {
        return Err(ToolError::invalid_argument(format!(
            "top_k must be between 1 and {MAX_TOP_K}"
        )));
    }
    Ok(usize::try_from(top_k).unwrap_or(10))
}

fn context_of(name: &str, n: i64) -> Result<u32> {
    if !(0..=MAX_CONTEXT).contains(&n) {
        return Err(ToolError::invalid_argument(format!(
            "{name} must be between 0 and {MAX_CONTEXT}"
        )));
    }
    Ok(u32::try_from(n).unwrap_or(0))
}

/// The text an input stands for: itself, or for an image the Markdown the
/// converter of the project answers, cut to the text limit (FR-NEW-004).
async fn text_of(state: &AppState, mount: &str, group: &Group) -> Result<String> {
    match &group.input {
        Prepared::Text(t) => Ok(t.clone()),
        Prepared::Image { path, bytes } => {
            let service = state.doc_service_for(mount).await?.ok_or_else(|| {
                ToolError::not_supported(
                    "no document converter is set up, an image cannot be turned into text",
                )
            })?;
            if bytes.len() as u64 > service.max_input_bytes() {
                return Err(ToolError::invalid_argument(format!(
                    "input {} image is over the converter limit of {} bytes",
                    group.position,
                    service.max_input_bytes()
                )));
            }
            let name = path.rsplit('/').next().unwrap_or(path);
            let markdown = service.to_markdown(bytes, name).await?;
            let text: String = markdown.trim().chars().take(MAX_TEXT_CHARS).collect();
            if text.is_empty() {
                return Err(ToolError::invalid_argument(format!(
                    "the converter found no text in {path}"
                )));
            }
            Ok(text)
        }
    }
}

/// One chunk to show, with what its answer needs.
struct Shown {
    path: String,
    number: u32,
    text: String,
    heading_path: Vec<String>,
}

/// `search.chunks`. Cost per input: one conversion (image), one embedding call,
/// one dense read and one full text read of at most 50 rows, one chunk window
/// per line, one similarity read of the entries shown.
pub(crate) async fn chunks(
    state: &AppState,
    person: &str,
    mount: &str,
    inputs: &[InputArg],
    top_k: i64,
    context_before: i64,
    context_after: i64,
) -> Result<Value> {
    let mut top = 0;
    let (mut before, mut after) = (0, 0);
    let p = prepare(state, person, mount, inputs, || {
        top = top_k_of(top_k)?;
        before = context_of("context_before", context_before)?;
        after = context_of("context_after", context_after)?;
        Ok(())
    })
    .await?;
    let keyword = p.mode == IndexMode::Both && p.backend.supported_modes().contains(&"bm25");
    let mut out = Vec::with_capacity(p.groups.len());
    for group in &p.groups {
        let text = text_of(state, mount, group).await?;
        let lines = chunk_lines(&p, mount, &text, keyword, top, before, after).await?;
        let mut g = json!({
            "label": group.label,
            "kind": if matches!(group.input, Prepared::Text(_)) { "text" } else { "image" },
            "lines": lines,
        });
        if matches!(group.input, Prepared::Image { .. }) {
            g["query_text"] = json!(text);
        }
        out.push(g);
    }
    Ok(json!({ "mode_used": if keyword { "both" } else { "rag" }, "inputs": out }))
}

async fn chunk_lines(
    p: &Prepare<'_>,
    mount: &str,
    text: &str,
    keyword: bool,
    top: usize,
    before: u32,
    after: u32,
) -> Result<Vec<Value>> {
    let backend = p.backend;
    let vector = backend.embed_input(mount, VectorKind::Text, text).await?;
    let dense = backend.nearest(mount, VectorKind::Text, &vector, CANDIDATES).await?;
    let kw_hits = if keyword {
        let q = keyword_query(text);
        if q.is_empty() {
            Vec::new()
        } else {
            // A failing keyword list never fails the call, as `search.query` in `both`.
            backend.query_bm25(mount, &q, CANDIDATES).await.unwrap_or_else(|e| {
                tracing::warn!(project = mount, error = %e, "search.chunks keyword list failed");
                Vec::new()
            })
        }
    } else {
        Vec::new()
    };
    // The text each list stored, for a chunk that has no default set.
    let mut stored: HashMap<(String, u32), String> = HashMap::new();
    for n in &dense {
        stored.entry((n.path.clone(), n.number)).or_insert_with(|| n.text.clone());
    }
    for r in &kw_hits {
        stored.entry((r.path.clone(), r.chunk_idx)).or_insert_with(|| r.chunk.clone());
    }
    let dense_ids: Vec<(String, u32)> = dense.iter().map(|n| (n.path.clone(), n.number)).collect();
    let kw_ids: Vec<(String, u32)> =
        kw_hits.iter().map(|r| (r.path.clone(), r.chunk_idx)).collect();
    let fused: Vec<_> = fuse_units(&dense_ids, &kw_ids).into_iter().take(top).collect();

    // Every chunk of a line's window, once: lines first, then context in line order.
    let mut claimed: HashSet<(String, u32)> =
        fused.iter().map(|f| (f.path.clone(), f.number)).collect();
    let store = p.client.meta.chunk_store();
    let mut windows: Vec<(Shown, Vec<(&'static str, Shown)>)> = Vec::new();
    for f in &fused {
        let set_id = match &store {
            Some(s) => s.default_set(&f.path).await?.map(|set| set.set_id),
            None => None,
        };
        let (Some(store), Some(set_id)) = (&store, set_id) else {
            let text = stored.get(&(f.path.clone(), f.number)).cloned().unwrap_or_default();
            windows.push((
                Shown { path: f.path.clone(), number: f.number, text, heading_path: Vec::new() },
                Vec::new(),
            ));
            continue;
        };
        let from = f.number.saturating_sub(before).max(1);
        let window = store.get_chunk_window(&set_id, from, f.number.saturating_add(after)).await?;
        let shown = |c: &crate::chunking::Chunk| Shown {
            path: f.path.clone(),
            number: c.number,
            text: c.text.clone(),
            heading_path: c.heading_path.clone(),
        };
        let own = match window.iter().find(|c| c.number == f.number) {
            Some(c) => shown(c),
            None => Shown {
                path: f.path.clone(),
                number: f.number,
                text: stored.get(&(f.path.clone(), f.number)).cloned().unwrap_or_default(),
                heading_path: Vec::new(),
            },
        };
        let mut around = Vec::new();
        for c in &window {
            if c.number == f.number || !claimed.insert((f.path.clone(), c.number)) {
                continue;
            }
            let position = if c.number < f.number { "before" } else { "after" };
            around.push((position, shown(c)));
        }
        windows.push((own, around));
    }
    let asked: Vec<(String, u32)> = windows
        .iter()
        .flat_map(|(own, around)| {
            std::iter::once((own.path.clone(), own.number))
                .chain(around.iter().map(|(_, c)| (c.path.clone(), c.number)))
        })
        .collect();
    let sims = backend.similarities(mount, VectorKind::Text, &vector, &asked).await?;
    let by_entry: HashMap<&(String, u32), Option<f32>> = asked.iter().zip(sims).collect();
    let sim =
        |s: &Shown| similarity_json(by_entry.get(&(s.path.clone(), s.number)).copied().flatten());
    Ok(windows
        .iter()
        .map(|(own, around)| {
            let mut line = json!({
                "document": own.path,
                "chunk": own.number,
                "text": own.text,
                "heading_path": own.heading_path,
                "similarity": sim(own),
            });
            if before > 0 || after > 0 {
                line["context"] = json!(
                    around
                        .iter()
                        .map(|(position, c)| json!({
                            "role": "context",
                            "position": position,
                            "chunk": c.number,
                            "text": c.text,
                            "heading_path": c.heading_path,
                            "similarity": sim(c),
                        }))
                        .collect::<Vec<_>>()
                );
            }
            line
        })
        .collect())
}

/// The `data:` URI a picture is embedded as (DEC-008 of SPEC-0108).
fn data_uri(path: &str, bytes: &[u8]) -> String {
    let mime = crate::docs::mime::guess(path).unwrap_or("application/octet-stream");
    format!(
        "data:{mime};base64,{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
    )
}

/// `search.images`: per input, the `top_k` stored images closest to it by the
/// image model of the project, as lines (document, image number, text, similarity).
/// A picture goes to the model as a `data:` URI, a text as it is (DEC-007).
/// Cost per input: one embedding call and one read of at most 50 rows.
pub(crate) async fn images(
    state: &AppState,
    person: &str,
    mount: &str,
    inputs: &[InputArg],
    top_k: i64,
) -> Result<Value> {
    let mut top = 0;
    let p = prepare(state, person, mount, inputs, || {
        top = top_k_of(top_k)?;
        Ok(())
    })
    .await?;
    let mut out = Vec::with_capacity(p.groups.len());
    for group in &p.groups {
        let (kind, input) = match &group.input {
            Prepared::Text(t) => ("text", t.clone()),
            Prepared::Image { path, bytes } => ("image", data_uri(path, bytes)),
        };
        let vector = p.backend.embed_input(mount, VectorKind::Image, &input).await?;
        let near = p.backend.nearest(mount, VectorKind::Image, &vector, CANDIDATES).await?;
        let lines: Vec<Value> = near
            .into_iter()
            .take(top)
            .map(|n| {
                json!({
                    "document": n.path,
                    "image": n.number,
                    "text": n.text,
                    "similarity": round6(n.similarity),
                })
            })
            .collect();
        out.push(json!({ "label": group.label, "kind": kind, "lines": lines }));
    }
    Ok(json!({ "inputs": out }))
}

#[cfg(test)]
mod tests {
    use super::keyword_query;

    /// DEC-003: only letters and digits reach the full text parser, in lower case.
    #[test]
    fn the_keyword_query_keeps_words_only() {
        assert_eq!(
            keyword_query("What is RUST: fast? \"AND\" (x) NOT"),
            "what is rust fast and x not"
        );
        assert_eq!(keyword_query("# Chart\n\n| a | b |"), "chart a b");
        assert_eq!(keyword_query("::: ... ---"), "");
        assert_eq!(keyword_query("café 42"), "café 42");
    }

    /// A converted page is cut to its first 32 distinct words (the full text engines
    /// match every word, and a 1300 term query is a scan).
    #[test]
    fn the_keyword_query_is_capped_to_distinct_words() {
        let text = (1..=100).map(|n| format!("w{n} w{n}")).collect::<Vec<_>>().join(" ");
        let q = keyword_query(&text);
        let words: Vec<&str> = q.split(' ').collect();
        assert_eq!(words.len(), 32);
        assert_eq!(words[0], "w1");
        assert_eq!(words[31], "w32");
    }
}
