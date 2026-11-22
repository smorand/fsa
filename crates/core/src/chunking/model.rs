//! The stored shapes of a chunk set and its chunks (design section 4), and the
//! paging marker of `fs.list_chunks` (design 5.3).

use crate::errors::{Result, ToolError};
use base64::Engine;
use serde_json::{Value, json};

/// Characters of a chunk shown by `fs.list_chunks` (FR-NEW-015).
pub const PREVIEW_CHARS: usize = 200;

/// One row of `chunk_sets`. `settings` and `requested_settings` are JSON
/// objects kept as text, the way they are stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Set {
    pub set_id: String,
    pub path: String,
    /// Canonical JSON of the requested strategy and settings (DEC-005).
    pub request_key: String,
    /// The strategy used, `fixed` after a fallback.
    pub strategy: String,
    pub settings: String,
    pub requested_strategy: String,
    pub requested_settings: String,
    pub is_default: bool,
    pub fallback_from: Option<String>,
    pub chunk_count: i64,
    pub token_count: i64,
    /// The file's `nodes.sha256` the set was cut from, empty when null.
    pub source_sha: String,
    /// RFC 3339 UTC seconds.
    pub built_at: String,
}

impl Set {
    /// The `Set` object of design 5.1. `file_sha` is the file's current
    /// `sha256` (empty when null): a differing source makes the set
    /// `out_of_date` (DEC-004), stable across moves.
    pub fn to_json(&self, file_sha: &str) -> Value {
        let settings: Value = serde_json::from_str(&self.settings)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        let state = if self.source_sha == file_sha { "current" } else { "out_of_date" };
        let mut v = json!({
            "set_id": self.set_id,
            "strategy": self.strategy,
            "settings": settings,
            "default": self.is_default,
            "chunk_count": self.chunk_count,
            "token_count": self.token_count,
            "state": state,
            "built_at": self.built_at,
        });
        if let Some(f) = &self.fallback_from {
            v["fallback_from"] = json!(f);
        }
        v
    }
}

/// One row of `chunks` (FR-NEW-009).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub number: u32,
    pub unit: u32,
    pub text: String,
    pub heading_path: Vec<String>,
    pub page_start: Option<u32>,
    pub page_end: Option<u32>,
    pub token_count: u32,
    pub figures: Vec<String>,
    pub document_title: String,
}

impl Chunk {
    fn fields(&self) -> Value {
        json!({
            "number": self.number,
            "unit": self.unit,
            "heading_path": self.heading_path,
            "page_start": self.page_start,
            "page_end": self.page_end,
            "token_count": self.token_count,
            "figures": self.figures,
            "document_title": self.document_title,
        })
    }

    /// The `Chunk` object of `fs.get_chunk`: every field plus the full text.
    pub fn to_json(&self) -> Value {
        let mut v = self.fields();
        v["text"] = json!(self.text);
        v
    }

    /// The `ChunkSummary` of `fs.list_chunks`: a preview instead of the text,
    /// counted in characters so a multibyte letter is never cut.
    pub fn to_summary_json(&self) -> Value {
        let mut v = self.fields();
        v["preview"] = json!(self.text.chars().take(PREVIEW_CHARS).collect::<String>());
        v
    }
}

/// The `fs.list_chunks` marker: base64 url safe without padding of
/// `["<set_id>", <last number>]` (design 5.3).
pub fn encode_marker(set_id: &str, last: u32) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json!([set_id, last]).to_string())
}

/// The last number a marker names. A marker of another set or one that does
/// not decode is refused the same way (FR-NEW-015).
pub fn decode_marker(raw: &str, set_id: &str) -> Result<u32> {
    let invalid = || ToolError::invalid_argument("marker is not valid");
    let bytes =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw).map_err(|_| invalid())?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    match v.as_array().map(Vec::as_slice) {
        Some([Value::String(s), n]) if s == set_id => {
            n.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(invalid)
        }
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_round_trips_for_its_own_set_only() {
        let m = encode_marker("set-0000abcd", 100);
        assert_eq!(decode_marker(&m, "set-0000abcd").unwrap(), 100);
        for bad in [m.as_str(), "zzz", "", "WzFd"] {
            let e = decode_marker(bad, "set-other").unwrap_err();
            assert_eq!(e.to_string(), "ERR_INVALID_ARGUMENT: marker is not valid");
        }
    }

    #[test]
    fn the_preview_counts_characters() {
        let c = Chunk {
            number: 1,
            unit: 1,
            text: "é".repeat(300),
            heading_path: vec![],
            page_start: None,
            page_end: None,
            token_count: 1,
            figures: vec![],
            document_title: "t".into(),
        };
        let v = c.to_summary_json();
        assert_eq!(v["preview"].as_str().unwrap().chars().count(), PREVIEW_CHARS);
        assert!(v.get("text").is_none());
    }
}
