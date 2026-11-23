//! Building one conversion's artifact set, weighing it for the session write
//! quota, reporting it, and committing it against the node revision it was
//! built from (SPEC-0019 DEC-005, DEC-008, FR-NEW-030).
//!
//! The caller charges [`PendingSet::bytes`] plus the sibling bytes ONCE, before
//! writing anything, then writes the sibling, then calls [`PendingSet::commit`].

use crate::docs::artifacts::headings;
use crate::docs::artifacts::parse::{PictureRef, parse};
use crate::docs::artifacts::{CsvQuality, table_csv, table_markdown, width};
use crate::docs::extract::BuiltinTable;
use crate::docs::manifest::{self, Heading, MAX_CSV_CHARS, MAX_HEADINGS, Manifest, TableMeta};
use crate::docs::service::Conversion;
use crate::errors::{Result, ToolError};
use crate::storage::VolumeClient;
use crate::storage::meta::{
    NewArtifactSet, NewDocHeading, NewDocImage, NewDocTable, RelationalMetaStore,
};

/// Source extensions whose pictures carry a page: the PDF page, the
/// PowerPoint slide (FR-NEW-001). Every other format has none.
const PAGED_SOURCES: [&str; 3] = ["pdf", "pptx", "ppt"];

/// The picture formats an image may have (FR-NEW-004).
const PICTURE_FORMATS: [&str; 6] = ["png", "jpeg", "gif", "webp", "tiff", "bmp"];

/// One image about to be kept, its bytes borrowed from the conversion.
struct SetImage<'a> {
    seq: i64,
    caption: String,
    page: Option<i64>,
    format: &'static str,
    bytes: &'a [u8],
    line: usize,
    /// The text read inside the picture, `None` when empty (FR-NEW-004).
    ocr_text: Option<String>,
}

/// One table about to be kept.
struct SetTable<'a> {
    seq: i64,
    caption: String,
    cells: Vec<Vec<String>>,
    quality: CsvQuality,
    line_start: usize,
    line_count: usize,
    /// The converter's exact CSV, borrowed from its bundle (never copied until
    /// the commit, so a file named by many entries costs one copy per row
    /// stored, and each is charged).
    csv: Option<&'a str>,
    /// The page the converter gave the table: metadata only.
    page: Option<i64>,
}

/// The set one conversion yields, not stored yet.
pub(crate) struct PendingSet<'a> {
    /// The conversion text.
    text: &'a str,
    images: Vec<SetImage<'a>>,
    tables: Vec<SetTable<'a>>,
    /// The report lines after the header: manifest level lines, then
    /// `<id>: <reason>` per item, images by id then tables by id
    /// (FR-NEW-015, SPEC-0103 FR-NEW-017).
    failures: Vec<String>,
    /// The structure stored with the set: the converter's list or the
    /// Markdown's headings. Empty for the built in extraction.
    headings: Vec<Heading>,
    /// Bytes of the titles the converter supplied. Derived titles are not
    /// charged: they already lie inside the charged conversion text.
    title_bytes: usize,
    /// The `heading <i>: invalid` and drop lines, last in the report.
    heading_lines: Vec<String>,
    /// Whether a version 2 (or newer) manifest was read: it adds the
    /// `metadata:` line (SPEC-0103 DEC-005).
    v2: bool,
    /// The extraction's damage note, kept with the set (SPEC-0094 DEC-005).
    note: Option<String>,
}

impl<'a> PendingSet<'a> {
    /// The built-in set of one text extraction (FR-NEW-017): its tables, exact,
    /// keeping only those whose whole Markdown lies before the cut when the text
    /// was cut (FR-NEW-034), numbered in reading order with every row.
    pub(crate) fn builtin(
        text: &'a str,
        tables: &[BuiltinTable],
        truncated_at: Option<usize>,
    ) -> Self {
        let tables = tables
            .iter()
            .filter(|t| truncated_at.is_none_or(|cut| t.char_end <= cut))
            .zip(1i64..)
            .map(|(t, seq)| SetTable {
                seq,
                caption: t.caption.clone(),
                cells: t.cells.clone(),
                quality: CsvQuality::Exact,
                line_start: t.line_start,
                line_count: t.line_count,
                csv: None,
                page: None,
            })
            .collect();
        Self {
            text,
            images: Vec::new(),
            tables,
            failures: Vec::new(),
            headings: Vec::new(),
            title_bytes: 0,
            heading_lines: Vec::new(),
            v2: false,
            note: None,
        }
    }

    /// The same set carrying `note`, committed in the set's own transaction so
    /// a cached answer can repeat it (SPEC-0094 DEC-005).
    pub(crate) fn with_note(self, note: Option<String>) -> Self {
        Self { note, ..self }
    }

    /// The set of an external conversion of `source` (FR-NEW-033): its
    /// pictures, resolved only among the converter's own files (DEC-007), and
    /// its pipe tables, approximate, a malformed one reported under its id and
    /// not kept (FR-NEW-015). A picture that cannot be kept leaves its id
    /// unused and its failure reported (US-0006).
    pub(crate) fn converter(conversion: &'a Conversion, source: &str) -> Self {
        let text = conversion.markdown.as_str();
        let parsed = parse(text);
        let paged = extension(source).is_some_and(|e| PAGED_SOURCES.contains(&e.as_str()));
        // Time O(manifest + pictures): one parse, then one map lookup per picture.
        let mut outcome = conversion.manifest.as_ref().map(manifest::parse).unwrap_or_default();
        let mut failures = std::mem::take(&mut outcome.notes);
        let mut images = Vec::with_capacity(parsed.pictures.len());
        for r in &parsed.pictures {
            match picture(conversion, outcome.manifest.as_ref(), r, paged) {
                Ok((image, lines)) => {
                    failures.extend(lines);
                    images.push(image);
                }
                Err(reason) => failures.push(format!("image-{}: {reason}", r.seq)),
            }
        }
        let entries: &[TableMeta] = outcome.manifest.as_ref().map_or(&[], |m| &m.tables);
        if entries.len() > parsed.tables.len() {
            failures.push(format!(
                "manifest lists {} tables, the document has {}",
                entries.len(),
                parsed.tables.len()
            ));
        }
        // Entry `i` applies to pipe table `i`, malformed ones counted (DEC-012).
        let mut tables = Vec::with_capacity(parsed.tables.len());
        for t in parsed.tables {
            let Some(cells) = t.cells else {
                failures.push(format!("table-{}: malformed table", t.seq));
                continue;
            };
            let entry = entries.get(t.seq as usize - 1);
            let (csv, lines) = entry.map(|e| table_entry(conversion, e, t.seq)).unwrap_or_default();
            failures.extend(lines);
            let caption =
                entry.filter(|e| !e.caption.is_empty()).map_or(t.caption, |e| e.caption.clone());
            tables.push(SetTable {
                seq: t.seq,
                caption,
                cells,
                quality: if csv.is_some() { CsvQuality::Exact } else { CsvQuality::Approximate },
                line_start: t.line_start,
                line_count: t.line_count,
                csv,
                page: entry.and_then(|e| e.page),
            });
        }
        let v2 = outcome.manifest.as_ref().is_some_and(|m| m.version >= 2);
        let (headings, heading_lines) = structure(text, outcome.manifest.as_mut(), v2);
        // Derived titles are stored too, so they are charged like supplied ones (DEC-010).
        let title_bytes = headings.iter().map(|h| h.title.len()).sum();
        Self {
            text,
            images,
            tables,
            failures,
            headings,
            title_bytes,
            heading_lines,
            v2,
            note: None,
        }
    }

    /// What this set adds to the session write quota beside the sibling
    /// (FR-NEW-013): each image's picture and caption bytes, each table's
    /// CSV, Markdown and caption bytes, plus the conversion text. A picture
    /// placed twice is charged twice: each placement is its own image.
    pub(crate) fn bytes(&self) -> i64 {
        let images: usize = self
            .images
            .iter()
            .map(|i| i.bytes.len() + i.caption.len() + i.ocr_text.as_ref().map_or(0, String::len))
            .sum();
        let tables: usize = self
            .tables
            .iter()
            .map(|t| {
                let csv = t.csv.map_or_else(|| table_csv(&t.cells).len(), str::len);
                csv + table_markdown(&t.cells).len() + t.caption.len()
            })
            .sum();
        (images + tables + self.title_bytes + self.text.len()) as i64
    }

    /// The conversion report (FR-NEW-030, SPEC-0103 FR-NEW-017):
    /// `<N> images, <M> tables`, the `metadata:` line of a version 2 manifest,
    /// one line per failed item, then the heading lines.
    pub(crate) fn report(&self) -> String {
        let head = format!("{} images, {} tables", self.images.len(), self.tables.len());
        let metadata = self.v2.then(|| {
            let ocr = self.images.iter().filter(|i| i.ocr_text.is_some()).count();
            let csv = self.tables.iter().filter(|t| t.csv.is_some()).count();
            format!(
                "metadata: {ocr} images with OCR text, {csv} tables with exact CSV, {} headings",
                self.headings.len()
            )
        });
        std::iter::once(head)
            .chain(metadata)
            .chain(self.failures.iter().cloned())
            .chain(self.heading_lines.iter().cloned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Store the conversion text and picture blobs, then commit the set as the
    /// current one for `path`, refused when the node no longer carries `rev`
    /// (DEC-008). A refused commit releases the blobs it stored that nothing
    /// else in the volume references, then returns the refusal unchanged
    /// (SPEC-0033): left in place they would never be read nor collected.
    pub(crate) async fn commit(
        &self,
        client: &VolumeClient,
        store: &RelationalMetaStore,
        path: &str,
        rev: &str,
    ) -> Result<()> {
        let text_sha = if self.text.is_empty() {
            None
        } else {
            let sha = VolumeClient::sha256_hex(self.text.as_bytes());
            client.blob.put(&sha, self.text.as_bytes()).await?;
            Some(sha)
        };
        let mut images = Vec::with_capacity(self.images.len());
        for i in &self.images {
            let sha = VolumeClient::sha256_hex(i.bytes);
            client.blob.put(&sha, i.bytes).await?;
            images.push(NewDocImage {
                seq: i.seq,
                caption: i.caption.clone(),
                page: i.page,
                format: i.format.to_string(),
                sha256: sha,
                size: i.bytes.len() as i64,
                line_start: i.line as i64,
                line_count: 1,
                ocr_text: i.ocr_text.clone(),
            });
        }
        let tables = self
            .tables
            .iter()
            .map(|t| {
                Ok(NewDocTable {
                    seq: t.seq,
                    caption: t.caption.clone(),
                    rows: t.cells.len().saturating_sub(1) as i64,
                    cols: width(&t.cells) as i64,
                    quality: t.quality.as_str().to_string(),
                    cells: serde_json::to_string(&t.cells)
                        .map_err(|e| ToolError::internal(format!("table cells: {e}")))?,
                    line_start: t.line_start as i64,
                    line_count: t.line_count as i64,
                    csv_text: t.csv.map(str::to_string),
                    page: t.page,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let headings = self
            .headings
            .iter()
            .zip(1i64..)
            .map(|(h, seq)| NewDocHeading {
                seq,
                level: i64::from(h.level),
                title: h.title.clone(),
                page: h.page,
            })
            .collect();
        let set = NewArtifactSet {
            set_id: uuid::Uuid::new_v4().to_string(),
            path: path.to_string(),
            node_rev: rev.to_string(),
            text_sha,
            text_len: self.text.len() as i64,
            report: self.report(),
            note: self.note.clone(),
            images,
            tables,
            headings,
        };
        let committed = store.commit_artifact_set(&set).await;
        if committed.is_err() {
            let stored = set.text_sha.iter().chain(set.images.iter().map(|i| &i.sha256));
            release_unreferenced(client, store, stored).await;
        }
        committed
    }
}

/// The structure of a converted document and its report lines: the manifest's
/// list when it supplied one (even empty), else the Markdown's headings. At
/// most [`MAX_HEADINGS`] are kept; the derived overflow is reported only for a
/// version 2 manifest, so a document with no manifest keeps its report.
fn structure(text: &str, manifest: Option<&mut Manifest>, v2: bool) -> (Vec<Heading>, Vec<String>) {
    let supplied = manifest.and_then(|m| {
        let list = m.headings.take()?;
        Some((list, std::mem::take(&mut m.heading_notes)))
    });
    if let Some(found) = supplied {
        return found;
    }
    let mut derived = headings::derive(text);
    let mut lines = Vec::new();
    if derived.len() > MAX_HEADINGS {
        if v2 {
            lines.push(format!(
                "headings over {MAX_HEADINGS} dropped: {}",
                derived.len() - MAX_HEADINGS
            ));
        }
        derived.truncate(MAX_HEADINGS);
    }
    (derived, lines)
}

/// Delete each distinct blob of `stored` that no `blob_refs` row holds, after
/// a refused commit. Best effort (SPEC-0033 DEC-003): a failure is logged and
/// skipped, because the refusal is what the caller must see. A referenced hash
/// is kept, whatever holds it: the same bytes may be a file or another set.
async fn release_unreferenced<'s>(
    client: &VolumeClient,
    store: &RelationalMetaStore,
    stored: impl Iterator<Item = &'s String>,
) {
    let distinct: std::collections::HashSet<&String> = stored.collect();
    for sha in distinct {
        let released = match store.blob_is_referenced(sha).await {
            Ok(true) => Ok(()),
            Ok(false) => client.blob.delete(sha).await,
            Err(e) => Err(e),
        };
        if let Err(e) = released {
            tracing::warn!(sha256 = %sha, error = %e.message, "releasing a refused artifact blob failed");
        }
    }
}

/// The exact CSV a table entry names and the report lines of the entry, in
/// the FR-NEW-017 order: `table-<i>: CSV unavailable`, then the
/// `tables entry <i>` lines. A converter supplied value never reaches a line.
fn table_entry<'a>(
    conversion: &'a Conversion,
    entry: &TableMeta,
    seq: i64,
) -> (Option<&'a str>, Vec<String>) {
    let mut lines = Vec::new();
    let csv = entry.csv.as_deref().and_then(|name| {
        let found = csv_text(conversion, name);
        if found.is_none() {
            lines.push(format!("table-{seq}: CSV unavailable"));
        }
        found
    });
    if entry.not_object {
        lines.push(format!("tables entry {seq}: not an object"));
    }
    lines.extend(entry.invalid.iter().map(|f| format!("tables entry {seq}: invalid {f}")));
    (csv, lines)
}

/// The text of the bundle file `name` names, resolved like a picture target
/// and looked up only among the converter's own files (DEC-008): UTF-8, at
/// most [`MAX_CSV_CHARS`] characters. The length test is O(MAX_CSV_CHARS) at
/// worst and skipped when the byte length already settles it.
fn csv_text<'a>(conversion: &'a Conversion, name: &str) -> Option<&'a str> {
    let bytes = conversion.files.get(&resolve_target(name)?)?;
    let text = std::str::from_utf8(bytes).ok()?;
    (text.len() <= MAX_CSV_CHARS || text.chars().nth(MAX_CSV_CHARS).is_none()).then_some(text)
}

/// The image a picture reference yields, or the FR-NEW-015 reason it cannot
/// be kept: its target resolves to nothing inside the converter's output, no
/// file of the conversion carries that key, or its format is not a picture
/// format.
fn picture<'a>(
    conversion: &'a Conversion,
    manifest: Option<&Manifest>,
    r: &PictureRef,
    paged: bool,
) -> std::result::Result<(SetImage<'a>, Vec<String>), String> {
    let unavailable = || "picture unavailable".to_string();
    let key = resolve_target(&r.target).ok_or_else(unavailable)?;
    let bytes = conversion.files.get(&key).ok_or_else(unavailable)?;
    let ext = extension(&key).unwrap_or_default();
    let format = picture_format(&key).ok_or_else(|| format!("unsupported picture format {ext}"))?;
    // The manifest is keyed by the target as written; a converter keying it
    // by the resolved file path is read the same way.
    let meta = manifest.and_then(|m| m.pictures.get(&r.target).or_else(|| m.pictures.get(&key)));
    let caption_failed = meta.is_some_and(|m| m.caption_failed);
    // A failed description leaves the caption empty (FR-NEW-033).
    let caption = meta.filter(|m| !m.caption_failed).map(|m| m.caption.clone()).unwrap_or_default();
    let page = if paged { meta.and_then(|m| m.page) } else { None };
    // Lines in the picture group order of FR-NEW-017. Only a kept picture gets
    // any: the refusals above return before this point (FR-NEW-023).
    let mut lines = Vec::new();
    let mut line = |text: &str| lines.push(format!("image-{}: {text}", r.seq));
    if caption_failed {
        line("caption unavailable");
    }
    if let Some(m) = meta {
        if m.not_object {
            line("not an object");
        }
        m.invalid.iter().for_each(|f| line(&format!("invalid {f}")));
        if m.ocr_too_long {
            line("OCR text too long");
        }
        if m.ocr_failed {
            line("OCR unavailable");
        }
    }
    let ocr_text = meta.map(|m| m.ocr_text.clone()).filter(|t| !t.is_empty());
    let image = SetImage { seq: r.seq, caption, page, format, bytes, line: r.line, ocr_text };
    Ok((image, lines))
}

/// The `Conversion.files` key a target names (DEC-007): its relative path
/// with `.` and empty segments dropped. A URL, an absolute path or a `..`
/// leaving the converter's output names nothing, so nothing outside it is
/// ever read.
fn resolve_target(target: &str) -> Option<String> {
    if target.contains(':') || target.starts_with('/') || target.starts_with('\\') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The lowercase extension of a path's last segment.
fn extension(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    let (_, ext) = name.rsplit_once('.')?;
    Some(ext.to_ascii_lowercase())
}

/// The FR-NEW-004 format of a picture file: `jpeg` for `.jpg`/`.jpeg`, `tiff`
/// for `.tif`/`.tiff`, otherwise its lowercase extension when it is listed.
fn picture_format(path: &str) -> Option<&'static str> {
    let ext = extension(path)?;
    let ext = match ext.as_str() {
        "jpg" => "jpeg",
        "tif" => "tiff",
        other => other,
    };
    PICTURE_FORMATS.iter().copied().find(|f| *f == ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter().map(|r| r.iter().map(|c| (*c).to_string()).collect()).collect()
    }

    #[test]
    fn a_cut_keeps_only_tables_ending_before_it() {
        let table = |end| BuiltinTable {
            caption: String::new(),
            cells: cells(&[&["h"]]),
            line_start: 0,
            line_count: 2,
            char_end: end,
        };
        let tables = vec![table(10), table(20)];
        assert_eq!(PendingSet::builtin("", &tables, None).tables.len(), 2);
        assert_eq!(PendingSet::builtin("", &tables, Some(20)).tables.len(), 2);
        assert_eq!(PendingSet::builtin("", &tables, Some(19)).tables.len(), 1);
        assert_eq!(PendingSet::builtin("", &tables, Some(9)).tables.len(), 0);
    }

    #[test]
    fn bytes_count_csv_markdown_caption_and_text() {
        let tables = vec![BuiltinTable {
            caption: "Q1".into(),
            cells: cells(&[&["h"], &["v"]]),
            line_start: 0,
            line_count: 3,
            char_end: 5,
        }];
        // csv "h\nv" (3) + markdown "| h |\n| --- |\n| v |" (19) + caption (2) + text (4)
        assert_eq!(PendingSet::builtin("abcd", &tables, None).bytes(), 3 + 19 + 2 + 4);
    }

    #[test]
    fn a_converter_set_keeps_ids_and_reports_malformed_tables_in_order() {
        let text = "| a | b |\n| --- |\n\n| c |\n| --- |\n\n| d | e |\n| - |\n";
        let conversion = Conversion::text(text);
        let set = PendingSet::converter(&conversion, "/t.pdf");
        assert_eq!(set.tables.iter().map(|t| t.seq).collect::<Vec<_>>(), vec![2]);
        assert_eq!(set.tables[0].quality, CsvQuality::Approximate);
        assert_eq!(
            set.report(),
            "0 images, 1 tables\ntable-1: malformed table\ntable-3: malformed table"
        );
        // The set is the kept table plus the text: `c` + `| c |\n| --- |`.
        assert_eq!(set.bytes(), (1 + 13 + text.len()) as i64);
    }

    fn pictures(
        md: &str,
        files: &[(&str, &[u8])],
        source: &str,
    ) -> Vec<(i64, String, Option<i64>)> {
        use crate::docs::service::{Manifest, PictureMeta};
        let conversion = Conversion {
            markdown: md.to_string(),
            files: files.iter().map(|(k, v)| ((*k).to_string(), v.to_vec())).collect(),
            manifest: Some(
                Manifest {
                    pictures: files
                        .iter()
                        .map(|(k, _)| {
                            let meta = PictureMeta {
                                caption: format!("c {k}"),
                                caption_failed: false,
                                page: Some(4),
                            };
                            ((*k).to_string(), meta)
                        })
                        .collect(),
                }
                .into(),
            ),
        };
        let set = PendingSet::converter(&conversion, source);
        set.images.iter().map(|i| (i.seq, i.format.to_string(), i.page)).collect()
    }

    #[test]
    fn targets_resolve_only_inside_the_converter_output() {
        assert_eq!(resolve_target("figures/f1.png").as_deref(), Some("figures/f1.png"));
        assert_eq!(resolve_target("./figures//f1.png").as_deref(), Some("figures/f1.png"));
        assert_eq!(resolve_target("a/../f1.png").as_deref(), Some("f1.png"));
        for escape in [
            "../secret.png",
            "a/../../secret.png",
            "/etc/x.png",
            "https://h/x.png",
            "data:image/png;base64,AA",
            "C:\\x.png",
            "\\\\host\\x.png",
            "",
            ".",
        ] {
            assert_eq!(resolve_target(escape), None, "{escape}");
        }
    }

    #[test]
    fn a_csv_is_bounded_in_characters_not_bytes() {
        let at = |text: String| {
            let mut c = Conversion::text("x");
            c.files.insert("t.csv".into(), text.into_bytes());
            c.files.insert("bad.csv".into(), vec![0xFF, 0xFE, 0x00]);
            c
        };
        // 2 500 001 two byte characters: over 5 000 000 bytes, under the limit.
        let wide = at("é".repeat(2_500_001));
        assert_eq!(csv_text(&wide, "t.csv").map(|t| t.chars().count()), Some(2_500_001));
        assert_eq!(
            csv_text(&at("é".repeat(MAX_CSV_CHARS)), "t.csv").map(str::len),
            Some(10_000_000)
        );
        assert_eq!(csv_text(&at("é".repeat(MAX_CSV_CHARS + 1)), "t.csv"), None);
        assert_eq!(csv_text(&wide, "bad.csv"), None);
        assert_eq!(csv_text(&wide, "../t.csv"), None);
        assert_eq!(csv_text(&wide, "missing.csv"), None);
    }

    #[test]
    fn picture_formats_follow_the_extension() {
        let cases = [
            ("a.PNG", Some("png")),
            ("a.jpg", Some("jpeg")),
            ("a.JPEG", Some("jpeg")),
            ("a.tif", Some("tiff")),
            ("a.tiff", Some("tiff")),
            ("a.gif", Some("gif")),
            ("a.webp", Some("webp")),
            ("a.bmp", Some("bmp")),
            ("a.svg", None),
            ("noext", None),
            ("dir.png/noext", None),
        ];
        for (path, want) in cases {
            assert_eq!(picture_format(path), want, "{path}");
        }
    }

    #[test]
    fn a_picture_that_cannot_be_kept_leaves_its_id_unused_without_failing() {
        let md = "![](a.png)\n![](missing.png)\n![](b.svg)\n![](../a.png)\n![](c.jpg)\n";
        let got = pictures(md, &[("a.png", b"a"), ("b.svg", b"b"), ("c.jpg", b"c")], "/r.pdf");
        assert_eq!(got, vec![(1, "png".into(), Some(4)), (5, "jpeg".into(), Some(4))]);
    }

    #[test]
    fn only_pdf_and_powerpoint_pictures_carry_a_page() {
        let files: &[(&str, &[u8])] = &[("a.png", b"a")];
        for (source, page) in [
            ("/r.pdf", Some(4)),
            ("/d.PPTX", Some(4)),
            ("/m.docx", None),
            ("/p.html", None),
            ("/x", None),
        ] {
            assert_eq!(pictures("![](a.png)\n", files, source)[0].2, page, "{source}");
        }
    }

    #[test]
    fn a_failed_or_absent_description_is_an_empty_caption_and_bytes_count_pictures() {
        use crate::docs::service::{Manifest, PictureMeta};
        let failed = PictureMeta { caption: "ignored".into(), caption_failed: true, page: None };
        let conversion = Conversion {
            markdown: "![](a.png)\n![](b.png)\n".into(),
            files: [("a.png".to_string(), b"aaaa".to_vec()), ("b.png".to_string(), b"bb".to_vec())]
                .into(),
            manifest: Some(Manifest { pictures: [("a.png".to_string(), failed)].into() }.into()),
        };
        let set = PendingSet::converter(&conversion, "/r.pdf");
        assert!(set.images.iter().all(|i| i.caption.is_empty()));
        assert_eq!(set.report(), "2 images, 0 tables\nimage-1: caption unavailable");
        assert_eq!(set.bytes(), (4 + 2 + conversion.markdown.len()) as i64);
    }
}
