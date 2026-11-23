//! SPEC-0103 end to end tests of the converter manifest: the real conversion
//! path (`core::fs_ops::documentize`, then `fs.list_images` and
//! `fs.list_tables`) over stand in converters in both forms, a shell script
//! writing a bundle into `{outdir}` (program form) and an axum stub answering
//! a JSON bundle (HTTP form). Each case runs through every form that can carry
//! its manifest: a text that is not JSON exists only in the program form.

use crate::config::DocServiceApiConfig;
use crate::docs::service::{ApiDocService, CliDocService, DocService};
use crate::tools::admin::test_support::Fixture;
use crate::tools::archive_testkit::{MOUNT, OWNER, office_fixture_allowance, seed};
use base64::Engine as _;
use serde_json::{Value, json};
use std::path::Path;

pub(crate) const DOC: &str = "/report.pdf";

/// Room for the 5 000 000 character CSV cases (SPEC-0103 FR-NEW-009).
const CAP: u64 = 16 * 1_048_576;

/// The sources the stand in converters accept.
fn extensions() -> Vec<String> {
    ["pdf", "docx", "xlsx"].map(String::from).to_vec()
}

/// How a file of a stand in bundle is written: plain text, `b64:` then base64
/// bytes (not UTF-8 content), or `rep:<n>` for n bytes of `x`.
enum Body<'a> {
    Text(&'a str),
    Base64(&'a str),
    Repeat(usize),
}

impl<'a> Body<'a> {
    fn of(v: &'a str) -> Self {
        if let Some(b) = v.strip_prefix("b64:") {
            Body::Base64(b)
        } else if let Some(n) = v.strip_prefix("rep:") {
            Body::Repeat(n.parse().unwrap())
        } else {
            Body::Text(v)
        }
    }
}

/// What one conversion left behind, as Alice sees it.
struct Seen {
    report: String,
    /// `(caption, page)` per listed image, by number.
    images: Vec<(String, Option<i64>)>,
    tables: Vec<String>,
    /// `(level, title, page)` of the stored structure, in order.
    headings: Vec<(i64, String, Option<i64>)>,
    /// The OCR text stored with each image, by number.
    ocr: Vec<Option<String>>,
    /// The caption of each listed table, by number.
    captions: Vec<String>,
    /// The page stored with each table, by number.
    pages: Vec<Option<i64>>,
}

/// A converter answer: the Markdown, its files and the manifest as text.
pub(crate) struct Answer {
    pub(crate) markdown: &'static str,
    pub(crate) files: Vec<(&'static str, &'static str)>,
    pub(crate) manifest: Option<&'static str>,
}

const PIC: &str = "![](a.png)\n";

fn answer(manifest: &'static str) -> Answer {
    Answer { markdown: PIC, files: vec![("a.png", "PNG")], manifest: Some(manifest) }
}

/// A script writing `answer` into `{outdir}` ($1), manifest text verbatim.
pub(crate) fn program(home: &Path, a: &Answer) -> CliDocService {
    program_cap(home, a, CAP)
}

fn program_cap(home: &Path, a: &Answer, cap: u64) -> CliDocService {
    use std::os::unix::fs::PermissionsExt as _;
    let mut body = String::from("#!/bin/sh\n");
    body.push_str(&format!("cat > \"$1/document.md\" <<'FSA_EOF'\n{}\nFSA_EOF\n", a.markdown));
    for (name, bytes) in &a.files {
        body.push_str(&format!("mkdir -p \"$(dirname \"$1/{name}\")\"\n"));
        body.push_str(&match Body::of(bytes) {
            Body::Text(t) => format!("printf '%s' '{t}' > \"$1/{name}\"\n"),
            Body::Base64(b) => format!("printf '%s' '{b}' | base64 -d > \"$1/{name}\"\n"),
            Body::Repeat(n) => {
                format!("head -c {n} /dev/zero | tr '\\0' 'x' > \"$1/{name}\"\n")
            }
        });
    }
    if let Some(m) = a.manifest {
        body.push_str(&format!("cat > \"$1/artifacts.json\" <<'FSA_EOF'\n{m}\nFSA_EOF\n"));
    }
    let path = home.join("conv.sh");
    std::fs::write(&path, body).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    CliDocService::new(
        crate::config::DocServiceCliConfig {
            command: vec![
                path.display().to_string(),
                crate::config::OUTDIR_PLACEHOLDER.into(),
                crate::config::DOC_PLACEHOLDER.into(),
            ],
            timeout_secs: 60,
        },
        extensions(),
        cap,
    )
}

/// An HTTP stub answering `answer` as a JSON bundle; `None` when its manifest
/// text is not JSON, which a JSON bundle cannot carry.
pub(crate) async fn http(a: &Answer) -> Option<ApiDocService> {
    http_cap(a, CAP).await
}

async fn http_cap(a: &Answer, cap: u64) -> Option<ApiDocService> {
    let manifest = match a.manifest {
        Some(m) => Some(serde_json::from_str::<Value>(m).ok()?),
        None => None,
    };
    let files: serde_json::Map<String, Value> = a
        .files
        .iter()
        .map(|(k, v)| {
            let b64 = base64::engine::general_purpose::STANDARD;
            let bytes = match Body::of(v) {
                Body::Text(t) => t.as_bytes().to_vec(),
                Body::Base64(b) => b64.decode(b).unwrap(),
                Body::Repeat(n) => vec![b'x'; n],
            };
            ((*k).to_string(), json!(b64.encode(bytes)))
        })
        .collect();
    let mut bundle = json!({"markdown": a.markdown, "files": files});
    if let Some(m) = manifest {
        bundle["manifest"] = m;
    }
    Some(serve(bundle.to_string(), cap).await)
}

/// A stub answering `body` as JSON, under the converter limit `cap`.
async fn serve(body: String, cap: u64) -> ApiDocService {
    let app = axum::Router::new().route(
        "/convert",
        axum::routing::post(move || {
            let body = body.clone();
            async move { ([("content-type", "application/json")], body) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let cfg = DocServiceApiConfig { url: format!("http://{addr}/convert"), ..Default::default() };
    ApiDocService::new(&cfg, extensions(), cap).unwrap()
}

pub(crate) async fn fixture() -> Fixture {
    let f = office_fixture_allowance(1_000_000_000).await;
    seed(&f, DOC, b"%PDF-1.4").await;
    f
}

/// Convert `/report.pdf` through `svc` and read back what Alice can list.
async fn convert(f: &Fixture, svc: &dyn DocService) -> Seen {
    convert_at(f, svc, DOC).await
}

async fn convert_at(f: &Fixture, svc: &dyn DocService, doc: &str) -> Seen {
    f.state.authorize(MOUNT, OWNER).await.unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let done =
        crate::core::fs_ops::documentize(&c, &f.state.safety, Some(svc), OWNER, MOUNT, doc, true)
            .await
            .expect("the conversion succeeds");
    let report = done["artifacts_report"].as_str().expect("a report").to_string();
    let list = crate::tools::artifacts::list_images(&f.state, MOUNT, doc, None).await.unwrap();
    let images = list["images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["caption"].as_str().unwrap().to_string(), i["page"].as_i64()))
        .collect();
    let list = crate::tools::artifacts::list_tables(&f.state, MOUNT, doc, None).await.unwrap();
    let tables =
        list["tables"].as_array().unwrap().iter().map(|t| t["csv_quality"].to_string()).collect();
    let captions =
        list["tables"].as_array().unwrap().iter().map(|t| t["caption"].as_str().unwrap().into());
    let store = c.trash.as_deref().expect("a relational volume");
    let headings = store.current_headings_for_test(doc).await.unwrap();
    let ocr = store.current_ocr_for_test(doc).await.unwrap();
    let pages = store.current_table_pages_for_test(doc).await.unwrap();
    Seen { report, images, tables, headings, ocr, captions: captions.collect(), pages }
}

/// `answer` through every form that can carry it.
async fn each_form(f: &Fixture, a: &Answer) -> Vec<Seen> {
    let home = tempfile::tempdir().unwrap();
    let mut seen = vec![convert(f, &program(home.path(), a)).await];
    if let Some(svc) = http(a).await {
        seen.push(convert(f, &svc).await);
    }
    seen
}

const BAD_VERSION: &str = "manifest ignored: version must be a whole number of 1 or more";
const NO_PROFILE: &str = "1 images, 0 tables";

/// The metadata line of a version 2 manifest (the CSV count comes later).
fn meta(headings: usize) -> String {
    meta_ocr(0, headings)
}

fn meta_ocr(ocr: usize, headings: usize) -> String {
    format!("metadata: {ocr} images with OCR text, 0 tables with exact CSV, {headings} headings")
}

fn no_caption() -> Vec<(String, Option<i64>)> {
    vec![(String::new(), None)]
}

#[tokio::test]
async fn e2e_0103_003_version_one_without_a_version_reads_as_today() {
    let f = fixture().await;
    let a = answer(r#"{"pictures":{"a.png":{"caption":"Sales","page":2}}}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, NO_PROFILE);
        assert_eq!(s.images, vec![("Sales".to_string(), Some(2))]);
        assert!(!s.report.contains("metadata:"));
    }
}

#[tokio::test]
async fn e2e_0103_004_version_one_stated_explicitly() {
    let f = fixture().await;
    let a =
        answer(r#"{"version":1,"pictures":{"a.png":{"caption":"Sales","ocr_text":"Total 42"}}}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, NO_PROFILE);
        assert_eq!(s.images, vec![("Sales".to_string(), None)]);
    }
}

#[tokio::test]
async fn e2e_0103_005_no_manifest_at_all() {
    let f = fixture().await;
    let a = Answer {
        markdown: "# One\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\n# Two\n\n| c |\n| --- |\n| 3 |\n",
        files: vec![],
        manifest: None,
    };
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, "0 images, 2 tables");
        assert_eq!(s.tables, vec![r#""CSV quality: approximate""#; 2]);
    }
}

/// The newer version note, its place in the report, the OCR count and the
/// unchanged picture lines.
#[tokio::test]
async fn e2e_0103_010_a_newer_version_is_read_as_version_two() {
    let f = fixture().await;
    let a = answer(
        r#"{"version":3,"pictures":{"a.png":{"caption":"Sales","ocr_text":"Total 42","layout":"x"}}}"#,
    );
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!(
                "1 images, 0 tables\n{}\nmanifest version 3 is newer than supported 2",
                meta_ocr(1, 0)
            )
        );
        assert_eq!(s.images, vec![("Sales".to_string(), None)]);
    }
}

#[tokio::test]
async fn e2e_0103_011_a_version_that_is_not_a_whole_number() {
    let f = fixture().await;
    for v in [r#""two""#, "0", "2.5"] {
        let m: &'static str = Box::leak(
            format!(r#"{{"version":{v},"pictures":{{"a.png":{{"caption":"Sales","page":2}}}}}}"#)
                .into_boxed_str(),
        );
        for s in each_form(&f, &answer(m)).await {
            assert_eq!(s.report, format!("{NO_PROFILE}\n{BAD_VERSION}"), "{v}");
            assert_eq!(s.images, no_caption(), "{v}");
        }
    }
}

#[tokio::test]
async fn e2e_0103_012_a_manifest_that_is_not_json() {
    let f = fixture().await;
    let home = tempfile::tempdir().unwrap();
    let s = convert(&f, &program(home.path(), &answer("{not json"))).await;
    assert_eq!(s.report, format!("{NO_PROFILE}\nmanifest ignored: not valid JSON"));
    assert_eq!(s.images, no_caption());
}

#[tokio::test]
async fn e2e_0103_013_a_manifest_that_is_not_an_object() {
    let f = fixture().await;
    for s in each_form(&f, &answer("[1, 2]")).await {
        assert_eq!(s.report, format!("{NO_PROFILE}\nmanifest ignored: not a JSON object"));
        assert_eq!(s.images, no_caption());
    }
}

#[tokio::test]
async fn e2e_0103_014_a_pictures_field_of_the_wrong_type() {
    let f = fixture().await;
    let a = answer(r#"{"version":2,"pictures":"none","tables":[],"headings":[]}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, format!("{NO_PROFILE}\nmanifest ignored: pictures must be an object"));
        assert_eq!(s.images, no_caption());
    }
}

#[tokio::test]
async fn e2e_0103_015_a_far_newer_version() {
    let f = fixture().await;
    let a = answer(
        r#"{"version":99,"pictures":{"a.png":{"caption":"Sales","ocr_text":"x"}},"headings":[{"level":1,"title":"T"}]}"#,
    );
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!(
                "1 images, 0 tables\n{}\nmanifest version 99 is newer than supported 2",
                meta_ocr(1, 1)
            )
        );
        assert_eq!(s.images, vec![("Sales".to_string(), None)]);
    }
}

#[tokio::test]
async fn e2e_0103_016_a_newer_version_note_appears_once() {
    let f = fixture().await;
    let a = Answer {
        markdown: "![](a.png)\n![](gone.png)\n![](b.png)\n",
        files: vec![("a.png", "PNG"), ("b.png", "PNG")],
        manifest: Some(
            r#"{"version":3,"pictures":{"a.png":{"caption":"A"},"b.png":{"caption":"B"}}}"#,
        ),
    };
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!(
                "2 images, 0 tables\n{}\nmanifest version 3 is newer than supported 2\nimage-2: picture unavailable",
                meta(0)
            )
        );
        assert_eq!(s.report.matches("is newer than").count(), 1);
    }
}

#[tokio::test]
async fn e2e_0103_017_a_version_given_as_text_or_a_boolean() {
    let f = fixture().await;
    for v in [r#""2""#, "true", "null", "-1", "2.0", "1e0"] {
        let m: &'static str = Box::leak(
            format!(r#"{{"version":{v},"pictures":{{"a.png":{{"caption":"Sales","page":2}}}}}}"#)
                .into_boxed_str(),
        );
        for s in each_form(&f, &answer(m)).await {
            assert_eq!(s.report, format!("{NO_PROFILE}\n{BAD_VERSION}"), "{v}");
            assert_eq!(s.images, no_caption(), "{v}");
        }
    }
}

#[tokio::test]
async fn e2e_0103_018_version_one_with_version_two_fields() {
    let f = fixture().await;
    let a = answer(
        r#"{"version":1,"pictures":{"a.png":{"caption":"Sales","ocr_text":"Total 42"}},
            "tables":[{"csv":"a,b"}],"headings":[{"level":1,"title":"T"}]}"#,
    );
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, NO_PROFILE);
        assert_eq!(s.images, vec![("Sales".to_string(), None)]);
    }
}

#[tokio::test]
async fn e2e_0103_057_headings_of_the_wrong_type() {
    let f = fixture().await;
    let a = answer(r#"{"version":2,"headings":"none","pictures":{"a.png":{"caption":"Sales"}}}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, format!("{NO_PROFILE}\nmanifest ignored: headings must be a list"));
        assert_eq!(s.images, no_caption());
    }
}

#[tokio::test]
async fn e2e_0103_103_headings_of_the_wrong_type_in_a_version_one_manifest() {
    let f = fixture().await;
    let a = answer(r#"{"headings":"none","pictures":{"a.png":{"caption":"Sales"}}}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, NO_PROFILE);
        assert_eq!(s.images, vec![("Sales".to_string(), None)]);
    }
}

/// FR-NEW-023, version 1 fields: the wrong typed field is dropped with a line,
/// the rest of the entry and of the manifest is kept.
#[tokio::test]
async fn e2e_0103_023_wrong_typed_picture_fields_are_dropped_with_a_line() {
    let f = fixture().await;
    let a = answer(r#"{"pictures":{"a.png":{"caption":5,"caption_failed":"no","page":0}}}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            "1 images, 0 tables\nimage-1: invalid caption\nimage-1: invalid caption_failed\nimage-1: invalid page"
        );
        assert_eq!(s.images, no_caption());
    }
    let a = answer(r#"{"pictures":{"a.png":{"caption":"Sales","page":2.5}}}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, "1 images, 0 tables\nimage-1: invalid page");
        assert_eq!(s.images, vec![("Sales".to_string(), None)]);
    }
}

// SPEC-0103 US-0291: the structure and the metadata line.

pub(crate) fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// A bundle with no picture file.
pub(crate) fn doc(markdown: &'static str, manifest: Option<&'static str>) -> Answer {
    Answer { markdown, files: vec![], manifest }
}

pub(crate) fn entry(level: impl std::fmt::Display, title: &str) -> String {
    format!(r#"{{"level":{level},"title":"{title}"}}"#)
}

/// A manifest of `version` listing `entries` (raw JSON texts) as headings.
pub(crate) fn listing(version: u32, entries: &[String]) -> &'static str {
    leak(format!(r#"{{"version":{version},"headings":[{}]}}"#, entries.join(",")))
}

pub(crate) fn numbered(n: usize) -> Vec<String> {
    (1..=n).map(|i| entry(1, &format!("H{i}"))).collect()
}

const NONE: &str = "0 images, 0 tables";
/// Text with no heading, since a converter answer with no text is refused.
pub(crate) const BODY: &str = "Body\n";

fn titles(s: &Seen) -> Vec<&str> {
    s.headings.iter().map(|h| h.1.as_str()).collect()
}

#[tokio::test]
async fn e2e_0103_050_headings_supplied_by_the_converter() {
    let f = fixture().await;
    let m = r#"{"version":2,"headings":[{"level":1,"title":"Sales"},
        {"level":2,"title":"Q1","page":2},{"level":2,"title":"Q2","page":3}]}"#;
    for s in each_form(&f, &doc("# Other\n\n## Other 2\n", Some(m))).await {
        assert_eq!(s.report, format!("{NONE}\n{}", meta(3)));
        assert_eq!(
            s.headings,
            vec![
                (1, "Sales".to_string(), None),
                (2, "Q1".to_string(), Some(2)),
                (2, "Q2".to_string(), Some(3))
            ]
        );
    }
}

#[tokio::test]
async fn e2e_0103_051_headings_derived_from_the_markdown() {
    let f = fixture().await;
    let md = "# Sales\n\n## Q1\n\n```\n# not a heading\n```\n\n## Q2\n";
    for s in each_form(&f, &doc(md, Some(r#"{"version":2}"#))).await {
        assert_eq!(s.report, format!("{NONE}\n{}", meta(3)));
        assert_eq!(titles(&s), ["Sales", "Q1", "Q2"]);
    }
}

#[tokio::test]
async fn e2e_0103_052_derived_headings_without_any_manifest() {
    let f = fixture().await;
    for s in each_form(&f, &doc("# One\n\n## Two\n", None)).await {
        assert_eq!(s.report, NONE);
        assert_eq!(titles(&s), ["One", "Two"]);
    }
}

#[tokio::test]
async fn e2e_0103_053_an_invalid_heading_entry() {
    let f = fixture().await;
    let m = listing(2, &[entry(0, "X"), entry(7, "Y"), entry(2, ""), entry(2, "Z")]);
    for s in each_form(&f, &doc(BODY, Some(m))).await {
        assert_eq!(
            s.report,
            format!(
                "{NONE}\n{}\nheading 1: invalid\nheading 2: invalid\nheading 3: invalid",
                meta(1)
            )
        );
        assert_eq!(s.headings, vec![(2, "Z".to_string(), None)]);
    }
}

#[tokio::test]
async fn e2e_0103_054_limits_of_the_heading_level() {
    let f = fixture().await;
    let long = "t".repeat(500);
    let m = listing(2, &[entry(1, "A"), entry(6, "B"), entry(6, &long)]);
    for s in each_form(&f, &doc(BODY, Some(m))).await {
        assert_eq!(s.report, format!("{NONE}\n{}", meta(3)));
        assert_eq!(s.headings.iter().map(|h| h.0).collect::<Vec<_>>(), [1, 6, 6]);
        assert_eq!(s.headings[2].1.chars().count(), 500);
    }
}

#[tokio::test]
async fn e2e_0103_055_too_many_headings() {
    let f = fixture().await;
    let over = listing(2, &numbered(10_001));
    for s in each_form(&f, &doc(BODY, Some(over))).await {
        assert_eq!(s.report, format!("{NONE}\n{}\nheadings over 10000 dropped: 1", meta(10_000)));
        assert_eq!(s.headings.len(), 10_000);
        assert_eq!(s.headings[9_999].1, "H10000");
    }
}

#[tokio::test]
async fn e2e_0103_056_a_heading_title_with_markup_and_accents() {
    let f = fixture().await;
    let m = listing(2, &[entry(1, "Résumé **final** <b>")]);
    for s in each_form(&f, &doc(BODY, Some(m))).await {
        assert_eq!(s.report, format!("{NONE}\n{}", meta(1)));
        assert_eq!(titles(&s), ["Résumé **final** <b>"]);
    }
}

#[tokio::test]
async fn e2e_0103_058_exactly_ten_thousand_headings() {
    let f = fixture().await;
    for s in each_form(&f, &doc(BODY, Some(listing(2, &numbered(10_000))))).await {
        assert_eq!(s.report, format!("{NONE}\n{}", meta(10_000)));
        assert_eq!(s.headings.len(), 10_000);
    }
}

#[tokio::test]
async fn e2e_0103_059_twenty_thousand_headings() {
    let f = fixture().await;
    for s in each_form(&f, &doc(BODY, Some(listing(2, &numbered(20_000))))).await {
        assert_eq!(
            s.report,
            format!("{NONE}\n{}\nheadings over 10000 dropped: 10000", meta(10_000))
        );
        assert_eq!(s.headings.len(), 10_000);
    }
}

#[tokio::test]
async fn e2e_0103_060_a_converter_that_sends_only_captions() {
    let f = fixture().await;
    let a = Answer {
        markdown: "# One\n\n![](a.png)\n\n## Two\n",
        files: vec![("a.png", "PNG")],
        manifest: Some(r#"{"version":2,"pictures":{"a.png":{"caption":"Sales","page":2}}}"#),
    };
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, format!("1 images, 0 tables\n{}", meta(2)));
        assert_eq!(s.images, vec![("Sales".to_string(), Some(2))]);
        assert_eq!(titles(&s), ["One", "Two"]);
    }
}

#[tokio::test]
async fn e2e_0103_061_a_version_two_manifest_with_nothing_in_it() {
    let f = fixture().await;
    for s in each_form(&f, &doc("# One\n", Some(r#"{"version": 2}"#))).await {
        assert_eq!(s.report, format!("{NONE}\n{}", meta(1)));
    }
}

#[tokio::test]
async fn e2e_0103_063_a_version_two_conversion_without_pictures() {
    let f = fixture().await;
    let md = "| a | b |\n| --- | --- |\n| 1 | 2 |\n";
    for s in each_form(&f, &doc(md, Some(r#"{"version":2}"#))).await {
        assert_eq!(s.report, format!("0 images, 1 tables\n{}", meta(0)));
    }
}

#[tokio::test]
async fn e2e_0103_089_a_heading_entry_that_is_not_an_object() {
    let f = fixture().await;
    let m = listing(2, &[r#""Intro""#.to_string(), entry(1, "Real")]);
    for s in each_form(&f, &doc(BODY, Some(m))).await {
        assert_eq!(s.report, format!("{NONE}\n{}\nheading 1: invalid", meta(1)));
        assert_eq!(titles(&s), ["Real"]);
    }
}

#[tokio::test]
async fn e2e_0103_090_a_heading_level_that_is_not_a_whole_number() {
    let f = fixture().await;
    let m = listing(2, &[entry("1.5", "A"), entry(r#""1""#, "B"), entry(3, "C")]);
    for s in each_form(&f, &doc(BODY, Some(m))).await {
        assert_eq!(
            s.report,
            format!("{NONE}\n{}\nheading 1: invalid\nheading 2: invalid", meta(1))
        );
        assert_eq!(s.headings, vec![(3, "C".to_string(), None)]);
    }
}

#[tokio::test]
async fn e2e_0103_091_a_heading_title_of_1001_characters() {
    let f = fixture().await;
    let (long, edge) = ("x".repeat(1001), "y".repeat(1000));
    let m = listing(2, &[entry(1, &long), entry(1, &edge)]);
    for s in each_form(&f, &doc(BODY, Some(m))).await {
        assert_eq!(s.report, format!("{NONE}\n{}\nheading 1: invalid", meta(1)));
        assert_eq!(titles(&s), [edge.as_str()]);
    }
}

#[tokio::test]
async fn e2e_0103_099_an_empty_headings_list() {
    let f = fixture().await;
    let md = "# A\n\n## B\n\n## C\n";
    for s in each_form(&f, &doc(md, Some(r#"{"version":2,"headings":[]}"#))).await {
        assert_eq!(s.report, format!("{NONE}\n{}", meta(0)));
        assert!(s.headings.is_empty());
    }
}

#[tokio::test]
async fn e2e_0103_100_a_hundred_and_fifty_invalid_heading_entries() {
    let f = fixture().await;
    let m = listing(2, &vec![entry(0, "X"); 150]);
    let mut expected = format!("{NONE}\n{}", meta(0));
    for i in 1..=100 {
        expected.push_str(&format!("\nheading {i}: invalid"));
    }
    expected.push_str("\nheading lines over 100 dropped: 50");
    for s in each_form(&f, &doc(BODY, Some(m))).await {
        assert_eq!(s.report, expected);
    }
}

#[tokio::test]
async fn e2e_0103_101_a_derived_heading_with_a_closing_run() {
    let f = fixture().await;
    let md = leak(format!("## Q1 ##\n\n# C#\n\n# {}\n", "x".repeat(1001)));
    for s in each_form(&f, &doc(md, Some(r#"{"version":2}"#))).await {
        assert_eq!(s.report, format!("{NONE}\n{}", meta(2)));
        assert_eq!(s.headings, vec![(2, "Q1".to_string(), None), (1, "C#".to_string(), None)]);
    }
}

#[tokio::test]
async fn e2e_0103_106_more_than_ten_thousand_derived_headings() {
    let f = fixture().await;
    let md = leak((1..=10_002).map(|i| format!("# H{i}\n")).collect());
    for s in each_form(&f, &doc(md, Some(r#"{"version":2}"#))).await {
        assert_eq!(s.report, format!("{NONE}\n{}\nheadings over 10000 dropped: 2", meta(10_000)));
        assert_eq!(s.headings.len(), 10_000);
    }
    for s in each_form(&f, &doc(md, None)).await {
        assert_eq!(s.report, NONE);
        assert_eq!(s.headings.len(), 10_000);
    }
}

/// SPEC-0019 E2E-001 with and without a `version`.
#[tokio::test]
async fn e2e_0103_mod001_converting_with_a_manifest() {
    let f = fixture().await;
    let md =
        "# Report\n\n![](a.png)\n\n| a |\n| --- |\n| 1 |\n\n![](b.png)\n\n| b |\n| --- |\n| 2 |\n";
    let pics = r#""pictures":{"a.png":{"caption":"Figure 1: Revenue 2024","page":1},"b.png":{"caption":"Schéma réseau 🌐","page":3}}"#;
    let expected = vec![
        ("Figure 1: Revenue 2024".to_string(), Some(1)),
        ("Schéma réseau 🌐".to_string(), Some(3)),
    ];
    for (manifest, tail) in [
        (format!("{{{pics}}}"), String::new()),
        (format!(r#"{{"version":2,{pics}}}"#), format!("\n{}", meta(1))),
    ] {
        let a = Answer {
            markdown: md,
            files: vec![("a.png", "PNG"), ("b.png", "PNG")],
            manifest: Some(leak(manifest)),
        };
        for s in each_form(&f, &a).await {
            assert_eq!(s.report, format!("2 images, 2 tables{tail}"));
            assert_eq!(s.images, expected);
        }
    }
}

// SPEC-0103 US-0292: the OCR text of images.

/// `n` pictures `p1.png` to `pn.png` under a version 2 manifest whose
/// `pictures` object holds `entries` (a raw JSON object body).
pub(crate) fn pics(n: usize, entries: &str) -> Answer {
    let markdown = leak((1..=n).map(|i| format!("![](p{i}.png)\n")).collect());
    let files = (1..=n).map(|i| (leak(format!("p{i}.png")), "PNG")).collect();
    let manifest = leak(format!(r#"{{"version":2,"pictures":{{{entries}}}}}"#));
    Answer { markdown, files, manifest: Some(manifest) }
}

pub(crate) fn ocr_of(n: usize, text: &str) -> String {
    format!(r#""p{n}.png":{{"ocr_text":"{text}"}}"#)
}

fn some(t: &str) -> Option<String> {
    Some(t.to_string())
}

#[tokio::test]
async fn e2e_0103_020_ocr_text_and_caption_together() {
    let f = fixture().await;
    let a = Answer {
        markdown: "# Title\n\n![](a.png)\n",
        files: vec![("a.png", "PNG")],
        manifest: Some(
            r#"{"version":2,"pictures":{"a.png":{"caption":"Sales","page":2,"ocr_text":"Total 42"}}}"#,
        ),
    };
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, format!("1 images, 0 tables\n{}", meta_ocr(1, 1)));
        assert_eq!(s.images, vec![("Sales".to_string(), Some(2))]);
        assert_eq!(s.ocr, vec![some("Total 42")]);
    }
}

#[tokio::test]
async fn e2e_0103_021_a_picture_without_ocr_text() {
    let f = fixture().await;
    let a = pics(2, &format!(r#"{},"p2.png":{{"caption":"B"}}"#, ocr_of(1, "Total 42")));
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, format!("2 images, 0 tables\n{}", meta_ocr(1, 0)));
        assert_eq!(s.images, vec![(String::new(), None), ("B".to_string(), None)]);
        assert_eq!(s.ocr, vec![some("Total 42"), None]);
    }
}

#[tokio::test]
async fn e2e_0103_022_ocr_text_of_an_empty_string() {
    let f = fixture().await;
    for entry in [ocr_of(1, ""), r#""p1.png":{"ocr_text":null}"#.to_string()] {
        for s in each_form(&f, &pics(1, &entry)).await {
            assert_eq!(s.report, format!("1 images, 0 tables\n{}", meta_ocr(0, 0)));
            assert_eq!(s.ocr, vec![None]);
        }
    }
}

#[tokio::test]
async fn e2e_0103_023_the_converter_says_the_ocr_failed() {
    let f = fixture().await;
    let a =
        pics(2, r#""p1.png":{"caption":"A","page":1,"ocr_text":"x"},"p2.png":{"ocr_failed":true}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!("2 images, 0 tables\n{}\nimage-2: OCR unavailable", meta_ocr(1, 0))
        );
        assert_eq!(s.images, vec![("A".to_string(), Some(1)), (String::new(), None)]);
        assert_eq!(s.ocr, vec![some("x"), None]);
    }
}

#[tokio::test]
async fn e2e_0103_024_ocr_failure_and_caption_failure_on_one_picture() {
    let f = fixture().await;
    let a = pics(1, r#""p1.png":{"caption_failed":true,"ocr_failed":true}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!(
                "1 images, 0 tables\n{}\nimage-1: caption unavailable\nimage-1: OCR unavailable",
                meta_ocr(0, 0)
            )
        );
    }
}

#[tokio::test]
async fn e2e_0103_025_ocr_text_and_ocr_failure_together() {
    let f = fixture().await;
    let a = pics(1, r#""p1.png":{"ocr_text":"Total 42","ocr_failed":true}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!("1 images, 0 tables\n{}\nimage-1: OCR unavailable", meta_ocr(1, 0))
        );
        assert_eq!(s.ocr, vec![some("Total 42")]);
    }
}

#[tokio::test]
async fn e2e_0103_026_ocr_text_too_long() {
    let f = fixture().await;
    let over = "a".repeat(100_001);
    for s in each_form(&f, &pics(1, &ocr_of(1, &over))).await {
        assert_eq!(
            s.report,
            format!("1 images, 0 tables\n{}\nimage-1: OCR text too long", meta_ocr(0, 0))
        );
        assert_eq!(s.ocr, vec![None]);
    }
    let edge = "a".repeat(100_000);
    for s in each_form(&f, &pics(1, &ocr_of(1, &edge))).await {
        assert_eq!(s.report, format!("1 images, 0 tables\n{}", meta_ocr(1, 0)));
        assert_eq!(s.ocr, vec![Some(edge.clone())]);
    }
}

#[tokio::test]
async fn e2e_0103_027_ocr_text_with_accents_and_emoji() {
    let f = fixture().await;
    for s in each_form(&f, &pics(1, &ocr_of(1, "Résumé 42 ✓ 日本語 😀"))).await {
        assert_eq!(s.report, format!("1 images, 0 tables\n{}", meta_ocr(1, 0)));
        assert_eq!(s.ocr, vec![some("Résumé 42 ✓ 日本語 😀")]);
    }
}

#[tokio::test]
async fn e2e_0103_028_exactly_100000_characters_of_ocr_text() {
    let f = fixture().await;
    let text = "é".repeat(100_000);
    for s in each_form(&f, &pics(1, &ocr_of(1, &text))).await {
        assert_eq!(s.report, format!("1 images, 0 tables\n{}", meta_ocr(1, 0)));
        assert_eq!(s.ocr, vec![Some(text.clone())]);
    }
}

#[tokio::test]
async fn e2e_0103_029_two_pictures_with_too_long_ocr_text() {
    let f = fixture().await;
    let over = "a".repeat(100_001);
    let entries = format!("{},{},{}", ocr_of(1, &over), ocr_of(2, "ok"), ocr_of(3, &over));
    for s in each_form(&f, &pics(3, &entries)).await {
        assert_eq!(
            s.report,
            format!(
                "3 images, 0 tables\n{}\nimage-1: OCR text too long\nimage-3: OCR text too long",
                meta_ocr(1, 0)
            )
        );
        assert_eq!(s.ocr, vec![None, some("ok"), None]);
    }
}

#[tokio::test]
async fn e2e_0103_087_ocr_text_is_not_searched_or_chunked_as_a_document_line() {
    let f = fixture().await;
    let a = pics(1, &ocr_of(1, "UniqueOcrWord"));
    let home = tempfile::tempdir().unwrap();
    let s = convert(&f, &program(home.path(), &a)).await;
    assert_eq!(s.ocr, vec![some("UniqueOcrWord")]);
    let view = crate::tools::artifacts::document_view(&f.state, MOUNT, DOC, true, None, None)
        .await
        .unwrap()
        .to_string();
    assert!(!view.contains("UniqueOcrWord"), "{view}");
}

#[tokio::test]
async fn e2e_0103_095_a_picture_field_of_the_wrong_type() {
    let f = fixture().await;
    let a = pics(
        2,
        r#""p1.png":{"caption":"A","page":1,"ocr_text":42},"p2.png":{"caption":"B","page":2,"ocr_failed":"yes"}"#,
    );
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!(
                "2 images, 0 tables\n{}\nimage-1: invalid ocr_text\nimage-2: invalid ocr_failed",
                meta_ocr(0, 0)
            )
        );
        assert_eq!(s.images, vec![("A".to_string(), Some(1)), ("B".to_string(), Some(2))]);
        assert_eq!(s.ocr, vec![None, None]);
    }
}

#[tokio::test]
async fn e2e_0103_097_a_picture_entry_that_is_not_an_object_and_a_bad_page() {
    let f = fixture().await;
    let a = pics(2, r#""p1.png":"x","p2.png":{"page":0}"#);
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!(
                "2 images, 0 tables\n{}\nimage-1: not an object\nimage-2: invalid page",
                meta_ocr(0, 0)
            )
        );
        assert_eq!(s.images, vec![(String::new(), None), (String::new(), None)]);
    }
}

#[tokio::test]
async fn e2e_0103_098_a_wrong_type_in_a_version_one_manifest() {
    let f = fixture().await;
    let a = Answer {
        markdown: "![](p1.png)\n",
        files: vec![("p1.png", "PNG")],
        manifest: Some(
            r#"{"pictures":{"p1.png":{"caption":42,"page":2,"ocr_text":7}},"ocr_text":7}"#,
        ),
    };
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, "1 images, 0 tables\nimage-1: invalid caption");
        assert_eq!(s.images, vec![(String::new(), Some(2))]);
        assert_eq!(s.ocr, vec![None]);
    }
}

#[tokio::test]
async fn e2e_0103_105_an_invalid_picture_page_on_a_source_that_is_not_paged() {
    let f = fixture().await;
    seed(&f, "/doc.docx", b"PK").await;
    let home = tempfile::tempdir().unwrap();
    let bad = pics(1, r#""p1.png":{"page":0}"#);
    let s = convert_at(&f, &program(home.path(), &bad), "/doc.docx").await;
    assert_eq!(s.report, format!("1 images, 0 tables\n{}\nimage-1: invalid page", meta_ocr(0, 0)));
    let good = pics(1, r#""p1.png":{"page":2}"#);
    let s = convert_at(&f, &program(home.path(), &good), "/doc.docx").await;
    assert_eq!(s.report, format!("1 images, 0 tables\n{}", meta_ocr(0, 0)));
    assert_eq!(s.images, vec![(String::new(), None)]);
}

#[tokio::test]
async fn e2e_0103_107_a_picture_that_is_not_kept() {
    let f = fixture().await;
    let a = Answer {
        markdown: PIC,
        files: vec![],
        manifest: Some(
            r#"{"version":2,"pictures":{"a.png":{"ocr_failed":true,"ocr_text":42,"caption":5}}}"#,
        ),
    };
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!("0 images, 0 tables\n{}\nimage-1: picture unavailable", meta_ocr(0, 0))
        );
        assert!(s.ocr.is_empty());
    }
}

#[tokio::test]
async fn e2e_0103_064_pictures_whose_targets_do_not_match_the_manifest() {
    let f = fixture().await;
    let a = Answer {
        markdown: "![](fig/a.png)\n",
        files: vec![("fig/a.png", "PNG")],
        manifest: Some(
            r#"{"version":2,"pictures":{"fig/x.png":{"caption":"X","ocr_text":"hidden"}}}"#,
        ),
    };
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, format!("1 images, 0 tables\n{}", meta_ocr(0, 0)));
        assert_eq!(s.images, no_caption());
        assert_eq!(s.ocr, vec![None]);
    }
}

// SPEC-0103 US-0293: tables with an exact CSV, a caption and a page.

const EXACT: &str = "\"CSV quality: exact\"";
const APPROX: &str = "\"CSV quality: approximate\"";
const T1: &str = "| a | b |\n| --- | --- |\n| 1 | 2 |\n";
/// The CSV the cells of [`T1`] derive.
const T1_CSV: &str = "a,b\n1,2";

/// `n` pipe tables, each separated by a blank line.
fn tabs(n: usize) -> &'static str {
    leak(vec![T1; n].join("\n"))
}

/// A version 2 manifest whose `tables` list holds `entries` (raw JSON texts).
fn tmani(entries: &[&str]) -> &'static str {
    leak(format!(r#"{{"version":2,"tables":[{}]}}"#, entries.join(",")))
}

fn tdoc(
    markdown: &'static str,
    files: Vec<(&'static str, &'static str)>,
    entries: &[&str],
) -> Answer {
    Answer { markdown, files, manifest: Some(tmani(entries)) }
}

/// The metadata line with `csv` tables exact and no image or heading.
fn metacsv(csv: usize) -> String {
    format!("metadata: 0 images with OCR text, {csv} tables with exact CSV, 0 headings")
}

/// The CSV `fs.get_table` answers for table `n` of `doc`.
async fn csv_of(f: &Fixture, doc: &str, n: usize) -> String {
    let out = crate::tools::artifacts::get_table(
        &f.state,
        MOUNT,
        doc,
        &format!("table-{n}"),
        Some("csv"),
    )
    .await
    .unwrap();
    out["content"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn e2e_0103_030_exact_csv_for_a_table() {
    let f = fixture().await;
    let csv = "a,b\n1,\"x,y\"\n";
    let a = tdoc(T1, vec![("tab/t1.csv", csv)], &[r#"{"csv":"tab/t1.csv"}"#]);
    let home = tempfile::tempdir().unwrap();
    let mut seen = vec![convert(&f, &program(home.path(), &a)).await];
    assert_eq!(csv_of(&f, DOC, 1).await, csv);
    seen.push(convert(&f, &http(&a).await.unwrap()).await);
    assert_eq!(csv_of(&f, DOC, 1).await, csv);
    for s in seen {
        assert_eq!(s.tables, [EXACT]);
        assert_eq!(s.report, format!("0 images, 1 tables\n{}", metacsv(1)));
    }
}

#[tokio::test]
async fn e2e_0103_031_a_table_entry_with_a_caption() {
    let f = fixture().await;
    let md = leak(format!("Table 1: Sales\n\n{T1}"));
    let a = tdoc(md, vec![], &[r#"{"caption":"Sales per region"}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(s.captions, ["Sales per region"]);
    }
    let a = tdoc(md, vec![], &[r#"{"caption":""}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(s.captions, ["Sales"]);
    }
}

#[tokio::test]
async fn e2e_0103_032_the_csv_file_is_missing_from_the_bundle() {
    let f = fixture().await;
    let a = tdoc(T1, vec![], &[r#"{"csv":"tab/missing.csv"}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!("0 images, 1 tables\n{}\ntable-1: CSV unavailable", metacsv(0))
        );
        assert_eq!(s.tables, [APPROX]);
    }
}

#[tokio::test]
async fn e2e_0103_033_the_csv_file_is_not_text() {
    let f = fixture().await;
    let a = tdoc(T1, vec![("t.csv", "b64://4A")], &[r#"{"csv":"t.csv"}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!("0 images, 1 tables\n{}\ntable-1: CSV unavailable", metacsv(0))
        );
        assert_eq!(s.tables, [APPROX]);
    }
}

#[tokio::test]
async fn e2e_0103_034_the_csv_file_is_too_large() {
    let f = fixture().await;
    let big = tdoc(T1, vec![("t.csv", "rep:5000001")], &[r#"{"csv":"t.csv"}"#]);
    for s in each_form(&f, &big).await {
        assert_eq!(
            s.report,
            format!("0 images, 1 tables\n{}\ntable-1: CSV unavailable", metacsv(0))
        );
        assert_eq!(s.tables, [APPROX]);
    }
    let edge = tdoc(T1, vec![("t.csv", "rep:5000000")], &[r#"{"csv":"t.csv"}"#]);
    for s in each_form(&f, &edge).await {
        assert_eq!(s.report, format!("0 images, 1 tables\n{}", metacsv(1)));
        assert_eq!(s.tables, [EXACT]);
        assert_eq!(csv_of(&f, DOC, 1).await.len(), 5_000_000);
    }
}

/// The payloads of the exploit: a climb out of the bundle, an absolute path
/// and a URL, with a decoy file at every place such a path could land.
#[tokio::test]
async fn e2e_0103_035_a_csv_reference_that_escapes_the_bundle() {
    let f = fixture().await;
    let decoy = "DECOY_CONTENT_9f3a";
    let a = tdoc(
        tabs(3),
        vec![
            ("etc/passwd", decoy),
            ("passwd", decoy),
            ("t.csv", decoy),
            ("example.com/t.csv", decoy),
        ],
        &[
            r#"{"csv":"../../etc/passwd"}"#,
            r#"{"csv":"/etc/passwd"}"#,
            r#"{"csv":"http://example.com/t.csv"}"#,
        ],
    );
    for s in each_form(&f, &a).await {
        let lines = "table-1: CSV unavailable\ntable-2: CSV unavailable\ntable-3: CSV unavailable";
        assert_eq!(s.report, format!("0 images, 3 tables\n{}\n{lines}", metacsv(0)));
        assert_eq!(s.tables, [APPROX, APPROX, APPROX]);
        for n in 1..=3 {
            let table = crate::tools::artifacts::get_table(
                &f.state,
                MOUNT,
                DOC,
                &format!("table-{n}"),
                None,
            )
            .await
            .unwrap()
            .to_string();
            let both = format!("{}{table}{}", csv_of(&f, DOC, n).await, s.report);
            // Neither the decoy of the bundle nor the real file was read.
            assert!(!both.contains(decoy) && !both.contains("root:"), "{both}");
        }
    }
}

#[tokio::test]
async fn e2e_0103_036_more_entries_than_tables() {
    let f = fixture().await;
    let a = tdoc(
        T1,
        vec![("t.csv", "k,v\n")],
        &[r#"{"csv":"t.csv"}"#, r#"{"csv":"t.csv"}"#, r#"{"caption":"x"}"#],
    );
    for s in each_form(&f, &a).await {
        let line = "manifest lists 3 tables, the document has 1";
        assert_eq!(s.report, format!("0 images, 1 tables\n{}\n{line}", metacsv(1)));
        assert_eq!(s.tables, [EXACT]);
        assert_eq!(s.captions, [""]);
    }
}

#[tokio::test]
async fn e2e_0103_037_fewer_entries_than_tables() {
    let f = fixture().await;
    let a = tdoc(tabs(3), vec![("t.csv", "k,v\n")], &[r#"{"csv":"t.csv"}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, format!("0 images, 3 tables\n{}", metacsv(1)));
        assert_eq!(s.tables, [EXACT, APPROX, APPROX]);
    }
}

#[tokio::test]
async fn e2e_0103_038_a_manifest_table_entry_for_a_malformed_table() {
    let f = fixture().await;
    let md = "| a | b |\n| --- |\n| 1 | 2 |\n";
    let a = tdoc(md, vec![("t.csv", "k,v\n")], &[r#"{"csv":"t.csv","page":3}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(
            s.report,
            format!("0 images, 0 tables\n{}\ntable-1: malformed table", metacsv(0))
        );
        assert!(s.tables.is_empty() && s.pages.is_empty());
    }
}

#[tokio::test]
async fn e2e_0103_039_a_table_entry_that_is_not_an_object() {
    let f = fixture().await;
    let a = tdoc(tabs(2), vec![("t2.csv", "k,v\n")], &["1", r#"{"csv":"t2.csv"}"#]);
    for s in each_form(&f, &a).await {
        let line = "tables entry 1: not an object";
        assert_eq!(s.report, format!("0 images, 2 tables\n{}\n{line}", metacsv(1)));
        assert_eq!(s.tables, [APPROX, EXACT]);
    }
}

#[tokio::test]
async fn e2e_0103_040_the_table_page_is_kept() {
    let f = fixture().await;
    seed(&f, "/doc.docx", b"PK").await;
    let a = tdoc(tabs(2), vec![], &[r#"{"page":2}"#, r#"{"page":2}"#]);
    let home = tempfile::tempdir().unwrap();
    let svc = program(home.path(), &a);
    for doc in [DOC, "/doc.docx"] {
        let s = convert_at(&f, &svc, doc).await;
        assert_eq!(s.report, format!("0 images, 2 tables\n{}", metacsv(0)));
        assert_eq!(s.pages, [Some(2), Some(2)], "{doc}");
    }
}

#[tokio::test]
async fn e2e_0103_041_a_table_page_that_is_invalid() {
    let f = fixture().await;
    let a = tdoc(
        tabs(2),
        vec![("t.csv", "k,v\n")],
        &[r#"{"csv":"t.csv","page":0}"#, r#"{"page":"two"}"#],
    );
    for s in each_form(&f, &a).await {
        let lines = "tables entry 1: invalid page\ntables entry 2: invalid page";
        assert_eq!(s.report, format!("0 images, 2 tables\n{}\n{lines}", metacsv(1)));
        assert_eq!(s.tables, [EXACT, APPROX]);
        assert_eq!(s.pages, [None, None]);
    }
}

#[tokio::test]
async fn e2e_0103_042_exactly_as_many_entries_as_tables() {
    let f = fixture().await;
    let a = tdoc(
        tabs(2),
        vec![("t1.csv", "one\n"), ("t2.csv", "two\n")],
        &[r#"{"csv":"t1.csv"}"#, r#"{"csv":"t2.csv"}"#],
    );
    for s in each_form(&f, &a).await {
        assert_eq!(s.report, format!("0 images, 2 tables\n{}", metacsv(2)));
        assert_eq!(csv_of(&f, DOC, 1).await, "one\n");
        assert_eq!(csv_of(&f, DOC, 2).await, "two\n");
    }
}

#[tokio::test]
async fn e2e_0103_043_a_table_without_csv_then_one_with() {
    let f = fixture().await;
    let a = tdoc(tabs(2), vec![("tab/t2.csv", "x,y\n9,9\n")], &["{}", r#"{"csv":"tab/t2.csv"}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(s.tables, [APPROX, EXACT]);
        assert_eq!(csv_of(&f, DOC, 2).await, "x,y\n9,9\n");
        assert_eq!(csv_of(&f, DOC, 1).await, T1_CSV);
    }
}

#[tokio::test]
async fn e2e_0103_082_a_table_csv_per_sheet_of_a_spreadsheet() {
    let f = fixture().await;
    seed(&f, "/sheet.xlsx", b"PK").await;
    let a = tdoc(
        tabs(3),
        vec![("s1.csv", "1\n"), ("s2.csv", "2\n"), ("s3.csv", "3\n")],
        &[r#"{"csv":"s1.csv"}"#, r#"{"csv":"s2.csv"}"#, r#"{"csv":"s3.csv"}"#],
    );
    let home = tempfile::tempdir().unwrap();
    let s = convert_at(&f, &program(home.path(), &a), "/sheet.xlsx").await;
    assert_eq!(s.tables, [EXACT, EXACT, EXACT]);
    assert_eq!(s.report, format!("0 images, 3 tables\n{}", metacsv(3)));
    assert_eq!(csv_of(&f, "/sheet.xlsx", 3).await, "3\n");
}

#[tokio::test]
async fn e2e_0103_084_the_markdown_caption_wins_when_the_manifest_has_none() {
    let f = fixture().await;
    let md = leak(format!("{T1}\nTable 2: Costs\n\n{T1}"));
    let a = tdoc(md, vec![], &["{}", r#"{"csv":null}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(s.captions, ["", "Costs"]);
    }
    let a = tdoc(md, vec![], &["{}", r#"{"caption":"Spend"}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(s.captions, ["", "Spend"]);
    }
}

#[tokio::test]
async fn e2e_0103_096_a_table_field_of_the_wrong_type() {
    let f = fixture().await;
    let a = tdoc(tabs(2), vec![], &[r#"{"csv":7}"#, r#"{"caption":5}"#]);
    for s in each_form(&f, &a).await {
        let lines = "tables entry 1: invalid csv\ntables entry 2: invalid caption";
        assert_eq!(s.report, format!("0 images, 2 tables\n{}\n{lines}", metacsv(0)));
        assert_eq!(s.tables, [APPROX, APPROX]);
    }
}

#[tokio::test]
async fn e2e_0103_104_null_fields_are_absent() {
    let f = fixture().await;
    let md = leak(format!("# T\n\n{PIC}\n{T1}"));
    let m = r#"{"version":2,
        "pictures":{"a.png":{"ocr_text":null,"caption":null,"page":null}},
        "tables":[{"csv":null,"caption":null,"page":null}],
        "headings":[{"level":1,"title":"T","page":null}]}"#;
    let a = Answer { markdown: md, files: vec![("a.png", "PNG")], manifest: Some(m) };
    for s in each_form(&f, &a).await {
        let meta = "metadata: 0 images with OCR text, 0 tables with exact CSV, 1 headings";
        assert_eq!(s.report, format!("1 images, 1 tables\n{meta}"));
        assert_eq!(s.headings, [(1, "T".to_string(), None)]);
        assert_eq!(s.tables, [APPROX]);
    }
}

#[tokio::test]
async fn e2e_0103_077_table_files_are_not_left_in_the_volume() {
    let f = fixture().await;
    let a = tdoc(T1, vec![("tab/t1.csv", "k,v\n")], &[r#"{"csv":"tab/t1.csv"}"#]);
    for s in each_form(&f, &a).await {
        assert_eq!(s.tables, [EXACT]);
        let c = f.state.stores.client(MOUNT).await.unwrap();
        let mut names: Vec<String> =
            c.list_dir("/").await.unwrap().into_iter().map(|n| n.path).collect();
        names.sort();
        assert!(names.iter().all(|n| !n.contains(".csv") && !n.contains("tab")), "{names:?}");
        assert!(names.contains(&"/report.pdf".to_string()), "{names:?}");
        assert_eq!(csv_of(&f, DOC, 1).await, "k,v\n");
    }
}

/// The HTTP form can name a bundle file `../tab.csv`; a table entry naming it
/// reads nothing and the conversion still succeeds.
#[tokio::test]
async fn e2e_0103_078_a_bundle_file_with_an_unsafe_name() {
    let f = fixture().await;
    let decoy = "DECOY_TAB_77c1";
    let a = tdoc(T1, vec![("../tab.csv", decoy), ("tab.csv", decoy)], &[r#"{"csv":"../tab.csv"}"#]);
    let s = convert(&f, &http(&a).await.unwrap()).await;
    assert_eq!(s.report, format!("0 images, 1 tables\n{}\ntable-1: CSV unavailable", metacsv(0)));
    assert_eq!(s.tables, [APPROX]);
    assert_eq!(csv_of(&f, DOC, 1).await, T1_CSV);
}

// SPEC-0103 US-0294: the output limit, the quota and the two forms.

/// The converter output limit of the boundary cases.
const LIMIT: u64 = 1_000_000;

/// What the program form's heredocs add to the Markdown and the manifest: one
/// newline each. The HTTP form counts their text as sent.
const HEREDOC: usize = 2;

/// A bundle of a picture, a pipe table with an exact CSV and a padding file
/// whose files, Markdown and manifest add up to `total` bytes in a form that
/// adds `extra` bytes of its own (the manifest is compact, keys sorted, so
/// both forms count the same text).
fn sized(total: usize, extra: usize) -> Answer {
    // The bulk is Markdown text, not a file: a file travels in base64 (4/3 of
    // its size) and the HTTP wire read is bounded by the limit plus framing.
    let head = "![](a.png)\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\n";
    let manifest: &'static str = r#"{"tables":[{"csv":"tab/t1.csv"}],"version":2}"#;
    let csv: &'static str = leak(format!("a,b\n1,2\n{}", "9,9\n".repeat(750)));
    let fixed = head.len() + manifest.len() + "PNG".len() + csv.len() + extra;
    let markdown = leak(format!("{head}{}", "x".repeat(total - fixed)));
    Answer {
        markdown,
        files: vec![("a.png", "PNG"), ("tab/t1.csv", csv)],
        manifest: Some(manifest),
    }
}

/// Ten pictures whose OCR text is most of a `total` byte bundle.
fn sized_ocr(total: usize, extra: usize) -> Answer {
    let build = |ocr: usize| {
        let entries: Vec<String> = (1..=10).map(|i| ocr_of(i, &"o".repeat(ocr))).collect();
        pics(10, leak(entries.join(",")))
    };
    let base = build(0);
    let fixed: usize = base.markdown.len()
        + base.manifest.unwrap().len()
        + base.files.iter().map(|(_, b)| b.len()).sum::<usize>()
        + extra;
    let per = (total - fixed) / 10;
    let mut a = build(per);
    let missing = total - fixed - per * 10;
    a.manifest = Some(leak(a.manifest.unwrap().replacen(
        &ocr_of(10, &"o".repeat(per)),
        &ocr_of(10, &"o".repeat(per + missing)),
        1,
    )));
    a
}

async fn try_convert(f: &Fixture, svc: &dyn DocService) -> crate::errors::Result<Value> {
    f.state.authorize(MOUNT, OWNER).await.unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    crate::core::fs_ops::documentize(&c, &f.state.safety, Some(svc), OWNER, MOUNT, DOC, true).await
}

const OVER: &str =
    "document service output is over the doc_service.max_input_bytes limit of 1000000";

/// No image, table, OCR text, heading or CSV file kept for `DOC`.
async fn nothing_kept(f: &Fixture) {
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let store = c.trash.as_deref().unwrap();
    assert!(store.current_ocr_for_test(DOC).await.unwrap().is_empty());
    assert!(store.current_headings_for_test(DOC).await.unwrap().is_empty());
    assert!(store.current_table_pages_for_test(DOC).await.unwrap().is_empty());
    let names: Vec<String> = c.list_dir("/").await.unwrap().into_iter().map(|n| n.path).collect();
    assert_eq!(names.iter().filter(|n| !n.starts_with("/docs")).count(), 1, "{names:?}");
}

/// `answer(extra)` at `total` converts, at `total + 1` it is refused, in both
/// forms, with the one text.
async fn boundary(build: impl Fn(usize, usize) -> Answer) {
    for over in [false, true] {
        let total = LIMIT as usize + usize::from(over);
        let f = fixture().await;
        let home = tempfile::tempdir().unwrap();
        let program = program_cap(home.path(), &build(total, HEREDOC), LIMIT);
        let api = http_cap(&build(total, 0), LIMIT).await.unwrap();
        for svc in [&program as &dyn DocService, &api] {
            let r = try_convert(&f, svc).await;
            if over {
                assert_eq!(r.expect_err("over the limit").message, OVER);
                nothing_kept(&f).await;
            } else {
                let done = r.expect("exactly at the limit");
                assert!(
                    done["artifacts_report"].as_str().unwrap().starts_with("1 images, 1 tables")
                );
            }
        }
    }
}

#[tokio::test]
async fn e2e_0103_065_a_bundle_exactly_at_the_converter_output_limit() {
    let f = fixture().await;
    let home = tempfile::tempdir().unwrap();
    let program = program_cap(home.path(), &sized(LIMIT as usize, HEREDOC), LIMIT);
    let api = http_cap(&sized(LIMIT as usize, 0), LIMIT).await.unwrap();
    for svc in [&program as &dyn DocService, &api] {
        let s = convert(&f, svc).await;
        assert_eq!(s.report, format!("1 images, 1 tables\n{}", metacsv(1)));
        assert_eq!(s.tables, [EXACT]);
        assert!(csv_of(&f, DOC, 1).await.starts_with("a,b\n1,2\n9,9\n"));
    }
}

#[tokio::test]
async fn e2e_0103_076_the_bundle_cap_counts_the_csv_files() {
    boundary(sized).await;
}

#[tokio::test]
async fn e2e_0103_092_the_manifest_bytes_count_in_the_output_limit_program_form() {
    for over in [false, true] {
        let f = fixture().await;
        let home = tempfile::tempdir().unwrap();
        let total = LIMIT as usize + usize::from(over);
        let svc = program_cap(home.path(), &sized_ocr(total, HEREDOC), LIMIT);
        let r = try_convert(&f, &svc).await;
        if over {
            assert_eq!(r.expect_err("over the limit").message, OVER);
            nothing_kept(&f).await;
        } else {
            r.expect("exactly at the limit");
        }
    }
}

#[tokio::test]
async fn e2e_0103_093_the_same_total_in_the_http_form() {
    for over in [false, true] {
        let f = fixture().await;
        let total = LIMIT as usize + usize::from(over);
        let svc = http_cap(&sized_ocr(total, 0), LIMIT).await.unwrap();
        let r = try_convert(&f, &svc).await;
        if over {
            assert_eq!(r.expect_err("over the limit").message, OVER);
            nothing_kept(&f).await;
        } else {
            r.expect("exactly at the limit");
        }
    }
}

#[tokio::test]
async fn e2e_0103_094_a_large_answer_is_stored_in_time() {
    let f = fixture().await;
    let markdown =
        leak((1..=1000).map(|i| format!("![](p{i}.png)\n")).collect::<String>() + tabs(500));
    let files = (1..=1000).map(|i| (leak(format!("p{i}.png")), "PNG")).collect();
    let a = Answer { markdown, files, manifest: Some(listing(2, &numbered(10_000))) };
    let svc = http(&a).await.unwrap();
    let t = std::time::Instant::now();
    svc.convert(b"%PDF-1.4", "report.pdf").await.unwrap();
    let converter = t.elapsed();
    let t = std::time::Instant::now();
    let s = convert(&f, &svc).await;
    assert!(t.elapsed() <= converter + std::time::Duration::from_secs(10), "{:?}", t.elapsed());
    assert!(s.report.starts_with("1000 images, 500 tables\n"), "{}", &s.report[..60]);
    assert!(s.report.contains("0 tables with exact CSV, 10000 headings"));
    assert_eq!(s.headings.len(), 10_000);
}

#[tokio::test]
async fn e2e_0103_075_storage_quota_counts_the_new_metadata() {
    for form in 0..2 {
        // The metadata is session weight (SPEC-0020 DEC-008): the allowance is
        // the 1 000 bytes of room, the refusal is the existing session one.
        let f = office_fixture_allowance(1000).await;
        seed(&f, DOC, b"%PDF-1.4").await;
        let csv = leak(format!("a,b\n{}\n", "1,".repeat(1000)));
        let ocr = "o".repeat(2000);
        let manifest = leak(format!(
            r#"{{"version":2,"pictures":{{"a.png":{{"ocr_text":"{ocr}"}}}},"tables":[{{"csv":"tab/t1.csv"}}]}}"#
        ));
        let a = Answer {
            markdown: leak(format!("![](a.png)\n\n{T1}")),
            files: vec![("a.png", "PNG"), ("tab/t1.csv", csv)],
            manifest: Some(manifest),
        };
        let home = tempfile::tempdir().unwrap();
        let svc: Box<dyn DocService> = if form == 0 {
            Box::new(program(home.path(), &a))
        } else {
            Box::new(http(&a).await.unwrap())
        };
        let e = try_convert(&f, svc.as_ref()).await.expect_err("over the project quota");
        assert_eq!(e.code, crate::errors::code::WRITE_QUOTA_EXCEEDED, "{e:?}");
        assert_eq!(e.message, "session write quota of 1000 bytes exceeded");
        nothing_kept(&f).await;
    }
}

#[tokio::test]
async fn e2e_0103_079_the_http_form_with_a_base64_error() {
    let f = fixture().await;
    let body = json!({
        "markdown": T1,
        "files": {"tab/t1.csv": "not base64 !!"},
        "manifest": {"version": 2, "tables": [{"csv": "tab/t1.csv"}]},
    });
    let svc = serve(body.to_string(), CAP).await;
    let e = try_convert(&f, &svc).await.expect_err("a base64 error");
    assert!(
        e.message.contains("document service bundle file 'tab/t1.csv' is not base64:"),
        "{}",
        e.message
    );
    nothing_kept(&f).await;
}

#[tokio::test]
async fn e2e_0103_066_http_form_with_unknown_top_level_keys() {
    let f = fixture().await;
    let manifest = json!({"version": 2, "pictures": {"a.png": {"caption": "Sales"}}});
    let plain = json!({"markdown": PIC, "files": {"a.png": "UE5H"}, "manifest": manifest});
    let mut extra = plain.clone();
    extra["extra"] = json!({"anything": [1, 2, 3]});
    let base = convert(&f, &serve(plain.to_string(), CAP).await).await;
    let with = convert(&f, &serve(extra.to_string(), CAP).await).await;
    assert_eq!(with.report, base.report);
    assert_eq!(with.images, base.images);
    assert_eq!(with.images, vec![("Sales".to_string(), None)]);
}

#[tokio::test]
async fn e2e_0103_067_folder_form_with_the_manifest_next_to_other_files() {
    let f = fixture().await;
    let a = Answer {
        markdown: leak(format!("![](fig/a.png)\n\n{T1}")),
        files: vec![("fig/a.png", "PNG"), ("tab/t1.csv", "a,b\n1,2\n"), ("notes.txt", "private")],
        manifest: Some(tmani(&[r#"{"csv":"tab/t1.csv"}"#])),
    };
    let home = tempfile::tempdir().unwrap();
    let s = convert(&f, &program(home.path(), &a)).await;
    assert_eq!(s.report, format!("1 images, 1 tables\n{}", metacsv(1)));
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let names: Vec<String> = c.list_dir("/").await.unwrap().into_iter().map(|n| n.path).collect();
    assert!(names.iter().all(|n| !n.contains("notes")), "{names:?}");
}

#[tokio::test]
async fn e2e_0103_002_the_same_answer_from_the_http_form() {
    let f = fixture().await;
    let a = Answer {
        markdown: leak(format!("# Title\n\n![](a.png)\n\n{T1}")),
        files: vec![("a.png", "PNG"), ("tab/t1.csv", "a,b\n1,2\n")],
        manifest: Some(leak(
            r#"{"version":2,"pictures":{"a.png":{"caption":"Sales","page":2,"ocr_text":"Total 42"}},"tables":[{"csv":"tab/t1.csv","caption":"Cap","page":3}]}"#
                .to_string(),
        )),
    };
    let seen = each_form(&f, &a).await;
    assert_eq!(seen.len(), 2);
    let (p, h) = (&seen[0], &seen[1]);
    assert_eq!(p.report, h.report);
    assert_eq!(p.images, h.images);
    assert_eq!(p.tables, h.tables);
    assert_eq!(p.headings, h.headings);
    assert_eq!(p.ocr, h.ocr);
    assert_eq!(p.captions, h.captions);
    assert_eq!(p.pages, h.pages);
    assert_eq!(p.ocr, [some("Total 42")]);
    assert_eq!(p.captions, ["Cap"]);
}

// SPEC-0103 US-0295: replacing, upgrading, the other paths.

const FULL_MD: &str = "# Report\n\n## Sales\n\n![](fig/a.png)\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\n## Costs\n\n![](fig/b.png)\n\n### Detail\n\n| c | d |\n| --- | --- |\n| 3 | 4 |\n";
const FULL_MANIFEST: &str = r#"{"version":2,
"pictures":{"fig/a.png":{"caption":"Sales","ocr_text":"Total 42"},"fig/b.png":{"caption":"Costs","ocr_text":"Net 7"}},
"tables":[{"csv":"tab/t1.csv"},{}],
"headings":[{"level":1,"title":"Report"},{"level":2,"title":"Sales"},{"level":2,"title":"Costs"},{"level":3,"title":"Detail"}]}"#;
const FULL_REPORT: &str =
    "2 images, 2 tables\nmetadata: 2 images with OCR text, 1 tables with exact CSV, 4 headings";
const T1_EXACT: &str = "a;b\n1;2\n";

/// The full version 2 answer of E2E-001.
fn full() -> Answer {
    Answer {
        markdown: FULL_MD,
        files: vec![("fig/a.png", "PNG"), ("fig/b.png", "PNG2"), ("tab/t1.csv", T1_EXACT)],
        manifest: Some(FULL_MANIFEST),
    }
}

/// The same pictures and tables under a version 1 manifest (no metadata).
fn plain() -> Answer {
    Answer {
        manifest: Some(
            r#"{"pictures":{"fig/a.png":{"caption":"Sales"},"fig/b.png":{"caption":"Costs"}}}"#,
        ),
        ..full()
    }
}

/// Counts the conversions asked of the converter it wraps.
pub(crate) struct Counting<'a> {
    inner: &'a dyn DocService,
    calls: std::sync::atomic::AtomicUsize,
}

impl<'a> Counting<'a> {
    pub(crate) fn new(inner: &'a dyn DocService) -> Self {
        Self { inner, calls: 0.into() }
    }

    pub(crate) fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl DocService for Counting<'_> {
    async fn convert(
        &self,
        bytes: &[u8],
        file_name: &str,
    ) -> crate::errors::Result<crate::docs::service::Conversion> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.convert(bytes, file_name).await
    }

    fn accepted_extensions(&self) -> &[String] {
        self.inner.accepted_extensions()
    }

    fn max_input_bytes(&self) -> u64 {
        self.inner.max_input_bytes()
    }
}

/// A converter that always fails with `message`.
struct Failing(&'static str, Vec<String>);

#[async_trait::async_trait]
impl DocService for Failing {
    async fn convert(
        &self,
        _bytes: &[u8],
        _file_name: &str,
    ) -> crate::errors::Result<crate::docs::service::Conversion> {
        Err(crate::errors::ToolError::internal(self.0))
    }

    fn accepted_extensions(&self) -> &[String] {
        &self.1
    }

    fn max_input_bytes(&self) -> u64 {
        CAP
    }
}

/// Everything Alice can read about `doc`: both lists, the stored structure and
/// OCR text, the exact CSV of each table.
async fn state_of(f: &Fixture, person: &str, doc: &str) -> Value {
    f.state.authorize(MOUNT, person).await.unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let images = crate::tools::artifacts::list_images(&f.state, MOUNT, doc, None).await.unwrap();
    let tables = crate::tools::artifacts::list_tables(&f.state, MOUNT, doc, None).await.unwrap();
    let mut csv = Vec::new();
    for n in 1..=tables["tables"].as_array().unwrap().len() {
        csv.push(csv_of(f, doc, n).await);
    }
    let store = c.trash.as_deref().unwrap();
    json!({
        "images": images, "tables": tables, "csv": csv,
        "ocr": store.current_ocr_for_test(doc).await.unwrap(),
        "headings": store.current_headings_for_test(doc).await.unwrap(),
    })
}

fn no_artifacts(r: crate::errors::Result<Value>) {
    let e = r.expect_err("no artifacts");
    assert_eq!(e.message, crate::docs::artifacts::NO_ARTIFACTS);
}

/// `/notes.docx` with one 2x2 table, a real package for the built in reader.
pub(crate) fn docx_bytes() -> Vec<u8> {
    let cell = |t: &str| format!("<w:tc><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc>");
    let doc = format!(
        "<w:document xmlns:w=\"x\"><w:body><w:tbl><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr></w:tbl></w:body></w:document>",
        cell("A"),
        cell("B"),
        cell("1"),
        cell("2")
    );
    crate::tools::archive_testkit::office_zip(&[("word/document.xml", doc.as_bytes())])
}

#[tokio::test]
async fn e2e_0103_001_a_full_version_two_answer_from_the_program_form() {
    let f = fixture().await;
    let home = tempfile::tempdir().unwrap();
    let s = convert(&f, &program(home.path(), &full())).await;
    assert_eq!(s.report, FULL_REPORT);
    assert_eq!(s.tables, [EXACT, APPROX]);
    assert_eq!(csv_of(&f, DOC, 1).await, T1_EXACT);
    assert_eq!(s.ocr, [some("Total 42"), some("Net 7")]);
    assert_eq!(s.headings.len(), 4);
}

#[tokio::test]
async fn e2e_0103_062_the_same_manifest_twice() {
    let f = fixture().await;
    let home = tempfile::tempdir().unwrap();
    let svc = program(home.path(), &full());
    let first = convert(&f, &svc).await;
    let before = state_of(&f, OWNER, DOC).await;
    let second = convert(&f, &svc).await;
    assert_eq!(first.report, second.report);
    assert_eq!(before, state_of(&f, OWNER, DOC).await);
    assert_eq!(second.images.len(), 2);
    assert_eq!(second.tables.len(), 2);
}

#[tokio::test]
async fn e2e_0103_068_an_extraction_option_does_not_change_a_converted_document() {
    let f = office_fixture_allowance(1_000_000_000).await;
    seed(&f, "/notes.docx", &docx_bytes()).await;
    let home = tempfile::tempdir().unwrap();
    let a = Answer {
        markdown: leak(format!("![](p.png)\n\n{T1}")),
        files: vec![("p.png", "PNG")],
        manifest: Some(
            r#"{"version":2,"pictures":{"p.png":{"ocr_text":"secret"}},"tables":[{"csv":"x.csv"}]}"#,
        ),
    };
    let svc = program(home.path(), &a);
    convert_at(&f, &svc, "/notes.docx").await;
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let store = c.trash.as_deref().unwrap();
    assert_eq!(store.current_ocr_for_test("/notes.docx").await.unwrap(), [some("secret")]);
    let done =
        crate::tools::archive_testkit::extract_text(&f, OWNER, "/notes.docx", true).await.unwrap();
    assert_eq!(done["artifacts_report"], "0 images, 1 tables");
    assert!(store.current_ocr_for_test("/notes.docx").await.unwrap().is_empty());
    let tables =
        crate::tools::artifacts::list_tables(&f.state, MOUNT, "/notes.docx", None).await.unwrap();
    assert_eq!(tables["tables"][0]["csv_quality"], "CSV quality: exact");
}

/// A set the previous version wrote: null OCR, CSV and page, no structure.
pub(crate) async fn old_set(f: &Fixture, doc: &str, caption: &str) {
    use crate::storage::meta::{NewArtifactSet, NewDocImage, NewDocTable};
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let store = c.trash.as_deref().unwrap();
    let rev = store.ensure_node_rev(doc).await.unwrap().expect("a rev");
    let text = "![](a.png)\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n";
    let (text_sha, pic) = (
        crate::storage::volume::VolumeClient::sha256_hex(text.as_bytes()),
        crate::storage::volume::VolumeClient::sha256_hex(b"OLDPNG"),
    );
    c.blob.put(&text_sha, text.as_bytes()).await.unwrap();
    c.blob.put(&pic, b"OLDPNG").await.unwrap();
    let set = NewArtifactSet {
        set_id: uuid::Uuid::new_v4().to_string(),
        path: doc.to_string(),
        node_rev: rev,
        text_sha: Some(text_sha),
        text_len: text.len() as i64,
        report: "1 images, 1 tables".into(),
        note: None,
        images: vec![NewDocImage {
            seq: 1,
            caption: caption.into(),
            page: Some(2),
            format: "png".into(),
            sha256: pic,
            size: 6,
            line_start: 0,
            line_count: 1,
            ocr_text: None,
        }],
        tables: vec![NewDocTable {
            seq: 1,
            caption: String::new(),
            rows: 1,
            cols: 2,
            quality: "approximate".into(),
            cells: r#"[["a","b"],["1","2"]]"#.into(),
            line_start: 2,
            line_count: 3,
            csv_text: None,
            page: None,
        }],
        headings: Vec::new(),
    };
    store.commit_artifact_set(&set).await.unwrap();
}

#[tokio::test]
async fn e2e_0103_069_a_document_converted_before_is_read_after_a_restart() {
    let f = fixture().await;
    old_set(&f, DOC, "Old chart").await;
    let before = state_of(&f, OWNER, DOC).await;
    // A restart keeps the stores and drops the in memory state: the next
    // read goes through the same rows and no converter is configured here.
    assert_eq!(state_of(&f, OWNER, DOC).await, before);
    assert_eq!(before["tables"]["tables"][0]["csv_quality"], APPROX_LABEL);
}

const APPROX_LABEL: &str = "CSV quality: approximate";

#[tokio::test]
async fn e2e_0103_072_documents_converted_before_keep_their_data() {
    let f = fixture().await;
    old_set(&f, DOC, "Old chart").await;
    let before = state_of(&f, OWNER, DOC).await;
    assert_eq!(before["images"]["images"][0]["caption"], "Old chart");
    assert_eq!(before["ocr"], json!([null]));
    assert_eq!(before["headings"], json!([]));
    assert_eq!(before["csv"], json!(["a,b\n1,2"]));
    let home = tempfile::tempdir().unwrap();
    let s = convert(&f, &program(home.path(), &full())).await;
    assert_eq!(s.report, FULL_REPORT);
    assert_eq!(s.ocr, [some("Total 42"), some("Net 7")]);
    assert_eq!(s.headings.len(), 4);
}

#[tokio::test]
async fn e2e_0103_070_a_reconversion_replaces_the_whole_set() {
    let f = fixture().await;
    let home = tempfile::tempdir().unwrap();
    convert(&f, &program(home.path(), &full())).await;
    let again = tempfile::tempdir().unwrap();
    let s = convert(&f, &program(again.path(), &plain())).await;
    assert_eq!(s.report, "2 images, 2 tables");
    assert_eq!(s.tables, [APPROX, APPROX]);
    assert_eq!(s.ocr, [None, None]);
    assert_ne!(csv_of(&f, DOC, 1).await, T1_EXACT);
    assert_eq!(csv_of(&f, DOC, 1).await, T1_CSV);
}

#[tokio::test]
async fn e2e_0103_071_a_failed_reconversion_keeps_the_previous_set() {
    let f = fixture().await;
    let home = tempfile::tempdir().unwrap();
    convert(&f, &program(home.path(), &full())).await;
    let before = state_of(&f, OWNER, DOC).await;
    let e = try_convert(&f, &Failing("doc service failed: boom", extensions()))
        .await
        .expect_err("the converter fails");
    assert_eq!(e.message, "doc service failed: boom");
    assert_eq!(state_of(&f, OWNER, DOC).await, before);
    assert_eq!(before["csv"][0], T1_EXACT);
}

#[tokio::test]
async fn e2e_0103_073_the_built_in_extraction_is_unchanged() {
    let f = office_fixture_allowance(1_000_000_000).await;
    seed(&f, "/notes.docx", &docx_bytes()).await;
    let done =
        crate::tools::archive_testkit::extract_text(&f, OWNER, "/notes.docx", false).await.unwrap();
    assert_eq!(done["artifacts_report"], "0 images, 1 tables");
    let tables =
        crate::tools::artifacts::list_tables(&f.state, MOUNT, "/notes.docx", None).await.unwrap();
    assert_eq!(tables["tables"][0]["csv_quality"], "CSV quality: exact");
}

#[tokio::test]
async fn e2e_0103_074_a_file_edited_after_conversion() {
    let f = fixture().await;
    let home = tempfile::tempdir().unwrap();
    convert(&f, &program(home.path(), &full())).await;
    let c = f.state.stores.client(MOUNT).await.unwrap();
    c.write_bytes_atomic(DOC, b"%PDF-1.7 edited").await.unwrap();
    no_artifacts(crate::tools::artifacts::list_images(&f.state, MOUNT, DOC, None).await);
    no_artifacts(crate::tools::artifacts::list_tables(&f.state, MOUNT, DOC, None).await);
}

#[tokio::test]
async fn e2e_0103_080_conversion_by_upload_carries_the_same_data() {
    let f = office_fixture_allowance(1_000_000_000).await;
    let home = tempfile::tempdir().unwrap();
    let svc = program(home.path(), &full());
    f.state.authorize(MOUNT, OWNER).await.unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let out = crate::core::fs_ops::write_bytes_documented(
        &c,
        &f.state.safety,
        Some(&svc),
        OWNER,
        MOUNT,
        DOC,
        b"%PDF-1.4",
        false,
        true,
        true,
    )
    .await
    .unwrap();
    assert_eq!(out["documentation"]["artifacts_report"], FULL_REPORT);
    let s = state_of(&f, OWNER, DOC).await;
    assert_eq!(s["csv"][0], T1_EXACT);
    assert_eq!(s["ocr"], json!(["Total 42", "Net 7"]));
}

#[tokio::test]
async fn e2e_0103_081_a_non_member_cannot_convert() {
    let f = fixture().await;
    let e = f.state.authorize(MOUNT, "dave@example.com").await.unwrap_err();
    assert!(
        e.to_string().ends_with("ERR_FORBIDDEN: 'dave@example.com' is not a member of 'atlas'"),
        "{e}"
    );
    nothing_kept(&f).await;
}

#[tokio::test]
async fn e2e_0103_083_a_second_member_sees_the_same_data() {
    let f = fixture().await;
    let carol = crate::tools::archive_testkit::add_second_member(&f).await;
    let home = tempfile::tempdir().unwrap();
    let svc = program(home.path(), &full());
    let counting = Counting::new(&svc);
    convert(&f, &counting).await;
    let s = state_of(&f, carol, DOC).await;
    assert_eq!(s["tables"]["tables"][0]["csv_quality"], "CSV quality: exact");
    assert_eq!(s["csv"][0], T1_EXACT);
    assert_eq!(counting.calls(), 1);
}

#[tokio::test]
async fn e2e_0103_085_a_failed_conversion_changes_nothing() {
    let f = fixture().await;
    let home = tempfile::tempdir().unwrap();
    let script = home.path().join("slow.sh");
    std::fs::write(&script, "#!/bin/sh\nexec sleep 30\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let svc = CliDocService::new(
        crate::config::DocServiceCliConfig {
            command: vec![
                script.display().to_string(),
                crate::config::OUTDIR_PLACEHOLDER.into(),
                crate::config::DOC_PLACEHOLDER.into(),
            ],
            timeout_secs: 1,
        },
        extensions(),
        CAP,
    );
    let e = try_convert(&f, &svc).await.expect_err("times out");
    assert!(e.message.contains("doc service timed out after 1s"), "{}", e.message);
    no_artifacts(crate::tools::artifacts::list_images(&f.state, MOUNT, DOC, None).await);
    no_artifacts(crate::tools::artifacts::list_tables(&f.state, MOUNT, DOC, None).await);
    nothing_kept(&f).await;
}

#[tokio::test]
async fn e2e_0103_086_an_old_document_and_a_new_one_in_the_same_folder() {
    let f = fixture().await;
    seed(&f, "/old.pdf", b"%PDF-1.4 old").await;
    seed(&f, "/new.pdf", b"%PDF-1.4 new").await;
    old_set(&f, "/old.pdf", "Old chart").await;
    let home = tempfile::tempdir().unwrap();
    convert_at(&f, &program(home.path(), &full()), "/new.pdf").await;
    let old = state_of(&f, OWNER, "/old.pdf").await;
    let new = state_of(&f, OWNER, "/new.pdf").await;
    assert_eq!(old["tables"]["tables"][0]["csv_quality"], APPROX_LABEL);
    assert_eq!(new["tables"]["tables"][0]["csv_quality"], "CSV quality: exact");
    assert_eq!(old["images"]["images"][0]["caption"], "Old chart");
    assert_eq!(new["ocr"], json!(["Total 42", "Net 7"]));
    assert_eq!(old["ocr"], json!([null]));
}

/// The two sheets of `/budget.xlsx`, one table each.
fn xlsx_bytes() -> Vec<u8> {
    let workbook = r#"<workbook xmlns:r="r"><sheets><sheet name="A" r:id="rId1"/><sheet name="B" r:id="rId2"/></sheets></workbook>"#;
    let rels = r#"<Relationships xmlns="x"><Relationship Id="rId1" Type="w" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="w" Target="worksheets/sheet2.xml"/></Relationships>"#;
    let sheet = |v: &str| {
        format!(
            r#"<worksheet><sheetData><row><c t="inlineStr"><is><t>{v}</t></is></c></row><row><c t="inlineStr"><is><t>x</t></is></c></row></sheetData></worksheet>"#
        )
    };
    let (s1, s2) = (sheet("h1"), sheet("h2"));
    crate::tools::archive_testkit::office_zip(&[
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
        ("xl/worksheets/sheet1.xml", s1.as_bytes()),
        ("xl/worksheets/sheet2.xml", s2.as_bytes()),
    ])
}

#[tokio::test]
async fn e2e_0103_088_the_built_in_extraction_of_a_spreadsheet() {
    let f = office_fixture_allowance(1_000_000_000).await;
    seed(&f, "/budget.xlsx", &xlsx_bytes()).await;
    let done = crate::tools::archive_testkit::extract_text(&f, OWNER, "/budget.xlsx", false)
        .await
        .unwrap();
    assert_eq!(done["artifacts_report"], "0 images, 2 tables");
    let tables =
        crate::tools::artifacts::list_tables(&f.state, MOUNT, "/budget.xlsx", None).await.unwrap();
    assert_eq!(tables["tables"][0]["csv_quality"], "CSV quality: exact");
    assert_eq!(tables["tables"][1]["csv_quality"], "CSV quality: exact");
}

/// An HTTP converter answering `status` with an error body.
async fn failing_http(status: u16) -> ApiDocService {
    let app = axum::Router::new().route(
        "/convert",
        axum::routing::post(move || async move {
            (axum::http::StatusCode::from_u16(status).unwrap(), "down")
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let cfg = DocServiceApiConfig { url: format!("http://{addr}/convert"), ..Default::default() };
    ApiDocService::new(&cfg, extensions(), CAP).unwrap()
}

#[tokio::test]
async fn e2e_0103_102_the_http_converter_fails() {
    let f = fixture().await;
    convert(&f, &http(&full()).await.unwrap()).await;
    let before = state_of(&f, OWNER, DOC).await;
    let e = try_convert(&f, &failing_http(503).await).await.expect_err("the service fails");
    assert!(e.message.contains("document service"), "{}", e.message);
    assert_eq!(state_of(&f, OWNER, DOC).await, before);
    assert_eq!(before["csv"][0], T1_EXACT);
}
