//! The weight of a stored artifact set (SPEC-0105 DEC-007): what a copy of the
//! set adds to the session write allowance. It is the formula of
//! `PendingSet::bytes()` applied to stored rows, so a copy is charged what the
//! conversion was charged and no column has to be added.

use super::{table_csv, table_markdown};

/// One stored image row, as far as its weight goes.
pub(crate) struct ImageWeight {
    pub(crate) size: i64,
    pub(crate) caption: String,
    pub(crate) ocr_text: Option<String>,
}

/// One stored table row, as far as its weight goes.
pub(crate) struct TableWeight {
    /// The converter's own CSV, when it gave one.
    pub(crate) csv_text: Option<String>,
    /// The JSON cells, the source of the rendered CSV and Markdown.
    pub(crate) cells: String,
    pub(crate) caption: String,
}

/// Picture, caption and OCR bytes per image, CSV (own or rendered), Markdown
/// and caption bytes per table, the heading titles and the conversion text.
/// Time linear in the bytes of the rows; a table whose cells do not parse
/// weighs its stored text only, since a set the reader can show never has one.
pub(crate) fn of_rows(
    images: &[ImageWeight],
    tables: &[TableWeight],
    title_bytes: i64,
    text_len: i64,
) -> i64 {
    let images: i64 = images
        .iter()
        .map(|i| {
            i.size + i.caption.len() as i64 + i.ocr_text.as_ref().map_or(0, |o| o.len() as i64)
        })
        .sum();
    let tables: i64 = tables.iter().map(table_weight).sum();
    images + tables + title_bytes + text_len
}

fn table_weight(t: &TableWeight) -> i64 {
    let cells: Vec<Vec<String>> = serde_json::from_str(&t.cells).unwrap_or_default();
    let csv = t.csv_text.as_ref().map_or_else(|| table_csv(&cells).len(), String::len);
    (csv + table_markdown(&cells).len() + t.caption.len()) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_formula_matches_a_pending_set() {
        let images = [ImageWeight { size: 3, caption: "Fig".into(), ocr_text: Some("ab".into()) }];
        let tables = [TableWeight {
            csv_text: None,
            cells: r#"[["a","b"],["1","2"]]"#.into(),
            caption: "T".into(),
        }];
        // CSV "a,b\n1,2" is 7, Markdown 33, caption 1.
        assert_eq!(of_rows(&images, &tables, 6, 10), 3 + 3 + 2 + 7 + 33 + 1 + 6 + 10);
    }

    #[test]
    fn an_own_csv_replaces_the_rendered_one() {
        let tables = [TableWeight {
            csv_text: Some("x".into()),
            cells: r#"[["a","b"],["1","2"]]"#.into(),
            caption: String::new(),
        }];
        assert_eq!(of_rows(&[], &tables, 0, 0), 1 + 33);
    }
}
