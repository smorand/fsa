//! The `structural` strategy (SPEC-0101 FR-NEW-001 to FR-NEW-006, DEC-003):
//! paragraph v3a sections 5.2 to 5.9 over the blocks of
//! [`super::blocks`], with the merge parent rule of FR-NEW-006 (top level
//! titles equal, or both absent). A unit above `max_tokens` is cut by its
//! kind (5.5, 5.8, 5.9, FR-NEW-007).
//!
//! Cost: one pass over the blocks, one token count per unit, and one count
//! per merge attempt of a text bounded by the unit sizes. An oversize unit
//! counts each of its paragraphs, lines and words at most once each, plus
//! one recount per packed piece, so the whole cut stays linear in the text.

use super::blocks::{self, Block};
use super::model::Chunk;
use super::tokens;

/// The settings of a structural set, already in range (FR-NEW-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralSettings {
    pub heading_cut_depth: u32,
    pub max_tokens: u32,
    pub list_max_items: u32,
}

/// The kind of a unit (v3a 5.3), which decides how it is cut above the
/// limit (5.5). A merged group is prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Prose,
    Table,
    List,
    Code,
}

/// One group of v3a 5.8 starting at `start`: returns the index after its
/// last element. `elements` hold their token counts, counted once. The
/// estimate adds 1 per join when the join is a string (an empty join, used
/// when the elements keep their own separators, costs nothing).
pub fn pack_group(
    elements: &[(&str, usize)],
    start: usize,
    limit: usize,
    head: &str,
    tail: &str,
    join: &str,
) -> usize {
    let join_cost = usize::from(!join.is_empty());
    let mut size = tokens::count(head) + tokens::count(tail) + elements[start].1;
    let mut end = start + 1;
    while end < elements.len() && size + join_cost + elements[end].1 <= limit {
        size += join_cost + elements[end].1;
        end += 1;
    }
    while end - start > 1
        && tokens::count(&assemble(head, &elements[start..end], join, tail)) > limit
    {
        end -= 1;
    }
    end
}

fn assemble(head: &str, elements: &[(&str, usize)], join: &str, tail: &str) -> String {
    let body = elements.iter().map(|e| e.0).collect::<Vec<_>>().join(join);
    format!("{head}{body}{tail}")
}

/// The groups of v3a 5.8, as ranges of `elements`.
pub fn pack(
    elements: &[(&str, usize)],
    limit: usize,
    head: &str,
    tail: &str,
    join: &str,
) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < elements.len() {
        let end = pack_group(elements, start, limit, head, tail, join);
        out.push(start..end);
        start = end;
    }
    out
}

fn counted(text: &str) -> (&str, usize) {
    (text, tokens::count(text))
}

/// The paragraphs of v3a 5.5: a cut after each blank line (a newline,
/// optional spaces or tabs, a newline, then any whitespace), the separator
/// kept at the end of the paragraph before it. One pass over the bytes.
fn paragraphs(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < b.len() {
        if b[i] == b'\n' {
            let mut j = i + 1;
            while j < b.len() && matches!(b[j], b' ' | b'\t') {
                j += 1;
            }
            if j < b.len() && b[j] == b'\n' {
                let mut k = j + 1;
                while k < b.len() && b[k].is_ascii_whitespace() {
                    k += 1;
                }
                out.push(&text[start..k]);
                start = k;
                i = k;
                continue;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// The words of a line, each with the whitespace before it, so a word reads
/// like the ruler's own tokens (` cat`).
fn words(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev_space = true;
    for (i, c) in line.char_indices() {
        let space = c.is_whitespace();
        if space && !prev_space && i > start {
            out.push(&line[start..i]);
            start = i;
        }
        prev_space = space;
    }
    if start < line.len() {
        out.push(&line[start..]);
    }
    out
}

/// The segments of the hard split (v3a 5.9): lines with their line end, a
/// line above `limit` as words, a word above `limit` as token prefixes.
fn hard_segments<'a>(text: &'a str, limit: usize, out: &mut Vec<(&'a str, usize)>) {
    for line in text.split_inclusive('\n') {
        let n = tokens::count(line);
        if n <= limit {
            out.push((line, n));
            continue;
        }
        for word in words(line) {
            let n = tokens::count(word);
            if n <= limit {
                out.push((word, n));
            } else {
                out.extend(tokens::token_cuts(word, limit).into_iter().map(counted));
            }
        }
    }
}

/// The hard split of v3a 5.9: the packed segments, which concatenated give
/// back `text`.
pub fn hard_split(text: &str, limit: usize) -> Vec<&str> {
    let mut segs = Vec::new();
    hard_segments(text, limit, &mut segs);
    let mut out = Vec::new();
    for r in pack(&segs, limit, "", "", "") {
        let start = segs[r.start].0.as_ptr() as usize - text.as_ptr() as usize;
        let last = segs[r.end - 1].0;
        let end = last.as_ptr() as usize - text.as_ptr() as usize + last.len();
        out.push(&text[start..end]);
    }
    out
}

/// Paragraphs, the ones above `limit` replaced by their hard split segments:
/// every element fits `limit` and they concatenate to `text`.
fn segments(text: &str, limit: usize) -> Vec<(&str, usize)> {
    let mut out = Vec::new();
    for p in paragraphs(text) {
        let n = tokens::count(p);
        if n <= limit {
            out.push((p, n));
        } else {
            hard_segments(p, limit, &mut out);
        }
    }
    out
}

/// Prose above the limit (v3a 5.5): segments packed, each piece trimmed,
/// empty pieces dropped.
fn split_prose(text: &str, max: usize) -> Vec<String> {
    let segs = segments(text, max);
    pack(&segs, max, "", "", "")
        .into_iter()
        .map(|r| segs[r].iter().map(|s| s.0).collect::<String>().trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// A table above the limit: row groups, each led by the 2 header lines.
fn split_table(text: &str, max: usize) -> Vec<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() <= 2 {
        return vec![text.to_string()];
    }
    let head = format!("{}\n{}\n", lines[0], lines[1]);
    let rows: Vec<(&str, usize)> = lines[2..].iter().map(|l| counted(l)).collect();
    pack(&rows, max, &head, "", "\n")
        .into_iter()
        .map(|r| assemble(&head, &rows[r], "\n", ""))
        .collect()
}

/// A list above the limit: items packed, the introduction (a first line that
/// is no item) on the first piece only; an item above the limit is cut as
/// prose into pieces of its own.
fn split_list(text: &str, max: usize) -> Vec<String> {
    let (intro, body) = match text.split_once('\n') {
        Some((first, rest)) if !blocks::is_list_item(first) => (Some(first), rest),
        _ => (None, text),
    };
    let items = blocks::list_items(body);
    let mut head = intro.map(|l| format!("{l}\n")).unwrap_or_default();
    let mut out = Vec::new();
    let mut run: Vec<(&str, usize)> = Vec::new();
    let flush = |run: &mut Vec<(&str, usize)>, head: &mut String, out: &mut Vec<String>| {
        let mut start = 0;
        while start < run.len() {
            let end = pack_group(run, start, max, head, "", "\n");
            out.push(assemble(head, &run[start..end], "\n", ""));
            head.clear();
            start = end;
        }
        run.clear();
    };
    for item in &items {
        let n = tokens::count(item);
        if n <= max {
            run.push((item, n));
            continue;
        }
        flush(&mut run, &mut head, &mut out);
        if !head.is_empty() {
            // The introduction alone ahead of an item that never fits by it.
            out.push(head.trim_end().to_string());
            head.clear();
        }
        out.extend(split_prose(item, max));
    }
    flush(&mut run, &mut head, &mut out);
    out
}

/// A code block above the limit: every piece carries the opening fence line
/// and a closing fence, the original one or the opening run when the block
/// was never closed; the body is cut at blank lines, lines, then hard split.
fn split_code(text: &str, max: usize) -> Vec<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let Some((ch, run)) = blocks::fence_open(lines[0]) else {
        return split_prose(text, max);
    };
    let closed = lines.len() > 1 && blocks::fence_close(lines[lines.len() - 1], ch, run);
    let (close, body_end) = if closed {
        (lines[lines.len() - 1].to_string(), text.len() - lines[lines.len() - 1].len())
    } else {
        (ch.to_string().repeat(run), text.len())
    };
    let body_start = (lines[0].len() + 1).min(body_end);
    let body = text[body_start..body_end].trim_end_matches('\n');
    if body.trim().is_empty() {
        return vec![text.to_string()];
    }
    let head = format!("{}\n", lines[0]);
    let tail = format!("\n{close}");
    let room = max.saturating_sub(tokens::count(&head) + tokens::count(&tail)).max(1);
    let segs = segments(body, room);
    pack(&segs, max, &head, &tail, "")
        .into_iter()
        .map(|r| {
            let inner: String = segs[r].iter().map(|s| s.0).collect();
            format!("{head}{}{tail}", inner.trim_end_matches('\n'))
        })
        .collect()
}

/// The pieces of one unit (v3a 5.5): the unit itself when it fits
/// `max_tokens`, else its cut by kind, with no overlap.
pub fn split_unit(text: &str, kind: Kind, max_tokens: usize) -> Vec<String> {
    if tokens::count(text) <= max_tokens {
        return vec![text.to_string()];
    }
    match kind {
        Kind::Prose => split_prose(text, max_tokens),
        Kind::Table => split_table(text, max_tokens),
        Kind::List => split_list(text, max_tokens),
        Kind::Code => split_code(text, max_tokens),
    }
}

/// Cuts `text` (already cleaned) by its structure.
pub fn cut(text: &str, title: &str, settings: &StructuralSettings) -> Vec<Chunk> {
    let lines: Vec<&str> = text.split('\n').collect();
    from_blocks(&blocks::parse(&lines), title, settings)
}

/// A unit of v3a 5.3: a prose run or one atomic element of a section.
struct Unit {
    text: String,
    /// The token count of `text`, counted once.
    tokens: usize,
    path: Vec<String>,
    pages: Option<(u32, u32)>,
    /// The index of its first block, to place the figures (5.6).
    first: usize,
    /// Its number before merging, from 1 in the order of the text.
    number: u32,
    /// Produced by cutting a larger element: never merged (5.4).
    split: bool,
    kind: Kind,
}

fn widen(a: Option<(u32, u32)>, b: Option<(u32, u32)>) -> Option<(u32, u32)> {
    match (a, b) {
        (Some((s1, e1)), Some((s2, e2))) => Some((s1.min(s2), e1.max(e2))),
        (x, None) | (None, x) => x,
    }
}

fn page_span(page: Option<u32>) -> Option<(u32, u32)> {
    page.map(|p| (p, p))
}

/// One block of a section with the page it was seen on.
struct Placed<'a> {
    index: usize,
    block: &'a Block,
    page: Option<u32>,
}

/// A buffered prose block of 5.3: its text may lose its last line to a list.
struct Prose {
    index: usize,
    text: String,
    paragraph: bool,
    page: Option<u32>,
}

/// Appends a unit, counting its tokens once and numbering it in text order.
fn push_unit(
    units: &mut Vec<Unit>,
    path: &[String],
    text: String,
    first: usize,
    pages: Option<(u32, u32)>,
    split: bool,
    kind: Kind,
) {
    let tokens = tokens::count(&text);
    let number = units.len() as u32 + 1;
    units.push(Unit { text, tokens, path: path.to_vec(), pages, first, number, split, kind });
}

/// Emits the prose buffer as one unit, its blocks joined by a blank line.
fn flush(buffer: &mut Vec<Prose>, units: &mut Vec<Unit>, path: &[String]) {
    if buffer.is_empty() {
        return;
    }
    let text = buffer.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n\n");
    let pages = buffer.iter().fold(None, |acc, p| widen(acc, page_span(p.page)));
    push_unit(units, path, text, buffer[0].index, pages, false, Kind::Prose);
    buffer.clear();
}

/// Builds the units of one section (v3a 5.3), appending to `units`.
fn section_units(
    blocks: &[Placed<'_>],
    path: &[String],
    s: &StructuralSettings,
    units: &mut Vec<Unit>,
) {
    let mut buffer: Vec<Prose> = Vec::new();
    for b in blocks {
        let atomic = match b.block {
            Block::Heading { text, .. } => {
                buffer.push(Prose {
                    index: b.index,
                    text: text.clone(),
                    paragraph: false,
                    page: b.page,
                });
                continue;
            }
            Block::Paragraph(text) => {
                buffer.push(Prose {
                    index: b.index,
                    text: text.clone(),
                    paragraph: true,
                    page: b.page,
                });
                continue;
            }
            Block::Table(t) | Block::List(t) | Block::Code(t) => t,
            _ => continue,
        };
        let mut first = b.index;
        let mut pages = page_span(b.page);
        // The list introduction: the last line of a paragraph ending with
        // `:` moves to the list; an emptied paragraph disappears.
        let mut intro = None;
        if matches!(b.block, Block::List(_))
            && let Some(last) = buffer.last_mut()
            && last.paragraph
            && last.text.lines().last().is_some_and(|l| l.trim_end().ends_with(':'))
        {
            pages = widen(pages, page_span(last.page));
            match last.text.rsplit_once('\n') {
                Some((rest, line)) => {
                    intro = Some(line.to_string());
                    last.text = rest.to_string();
                }
                None => {
                    intro = Some(std::mem::take(&mut last.text));
                    first = last.index;
                    buffer.pop();
                }
            }
        }
        flush(&mut buffer, units, path);
        let kind = match b.block {
            Block::Table(_) => Kind::Table,
            Block::List(_) => Kind::List,
            _ => Kind::Code,
        };
        if let Block::List(list) = b.block {
            let items = blocks::list_items(list);
            let max = s.list_max_items as usize;
            if items.len() > max {
                // Batches of items, the introduction on the first only.
                for (i, batch) in items.chunks(max).enumerate() {
                    let body = batch.join("\n");
                    let text = match (&intro, i) {
                        (Some(l), 0) => format!("{l}\n{body}"),
                        _ => body,
                    };
                    push_unit(units, path, text, first, pages, true, Kind::List);
                }
                continue;
            }
        }
        let text = match intro {
            Some(l) => format!("{l}\n{atomic}"),
            None => atomic.clone(),
        };
        push_unit(units, path, text, first, pages, false, kind);
    }
    flush(&mut buffer, units, path);
}

/// The top level title of a unit, the merge parent of FR-NEW-006.
fn parent(path: &[String]) -> Option<&str> {
    path.first().map(String::as_str)
}

fn common_prefix(a: &[String], b: &[String]) -> Vec<String> {
    a.iter().zip(b).take_while(|(x, y)| x == y).map(|(x, _)| x.clone()).collect()
}

/// Cuts the blocks of a text, so the dispatcher parses only once.
pub fn from_blocks(blocks: &[Block], title: &str, s: &StructuralSettings) -> Vec<Chunk> {
    let depth = s.heading_cut_depth as usize;
    let max = s.max_tokens as usize;
    // 5.2: sections, cut at a heading that leaves at most `depth` open.
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut page: Option<u32> = None;
    let mut figures: Vec<(usize, String)> = Vec::new();
    let mut units: Vec<Unit> = Vec::new();
    let mut section: Vec<Placed<'_>> = Vec::new();
    let mut path: Vec<String> = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        match block {
            Block::PageMarker(n) => {
                page = Some(*n);
                continue;
            }
            Block::ThematicBreak | Block::MetadataTable => continue,
            Block::Figure(reference) => {
                if let Some(r) = reference {
                    figures.push((index, r.clone()));
                }
                continue;
            }
            Block::Heading { level, title, .. } => {
                while stack.last().is_some_and(|(l, _)| l >= level) {
                    stack.pop();
                }
                stack.push((*level, title.clone()));
                if stack.len() <= depth {
                    section_units(&section, &path, s, &mut units);
                    section.clear();
                }
            }
            _ => {}
        }
        if section.is_empty() {
            // FR-NEW-002: at most 2 titles, the top level first.
            path = stack.iter().take(2).map(|(_, t)| t.clone()).collect();
        }
        section.push(Placed { index, block, page });
    }
    section_units(&section, &path, s, &mut units);

    // 5.4: merging, with the parent rule of FR-NEW-006.
    struct Group {
        text: String,
        tokens: usize,
        path: Vec<String>,
        pages: Option<(u32, u32)>,
        number: u32,
        split: bool,
        kind: Kind,
    }
    let mut groups: Vec<Group> = Vec::new();
    // The group each unit ended in, to place the figures.
    let mut group_of: Vec<usize> = Vec::with_capacity(units.len());
    for u in units.iter() {
        if let Some(g) = groups.last_mut()
            && g.tokens < max
            && !g.split
            && !u.split
            && parent(&g.path) == parent(&u.path)
        {
            let joined = format!("{}\n\n{}", g.text, u.text);
            let n = tokens::count(&joined);
            if n <= max {
                g.text = joined;
                g.tokens = n;
                g.path = common_prefix(&g.path, &u.path);
                g.pages = widen(g.pages, u.pages);
                g.kind = Kind::Prose;
                group_of.push(groups.len() - 1);
                continue;
            }
        }
        groups.push(Group {
            text: u.text.clone(),
            tokens: u.tokens,
            path: u.path.clone(),
            pages: u.pages,
            number: u.number,
            split: u.split,
            kind: u.kind,
        });
        group_of.push(groups.len() - 1);
    }

    // 5.5 and 5.7: a group above the limit is cut by its kind; its pieces
    // share its unit number.
    let mut chunks: Vec<Chunk> = Vec::with_capacity(groups.len());
    // The last chunk of each group, to place the figures.
    let mut last_chunk: Vec<usize> = Vec::with_capacity(groups.len());
    for g in groups {
        let pieces = if g.tokens <= max { vec![g.text] } else { split_unit(&g.text, g.kind, max) };
        for text in pieces {
            let token_count = tokens::count(&text) as u32;
            chunks.push(Chunk {
                number: chunks.len() as u32 + 1,
                unit: g.number,
                text,
                heading_path: g.path.clone(),
                page_start: g.pages.map(|p| p.0),
                page_end: g.pages.map(|p| p.1),
                token_count,
                figures: Vec::new(),
                document_title: title.to_string(),
            });
        }
        last_chunk.push(chunks.len() - 1);
    }

    // 5.6: a figure goes to the last unit starting before it, else the first.
    if !units.is_empty() {
        for (index, reference) in figures {
            let before = units.partition_point(|u| u.first < index);
            let unit = before.saturating_sub(1);
            chunks[last_chunk[group_of[unit]]].figures.push(reference);
        }
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::e2e::prose;

    const S: StructuralSettings =
        StructuralSettings { heading_cut_depth: 2, max_tokens: 2048, list_max_items: 40 };

    fn run(text: &str) -> Vec<Chunk> {
        cut(text, "doc", &S)
    }

    fn paths(c: &[Chunk]) -> Vec<String> {
        c.iter().map(|c| c.heading_path.join(" > ")).collect()
    }

    /// v3a 10 case 1: text before any heading is one section, empty path.
    #[test]
    fn case_01_text_before_any_heading() {
        let c = run(&format!("{}\n\n# A\n\n{}", prose(100), prose(2000)));
        assert_eq!(paths(&c), ["", "A"]);
        assert_eq!(c[0].text, prose(100));
        assert_eq!((c[0].number, c[1].number), (1, 2));
        assert_eq!(c[0].document_title, "doc");
    }

    /// v3a 10 case 2: sections A and A > B; C stays inside A > B.
    #[test]
    fn case_02_deeper_headings_stay_inside() {
        let t =
            format!("# A\n\n{}\n\n## B\n\n{}\n\n### C\n\n{}", prose(1500), prose(500), prose(500));
        let c = run(&t);
        assert_eq!(paths(&c), ["A", "A > B"]);
        assert!(c[0].text.starts_with("# A\n\n"));
        assert_eq!(c[1].text, format!("## B\n\n{}\n\n### C\n\n{}", prose(500), prose(500)));
    }

    /// v3a 10 case 3: a document starting at level 3 cuts there.
    #[test]
    fn case_03_starting_at_level_3() {
        let t = format!("### X\n\n{}\n\n### Y\n\n{}", prose(1500), prose(1500));
        assert_eq!(paths(&run(&t)), ["X", "Y"]);
    }

    /// v3a 10 case 4: a page marker cuts nothing and records the page.
    #[test]
    fn case_04_page_marker() {
        let c = run("# T\n\none\n\n---\n*[Page 3]*\n\ntwo\n\n---\n\n*[Page 4]*\nthree");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "# T\n\none\n\ntwo\n\nthree");
        assert_eq!((c[0].page_start, c[0].page_end), (Some(3), Some(4)));
        let none = run("# T\n\none");
        assert_eq!((none[0].page_start, none[0].page_end), (None, None));
    }

    /// v3a 10 case 5: a lone `---` is ignored.
    #[test]
    fn case_05_thematic_break() {
        let c = run("# T\n\none\n---\ntwo");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "# T\n\none\n\ntwo");
    }

    /// v3a 10 case 6: the metadata table appears in no chunk.
    #[test]
    fn case_06_metadata_table() {
        let c = run("| | |\n|---|---|\n| Author | Smith |\n\n# A\n\ntext");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "# A\n\ntext");
    }

    /// v3a 10 case 7: `Causes:` is the first line of the list unit.
    #[test]
    fn case_07_list_introduction() {
        let t = format!("# T\n\n{}\nCauses:\n- one\n- two\n- three", prose(1500));
        let c = cut(&t, "doc", &StructuralSettings { max_tokens: 1000, ..S });
        assert_eq!(c.len(), 3, "the oversize prose unit in 2 pieces, the list is its own");
        assert_eq!(c[2].text, "Causes:\n- one\n- two\n- three");
        assert!(!c[0].text.contains("Causes:") && !c[1].text.contains("Causes:"));
        // A paragraph that is only the introduction disappears into the list.
        let c = run("# T\n\nCauses:\n\n- one");
        assert_eq!(c[0].text, "# T\n\nCauses:\n- one");
        assert_eq!(c[0].text.matches("Causes:").count(), 1);
    }

    /// v3a 10 case 8: 95 items make units of 40, 40 and 15, never merged.
    #[test]
    fn case_08_list_batches() {
        let items: Vec<String> = (1..=95).map(|i| format!("- item {i}")).collect();
        let c = run(&format!("# T\n\nintro\n\n{}\n\nafter", items.join("\n")));
        let counts: Vec<usize> = c.iter().map(|c| c.text.matches("- item").count()).collect();
        assert_eq!(counts, [0, 40, 40, 15, 0], "{:?}", paths(&c));
        assert_eq!(c[1].text, items[..40].join("\n"));
        assert_eq!(c[3].text, items[80..].join("\n"));
        let units: Vec<u32> = c.iter().map(|c| c.unit).collect();
        assert_eq!(units, [1, 2, 3, 4, 5]);
    }

    /// v3a 10 case 11: two short sections under the same `#` merge.
    #[test]
    fn case_11_same_top_level_merges() {
        let c = run(&format!(
            "# A\n\n{}\n\n## B\n\n{}\n\n## C\n\n{}",
            prose(100),
            prose(100),
            prose(100)
        ));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].heading_path, ["A"]);
        assert_eq!(c[0].text.matches("# ").count(), 3);
        assert_eq!(c[0].unit, 1);
        assert_eq!(c[0].token_count as usize, tokens::count(&c[0].text));
    }

    /// v3a 10 case 12: two short sections under different `#` do not merge.
    #[test]
    fn case_12_different_top_level() {
        let c = run(&format!("# A\n\n{}\n\n# B\n\n{}", prose(100), prose(100)));
        assert_eq!(paths(&c), ["A", "B"]);
        assert_eq!((c[0].number, c[1].number), (1, 2));
        assert_eq!((c[0].unit, c[1].unit), (1, 2));
    }

    /// v3a 10 case 16: CRLF and NUL give the chunks of the cleaned text.
    #[test]
    fn case_16_crlf_and_nul() {
        let clean = "# A\n\none\n\n## B\n\ntwo\n";
        let dirty = "# A\r\n\r\no\0ne\r\n\r\n## B\r\n\r\ntwo\0\r\n";
        assert_eq!(run(&tokens::clean(dirty)), run(clean));
        assert!(run(&tokens::clean(dirty)).iter().all(|c| !c.text.contains('\0')));
    }

    /// v3a 10 case 19: the same file chunked twice gives the same output.
    #[test]
    fn case_19_deterministic() {
        let t = format!(
            "# A\n\n{}\n\n```\ncode\n```\n\n| a |\n|---|\n| 1 |\n\n# B\n\n{}",
            prose(900),
            prose(1900)
        );
        let c = run(&t);
        assert!(c.len() >= 2);
        assert_eq!(c, run(&t));
    }

    /// FR-NEW-004: a figure goes to the unit before it, or the first one.
    #[test]
    fn figures_follow_the_unit_before_them() {
        let c = run("# A\n\none\n\n![chart](chart.png)\n\n# B\n\ntwo");
        assert_eq!(c[0].figures, ["chart.png"]);
        assert!(c[1].figures.is_empty());
        assert!(c.iter().all(|c| !c.text.contains("chart")));
        let c = run("[Figure]\n![logo](logo.png)\n\none\n\n# A\n\ntwo");
        assert_eq!(c[0].figures, ["logo.png"]);
        assert_eq!(c.iter().map(|c| c.figures.len()).sum::<usize>(), 1);
        let c = run("# T\n\nsee ![chart](chart.png) for details");
        assert!(c[0].figures.is_empty());
        assert!(c[0].text.ends_with("see ![chart](chart.png) for details"));
    }

    /// FR-NEW-001: `heading_cut_depth` 1 keeps level 2 headings inside.
    #[test]
    fn heading_cut_depth_one() {
        let t =
            format!("# A\n\n{}\n\n## B\n\n{}\n\n# C\n\n{}", prose(1500), prose(1500), prose(1500));
        let one = cut(&t, "d", &StructuralSettings { heading_cut_depth: 1, ..S });
        let mut sections = paths(&one);
        sections.dedup();
        assert_eq!(sections, ["A", "C"]);
        assert!(one[0].text.contains("## B"));
        assert_eq!(paths(&run(&t)), ["A", "A > B", "C"]);
    }

    /// FR-NEW-002: a heading path holds at most 2 titles at any depth.
    #[test]
    fn a_heading_path_holds_at_most_two_titles() {
        let t = format!("# A\n\n## B\n\n### C\n\n{}\n\n# D\n\n{}", prose(2046), prose(1500));
        let c = cut(&t, "d", &StructuralSettings { heading_cut_depth: 3, ..S });
        assert!(c.iter().all(|c| c.heading_path.len() <= 2), "{:?}", paths(&c));
        assert!(paths(&c).contains(&"A > B".to_string()));
    }

    /// FR-NEW-005: tables, lists and code are units of their own, merged only
    /// by the merge rule, and never cut.
    #[test]
    fn atomic_units_merge_only_by_the_rule() {
        let t = "# A\n\nintro\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```\nx\n\ny\n```";
        let c = run(t);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, t);
        let big = format!("# A\n\n{}\n\n```\nx\n```", prose(2043));
        let c = run(&big);
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].text, "```\nx\n```");
        assert!(c[0].token_count <= 2048 && c[0].token_count + c[1].token_count > 2046);
    }

    fn bounded(c: &[Chunk], max: usize) {
        for c in c {
            assert!(
                tokens::count(&c.text) <= max,
                "chunk {} has {}",
                c.number,
                tokens::count(&c.text)
            );
            assert_eq!(c.token_count as usize, tokens::count(&c.text));
        }
    }

    /// v3a 10 case 9: a table of 5000 tokens is cut into row groups of at
    /// most 2048 tokens, each starting with the header and separator rows.
    #[test]
    fn case_09_table_row_groups() {
        let rows: Vec<String> = (1..=200).map(|i| format!("| r{i} | {} |", prose(20))).collect();
        let t = format!("| a | b |\n|---|---|\n{}", rows.join("\n"));
        let c = run(&format!("# T\n\n{t}"));
        bounded(&c, 2048);
        let groups: Vec<&Chunk> = c.iter().filter(|c| c.text.contains("| r")).collect();
        assert!(groups.len() >= 3);
        let mut seen = Vec::new();
        for g in &groups {
            assert!(g.text.starts_with("| a | b |\n|---|---|\n"));
            seen.extend(g.text.lines().skip(2).map(str::to_string));
            assert_eq!(g.unit, groups[0].unit);
        }
        assert_eq!(seen, rows);
    }

    /// The one chunk allowed above the limit: a row with its header rows
    /// when they alone exceed it.
    #[test]
    fn a_row_with_its_header_may_exceed_the_limit() {
        let t = format!("| {} |\n|---|\n| {} |\n| x |", prose(40), prose(40));
        let p = split_unit(&t, Kind::Table, 64);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(tokens::count(&p[0]) > 64);
        assert!(p[1].ends_with("\n| x |") && tokens::count(&p[1]) <= 64);
    }

    /// v3a 10 case 10: an unclosed fence runs to the end; every piece carries
    /// the opening line and a closing fence.
    #[test]
    fn case_10_unclosed_code_fence() {
        let lines: Vec<String> = (1..=300).map(|i| format!("let v{i} = {};", prose(6))).collect();
        let c = run(&format!("# C\n\n~~~~ rust\n{}", lines.join("\n")));
        bounded(&c, 2048);
        let code: Vec<&Chunk> = c.iter().filter(|c| c.text.starts_with("~~~~")).collect();
        assert!(code.len() >= 2);
        let mut body = Vec::new();
        for p in &code {
            assert!(p.text.starts_with("~~~~ rust\n") && p.text.ends_with("\n~~~~"));
            let l: Vec<&str> = p.text.lines().collect();
            body.extend(l[1..l.len() - 1].iter().map(|s| s.to_string()));
        }
        assert_eq!(body, lines);
        // A closed block repeats its own closing fence.
        let closed = format!("```py\n{}\n`````", lines[..30].join("\n\n"));
        let p = split_unit(&closed, Kind::Code, 128);
        assert!(
            p.len() > 1 && p.iter().all(|p| p.starts_with("```py\n") && p.ends_with("\n`````"))
        );
        assert!(p.iter().all(|p| tokens::count(p) <= 128));
    }

    /// v3a 10 case 13: prose of 5000 tokens is cut at blank lines.
    #[test]
    fn case_13_prose_cut_at_blank_lines() {
        let paras: Vec<String> = (1..=25).map(|i| format!("p{i} {}", prose(200))).collect();
        let body = paras.join("\n\n");
        let text = format!("# P\n\n{body}");
        let c = run(&text);
        bounded(&c, 2048);
        assert!(c.len() >= 3);
        let joined = c.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join("\n\n");
        assert_eq!(joined, text, "no text repeated or lost");
        assert!(c.iter().all(|c| c.unit == 1));
        // A blank line holding spaces separates too, and the pieces are trimmed.
        let spaced = format!("{}\n  \t\n\n {}", prose(40), prose(40));
        assert_eq!(split_unit(&spaced, Kind::Prose, 64), [prose(40), prose(40)]);
    }

    /// v3a 10 case 14: a paragraph with no line break is cut between words.
    #[test]
    fn case_14_cut_between_words() {
        let text = format!("# W\n\n{}", prose(3000));
        let c = run(&text);
        assert_eq!(c.len(), 2);
        bounded(&c, 2048);
        assert_eq!(format!("{} {}", c[0].text, c[1].text), text);
    }

    /// v3a 10 case 15: a string with no space is cut by token count.
    #[test]
    fn case_15_cut_by_token_count() {
        let s: String = (0..3000).map(|i| format!("{:03}", 100 + i % 900)).collect();
        let p = split_unit(&s, Kind::Prose, 2048);
        assert_eq!(p.len(), 2);
        assert_eq!(tokens::count(&p[0]), 2048);
        assert_eq!(p.concat(), s);
    }

    /// v3a 10 case 20: every chunk at most 2048 tokens, at most 2 titles.
    #[test]
    fn case_20_every_chunk_bounded() {
        let items: Vec<String> = (1..=30).map(|i| format!("- item {i} {}", prose(100))).collect();
        let t = format!(
            "# A\n\n## B\n\n### C\n\n#### D\n\n{}\n\nList:\n{}\n\n```\n{}\n```\n\n# E\n\n{}",
            prose(5000),
            items.join("\n"),
            prose(3000),
            prose(9000)
        );
        let c = run(&t);
        bounded(&c, 2048);
        assert!(c.iter().all(|c| c.heading_path.len() <= 2));
        let small = cut(&t, "d", &StructuralSettings { max_tokens: 64, ..S });
        bounded(&small, 64);
    }

    /// v3a 5.5: a list above the limit packs its items, the introduction on
    /// the first piece only; an item above the limit is cut as prose.
    #[test]
    fn a_list_above_the_limit_packs_items() {
        let items: Vec<String> = (1..=6).map(|i| format!("- i{i} {}", prose(20))).collect();
        let t = format!("Intro:\n{}", items.join("\n"));
        let p = split_unit(&t, Kind::List, 64);
        assert!(p.len() >= 3, "{p:?}");
        assert!(p[0].starts_with("Intro:\n- i1"));
        assert_eq!(p.iter().filter(|p| p.contains("Intro:")).count(), 1);
        assert!(p.iter().all(|p| tokens::count(p) <= 64));
        assert_eq!(p.join("\n"), t);
        let big = format!("- {}\n- tail", prose(150));
        let p = split_unit(&big, Kind::List, 64);
        assert!(p.len() >= 3 && p.iter().all(|p| tokens::count(p) <= 64), "{p:?}");
        assert_eq!(p.last().unwrap(), "- tail");
    }

    /// v3a 5.8: the estimate counts each element, 1 per join, head and tail;
    /// the recount drops the last element while above the limit.
    #[test]
    fn packing_counts_joins_head_and_tail() {
        let e = [("aa", 4), ("bb", 4), ("cc", 4), ("dd", 4)];
        assert_eq!(pack(&e, 9, "", "", "\n"), [0..2, 2..4]);
        assert_eq!(pack(&e, 9, "", "", ""), [0..2, 2..4]);
        assert_eq!(pack(&e, 14, "", "", ""), [0..3, 3..4]);
        assert_eq!(pack(&e, 3, "", "", "\n"), [0..1, 1..2, 2..3, 3..4], "always one element");
        // A head costs its count on every group.
        let h = prose(4);
        assert_eq!(pack(&e, 9, &h, "", ""), [0..1, 1..2, 2..3, 3..4]);
        // An estimate below the real count: the recount drops the last element.
        let lie = [("one two three four", 1), ("five six seven eight", 1)];
        assert_eq!(pack(&lie, 5, "", "", " "), [0..1, 1..2]);
    }

    /// v3a 5.9: lines, then words, then token prefixes; the pieces give back
    /// the text.
    #[test]
    fn hard_split_gives_back_the_text() {
        let lines = format!("{}\n{}\n{}", prose(30), prose(30), prose(30));
        let p = hard_split(&lines, 64);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(p[0].ends_with('\n'), "a cut between lines");
        assert_eq!(p.concat(), lines);
        let words = prose(200);
        let p = hard_split(&words, 64);
        assert!(p.iter().all(|p| tokens::count(p) <= 64) && p.len() >= 4);
        assert_eq!(p.concat(), words);
        let s: String = (0..300).map(|i| format!("{:03}", 100 + i % 900)).collect();
        let p = hard_split(&s, 64);
        assert!(p.iter().all(|p| tokens::count(p) <= 64) && p.len() == 5);
        assert_eq!(p.concat(), s);
    }
}
