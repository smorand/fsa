//! The `fixed` strategy (SPEC-0101 FR-NEW-011): windows of `size` tokens,
//! each starting `size - overlap` tokens after the previous one, no sentence
//! rule. One encoding of the whole text, then one decoding per window, so the
//! cost is linear in the text times `size / (size - overlap)`.

use super::model::Chunk;
use super::tokens;

/// Cuts `text` (already cleaned) into fixed windows. `overlap` must be below
/// `size`, which the tool checks first; it is clamped here so a caller error
/// can never loop forever. The last chunk holds what remains, and no chunk is
/// made once a chunk reaches the end of the text.
pub fn cut(text: &str, title: &str, size: u32, overlap: u32) -> Vec<Chunk> {
    let toks = tokens::encode(text);
    let size = size.max(1) as usize;
    let step = size - (overlap as usize).min(size - 1);
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < toks.len() {
        let end = (start + size).min(toks.len());
        let number = out.len() as u32 + 1;
        out.push(Chunk {
            number,
            unit: number,
            text: tokens::decode_lossy(&toks[start..end]),
            heading_path: Vec::new(),
            page_start: None,
            page_end: None,
            // The count of the slice, per FR-NEW-011, never above `size`.
            token_count: (end - start) as u32,
            figures: Vec::new(),
            document_title: title.to_string(),
        });
        if end == toks.len() {
            break;
        }
        start += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::e2e::prose;

    #[test]
    fn windows_start_size_minus_overlap_apart_and_stop_at_the_end() {
        let t = prose(3000);
        assert_eq!(tokens::count(&t), 3000);
        let c = cut(&t, "essay", 1000, 200);
        assert_eq!(c.len(), 4);
        let toks = tokens::encode(&t);
        for (i, ch) in c.iter().enumerate() {
            let start = i * 800;
            let end = (start + 1000).min(3000);
            assert_eq!(ch.text, tokens::decode_lossy(&toks[start..end]));
            assert_eq!(ch.token_count as usize, end - start);
            assert_eq!((ch.number, ch.unit), (i as u32 + 1, i as u32 + 1));
            assert!(ch.heading_path.is_empty() && ch.figures.is_empty());
            assert_eq!((ch.page_start, ch.page_end), (None, None));
            assert_eq!(ch.document_title, "essay");
        }
    }

    #[test]
    fn without_overlap_the_last_chunk_holds_the_rest_and_cuts_repeat() {
        let t = prose(10000);
        let c = cut(&t, "e", 2048, 0);
        assert_eq!(c.len(), 5);
        assert_eq!(c[4].token_count, 1808);
        assert_eq!(c, cut(&t, "e", 2048, 0), "deterministic");
        assert_eq!(cut(&t, "e", 1000, 200).len(), 13);
    }

    #[test]
    fn empty_text_and_exact_fit() {
        assert!(cut("", "e", 32, 0).is_empty());
        let t = prose(64);
        assert_eq!(cut(&t, "e", 64, 10).len(), 1, "no chunk after one that reaches the end");
        assert_eq!(cut(&t, "e", 32, 0).len(), 2);
    }
}
