//! The `cl100k_base` ruler every chunk size is measured with (SPEC-0101
//! FR-NEW-010, DEC-001). The ranks are embedded in `tiktoken-rs`, so nothing
//! is read from disk or the network; the crate keeps one parsed instance for
//! the process (DEC-016), built at first use.

use tiktoken_rs::{CoreBPE, Rank, cl100k_base_singleton};

fn ruler() -> &'static CoreBPE {
    cl100k_base_singleton()
}

/// The tokens of `text`. Special token spellings such as `<|endoftext|>` are
/// ordinary text in a user file, so they are encoded as plain bytes.
pub fn encode(text: &str) -> Vec<Rank> {
    ruler().encode_ordinary(text)
}

/// The number of tokens of `text`.
pub fn count(text: &str) -> usize {
    encode(text).len()
}

/// The text of a token slice. A slice may cut a multibyte character in two:
/// the broken bytes become U+FFFD (FR-NEW-011) instead of failing.
pub fn decode_lossy(tokens: &[Rank]) -> String {
    // Every rank came from `encode`, so a key error cannot happen; an empty
    // text is still the safe answer should it ever.
    let bytes = ruler().decode_bytes(tokens).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `text` cut by token count (v3a 5.9, the last resort of the hard split):
/// pieces of at most `limit` tokens, each cut at a character boundary, that
/// concatenated give back `text`.
pub fn token_cuts(text: &str, limit: usize) -> Vec<&str> {
    let limit = limit.max(1);
    let toks = encode(text);
    // The byte end of every token prefix: the tokens are a byte level split
    // of `text`, so these are offsets into it. One encode for the whole text
    // keeps the cut linear.
    let mut ends = Vec::with_capacity(toks.len() + 1);
    ends.push(0usize);
    for t in &toks {
        let len = ruler().decode_bytes(std::slice::from_ref(t)).map(|b| b.len()).unwrap_or(0);
        ends.push(ends.last().copied().unwrap_or(0) + len);
    }
    let mut out = Vec::new();
    let (mut start, mut next) = (0usize, 0usize);
    while start < text.len() {
        let mut take = limit.min(toks.len() - next).max(1);
        let piece = loop {
            let mut end = ends[next + take].min(text.len());
            // A token may end inside a character: cut before that character.
            while end > start && !text.is_char_boundary(end) {
                end -= 1;
            }
            if end <= start {
                // Not one whole character fits: take one, the only progress.
                end = start + text[start..].chars().next().map_or(1, char::len_utf8);
                break end;
            }
            // Read alone, a piece may encode into more tokens than its slice.
            if take == 1 || count(&text[start..end]) <= limit {
                break end;
            }
            take -= 1;
        };
        out.push(&text[start..piece]);
        start = piece;
        while next < toks.len() && ends[next + 1] <= start {
            next += 1;
        }
    }
    out
}

/// The cleaning of FR-NEW-010, applied before any cut: NUL characters out,
/// `\r\n` to `\n`.
pub fn clean(text: &str) -> String {
    text.replace('\0', "").replace("\r\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_and_decode_round_trip_ascii_and_count_agrees() {
        let t = "The quick brown fox jumps over the lazy dog.";
        let toks = encode(t);
        assert_eq!(count(t), toks.len());
        assert_eq!(decode_lossy(&toks), t);
        // A known cl100k_base value, so another ruler would be caught.
        assert_eq!(count("hello world"), 2);
    }

    #[test]
    fn a_broken_multibyte_character_decodes_to_the_replacement_character() {
        let toks = encode("日本語のテキスト");
        let first = decode_lossy(&toks[..1]);
        let whole: String = (0..toks.len()).map(|i| decode_lossy(&toks[i..=i])).collect();
        assert!(first.contains('\u{FFFD}') || whole.contains('\u{FFFD}'), "{whole}");
    }

    #[test]
    fn token_cuts_give_back_the_text_within_the_limit() {
        let digits: String = (0..300).map(|i| format!("{:03}", 100 + i % 900)).collect();
        let cuts = token_cuts(&digits, 128);
        assert_eq!(cuts.iter().map(|c| count(c)).collect::<Vec<_>>(), [128, 128, 44]);
        assert_eq!(cuts.concat(), digits);
        let wide = "日本語のテキスト".repeat(40);
        let cuts = token_cuts(&wide, 7);
        assert!(cuts.len() > 1 && cuts.iter().all(|c| count(c) <= 7 && !c.is_empty()));
        assert_eq!(cuts.concat(), wide);
        assert_eq!(token_cuts("short", 10), ["short"]);
        assert!(token_cuts("", 10).is_empty());
    }

    #[test]
    fn special_spellings_are_plain_text_and_cleaning_drops_nul_and_cr() {
        assert!(count("<|endoftext|>") > 1);
        assert_eq!(clean("a\0b\r\nc\rd"), "ab\nc\rd");
    }
}
