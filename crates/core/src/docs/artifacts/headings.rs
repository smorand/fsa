//! The structure of a conversion text when the converter supplied none
//! (SPEC-0103 DEC-007): its Markdown headings, in reading order.

use crate::chunking::blocks;
use crate::docs::manifest::{Heading, MAX_TITLE_CHARS};

/// The headings of `markdown`. A line starting with three backticks opens or
/// closes a code block, as `parse` reads it, and is never a heading; every
/// other line follows the chunking heading rule (a closing run of `#` is
/// removed, `C#` keeps its mark). A title over 1 000 characters is dropped
/// without a line. Pages are unknown. Time O(text length), one pass.
pub(crate) fn derive(markdown: &str) -> Vec<Heading> {
    let mut in_fence = false;
    let mut out = Vec::new();
    for line in markdown.lines() {
        if line.starts_with("```") {
            in_fence = !in_fence;
        } else if !in_fence
            && let Some((level, title)) = blocks::heading(line)
            && title.chars().count() <= MAX_TITLE_CHARS
        {
            out.push(Heading { level, title, page: None });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_skips_fences_strips_closing_runs_and_drops_long_titles() {
        let long = "x".repeat(1001);
        let md =
            format!("# Sales\n\n```\n# not a heading\n```\n## Q1 ##\n# C#\n# {long}\n#nospace\n");
        let got: Vec<(u8, String)> = derive(&md).into_iter().map(|h| (h.level, h.title)).collect();
        assert_eq!(got, [(1, "Sales".to_string()), (2, "Q1".to_string()), (1, "C#".to_string())]);
    }
}
