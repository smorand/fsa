//! The block parser of paragraph v3a section 5.1 (SPEC-0101 FR-NEW-001,
//! FR-NEW-003 to FR-NEW-005, DEC-003). One pass over the lines, the first
//! matching rule wins. Every look ahead (a page marker after `---`, an image
//! link after `[Figure]`, the blank lines inside a list) only skips blank
//! lines that the main loop would skip anyway, so each line is examined a
//! bounded number of times and nothing backtracks across lines.

/// One structural element of the text. A text is the original lines joined
/// with `\n`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        title: String,
        text: String,
    },
    Paragraph(String),
    Table(String),
    List(String),
    Code(String),
    /// The link target, when the figure names one.
    Figure(Option<String>),
    PageMarker(u32),
    ThematicBreak,
    /// The first block of the text, a table whose header cells are all empty:
    /// it is left out of every chunk, so its text is not kept.
    MetadataTable,
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// Up to 3 spaces then 3 or more backticks or tildes: the fence character
/// and the length of its run.
pub fn fence_open(line: &str) -> Option<(char, usize)> {
    let rest = line.trim_start_matches(' ');
    if line.len() - rest.len() > 3 {
        return None;
    }
    let ch = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let run = rest.chars().take_while(|c| *c == ch).count();
    (run >= 3).then_some((ch, run))
}

pub fn fence_close(line: &str, ch: char, run: usize) -> bool {
    let t = line.trim();
    t.len() >= run && t.chars().all(|c| c == ch)
}

fn is_rule(line: &str) -> bool {
    line.trim_end() == "---"
}

fn page_number(line: &str) -> Option<u32> {
    line.trim().strip_prefix("*[Page ")?.strip_suffix("]*")?.trim().parse().ok()
}

/// 1 to 6 `#`, then a space or a tab, then a title that is not empty once
/// an optional closing run of `#` is taken off.
pub fn heading(line: &str) -> Option<(u8, String)> {
    let level = line.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let raw = rest.trim();
    let stripped = raw.trim_end_matches('#');
    // A closing run counts only when it stands apart from the title, so
    // `C#` keeps its mark.
    let title =
        if stripped.is_empty() || stripped.ends_with([' ', '\t']) { stripped.trim() } else { raw };
    (!title.is_empty()).then(|| (level as u8, title.to_string()))
}

fn is_separator_row(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.ends_with('|') && t.chars().all(|c| matches!(c, '|' | ':' | '-' | ' '))
}

fn starts_table(lines: &[&str], i: usize) -> bool {
    lines[i].trim().starts_with('|') && lines.get(i + 1).is_some_and(|l| is_separator_row(l))
}

fn header_cells_empty(line: &str) -> bool {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').all(|c| c.trim().is_empty())
}

/// The target of a line that holds only an image link `![alt](target)`.
fn image_target(line: &str) -> Option<String> {
    let t = line.trim();
    let inner = t.strip_prefix("![")?.strip_suffix(')')?;
    let (_, target) = inner.split_once("](")?;
    Some(target.trim().to_string())
}

fn is_figure_line(line: &str) -> bool {
    let t = line.trim();
    t == "[Figure]" || t == "<!-- image -->" || image_target(t).is_some()
}

fn indent(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// Optional indentation, a marker (`-`, `*`, `+`, or digits then `.` or
/// `)`), then whitespace.
pub fn is_list_item(line: &str) -> bool {
    let rest = line.trim_start_matches([' ', '\t']);
    let after = match rest.as_bytes().first() {
        Some(b'-' | b'*' | b'+') => &rest[1..],
        Some(b) if b.is_ascii_digit() => {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            match rest.as_bytes().get(digits) {
                Some(b'.' | b')') => &rest[digits + 1..],
                _ => return false,
            }
        }
        _ => return false,
    };
    after.starts_with([' ', '\t'])
}

/// Rules 1 to 6 of v3a 5.1 at line `i`: the lines that end a paragraph.
fn starts_block(lines: &[&str], i: usize) -> bool {
    let l = lines[i];
    fence_open(l).is_some()
        || is_rule(l)
        || heading(l).is_some()
        || starts_table(lines, i)
        || is_figure_line(l)
        || is_list_item(l)
}

/// The index of the first non blank line at or after `i`.
fn next_filled(lines: &[&str], mut i: usize) -> usize {
    while i < lines.len() && is_blank(lines[i]) {
        i += 1;
    }
    i
}

/// The blocks of `lines` in order (v3a 5.1).
pub fn parse(lines: &[&str]) -> Vec<Block> {
    let n = lines.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let line = lines[i];
        if is_blank(line) {
            i += 1;
            continue;
        }
        // Rule 1: code, to its closing fence or to the end of the text.
        if let Some((ch, run)) = fence_open(line) {
            let mut j = i + 1;
            while j < n && !fence_close(lines[j], ch, run) {
                j += 1;
            }
            let end = (j + 1).min(n);
            out.push(Block::Code(lines[i..end].join("\n")));
            i = end;
            continue;
        }
        // Rule 2: a page marker when `*[Page N]*` follows, else a break.
        if is_rule(line) {
            let j = next_filled(lines, i + 1);
            if let Some(page) = lines.get(j).and_then(|l| page_number(l)) {
                out.push(Block::PageMarker(page));
                i = j + 1;
            } else {
                out.push(Block::ThematicBreak);
                i += 1;
            }
            continue;
        }
        // Rule 3.
        if let Some((level, title)) = heading(line) {
            out.push(Block::Heading { level, title, text: line.to_string() });
            i += 1;
            continue;
        }
        // Rule 4.
        if starts_table(lines, i) {
            let mut j = i + 2;
            while j < n && lines[j].trim().starts_with('|') {
                j += 1;
            }
            out.push(if out.is_empty() && header_cells_empty(line) {
                Block::MetadataTable
            } else {
                Block::Table(lines[i..j].join("\n"))
            });
            i = j;
            continue;
        }
        // Rule 5: a `[Figure]` label takes the image link that follows it.
        if is_figure_line(line) {
            let t = line.trim();
            if t == "[Figure]" {
                let j = next_filled(lines, i + 1);
                if let Some(target) = lines.get(j).and_then(|l| image_target(l)) {
                    out.push(Block::Figure(Some(target)));
                    i = j + 1;
                    continue;
                }
                out.push(Block::Figure(None));
            } else {
                out.push(Block::Figure(image_target(t)));
            }
            i += 1;
            continue;
        }
        // Rule 6: item lines and lines of 2 leading spaces; a blank run stays
        // inside only when such a line follows it.
        if is_list_item(line) {
            let mut end = i + 1;
            loop {
                let j = next_filled(lines, end);
                match lines.get(j) {
                    Some(l) if is_list_item(l) || l.starts_with("  ") => end = j + 1,
                    _ => break,
                }
            }
            out.push(Block::List(lines[i..end].join("\n")));
            i = end;
            continue;
        }
        // Rule 7.
        let mut j = i + 1;
        while j < n && !is_blank(lines[j]) && !starts_block(lines, j) {
            j += 1;
        }
        out.push(Block::Paragraph(lines[i..j].join("\n")));
        i = j;
    }
    out
}

/// The top level items of a list (v3a 5.3): an item starts at a marker line
/// indented no more than the first line; other lines belong to the current
/// item; the blank lines ending an item are dropped.
pub fn list_items(list: &str) -> Vec<String> {
    let lines: Vec<&str> = list.split('\n').collect();
    let first = lines.first().map(|l| indent(l)).unwrap_or(0);
    let mut items: Vec<Vec<&str>> = Vec::new();
    for l in lines {
        if items.is_empty() || (is_list_item(l) && indent(l) <= first) {
            items.push(vec![l]);
        } else if let Some(cur) = items.last_mut() {
            cur.push(l);
        }
    }
    items
        .into_iter()
        .map(|mut it| {
            while it.len() > 1 && it.last().is_some_and(|l| is_blank(l)) {
                it.pop();
            }
            it.join("\n")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> Vec<Block> {
        parse(&text.split('\n').collect::<Vec<_>>())
    }

    #[test]
    fn headings_need_a_space_and_a_title_and_lose_their_closing_run() {
        assert_eq!(heading("## Title ##"), Some((2, "Title".into())));
        assert_eq!(heading("#\tTab"), Some((1, "Tab".into())));
        assert_eq!(heading("# C#"), Some((1, "C#".into())));
        for no in ["#Title", "####### seven", "# ", "# ##", " # indented", "#"] {
            assert_eq!(heading(no), None, "{no:?}");
        }
    }

    #[test]
    fn a_heading_inside_code_is_code_and_an_unclosed_fence_runs_to_the_end() {
        let b = p("```\n# not a heading\n```\n\n# Real");
        assert_eq!(b[0], Block::Code("```\n# not a heading\n```".into()));
        assert!(matches!(&b[1], Block::Heading { level: 1, title, .. } if title == "Real"));
        let b = p("text\n~~~~ rust\nfn x()\n~~~\nstill code");
        assert_eq!(b.len(), 2, "{b:?}");
        assert_eq!(b[1], Block::Code("~~~~ rust\nfn x()\n~~~\nstill code".into()));
        assert_eq!(p("    ```\nx").len(), 1, "4 spaces is no fence: one paragraph");
    }

    #[test]
    fn rules_page_markers_and_breaks() {
        assert_eq!(
            p("one\n---  \n\n*[Page 3]*\ntwo\n---\nthree"),
            vec![
                Block::Paragraph("one".into()),
                Block::PageMarker(3),
                Block::Paragraph("two".into()),
                Block::ThematicBreak,
                Block::Paragraph("three".into()),
            ]
        );
    }

    #[test]
    fn tables_and_the_metadata_table() {
        let b = p("| | |\n|---|---|\n| Author | Smith |\n\n| a | b |\n|:-|-:|\n| 1 | 2 |\ntail");
        assert_eq!(b[0], Block::MetadataTable);
        assert_eq!(b[1], Block::Table("| a | b |\n|:-|-:|\n| 1 | 2 |".into()));
        assert_eq!(b[2], Block::Paragraph("tail".into()));
        // Empty header cells later in the text make an ordinary table.
        let b = p("x\n\n| | |\n|---|---|\n| a | b |");
        assert!(matches!(b[1], Block::Table(_)));
        // A pipe line with no separator row is prose.
        assert_eq!(p("| a |\n| b |"), vec![Block::Paragraph("| a |\n| b |".into())]);
    }

    #[test]
    fn figures() {
        assert_eq!(
            p("[Figure]\n\n![logo](logo.png)\n<!-- image -->\n![c](c.png)\n[Figure]\ntext"),
            vec![
                Block::Figure(Some("logo.png".into())),
                Block::Figure(None),
                Block::Figure(Some("c.png".into())),
                Block::Figure(None),
                Block::Paragraph("text".into()),
            ]
        );
        assert_eq!(p("see ![c](c.png) here").len(), 1);
        assert!(matches!(&p("see ![c](c.png) here")[0], Block::Paragraph(_)));
    }

    #[test]
    fn lists_keep_continuations_and_inner_blank_lines() {
        let b = p("Intro:\n- a\n  more a\n\n- b\n\n\n3) c\nafter\n\nnext");
        assert_eq!(b[0], Block::Paragraph("Intro:".into()));
        assert_eq!(b[1], Block::List("- a\n  more a\n\n- b\n\n\n3) c".into()));
        assert_eq!(b[2], Block::Paragraph("after".into()));
        assert_eq!(b[3], Block::Paragraph("next".into()));
        assert!(!is_list_item("-x") && !is_list_item("1.x") && is_list_item("  12. x"));
    }

    #[test]
    fn list_items_split_at_the_first_indentation() {
        let items = list_items("- a\n  - sub\n\n- b\n\n1. c");
        assert_eq!(items, ["- a\n  - sub", "- b", "1. c"]);
    }
}
