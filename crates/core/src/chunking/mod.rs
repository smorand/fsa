//! Chunk sets as file metadata (SPEC-0101).

pub mod blocks;
pub mod fixed;
pub mod model;
pub mod service;
pub mod store;
pub mod structural;
pub mod tokens;

#[cfg(test)]
mod e2e;

pub use model::{Chunk, Set};
pub use store::ChunkStore;

/// The server wide default settings of the chunk sets, already validated.
/// Plain numbers so the scheduler and the tools can copy it freely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkDefaults {
    pub fixed_size_tokens: u32,
    pub fixed_overlap_tokens: u32,
    pub heading_cut_depth: u32,
    pub max_tokens: u32,
    pub list_max_items: u32,
}

impl Default for ChunkDefaults {
    /// The values an absent configuration reads as (FR-NEW-027).
    fn default() -> Self {
        Self {
            fixed_size_tokens: 2048,
            fixed_overlap_tokens: 0,
            heading_cut_depth: 2,
            max_tokens: 2048,
            list_max_items: 40,
        }
    }
}

/// Cleans then cuts `text` with the strategy of `req` (design 3). A
/// `structural` request on a text with no heading line, a `#` line inside
/// code not counting, is cut by `fixed` with the server fixed defaults and
/// reports `fallback_from: structural` (FR-NEW-012); an empty text is that
/// fallback with 0 chunks. The text is parsed once: the heading check reads
/// the same blocks the cut uses.
pub fn cut(text: &str, title: &str, req: &service::Requested, d: &ChunkDefaults) -> service::Cut {
    use service::{Cut, Requested, Strategy};
    let text = tokens::clean(text);
    let fixed_cut = |r: &Requested, fallback_from| {
        let (size, overlap) = r.fixed(d);
        Cut {
            used: Strategy::Fixed,
            fallback_from,
            settings: r.filled(d),
            chunks: fixed::cut(&text, title, size, overlap),
        }
    };
    if req.strategy == Strategy::Fixed {
        return fixed_cut(req, None);
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let parsed = blocks::parse(&lines);
    if !parsed.iter().any(|b| matches!(b, blocks::Block::Heading { .. })) {
        let defaults = Requested { strategy: Strategy::Fixed, settings: Vec::new() };
        return fixed_cut(&defaults, Some(Strategy::Structural));
    }
    Cut {
        used: Strategy::Structural,
        fallback_from: None,
        settings: req.filled(d),
        chunks: structural::from_blocks(&parsed, title, &req.structural(d)),
    }
}
