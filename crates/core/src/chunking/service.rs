//! Building chunk sets (SPEC-0101 DEC-002): the request checks of
//! `fs.create_chunk_set` (FR-NEW-017), the canonical request key (DEC-005),
//! the text a file is cut from (DEC-008) and the cut itself, run off the
//! runtime threads (DEC-016).

use super::model::{Chunk, Set};
use super::structural::StructuralSettings;
use super::{ChunkDefaults, ChunkStore};
use crate::errors::{Result, ToolError};
use crate::state::AppState;
use crate::storage::volume::VolumeClient;
use serde_json::{Map, Value, json};

/// A chunking strategy a set may request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    Fixed,
    Structural,
}

impl Strategy {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "fixed" => Ok(Self::Fixed),
            "structural" => Ok(Self::Structural),
            _ => Err(ToolError::invalid_argument("strategy must be 'fixed' or 'structural'")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Structural => "structural",
        }
    }
}

/// Every setting a strategy knows, with its strategy and its allowed range
/// (FR-NEW-001, FR-NEW-006, FR-NEW-008, FR-NEW-011).
const SETTINGS: [(&str, Strategy, u32, u32); 5] = [
    ("size_tokens", Strategy::Fixed, 32, 8192),
    ("overlap_tokens", Strategy::Fixed, 0, 8191),
    ("heading_cut_depth", Strategy::Structural, 1, 6),
    ("max_tokens", Strategy::Structural, 64, 8192),
    ("list_max_items", Strategy::Structural, 2, 200),
];

/// A request whose strategy and settings passed every check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requested {
    pub strategy: Strategy,
    /// The settings as requested, in the order of the request.
    pub settings: Vec<(&'static str, u32)>,
}

/// A setting value as a whole number, or `None`. Only a JSON integer is one:
/// `2.5`, `abc` and a string of digits are not (FR-NEW-017).
fn whole(v: &Value) -> Option<i128> {
    match v {
        Value::Number(n) => n.as_i64().map(i128::from).or_else(|| n.as_u64().map(i128::from)),
        _ => None,
    }
}

impl Requested {
    /// The checks of FR-NEW-017 in their order: strategy, then each setting
    /// in the order of the request (unknown, does not apply, not whole, out
    /// of range), then the overlap against the size with the defaults filling
    /// what was omitted. The first failure is answered.
    pub fn parse(strategy: &str, settings: &Map<String, Value>, d: &ChunkDefaults) -> Result<Self> {
        let strategy = Strategy::parse(strategy)?;
        let mut out = Vec::with_capacity(settings.len());
        for (name, value) in settings {
            let Some(&(known, owner, min, max)) = SETTINGS.iter().find(|s| s.0 == name) else {
                return Err(ToolError::invalid_argument(format!("unknown setting '{name}'")));
            };
            if owner != strategy {
                return Err(ToolError::invalid_argument(format!(
                    "setting '{name}' does not apply to strategy '{}'",
                    strategy.as_str()
                )));
            }
            let n = whole(value).ok_or_else(|| {
                ToolError::invalid_argument(format!(
                    "{name} must be a whole number between {min} and {max}"
                ))
            })?;
            if n < i128::from(min) || n > i128::from(max) {
                return Err(ToolError::invalid_argument(format!(
                    "{name} must be between {min} and {max}"
                )));
            }
            out.push((known, n as u32));
        }
        let req = Self { strategy, settings: out };
        if strategy == Strategy::Fixed {
            let (size, overlap) = req.fixed(d);
            if overlap >= size {
                return Err(ToolError::invalid_argument(
                    "overlap_tokens must be smaller than size_tokens",
                ));
            }
        }
        Ok(req)
    }

    fn get(&self, name: &str) -> Option<u32> {
        self.settings.iter().find(|s| s.0 == name).map(|s| s.1)
    }

    /// `size_tokens` and `overlap_tokens`, the defaults filling what was omitted.
    pub fn fixed(&self, d: &ChunkDefaults) -> (u32, u32) {
        (
            self.get("size_tokens").unwrap_or(d.fixed_size_tokens),
            self.get("overlap_tokens").unwrap_or(d.fixed_overlap_tokens),
        )
    }

    /// The structural settings, the defaults filling what was omitted.
    pub fn structural(&self, d: &ChunkDefaults) -> StructuralSettings {
        StructuralSettings {
            heading_cut_depth: self.get("heading_cut_depth").unwrap_or(d.heading_cut_depth),
            max_tokens: self.get("max_tokens").unwrap_or(d.max_tokens),
            list_max_items: self.get("list_max_items").unwrap_or(d.list_max_items),
        }
    }

    /// Every setting of the strategy, omitted ones from the defaults, by name.
    /// These are the settings a set records (FR-NEW-013).
    pub fn filled(&self, d: &ChunkDefaults) -> Map<String, Value> {
        let mut m = Map::new();
        match self.strategy {
            Strategy::Fixed => {
                let (size, overlap) = self.fixed(d);
                m.insert("overlap_tokens".into(), json!(overlap));
                m.insert("size_tokens".into(), json!(size));
            }
            Strategy::Structural => {
                let g = |n: &str, def: u32| json!(self.get(n).unwrap_or(def));
                m.insert("heading_cut_depth".into(), g("heading_cut_depth", d.heading_cut_depth));
                m.insert("list_max_items".into(), g("list_max_items", d.list_max_items));
                m.insert("max_tokens".into(), g("max_tokens", d.max_tokens));
            }
        }
        m
    }

    /// The canonical key of DEC-005: keys in name order, the defaults filled
    /// in, so `{}` and the default values spelled out are one request.
    pub fn request_key(&self, d: &ChunkDefaults) -> String {
        // Inserted in name order: the map keeps insertion order.
        json!({"settings": self.filled(d), "strategy": self.strategy.as_str()}).to_string()
    }

    /// The settings as requested, kept beside the used ones (DEC-017).
    pub fn requested_json(&self) -> String {
        let m: Map<String, Value> =
            self.settings.iter().map(|(n, v)| ((*n).to_string(), json!(v))).collect();
        Value::Object(m).to_string()
    }
}

/// The title of a document: the file name without its extension (FR-NEW-009).
pub fn title(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The text a file is cut from (DEC-008): its bytes when they are UTF-8,
/// else the document view of its current conversion (image captions, tables
/// as Markdown), else nothing to cut.
pub async fn source_text(
    state: &AppState,
    client: &VolumeClient,
    mount_id: &str,
    path: &str,
) -> Result<String> {
    if let Ok(text) = String::from_utf8(client.read_bytes(path).await?) {
        return Ok(text);
    }
    let converted = match &client.trash {
        Some(meta) => meta.current_artifact_set(path).await?.is_some(),
        None => false,
    };
    if !converted {
        return Err(ToolError::not_supported(format!("no text to cut for {path}")));
    }
    let view =
        crate::tools::artifacts::document_view(state, mount_id, path, true, Some("markdown"), None)
            .await?;
    Ok(view["text"].as_str().unwrap_or_default().to_string())
}

/// The outcome of a cut: the strategy and settings used and the chunks.
pub struct Cut {
    pub used: Strategy,
    pub fallback_from: Option<Strategy>,
    /// Every setting of the strategy used, by name: the server fixed defaults
    /// after a fallback (FR-NEW-012).
    pub settings: Map<String, Value>,
    pub chunks: Vec<Chunk>,
}

/// Cleans then cuts `text` on the blocking pool, so a long text never holds a
/// runtime thread (DEC-016).
pub async fn cut(text: String, title: String, req: Requested, d: ChunkDefaults) -> Result<Cut> {
    tokio::task::spawn_blocking(move || super::cut(&text, &title, &req, &d))
        .await
        .map_err(|e| ToolError::internal(format!("chunking stopped unexpectedly: {e}")))
}

/// A new set id: `set-` plus 8 hex digits of a uuid (design 5.1).
pub fn new_set_id() -> String {
    format!("set-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
}

/// `fs.create_chunk_set` once the file checks passed: the set the file holds
/// for this request, built only when it holds none (FR-NEW-013, FR-NEW-029).
/// Order of the remaining checks: text, then the number of sets inside the
/// store's transaction. Cost: one lookup, one read of the file, one linear
/// cut, one transaction of one row per chunk.
#[allow(clippy::too_many_arguments)]
pub async fn create_set(
    state: &AppState,
    client: &VolumeClient,
    store: &ChunkStore,
    mount_id: &str,
    path: &str,
    sha: &str,
    req: Requested,
) -> Result<Set> {
    let d = state.config.search.chunking();
    let key = req.request_key(&d);
    if let Some(existing) = store.find_set(path, &key).await? {
        return Ok(existing);
    }
    let text = source_text(state, client, mount_id, path).await?;
    let requested = req.requested_json();
    let strategy = req.strategy;
    let cut = cut(text, title(path), req, d).await?;
    let set = Set {
        set_id: new_set_id(),
        path: path.to_string(),
        request_key: key,
        strategy: cut.used.as_str().into(),
        settings: Value::Object(cut.settings.clone()).to_string(),
        requested_strategy: strategy.as_str().into(),
        requested_settings: requested,
        is_default: false,
        fallback_from: cut.fallback_from.map(|s| s.as_str().to_string()),
        chunk_count: 0,
        token_count: 0,
        source_sha: sha.to_string(),
        built_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    store.insert_set(&set, &cut.chunks).await
}

/// The request of a default set: `structural`, every setting from the server
/// defaults of the moment.
pub fn default_request() -> Requested {
    Requested { strategy: Strategy::Structural, settings: Vec::new() }
}

/// The request a stored set is rebuilt with (DEC-017). Its `request_key`
/// holds every setting filled in when it was made, so rebuilding from it keeps
/// both its settings and its key, whatever the server defaults became; a key
/// that does not read back falls back on the request as typed. `None` leaves
/// the set as it is.
fn stored_request(set: &Set, d: &ChunkDefaults) -> Option<Requested> {
    let from_key = serde_json::from_str::<Value>(&set.request_key).ok().and_then(|v| {
        let settings = v["settings"].as_object()?.clone();
        Requested::parse(v["strategy"].as_str()?, &settings, d).ok()
    });
    from_key.or_else(|| {
        let settings = serde_json::from_str::<Value>(&set.requested_settings).ok()?;
        Requested::parse(&set.requested_strategy, settings.as_object()?, d).ok()
    })
}

/// One set as the rebuild will write it.
struct Planned {
    set: Set,
    chunks: Vec<Chunk>,
    /// No row holds this set yet: it is inserted, not replaced.
    insert: bool,
}

/// The sets of one file cut again from its current text, not written yet, so
/// a rebuild that fails or is discarded before [`Built::commit`] leaves the
/// previous sets as they were (FR-NEW-022, DEC-013).
pub struct Built {
    /// The chunks of the first set cut, the default set for a file rebuild:
    /// the search entries are made of them.
    pub default_chunks: Vec<Chunk>,
    planned: Vec<Planned>,
}

/// Cuts every set of `path` from `text` (FR-NEW-021): the default set with
/// the server defaults `d` of the moment, created when the file has none,
/// every extra set with its own request, each keeping its id. A default whose
/// key changed with the defaults takes the new key; a set made by hand with
/// that key is absorbed into the default (DEC-005): kept as the default when
/// the file had no default, else removed when the default is written. Writes
/// nothing. Cost: one read of the sets (at most 5), then one linear cut per set.
pub async fn rebuild_file_sets(
    store: &ChunkStore,
    path: &str,
    sha: &str,
    text: &str,
    d: ChunkDefaults,
) -> Result<Built> {
    let sets = store.list_sets(path).await?;
    let req = default_request();
    let key = req.request_key(&d);
    let default = sets.iter().find(|s| s.is_default);
    let same_key = sets.iter().find(|s| !s.is_default && s.request_key == key);
    let (default_id, insert) = match (default, same_key) {
        (Some(s), _) | (None, Some(s)) => (s.set_id.clone(), false),
        (None, None) => (new_set_id(), true),
    };
    let built_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut planned = Vec::with_capacity(sets.len() + 1);
    let mut todo: Vec<(String, Requested, bool, bool)> =
        vec![(default_id.clone(), req, true, insert)];
    for s in sets.iter().filter(|s| s.set_id != default_id && s.request_key != key) {
        if let Some(r) = stored_request(s, &d) {
            todo.push((s.set_id.clone(), r, false, false));
        }
    }
    for (set_id, r, is_default, insert) in todo {
        planned.push(cut_set(path, sha, text, &d, &built_at, set_id, r, is_default, insert).await?);
    }
    let default_chunks = planned[0].chunks.clone();
    Ok(Built { default_chunks, planned })
}

/// One set cut from `text` with the request `r`, not written yet.
#[allow(clippy::too_many_arguments)]
async fn cut_set(
    path: &str,
    sha: &str,
    text: &str,
    d: &ChunkDefaults,
    built_at: &str,
    set_id: String,
    r: Requested,
    is_default: bool,
    insert: bool,
) -> Result<Planned> {
    let request_key = r.request_key(d);
    let requested = r.requested_json();
    let strategy = r.strategy;
    let cut = cut(text.to_string(), title(path), r, *d).await?;
    let set = Set {
        set_id,
        path: path.to_string(),
        request_key,
        strategy: cut.used.as_str().into(),
        settings: Value::Object(cut.settings).to_string(),
        requested_strategy: strategy.as_str().into(),
        requested_settings: requested,
        is_default,
        fallback_from: cut.fallback_from.map(|s| s.as_str().to_string()),
        chunk_count: 0,
        token_count: 0,
        source_sha: sha.to_string(),
        built_at: built_at.to_string(),
    };
    Ok(Planned { set, chunks: cut.chunks, insert })
}

/// Cuts the one set `set_id` of `path` from `text` (SPEC-0101 FR-NEW-019),
/// not written yet. The default set takes the default request and the server
/// defaults of the moment, an extra set its own stored request. `None` when the
/// set is gone (removed by hand meanwhile) or its stored request no longer
/// reads, so there is nothing to rebuild. Cost: one lookup, one linear cut.
pub async fn rebuild_one_set(
    store: &ChunkStore,
    path: &str,
    set_id: &str,
    sha: &str,
    text: &str,
    d: ChunkDefaults,
) -> Result<Option<Built>> {
    let Some(set) = store.get_set(set_id).await?.filter(|s| s.path == path) else {
        return Ok(None);
    };
    let req = if set.is_default { Some(default_request()) } else { stored_request(&set, &d) };
    let Some(req) = req else { return Ok(None) };
    let built_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let planned =
        cut_set(path, sha, text, &d, &built_at, set.set_id, req, set.is_default, false).await?;
    Ok(Some(Built { default_chunks: planned.chunks.clone(), planned: vec![planned] }))
}

impl Built {
    /// Whether the first set cut is the default set (always true for a file
    /// rebuild, the question of a rebuild of one set).
    pub fn first_is_default(&self) -> bool {
        self.planned[0].set.is_default
    }

    /// Writes every set, one transaction each, the default first; `still_current`
    /// is the last check before each transaction (DEC-012). Answers false, the
    /// remaining sets unwritten, when it refuses. A set removed by hand since the
    /// cut stays removed; a default removed meanwhile is created again.
    pub async fn commit(
        self,
        store: &ChunkStore,
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<bool> {
        for Planned { mut set, chunks, insert } in self.planned {
            if !still_current() {
                return Ok(false);
            }
            if !insert && store.replace_set(&set, &chunks).await? {
                continue;
            }
            if !set.is_default {
                continue;
            }
            // An equal request stored meanwhile answers the insert: it becomes the default.
            let stored = store.insert_set(&set, &chunks).await?;
            if stored.set_id != set.set_id {
                set.set_id = stored.set_id;
                store.replace_set(&set, &chunks).await?;
            }
        }
        Ok(true)
    }
}

/// A file that is no longer text (FR-NEW-021, design 3.2): a converted
/// document keeps its sets, which read `out_of_date` until a set is rebuilt by
/// hand; any other file loses every set. `still_current` is checked before the
/// removal. Answers whether the sets were removed, so the caller removes the
/// search entries too.
pub async fn drop_file_sets(
    client: &VolumeClient,
    store: &ChunkStore,
    path: &str,
    still_current: &(dyn Fn() -> bool + Send + Sync),
) -> Result<bool> {
    let converted = match &client.trash {
        Some(meta) => meta.current_artifact_set(path).await?.is_some(),
        None => false,
    };
    if converted || !still_current() {
        return Ok(false);
    }
    store.delete_sets_of(path).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: ChunkDefaults = ChunkDefaults {
        fixed_size_tokens: 2048,
        fixed_overlap_tokens: 0,
        heading_cut_depth: 2,
        max_tokens: 2048,
        list_max_items: 40,
    };

    fn parse(strategy: &str, settings: Value) -> std::result::Result<Requested, String> {
        let m = settings.as_object().cloned().unwrap_or_default();
        Requested::parse(strategy, &m, &D).map_err(|e| e.to_string())
    }

    #[test]
    fn the_checks_come_in_the_contract_order() {
        let e = |s: &str, v: Value| parse(s, v).unwrap_err();
        assert_eq!(
            e("sentence", json!({"window": 1})),
            "ERR_INVALID_ARGUMENT: strategy must be 'fixed' or 'structural'"
        );
        assert_eq!(
            e("fixed", json!({"window": "x", "heading_cut_depth": 3})),
            "ERR_INVALID_ARGUMENT: unknown setting 'window'"
        );
        assert_eq!(
            e("fixed", json!({"heading_cut_depth": "x", "window": 1})),
            "ERR_INVALID_ARGUMENT: setting 'heading_cut_depth' does not apply to strategy 'fixed'"
        );
        assert_eq!(
            e("structural", json!({"max_tokens": 1.5})),
            "ERR_INVALID_ARGUMENT: max_tokens must be a whole number between 64 and 8192"
        );
        assert_eq!(
            e("structural", json!({"list_max_items": 201})),
            "ERR_INVALID_ARGUMENT: list_max_items must be between 2 and 200"
        );
        assert_eq!(
            e("fixed", json!({"overlap_tokens": 8192})),
            "ERR_INVALID_ARGUMENT: overlap_tokens must be between 0 and 8191"
        );
        assert_eq!(
            e("fixed", json!({"overlap_tokens": 2048})),
            "ERR_INVALID_ARGUMENT: overlap_tokens must be smaller than size_tokens",
            "the default size is the reference"
        );
        assert_eq!(
            e("fixed", json!({"size_tokens": -1})),
            "ERR_INVALID_ARGUMENT: size_tokens must be between 32 and 8192"
        );
        assert!(parse("fixed", json!({"size_tokens": "500"})).is_err());
    }

    #[test]
    fn the_request_key_fills_defaults_and_ignores_order() {
        let a = parse("fixed", json!({})).unwrap();
        let b = parse("fixed", json!({"overlap_tokens": 0, "size_tokens": 2048})).unwrap();
        let c = parse("fixed", json!({"size_tokens": 2048, "overlap_tokens": 0})).unwrap();
        assert_eq!(a.request_key(&D), b.request_key(&D));
        assert_eq!(b.request_key(&D), c.request_key(&D));
        assert_eq!(
            a.request_key(&D),
            r#"{"settings":{"overlap_tokens":0,"size_tokens":2048},"strategy":"fixed"}"#
        );
        assert_eq!(a.requested_json(), "{}");
        assert_eq!(c.requested_json(), r#"{"size_tokens":2048,"overlap_tokens":0}"#);
        let s = parse("structural", json!({"max_tokens": 64})).unwrap();
        assert_eq!(
            Value::Object(s.filled(&D)).to_string(),
            r#"{"heading_cut_depth":2,"list_max_items":40,"max_tokens":64}"#
        );
    }

    #[test]
    fn the_title_is_the_file_name_without_extension() {
        assert_eq!(title("/docs/essay.md"), "essay");
        assert_eq!(title("/a.tar.gz"), "a.tar");
        assert_eq!(title("/README"), "README");
    }
}
