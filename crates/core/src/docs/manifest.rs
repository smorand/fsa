//! The converter's manifest, read tolerantly by version (SPEC-0103 DEC-002).
//!
//! The manifest comes from a program or an HTTP endpoint we do not control, so
//! it is read by hand from a `serde_json::Value`: one wrong field costs that
//! field and one report line, never the conversion. [`parse`] never fails.
//! A converter supplied value is never copied into a line: lines hold fixed
//! texts, the version number and field names of ours.

use serde_json::Value;
use std::collections::BTreeMap;

/// The newest manifest version this reader knows.
pub const SUPPORTED_VERSION: u32 = 2;

/// Stands for an `artifacts.json` that is not JSON (program form). A reserved
/// key rather than a variant keeps `Conversion.manifest` one raw `Value` for
/// both forms (DEC-003); an HTTP converter sending it only gets the same line.
pub const NOT_JSON_KEY: &str = "$fsa_manifest_not_json";

/// The raw value standing for a manifest text that is not JSON.
pub fn not_json() -> Value {
    serde_json::json!({ NOT_JSON_KEY: true })
}

/// A manifest that was read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Manifest {
    /// 1 for an absent or explicit 1, 2 for 2 and every newer version.
    pub version: u32,
    /// Keyed by the picture reference's target, as written in the text.
    pub pictures: BTreeMap<String, PictureMeta>,
    /// `Some` when a version 2 manifest supplied a list, even an empty one:
    /// the valid entries are then the structure and nothing is derived.
    pub headings: Option<Vec<Heading>>,
    /// The `tables` entries of a version 2 manifest, entry `i` for pipe table
    /// `i`; empty otherwise.
    pub tables: Vec<TableMeta>,
    /// The `heading <i>: invalid` and drop lines of `headings`, in report order.
    pub heading_notes: Vec<String>,
}

/// One entry of the document structure, supplied or derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub title: String,
    pub page: Option<i64>,
}

/// The most heading entries kept for one document (FR-NEW-015).
pub const MAX_HEADINGS: usize = 10_000;
/// The most characters of a heading title (FR-NEW-012).
pub const MAX_TITLE_CHARS: usize = 1_000;
/// The most `heading <i>: invalid` lines in one report (FR-NEW-014).
const MAX_HEADING_LINES: usize = 100;

/// What the converter says about one picture. A field of the wrong type is
/// treated as absent and its name is kept in `invalid`, in field order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PictureMeta {
    pub caption: String,
    pub caption_failed: bool,
    pub page: Option<i64>,
    /// The text read inside the picture, empty for none (version 2 only).
    pub ocr_text: String,
    pub ocr_failed: bool,
    /// An `ocr_text` over [`MAX_OCR_CHARS`]: no text is kept (FR-NEW-006).
    pub ocr_too_long: bool,
    /// The entry itself is not an object.
    pub not_object: bool,
    pub invalid: Vec<&'static str>,
}

/// What the converter says about one table (SPEC-0103 FR-NEW-007). A field of
/// the wrong type is absent and its name is kept in `invalid`, in field order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableMeta {
    /// The bundle file holding the exact CSV, as written; `None` for absent,
    /// null or empty.
    pub csv: Option<String>,
    /// Empty for none.
    pub caption: String,
    pub page: Option<i64>,
    /// The entry itself is not an object.
    pub not_object: bool,
    pub invalid: Vec<&'static str>,
}

/// The most characters of a table's exact CSV file (FR-NEW-009).
pub const MAX_CSV_CHARS: usize = 5_000_000;

/// The most characters of the OCR text of one picture (FR-NEW-006).
pub const MAX_OCR_CHARS: usize = 100_000;

/// The reading of one manifest: `manifest` is `None` when it was ignored as a
/// whole; `notes` are the manifest level report lines, in report order.
#[derive(Debug, Default, PartialEq)]
pub struct ManifestOutcome {
    pub manifest: Option<Manifest>,
    pub notes: Vec<String>,
}

fn ignored(reason: &str) -> ManifestOutcome {
    ManifestOutcome { manifest: None, notes: vec![format!("manifest ignored: {reason}")] }
}

/// Read `raw`. Time O(size of the manifest), one pass over the pictures.
pub fn parse(raw: &Value) -> ManifestOutcome {
    let Some(map) = raw.as_object() else { return ignored("not a JSON object") };
    if map.contains_key(NOT_JSON_KEY) {
        return ignored("not valid JSON");
    }
    // `version` present but null, text, boolean, fractional or below 1 is
    // invalid; a whole number is a `Number` that is u64 or i64, so 2.0 is not.
    let declared = match map.get("version") {
        None => 1,
        Some(Value::Number(n)) => match n.as_u64() {
            Some(v) if v >= 1 => v,
            _ => return ignored("version must be a whole number of 1 or more"),
        },
        Some(_) => return ignored("version must be a whole number of 1 or more"),
    };
    let version = declared.min(u64::from(SUPPORTED_VERSION)) as u32;
    // A null field is absent (FR-NEW-023); otherwise the type must fit.
    let present = |key: &str| map.get(key).filter(|v| !v.is_null());
    let pictures = match present("pictures") {
        None => None,
        Some(Value::Object(p)) => Some(p),
        Some(_) => return ignored("pictures must be an object"),
    };
    if version >= 2 {
        if present("tables").is_some_and(|v| !v.is_array()) {
            return ignored("tables must be a list");
        }
        if present("headings").is_some_and(|v| !v.is_array()) {
            return ignored("headings must be a list");
        }
    }
    let mut notes = Vec::new();
    if declared > u64::from(SUPPORTED_VERSION) {
        notes.push(format!(
            "manifest version {declared} is newer than supported {SUPPORTED_VERSION}"
        ));
    }
    let pictures = pictures
        .into_iter()
        .flatten()
        .map(|(target, entry)| (target.clone(), picture(entry, version >= 2)))
        .collect();
    let (headings, heading_notes) = match present("headings") {
        Some(Value::Array(list)) if version >= 2 => {
            let (kept, notes) = read_headings(list);
            (Some(kept), notes)
        }
        _ => (None, Vec::new()),
    };
    let tables = match present("tables") {
        Some(Value::Array(list)) if version >= 2 => list.iter().map(table).collect(),
        _ => Vec::new(),
    };
    ManifestOutcome {
        manifest: Some(Manifest { version, pictures, headings, heading_notes, tables }),
        notes,
    }
}

/// The valid entries of a `headings` list, at most [`MAX_HEADINGS`], and the
/// report lines for the rest. Time and space O(entries): one pass.
fn read_headings(list: &[Value]) -> (Vec<Heading>, Vec<String>) {
    let mut kept = Vec::new();
    let mut notes = Vec::new();
    let mut invalid = 0usize;
    for (i, entry) in list.iter().enumerate() {
        match heading(entry) {
            Some(h) => kept.push(h),
            None => {
                invalid += 1;
                if invalid <= MAX_HEADING_LINES {
                    notes.push(format!("heading {}: invalid", i + 1));
                }
            }
        }
    }
    if invalid > MAX_HEADING_LINES {
        notes.push(format!(
            "heading lines over {MAX_HEADING_LINES} dropped: {}",
            invalid - MAX_HEADING_LINES
        ));
    }
    if kept.len() > MAX_HEADINGS {
        notes.push(format!("headings over {MAX_HEADINGS} dropped: {}", kept.len() - MAX_HEADINGS));
        kept.truncate(MAX_HEADINGS);
    }
    (kept, notes)
}

/// One entry, or `None` when it breaks a rule: an object, `level` a whole
/// number from 1 to 6, `title` text of 1 to 1 000 characters once trimmed,
/// `page` absent, null or a whole number of 1 or more.
fn heading(entry: &Value) -> Option<Heading> {
    let fields = entry.as_object()?;
    let level = fields.get("level")?.as_u64().filter(|l| (1..=6).contains(l))? as u8;
    let title = fields.get("title")?.as_str()?.trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return None;
    }
    let page = match fields.get("page").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => Some(v.as_i64().filter(|p| *p >= 1)?),
    };
    Some(Heading { level, title: title.to_string(), page })
}

/// One `tables` entry; one that is not an object reads as empty. Time O(entry).
fn table(entry: &Value) -> TableMeta {
    let mut meta = TableMeta::default();
    let Some(fields) = entry.as_object() else {
        meta.not_object = true;
        return meta;
    };
    let field = |key: &str| fields.get(key).filter(|v| !v.is_null());
    match field("csv") {
        None => {}
        Some(Value::String(s)) => meta.csv = Some(s.clone()).filter(|s| !s.is_empty()),
        Some(_) => meta.invalid.push("csv"),
    }
    match field("caption") {
        None => {}
        Some(Value::String(s)) => meta.caption = s.clone(),
        Some(_) => meta.invalid.push("caption"),
    }
    match field("page") {
        None => {}
        Some(v) => match v.as_i64().filter(|p| *p >= 1) {
            Some(p) => meta.page = Some(p),
            None => meta.invalid.push("page"),
        },
    }
    meta
}

/// One `pictures` entry; an entry that is not an object reads as empty. The
/// OCR fields are read from version 2 on only. Length is counted in `chars()`
/// with a bounded scan: O(MAX_OCR_CHARS) at worst, not O(text).
fn picture(entry: &Value, v2: bool) -> PictureMeta {
    let mut meta = PictureMeta::default();
    let Some(fields) = entry.as_object() else {
        meta.not_object = true;
        return meta;
    };
    let field = |key: &str| fields.get(key).filter(|v| !v.is_null());
    match field("caption") {
        None => {}
        Some(Value::String(s)) => meta.caption = s.clone(),
        Some(_) => meta.invalid.push("caption"),
    }
    match field("caption_failed") {
        None => {}
        Some(Value::Bool(b)) => meta.caption_failed = *b,
        Some(_) => meta.invalid.push("caption_failed"),
    }
    match field("page") {
        None => {}
        Some(v) => match v.as_i64().filter(|p| *p >= 1) {
            Some(p) => meta.page = Some(p),
            None => meta.invalid.push("page"),
        },
    }
    if !v2 {
        return meta;
    }
    match field("ocr_text") {
        None => {}
        Some(Value::String(s)) if s.chars().nth(MAX_OCR_CHARS).is_some() => {
            meta.ocr_too_long = true;
        }
        Some(Value::String(s)) => meta.ocr_text = s.clone(),
        Some(_) => meta.invalid.push("ocr_text"),
    }
    match field("ocr_failed") {
        None => {}
        Some(Value::Bool(b)) => meta.ocr_failed = *b,
        Some(_) => meta.invalid.push("ocr_failed"),
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const BAD_VERSION: &str = "manifest ignored: version must be a whole number of 1 or more";

    #[test]
    fn absent_and_explicit_one_are_version_one() {
        for raw in [json!({}), json!({"version": 1})] {
            let out = parse(&raw);
            assert_eq!(out.manifest.unwrap().version, 1);
            assert!(out.notes.is_empty());
        }
    }

    #[test]
    fn a_newer_version_reads_as_two_with_one_note() {
        let out = parse(&json!({"version": 99}));
        assert_eq!(out.manifest.unwrap().version, 2);
        assert_eq!(out.notes, ["manifest version 99 is newer than supported 2"]);
        assert!(parse(&json!({"version": 2})).notes.is_empty());
    }

    #[test]
    fn an_invalid_version_ignores_the_manifest() {
        let bad = [
            json!("two"),
            json!(0),
            json!(2.5),
            json!("2"),
            json!(true),
            json!(null),
            json!(-1),
            json!(2.0),
            serde_json::from_str("1e0").unwrap(),
        ];
        for v in bad {
            let out = parse(&json!({"version": v, "pictures": {}}));
            assert_eq!(
                out,
                ManifestOutcome { manifest: None, notes: vec![BAD_VERSION.into()] },
                "{v}"
            );
        }
    }

    #[test]
    fn a_malformed_manifest_is_ignored_with_its_reason() {
        let cases = [
            (json!([1, 2]), "not a JSON object"),
            (not_json(), "not valid JSON"),
            (json!({"pictures": "none"}), "pictures must be an object"),
            (json!({"version": 2, "tables": 1}), "tables must be a list"),
            (json!({"version": 2, "headings": "none"}), "headings must be a list"),
        ];
        for (raw, reason) in cases {
            let out = parse(&raw);
            assert_eq!(out.manifest, None);
            assert_eq!(out.notes, [format!("manifest ignored: {reason}")]);
        }
    }

    #[test]
    fn tables_and_headings_of_the_wrong_type_are_ignored_silently_before_version_two() {
        let out = parse(&json!({"headings": "none", "tables": {}}));
        assert!(out.manifest.is_some() && out.notes.is_empty());
    }

    #[test]
    fn heading_entries_are_checked_and_trimmed() {
        let out = parse(&json!({"version": 2, "headings": [
            {"level": 1, "title": "  Sales  ", "page": 2},
            {"level": 7, "title": "x"}, {"level": 1.5, "title": "x"}, {"level": 1},
            {"level": 1, "title": "   "}, {"level": 1, "title": "x", "page": 0},
            {"level": 2, "title": "ok", "page": null}, "text"]}));
        let m = out.manifest.unwrap();
        let kept = m.headings.unwrap();
        assert_eq!(kept.len(), 2);
        assert_eq!((kept[0].level, kept[0].title.as_str(), kept[0].page), (1, "Sales", Some(2)));
        assert_eq!(kept[1].page, None);
        assert_eq!(m.heading_notes.len(), 6);
    }

    #[test]
    fn a_version_one_manifest_keeps_no_headings() {
        let m = parse(&json!({"headings": [{"level": 1, "title": "x"}]})).manifest.unwrap();
        assert_eq!(m.headings, None);
    }

    #[test]
    fn a_wrong_typed_picture_field_is_absent_and_named() {
        let out = parse(&json!({"pictures": {"a.png": {
            "caption": 3, "caption_failed": "yes", "page": 0, "ocr_text": "x"}}}));
        let meta = &out.manifest.unwrap().pictures["a.png"];
        assert_eq!(meta.invalid, ["caption", "caption_failed", "page"]);
        assert_eq!((meta.caption.as_str(), meta.caption_failed, meta.page), ("", false, None));
    }

    #[test]
    fn null_fields_are_absent_without_a_line() {
        let out = parse(&json!({"pictures": {"a.png": {
            "caption": null, "caption_failed": null, "page": null}}}));
        assert!(out.manifest.unwrap().pictures["a.png"].invalid.is_empty());
    }

    #[test]
    fn ocr_fields_are_read_from_version_two_only() {
        let pic = json!({"ocr_text": "Total 42", "ocr_failed": true});
        let v1 = parse(&json!({"pictures": {"a.png": pic}})).manifest.unwrap();
        assert_eq!(v1.pictures["a.png"], PictureMeta::default());
        let v2 = parse(&json!({"version": 2, "pictures": {"a.png": pic}})).manifest.unwrap();
        let meta = &v2.pictures["a.png"];
        assert_eq!((meta.ocr_text.as_str(), meta.ocr_failed), ("Total 42", true));
    }

    #[test]
    fn ocr_text_is_bounded_in_characters_and_typed() {
        let at = |t: &str| json!({"version": 2, "pictures": {"a.png": {"ocr_text": t}}});
        let edge = parse(&at(&"é".repeat(MAX_OCR_CHARS))).manifest.unwrap();
        assert_eq!(edge.pictures["a.png"].ocr_text.chars().count(), MAX_OCR_CHARS);
        let over = parse(&at(&"é".repeat(MAX_OCR_CHARS + 1))).manifest.unwrap();
        let meta = &over.pictures["a.png"];
        assert!(meta.ocr_too_long && meta.ocr_text.is_empty());
        let bad = parse(
            &json!({"version": 2, "pictures": {"a.png": {"ocr_text": 4, "ocr_failed": "y"}}}),
        );
        assert_eq!(bad.manifest.unwrap().pictures["a.png"].invalid, ["ocr_text", "ocr_failed"]);
    }

    #[test]
    fn a_picture_entry_that_is_not_an_object_is_flagged() {
        let m = parse(&json!({"pictures": {"a.png": "x"}})).manifest.unwrap();
        assert!(m.pictures["a.png"].not_object);
    }

    #[test]
    fn table_entries_are_read_from_version_two_with_typed_fields() {
        let raw = |v: u32| {
            json!({"version": v, "tables": [
            {"csv": "t.csv", "caption": "C", "page": 2}, 1, {"csv": 7, "caption": 5, "page": 0},
            {"csv": null, "caption": null, "page": null}, {"csv": ""}]})
        };
        let t = parse(&raw(2)).manifest.unwrap().tables;
        assert_eq!(t.len(), 5);
        assert_eq!(
            (t[0].csv.as_deref(), t[0].caption.as_str(), t[0].page),
            (Some("t.csv"), "C", Some(2))
        );
        assert!(t[1].not_object);
        assert_eq!(t[2].invalid, ["csv", "caption", "page"]);
        assert_eq!((t[3].clone(), t[4].clone()), (TableMeta::default(), TableMeta::default()));
        assert!(parse(&raw(1)).manifest.unwrap().tables.is_empty());
    }

    #[test]
    fn a_valid_picture_is_read() {
        let out = parse(&json!({"pictures": {"a.png": {
            "caption": "Sales", "caption_failed": true, "page": 2}}}));
        let meta = &out.manifest.unwrap().pictures["a.png"];
        assert_eq!(
            (meta.caption.as_str(), meta.caption_failed, meta.page),
            ("Sales", true, Some(2))
        );
    }
}
