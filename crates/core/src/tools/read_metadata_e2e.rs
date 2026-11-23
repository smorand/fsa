//! SPEC-0104 end to end tests of the read tools of the document metadata: the
//! real `McpServer` tool methods and the real data plane router, on documents
//! converted by the SPEC-0103 stand in converters (`docs::manifest_e2e`).

use crate::docs::manifest_e2e::{
    Answer, BODY, DOC, docx_bytes, entry, fixture, http, leak, listing, numbered, ocr_of, old_set,
    pics, program,
};
use crate::mcp::server::{
    DocumentViewArgs, GetDocumentStructureArgs, GetImageArgs, ListImagesArgs, McpServer,
};
use crate::tools::admin::test_support::Fixture;
use crate::tools::archive_testkit::{MOUNT, OWNER, seed};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value, json};

const REVENUE: &str = "Revenue 2024: 4.2 M EUR";

fn text_of(r: &CallToolResult) -> String {
    r.content
        .iter()
        .find_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .expect("a text content block")
}

fn ok_json(r: Result<CallToolResult, rmcp::ErrorData>) -> Value {
    let r = r.expect("a tool result, not a protocol error");
    assert_ne!(r.is_error, Some(true), "expected success, got: {}", text_of(&r));
    serde_json::from_str(&text_of(&r)).expect("the tool result is JSON")
}

fn err_text(r: Result<CallToolResult, rmcp::ErrorData>) -> String {
    let r = r.expect("a tool result, not a protocol error");
    assert_eq!(r.is_error, Some(true), "expected an error, got: {}", text_of(&r));
    text_of(&r)
}

/// Document D of the spec: two pictures, the first with OCR text.
fn document_d() -> Answer {
    let entries = format!(
        r#""p1.png":{{"caption":"Figure 1: Revenue 2024","page":1,"ocr_text":"{REVENUE}"}},"p2.png":{{"caption":"Schéma réseau 🌐","page":3}}"#
    );
    pics(2, &entries)
}

/// Convert `answer` at `doc` through the program form converter.
async fn convert_at(f: &Fixture, a: &Answer, doc: &str) {
    seed(f, doc, b"%PDF-1.4").await;
    let home = tempfile::tempdir().unwrap();
    let svc = program(home.path(), a);
    f.state.authorize(MOUNT, OWNER).await.unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    crate::core::fs_ops::documentize(&c, &f.state.safety, Some(&svc), OWNER, MOUNT, doc, true)
        .await
        .expect("the conversion succeeds");
}

/// Like `convert_at` through the HTTP form, which returns the Markdown byte for
/// byte (the program form's here document adds a final line break).
async fn convert_exact(f: &Fixture, a: &Answer, doc: &str) {
    seed(f, doc, b"%PDF-1.4").await;
    let svc = http(a).await.expect("a JSON manifest");
    f.state.authorize(MOUNT, OWNER).await.unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    crate::core::fs_ops::documentize(&c, &f.state.safety, Some(&svc), OWNER, MOUNT, doc, true)
        .await
        .expect("the conversion succeeds");
}

/// A fixture holding D converted at `/report.pdf`, and Alice's tool server.
async fn world() -> (Fixture, McpServer) {
    let f = fixture().await;
    convert_at(&f, &document_d(), DOC).await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    (f, server)
}

async fn list(server: &McpServer, path: &str, marker: Option<&str>) -> Value {
    let a = ListImagesArgs {
        mount_id: MOUNT.into(),
        path: path.into(),
        marker: marker.map(str::to_string),
    };
    ok_json(server.fs_list_images(Parameters(a)).await)
}

fn get_args(path: &str, id: &str) -> GetImageArgs {
    GetImageArgs { mount_id: MOUNT.into(), path: path.into(), id: id.into() }
}

async fn get(server: &McpServer, path: &str, id: &str) -> Value {
    ok_json(server.fs_get_image(Parameters(get_args(path, id))).await)
}

/// `GET <uri>` through the real router, as the owner: status, headers, body.
async fn web_get(
    f: &Fixture,
    uri: &str,
) -> (axum::http::StatusCode, axum::http::HeaderMap, axum::body::Bytes) {
    web_get_as(f, OWNER, uri).await
}

/// Like `web_get` for any person (a token is minted for `who`).
async fn web_get_as(
    f: &Fixture,
    who: &str,
    uri: &str,
) -> (axum::http::StatusCode, axum::http::HeaderMap, axum::body::Bytes) {
    use tower::ServiceExt as _;
    let keys = tempfile::tempdir().unwrap();
    let (key, public) = crate::keys::write_keypair(keys.path()).unwrap();
    let mut config = (*f.state.config).clone();
    config.auth.jwt.public_key_path = public.display().to_string();
    let s = &f.state;
    let state = crate::state::AppState {
        config: std::sync::Arc::new(config.clone()),
        admin: s.admin.clone(),
        stores: s.stores.clone(),
        safety: s.safety.clone(),
        identity: std::sync::Arc::new(crate::identity::IdentityResolver::new(&config.auth)),
        editors: s.editors.clone(),
        doc_service: None,
        search: None,
    };
    let token = crate::keys::mint_token_from_file(
        &key,
        who,
        crate::keys::DEFAULT_ISSUER,
        crate::keys::DEFAULT_CLAIM,
        3600,
    )
    .unwrap();
    let req = axum::http::Request::builder()
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp =
        crate::api::dataplane::router(std::sync::Arc::new(state)).oneshot(req).await.unwrap();
    let (status, headers) = (resp.status(), resp.headers().clone());
    (status, headers, axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap())
}

/// `GET /api/fs/{mount}/images?path=` through the real router, as the owner.
async fn web_list(f: &Fixture, path: &str) -> Value {
    let (status, _, bytes) = web_get(f, &format!("/api/fs/{MOUNT}/images?path={path}")).await;
    assert_eq!(status.as_u16(), 200);
    serde_json::from_slice(&bytes).unwrap()
}

/// `GET /api/fs/{mount}/image-file?path=&id=`: the picture file download.
async fn download(
    f: &Fixture,
    path: &str,
    id: &str,
) -> (axum::http::StatusCode, axum::http::HeaderMap, axum::body::Bytes) {
    web_get(f, &format!("/api/fs/{MOUNT}/image-file?path={path}&id={id}")).await
}

/// The file the stand in converter writes for `rep:<n>`: n bytes of `x`.
fn supplied(n: usize) -> Vec<u8> {
    vec![b'x'; n]
}

/// Converted at DOC: `files` are `(name, size)` pictures, none with a manifest entry.
async fn world_with(files: &[(&'static str, usize)]) -> Fixture {
    let f = fixture().await;
    let markdown = leak(files.iter().map(|(n, _)| format!("![]({n})\n")).collect());
    let files = files.iter().map(|(n, s)| (*n, leak(format!("rep:{s}")))).collect();
    let a = Answer { markdown, files, manifest: Some(r#"{"version":2,"pictures":{}}"#) };
    convert_at(&f, &a, DOC).await;
    f
}

fn header<'a>(h: &'a axum::http::HeaderMap, name: &str) -> &'a str {
    h.get(name).unwrap_or_else(|| panic!("header {name}")).to_str().unwrap()
}

/// A refusal through the web interface: status, JSON body, no picture byte.
fn refused(r: (axum::http::StatusCode, axum::http::HeaderMap, axum::body::Bytes), detail: &str) {
    let (status, headers, body) = r;
    assert_eq!(status.as_u16(), 404, "{}", String::from_utf8_lossy(&body));
    assert_eq!(header(&headers, "content-type"), "application/json");
    let v: Value = serde_json::from_slice(&body).expect("a JSON error body");
    assert_eq!(v, json!({"error": "ERR_NOT_FOUND", "detail": detail}));
}

/// SPEC-0104/E2E-002: the image list carries the OCR text (FR-MOD-001).
#[tokio::test]
async fn e2e_0104_002_the_image_list_carries_the_ocr_text() {
    let (_f, server) = world().await;
    assert_eq!(
        list(&server, DOC, None).await,
        json!({"images": [
            {"id": "image-1", "caption": "Figure 1: Revenue 2024", "page": 1, "ocr_text": REVENUE},
            {"id": "image-2", "caption": "Schéma réseau 🌐", "page": 3, "ocr_text": null},
        ], "marker": null})
    );
}

/// SPEC-0104/E2E-003: one image carries the OCR text (FR-MOD-002).
#[tokio::test]
async fn e2e_0104_003_one_image_carries_the_ocr_text() {
    let (_f, server) = world().await;
    let v = get(&server, DOC, "image-1").await;
    assert_eq!(v["id"], "image-1");
    assert_eq!(v["format"], "png");
    assert_eq!(v["caption"], "Figure 1: Revenue 2024");
    assert_eq!(v["page"], 1);
    assert_eq!(v["ocr_text"], REVENUE);
    assert!(v["base64"].as_str().is_some_and(|b| !b.is_empty()));
}

/// SPEC-0104/E2E-021: an unknown image is refused (FR-MOD-002).
#[tokio::test]
async fn e2e_0104_021_get_an_unknown_image() {
    let (_f, server) = world().await;
    let e = err_text(server.fs_get_image(Parameters(get_args(DOC, "image-9"))).await);
    assert!(e.contains("ERR_NOT_FOUND") && e.contains("Not found: image-9"), "got: {e}");
}

/// SPEC-0104/E2E-025: an image with no OCR text reads `null` (FR-MOD-001, 002).
#[tokio::test]
async fn e2e_0104_025_an_image_with_no_ocr_text() {
    let (f, server) = world().await;
    let v = get(&server, DOC, "image-2").await;
    assert_eq!(v.get("ocr_text"), Some(&Value::Null));
    assert_eq!(v["caption"], "Schéma réseau 🌐");
    assert_eq!(v["page"], 3);
    // An empty or absent text is never kept, so never an empty string.
    let entries = format!(r#"{},"p2.png":{{"ocr_text":null}}"#, ocr_of(1, ""));
    convert_at(&f, &pics(2, leak(entries)), "/empty.pdf").await;
    for id in ["image-1", "image-2"] {
        assert_eq!(get(&server, "/empty.pdf", id).await.get("ocr_text"), Some(&Value::Null));
    }
}

/// SPEC-0104/E2E-031: exactly 100 and 101 images (FR-MOD-001).
#[tokio::test]
async fn e2e_0104_031_exactly_100_and_101_images() {
    let f = fixture().await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    for (doc, n) in [("/h100.pdf", 100), ("/h101.pdf", 101)] {
        let entries: Vec<String> = (1..=n).map(|i| ocr_of(i, &format!("t{i}"))).collect();
        convert_at(&f, &pics(n, leak(entries.join(","))), doc).await;
        let page = list(&server, doc, None).await;
        let images = page["images"].as_array().unwrap();
        assert_eq!(images.len(), 100, "{doc}");
        assert_eq!(images[99]["ocr_text"], "t100");
        if n == 100 {
            assert_eq!(page["marker"], Value::Null);
        } else {
            let marker = page["marker"].as_str().expect("a next page marker");
            let next = list(&server, doc, Some(marker)).await;
            assert_eq!(
                next,
                json!({"images": [
                    {"id": "image-101", "caption": "", "page": null, "ocr_text": "t101"}
                ], "marker": null})
            );
        }
    }
}

/// SPEC-0104/E2E-038: the web list and the tool list are identical (FR-MOD-001).
#[tokio::test]
async fn e2e_0104_038_the_image_list_through_the_web_interface() {
    let (f, server) = world().await;
    let web = web_list(&f, DOC).await;
    assert_eq!(web, list(&server, DOC, None).await);
    assert_eq!(web["images"][0]["ocr_text"], REVENUE);
}

/// SPEC-0104/E2E-048: OCR text made only of spaces is kept as stored (FR-MOD-001).
#[tokio::test]
async fn e2e_0104_048_ocr_text_made_only_of_spaces() {
    let f = fixture().await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    let entries = format!(r#"{},{}"#, ocr_of(1, "   "), ocr_of(2, ""));
    convert_at(&f, &pics(2, leak(entries)), DOC).await;
    let images = list(&server, DOC, None).await["images"].clone();
    assert_eq!(images[0]["ocr_text"], "   ");
    assert_eq!(images[1]["ocr_text"], Value::Null);
}

/// SPEC-0104/E2E-004: a PNG picture downloads byte for byte (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_004_download_a_png_picture() {
    let f = world_with(&[("p1.png", 2048), ("p2.jpg", 1500)]).await;
    let (status, headers, body) = download(&f, DOC, "image-1").await;
    assert_eq!(status.as_u16(), 200);
    assert_eq!(header(&headers, "content-type"), "image/png");
    assert_eq!(body.as_ref(), supplied(2048).as_slice());
}

/// SPEC-0104/E2E-005: a JPEG picture downloads byte for byte (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_005_download_a_jpeg_picture() {
    let f = world_with(&[("p1.png", 2048), ("p2.jpg", 1500)]).await;
    let (status, headers, body) = download(&f, DOC, "image-2").await;
    assert_eq!(status.as_u16(), 200);
    assert_eq!(header(&headers, "content-type"), "image/jpeg");
    assert_eq!(body.as_ref(), supplied(1500).as_slice());
}

/// SPEC-0104/E2E-012: an unknown image is refused, no byte delivered (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_012_download_an_unknown_image() {
    let (f, _server) = world().await;
    let e = crate::tools::artifacts::image_file(&f.state, MOUNT, DOC, "image-9").await;
    assert_eq!(e.unwrap_err().to_string(), "ERR_NOT_FOUND: Not found: image-9");
    refused(download(&f, DOC, "image-9").await, "ERR_NOT_FOUND: Not found: image-9");
}

/// SPEC-0104/E2E-013: a path that is not a file is refused (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_013_download_from_a_path_that_is_not_a_file() {
    let (f, _server) = world().await;
    f.state.authorize(MOUNT, OWNER).await.unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    crate::core::fs_ops::mkdir(&c, &f.state.safety, OWNER, MOUNT, "/docs", true, true)
        .await
        .unwrap();
    let e = crate::tools::artifacts::image_file(&f.state, MOUNT, "/docs", "image-1").await;
    assert_eq!(e.unwrap_err().to_string(), "ERR_NOT_FOUND: not a file: /docs");
    refused(download(&f, "/docs", "image-1").await, "ERR_NOT_FOUND: not a file: /docs");
}

/// SPEC-0104/E2E-014: a document never converted is refused (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_014_download_from_a_document_never_converted() {
    let (f, _server) = world().await;
    seed(&f, "/plain.pdf", b"%PDF-1.4").await;
    let want = "ERR_NOT_FOUND: No artifacts: document not extracted";
    let e = crate::tools::artifacts::image_file(&f.state, MOUNT, "/plain.pdf", "image-1").await;
    assert_eq!(e.unwrap_err().to_string(), want);
    refused(download(&f, "/plain.pdf", "image-1").await, want);
}

/// SPEC-0104/E2E-022: an id that is not canonical is refused as typed (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_022_download_with_an_id_that_is_not_canonical() {
    let (f, _server) = world().await;
    for id in ["image-01", "image-0", "picture-1"] {
        refused(download(&f, DOC, id).await, &format!("ERR_NOT_FOUND: Not found: {id}"));
    }
}

/// SPEC-0104/E2E-024: an image a re-conversion removed is refused (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_024_an_image_that_a_reconversion_removed() {
    let f = world_with(&[("p1.png", 2048), ("p2.jpg", 1500)]).await;
    assert_eq!(download(&f, DOC, "image-2").await.2.len(), 1500);
    let a = Answer {
        markdown: "![](q1.png)\n",
        files: vec![("q1.png", "rep:700")],
        manifest: Some(r#"{"version":2,"pictures":{}}"#),
    };
    convert_at(&f, &a, DOC).await;
    refused(download(&f, DOC, "image-2").await, "ERR_NOT_FOUND: Not found: image-2");
    let (status, _, body) = download(&f, DOC, "image-1").await;
    assert_eq!(status.as_u16(), 200);
    assert_eq!(body.as_ref(), supplied(700).as_slice());
}

/// SPEC-0104/E2E-032: a 5 000 000 byte picture downloads whole (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_032_a_large_picture_downloads_whole() {
    let f = world_with(&[("big.png", 5_000_000)]).await;
    let (status, headers, body) = download(&f, DOC, "image-1").await;
    assert_eq!(status.as_u16(), 200);
    assert_eq!(header(&headers, "content-length"), "5000000");
    assert!(body.as_ref() == supplied(5_000_000).as_slice());
}

/// SPEC-0104/E2E-040: two downloads give the same bytes (FR-NEW-001, 009).
#[tokio::test]
async fn e2e_0104_040_downloading_twice_gives_the_same_bytes() {
    let f = world_with(&[("p1.png", 2048)]).await;
    let first = download(&f, DOC, "image-1").await.2;
    let second = download(&f, DOC, "image-1").await.2;
    assert_eq!(first, second);
    assert_eq!(first.as_ref(), supplied(2048).as_slice());
}

/// SPEC-0104/E2E-041: every format is labelled with its own media type (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_041_each_format_is_labelled_with_its_media_type() {
    let names = ["a.tif", "b.gif", "c.webp", "d.bmp", "e.png", "f.jpeg"];
    let sizes: Vec<_> = names.iter().zip([10, 20, 30, 40, 50, 60]).map(|(n, s)| (*n, s)).collect();
    let f = world_with(&sizes).await;
    let want = ["tiff", "gif", "webp", "bmp", "png", "jpeg"];
    for (i, kind) in want.iter().enumerate() {
        let (status, headers, body) = download(&f, DOC, &format!("image-{}", i + 1)).await;
        assert_eq!(status.as_u16(), 200);
        assert_eq!(header(&headers, "content-type"), format!("image/{kind}"));
        assert_eq!(body.as_ref(), supplied(sizes[i].1).as_slice());
    }
}

/// SPEC-0104/E2E-047: a refused download is a JSON error, no picture byte (FR-NEW-001, 008).
#[tokio::test]
async fn e2e_0104_047_a_refused_download_through_the_web_interface() {
    let (f, _server) = world().await;
    refused(download(&f, DOC, "image-9").await, "ERR_NOT_FOUND: Not found: image-9");
}

/// SPEC-0104/E2E-051: the download headers (FR-NEW-001).
#[tokio::test]
async fn e2e_0104_051_download_headers() {
    let f = world_with(&[("p1.png", 2048), ("p2.jpg", 1500)]).await;
    let (_, h1, _) = download(&f, DOC, "image-1").await;
    assert_eq!(header(&h1, "content-disposition"), "attachment; filename=\"image-1.png\"");
    assert_eq!(header(&h1, "x-content-type-options"), "nosniff");
    assert_eq!(header(&h1, "content-length"), "2048");
    let (_, h2, _) = download(&f, DOC, "image-2").await;
    assert_eq!(header(&h2, "content-disposition"), "attachment; filename=\"image-2.jpg\"");
    assert_eq!(header(&h2, "content-length"), "1500");
}

fn structure_args(path: &str, marker: Option<&str>) -> GetDocumentStructureArgs {
    GetDocumentStructureArgs {
        mount_id: MOUNT.into(),
        path: path.into(),
        marker: marker.map(str::to_string),
    }
}

async fn structure(server: &McpServer, path: &str, marker: Option<&str>) -> Value {
    ok_json(server.fs_get_document_structure(Parameters(structure_args(path, marker))).await)
}

async fn structure_err(server: &McpServer, path: &str, marker: Option<&str>) -> String {
    err_text(server.fs_get_document_structure(Parameters(structure_args(path, marker))).await)
}

/// `/report.pdf` converted with a version 2 manifest listing `entries` as headings.
async fn with_headings(entries: &[String]) -> (Fixture, McpServer) {
    let f = fixture().await;
    let a = Answer { markdown: BODY, files: vec![], manifest: Some(listing(2, entries)) };
    convert_at(&f, &a, DOC).await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    (f, server)
}

fn titled(page: &Value) -> Vec<String> {
    page["headings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["title"].as_str().unwrap().into())
        .collect()
}

/// SPEC-0104/E2E-006: the structure in reading order (FR-NEW-002).
#[tokio::test]
async fn e2e_0104_006_the_structure_in_reading_order() {
    let m = |l: u8, t: &str, p: u8| format!(r#"{{"level":{l},"title":"{t}","page":{p}}}"#);
    let (_f, server) =
        with_headings(&[m(1, "Report", 1), m(2, "Costs", 2), m(2, "Outlook", 3)]).await;
    assert_eq!(
        structure(&server, DOC, None).await,
        json!({"headings": [
            {"level": 1, "title": "Report", "page": 1},
            {"level": 2, "title": "Costs", "page": 2},
            {"level": 2, "title": "Outlook", "page": 3},
        ], "marker": null})
    );
}

/// SPEC-0104/E2E-007: the structure is read 100 headings at a time (FR-NEW-002).
#[tokio::test]
async fn e2e_0104_007_the_structure_is_read_100_at_a_time() {
    let entries: Vec<String> = (1..=150).map(|i| entry(1, &format!("Section {i}"))).collect();
    let (_f, server) = with_headings(&entries).await;
    let first = structure(&server, DOC, None).await;
    let want = |r: std::ops::RangeInclusive<usize>| -> Vec<String> {
        r.map(|i| format!("Section {i}")).collect()
    };
    assert_eq!(titled(&first), want(1..=100));
    let marker = first["marker"].as_str().expect("a next page marker");
    let second = structure(&server, DOC, Some(marker)).await;
    assert_eq!(titled(&second), want(101..=150));
    assert_eq!(second["marker"], Value::Null);
}

/// SPEC-0104/E2E-015: the structure of a document never converted (FR-NEW-003).
#[tokio::test]
async fn e2e_0104_015_structure_of_a_document_never_converted() {
    let (f, server) = world().await;
    seed(&f, "/plain.pdf", b"%PDF-1.4").await;
    let e = structure_err(&server, "/plain.pdf", None).await;
    assert!(
        e.contains("ERR_NOT_FOUND") && e.contains("No artifacts: document not extracted"),
        "got: {e}"
    );
}

/// SPEC-0104/E2E-016: the structure of a path that is not a file (FR-NEW-003).
#[tokio::test]
async fn e2e_0104_016_structure_of_a_path_that_is_not_a_file() {
    let (f, server) = world().await;
    seed(&f, "/docs/a.txt", b"x").await;
    let e = structure_err(&server, "/docs", None).await;
    assert!(e.contains("ERR_NOT_FOUND") && e.contains("not a file: /docs"), "got: {e}");
}

/// SPEC-0104/E2E-017: a marker that is not valid (FR-NEW-002, 003).
#[tokio::test]
async fn e2e_0104_017_structure_with_an_invalid_marker() {
    let (_f, server) = with_headings(&[entry(1, "A")]).await;
    let e = structure_err(&server, DOC, Some("garbage")).await;
    assert!(
        e.contains("ERR_INVALID_ARGUMENT") && e.contains("Invalid continuation marker: garbage"),
        "got: {e}"
    );
    assert!(!e.contains("\"headings\""), "no heading is returned: {e}");
}

/// SPEC-0104/E2E-034: a heading without a page reads `null` (FR-NEW-002).
#[tokio::test]
async fn e2e_0104_034_a_heading_without_a_page() {
    let (_f, server) = with_headings(&[entry(2, "Appendix")]).await;
    assert_eq!(
        structure(&server, DOC, None).await,
        json!({"headings": [{"level": 2, "title": "Appendix", "page": null}], "marker": null})
    );
}

/// SPEC-0104/E2E-045: a marker from another read is refused (FR-NEW-002, 003).
#[tokio::test]
async fn e2e_0104_045_a_marker_from_another_read() {
    let (f, server) = with_headings(&[entry(1, "A")]).await;
    let entries: Vec<String> = (1..=150).map(|i| ocr_of(i, &format!("t{i}"))).collect();
    convert_at(&f, &pics(150, leak(entries.join(","))), "/pics.pdf").await;
    let page = list(&server, "/pics.pdf", None).await;
    let marker = page["marker"].as_str().expect("an image marker").to_string();
    let e = structure_err(&server, DOC, Some(&marker)).await;
    assert!(
        e.contains("ERR_INVALID_ARGUMENT")
            && e.contains(&format!("Invalid continuation marker: {marker}")),
        "got: {e}"
    );
    assert!(!e.contains("\"headings\""), "no heading is returned: {e}");
}

/// SPEC-0104/E2E-046: 10 000 headings read page by page (FR-NEW-002).
#[tokio::test]
async fn e2e_0104_046_ten_thousand_headings_page_by_page() {
    let (f, server) = with_headings(&numbered(10_000)).await;
    let (mut got, mut marker, mut answers) = (Vec::new(), None::<String>, 0);
    loop {
        let page = structure(&server, DOC, marker.as_deref()).await;
        answers += 1;
        assert_eq!(page["headings"].as_array().unwrap().len(), 100);
        got.extend(titled(&page));
        marker = page["marker"].as_str().map(str::to_string);
        if marker.is_none() {
            break;
        }
    }
    assert_eq!(answers, 100);
    let want: Vec<String> = (1..=10_000).map(|i| format!("H{i}")).collect();
    assert_eq!(got, want);
    // The full view carries the whole structure, past the page of 100.
    let full = view(&server, DOC, "full", None, true).await;
    assert_eq!(full["structure"].as_array().unwrap().len(), 10_000);
    let _ = f;
}

// SPEC-0104 US-0299: the views of a converted document.

const TOTAL: &str = "[Image image-1: Figure 1: Revenue 2024]";
const REPORT_MD: &str = "# Report\n\nIntro\n\n![](p1.png)\n\nFigure 1: Revenue 2024\n\n## Costs\n\nTable 1: Costs\n\n| Item | EUR |\n| --- | --- |\n| Rent | 900 |\n\n![](p2.png)\n\n## Outlook\n\nEnd\n";

/// A report with headings, two pictures (the first read by OCR) and one table.
fn report() -> Answer {
    let manifest = leak(format!(
        r#"{{"version":2,"headings":[{{"level":1,"title":"Report","page":1}},{{"level":2,"title":"Costs","page":2}},{{"level":2,"title":"Outlook","page":3}}],"pictures":{{"p1.png":{{"caption":"Figure 1: Revenue 2024","page":1,"ocr_text":"{REVENUE}"}},"p2.png":{{"page":3}}}}}}"#
    ));
    Answer {
        markdown: REPORT_MD,
        files: vec![("p1.png", "PNG"), ("p2.png", "PNG")],
        manifest: Some(manifest),
    }
}

async fn report_world() -> (Fixture, McpServer) {
    let f = fixture().await;
    convert_exact(&f, &report(), DOC).await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    (f, server)
}

fn view_args(
    path: &str,
    view: Option<&str>,
    mode: Option<&str>,
    captions: bool,
) -> DocumentViewArgs {
    DocumentViewArgs {
        mount_id: MOUNT.into(),
        path: path.into(),
        captions,
        table_mode: mode.map(str::to_string),
        view: view.map(str::to_string),
    }
}

async fn view_in(
    server: &McpServer,
    path: &str,
    v: Option<&str>,
    m: Option<&str>,
    c: bool,
) -> Value {
    ok_json(server.fs_get_document_view(Parameters(view_args(path, v, m, c))).await)
}

async fn view(server: &McpServer, path: &str, v: &str, m: Option<&str>, c: bool) -> Value {
    view_in(server, path, Some(v), m, c).await
}

async fn view_err(server: &McpServer, v: Option<&str>, m: Option<&str>) -> String {
    err_text(server.fs_get_document_view(Parameters(view_args(DOC, v, m, true))).await)
}

fn text(v: &Value) -> String {
    v["text"].as_str().expect("a text").to_string()
}

/// SPEC-0104/E2E-008: the text only view (FR-NEW-004, 005).
#[tokio::test]
async fn e2e_0104_008_the_text_only_view() {
    let (_f, server) = report_world().await;
    let v = view(&server, DOC, "text", None, true).await;
    assert_eq!(v, json!({"path": DOC, "text": REPORT_MD}));
    assert!(!text(&v).contains("[Image") && !text(&v).contains("[OCR"));
}

/// SPEC-0104/E2E-009: the full view (FR-NEW-004, 006).
#[tokio::test]
async fn e2e_0104_009_the_full_view() {
    let (_f, server) = report_world().await;
    let std = text(&view(&server, DOC, "standard", None, true).await);
    let v = view(&server, DOC, "full", None, true).await;
    let ocr = format!("[OCR image-1: {REVENUE}]");
    assert_eq!(text(&v), std.replacen(&format!("{TOTAL}\n"), &format!("{TOTAL}\n{ocr}\n"), 1));
    assert!(!text(&v).contains("[OCR image-2"));
    assert_eq!(
        v["structure"],
        json!([
            {"level": 1, "title": "Report", "page": 1},
            {"level": 2, "title": "Costs", "page": 2},
            {"level": 2, "title": "Outlook", "page": 3},
        ])
    );
}

/// SPEC-0104/E2E-010: the default view is unchanged (FR-MOD-003, FR-NEW-004).
#[tokio::test]
async fn e2e_0104_010_the_default_view_is_unchanged() {
    let (_f, server) = report_world().await;
    let none = view_in(&server, DOC, None, None, true).await;
    let std = view(&server, DOC, "standard", None, true).await;
    assert_eq!(none, std);
    assert_eq!(std.as_object().unwrap().len(), 2, "path and text only: {std}");
    let t = text(&std);
    assert!(t.contains(TOTAL) && t.contains("| Rent | 900 |") && !t.contains("[OCR"));
}

/// SPEC-0104/E2E-018: an unknown view (FR-NEW-004).
#[tokio::test]
async fn e2e_0104_018_an_unknown_view() {
    let (_f, server) = report_world().await;
    let e = view_err(&server, Some("pretty"), None).await;
    assert!(
        e.contains("ERR_INVALID_ARGUMENT")
            && e.contains("Unsupported view: pretty (use standard, text or full)"),
        "got: {e}"
    );
}

/// SPEC-0104/E2E-019: an unknown table mode with the full view (FR-NEW-004).
#[tokio::test]
async fn e2e_0104_019_an_unknown_table_mode_with_the_full_view() {
    let (_f, server) = report_world().await;
    let e = view_err(&server, Some("full"), Some("grid")).await;
    assert!(
        e.contains("ERR_INVALID_ARGUMENT")
            && e.contains("Unsupported table mode: grid (use markdown, csv-reference or both)"),
        "got: {e}"
    );
}

/// SPEC-0104/E2E-020: the view name is case sensitive (FR-NEW-004).
#[tokio::test]
async fn e2e_0104_020_the_view_name_is_case_sensitive() {
    let (_f, server) = report_world().await;
    let e = view_err(&server, Some("Full"), None).await;
    assert!(e.contains("Unsupported view: Full (use standard, text or full)"), "got: {e}");
}

/// SPEC-0104/E2E-027: accents, emoji and line breaks in OCR text (FR-NEW-006).
#[tokio::test]
async fn e2e_0104_027_accents_emoji_and_line_breaks_in_ocr_text() {
    let f = fixture().await;
    let a = pics(1, r#""p1.png":{"ocr_text":"Café 🌐\nligne 2"}"#);
    convert_exact(&f, &a, DOC).await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    assert_eq!(list(&server, DOC, None).await["images"][0]["ocr_text"], "Café 🌐\nligne 2");
    let t = text(&view(&server, DOC, "full", None, true).await);
    assert_eq!(t, "[Image image-1]\n[OCR image-1: Café 🌐\nligne 2]\n");
}

/// SPEC-0104/E2E-028: captions hidden, OCR text still shown (FR-NEW-006).
#[tokio::test]
async fn e2e_0104_028_captions_hidden_ocr_text_still_shown() {
    let (_f, server) = report_world().await;
    let t = text(&view(&server, DOC, "full", None, false).await);
    assert!(t.contains(&format!("[Image image-1]\n[OCR image-1: {REVENUE}]\n")), "got: {t}");
    assert!(!t.contains(TOTAL));
}

/// SPEC-0104/E2E-029: the full view of a document with no headings (FR-NEW-006, 003).
#[tokio::test]
async fn e2e_0104_029_the_full_view_of_a_document_with_no_headings() {
    let f = fixture().await;
    convert_exact(&f, &Answer { markdown: BODY, files: vec![], manifest: None }, DOC).await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    let v = view(&server, DOC, "full", None, true).await;
    assert_eq!(v, json!({"path": DOC, "text": BODY, "structure": []}));
    assert_eq!(structure(&server, DOC, None).await, json!({"headings": [], "marker": null}));
}

/// SPEC-0104/E2E-033: a table mode is ignored by the text view (FR-NEW-005).
#[tokio::test]
async fn e2e_0104_033_a_table_mode_is_ignored_by_the_text_view() {
    let (_f, server) = report_world().await;
    let plain = view(&server, DOC, "text", None, true).await;
    assert_eq!(view(&server, DOC, "text", Some("csv-reference"), true).await, plain);
}

/// SPEC-0104/E2E-042: tables in the full view follow the chosen mode (FR-NEW-006).
#[tokio::test]
async fn e2e_0104_042_tables_in_the_full_view_follow_the_chosen_mode() {
    let (_f, server) = report_world().await;
    let v = view(&server, DOC, "full", Some("csv-reference"), true).await;
    let t = text(&v);
    assert!(t.contains("[Table table-1: Costs (CSV)]") && !t.contains("| Rent |"), "got: {t}");
    assert!(t.contains(&format!("[OCR image-1: {REVENUE}]")));
    assert_eq!(v["structure"].as_array().unwrap().len(), 3);
}

/// SPEC-0104/E2E-043: a table mode is checked whatever the view (FR-NEW-004).
#[tokio::test]
async fn e2e_0104_043_a_table_mode_is_checked_whatever_the_view() {
    let (_f, server) = report_world().await;
    let e = view_err(&server, Some("text"), Some("grid")).await;
    assert!(e.contains("Unsupported table mode: grid (use markdown, csv-reference or both)"));
}

/// SPEC-0104/E2E-049: the OCR line when the image line ends the text (FR-NEW-006).
#[tokio::test]
async fn e2e_0104_049_the_ocr_line_when_the_image_line_ends_the_text() {
    let f = fixture().await;
    let mut a = pics(1, r#""p1.png":{"ocr_text":"fin"}"#);
    a.markdown = "![](p1.png)";
    convert_exact(&f, &a, DOC).await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    let t = text(&view(&server, DOC, "full", None, true).await);
    assert_eq!(t, "[Image image-1]\n[OCR image-1: fin]");
}

/// SPEC-0104/E2E-050: two adjacent images (FR-NEW-006).
#[tokio::test]
async fn e2e_0104_050_two_adjacent_images() {
    let f = fixture().await;
    let a = pics(2, &format!("{},{}", ocr_of(1, "a"), ocr_of(2, "b")));
    convert_exact(&f, &a, DOC).await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    let t = text(&view(&server, DOC, "full", None, true).await);
    assert_eq!(t, "[Image image-1]\n[OCR image-1: a]\n\n[Image image-2]\n[OCR image-2: b]\n");
}

/// The web answer as JSON, whatever the status.
async fn web_json(f: &Fixture, who: &str, uri: &str) -> (u16, Value) {
    let (status, _, body) = web_get_as(f, who, uri).await;
    (status.as_u16(), serde_json::from_slice(&body).expect("a JSON body"))
}

/// SPEC-0104/E2E-036: the structure through the web interface (FR-NEW-008).
#[tokio::test]
async fn e2e_0104_036_the_structure_through_the_web_interface() {
    let (f, server) = report_world().await;
    let (status, web) = web_json(&f, OWNER, &format!("/api/fs/{MOUNT}/structure?path={DOC}")).await;
    assert_eq!(status, 200, "{web}");
    assert_eq!(web, structure(&server, DOC, None).await);
    assert_eq!(web["headings"].as_array().unwrap().len(), 3);
}

/// SPEC-0104/E2E-037: every view through the web interface (FR-NEW-008).
#[tokio::test]
async fn e2e_0104_037_every_view_through_the_web_interface() {
    let (f, server) = report_world().await;
    for v in ["standard", "text", "full"] {
        let uri = format!("/api/fs/{MOUNT}/document-view?path={DOC}&view={v}");
        let (status, web) = web_json(&f, OWNER, &uri).await;
        assert_eq!(status, 200, "{web}");
        assert_eq!(web, view(&server, DOC, v, None, true).await, "view {v}");
    }
    let (status, web) =
        web_json(&f, OWNER, &format!("/api/fs/{MOUNT}/document-view?path={DOC}&view=pretty")).await;
    let tool = view_err(&server, Some("pretty"), None).await;
    assert_eq!(status, 400);
    assert_eq!(web["error"], "ERR_INVALID_ARGUMENT");
    assert!(tool.contains(web["detail"].as_str().unwrap()), "{tool} vs {web}");
}

/// SPEC-0104/E2E-044: a non member through the web interface (FR-NEW-008, 009).
#[tokio::test]
async fn e2e_0104_044_a_non_member_through_the_web_interface() {
    let (f, _server) = world().await;
    let eve = "eve@example.com";
    let want = json!({
        "error": "ERR_FORBIDDEN",
        "detail": format!("ERR_FORBIDDEN: 'eve@example.com' is not a member of '{MOUNT}'"),
    });
    for sub in ["images?path=", "structure?path=", "image-file?id=image-1&path="] {
        let (status, headers, body) =
            web_get_as(&f, eve, &format!("/api/fs/{MOUNT}/{sub}{DOC}")).await;
        assert_eq!(status.as_u16(), 403, "{sub}");
        assert_eq!(header(&headers, "content-type"), "application/json", "{sub}");
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), want, "{sub}");
    }
}

/// Everything a read must leave alone: the file list, the access time of `doc`,
/// the used storage and what is read back of the conversion.
async fn footprint(f: &Fixture, server: &McpServer, doc: &str) -> Value {
    f.state.authorize(MOUNT, OWNER).await.unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let files = crate::core::fs_ops::glob_files(&c, "/", "*", &[]).await.unwrap();
    let stat = crate::core::fs_ops::stat_info(&c, doc).await.unwrap();
    let store = c.trash.as_deref().unwrap();
    json!({
        "files": files,
        "atime": stat["atime"],
        "used": crate::quota::used_bytes(&c).await.unwrap(),
        "images": list(server, doc, None).await,
        "headings": store.current_headings_for_test(doc).await.unwrap(),
        "ocr": store.current_ocr_for_test(doc).await.unwrap(),
    })
}

/// SPEC-0104/E2E-001: a converted document read end to end (FR-NEW-007, 009).
#[tokio::test]
async fn e2e_0104_001_a_converted_document_read_end_to_end() {
    let f = fixture().await;
    convert_exact(&f, &report(), DOC).await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    let before = footprint(&f, &server, DOC).await;
    let l = list(&server, DOC, None).await;
    assert_eq!(l["images"][0]["ocr_text"], REVENUE);
    assert_eq!(l["images"][1]["ocr_text"], Value::Null);
    let one = get(&server, DOC, "image-1").await;
    assert_eq!(one["ocr_text"], REVENUE);
    assert!(one["base64"].as_str().is_some_and(|b| !b.is_empty()));
    let (status, _, bytes) = download(&f, DOC, "image-1").await;
    assert_eq!(status.as_u16(), 200);
    assert_eq!(&bytes[..], b"PNG");
    assert_eq!(titled(&structure(&server, DOC, None).await), ["Report", "Costs", "Outlook"]);
    let t = text(&view(&server, DOC, "full", None, true).await);
    assert!(t.contains(&format!("{TOTAL}\n[OCR image-1: {REVENUE}]\n")), "got: {t}");
    assert_eq!(footprint(&f, &server, DOC).await, before);
}

/// SPEC-0104/E2E-011: a non member reads nothing (FR-NEW-009).
#[tokio::test]
async fn e2e_0104_011_a_non_member_reads_nothing() {
    let (f, _alice) = report_world().await;
    let eve = McpServer::new(f.state.clone(), "eve@example.com".to_string());
    let want = format!("ERR_FORBIDDEN: 'eve@example.com' is not a member of '{MOUNT}'");
    let leaks = |e: &str| {
        assert!(e.contains(&want), "got: {e}");
        for secret in [REVENUE, "Costs", "Outlook", "PNG", "base64"] {
            assert!(!e.contains(secret), "{secret} leaked: {e}");
        }
    };
    let l = ListImagesArgs { mount_id: MOUNT.into(), path: DOC.into(), marker: None };
    leaks(&err_text(eve.fs_list_images(Parameters(l)).await));
    leaks(&err_text(eve.fs_get_image(Parameters(get_args(DOC, "image-1"))).await));
    leaks(&err_text(eve.fs_get_document_structure(Parameters(structure_args(DOC, None))).await));
    leaks(&err_text(
        eve.fs_get_document_view(Parameters(view_args(DOC, Some("full"), None, true))).await,
    ));
    let (status, _, body) = web_get_as(
        &f,
        "eve@example.com",
        &format!("/api/fs/{MOUNT}/image-file?path={DOC}&id=image-1"),
    )
    .await;
    assert_eq!(status.as_u16(), 403);
    assert!(!body.starts_with(b"PNG"));
}

/// SPEC-0104/E2E-023: an unknown project (FR-NEW-009).
#[tokio::test]
async fn e2e_0104_023_an_unknown_project() {
    let (f, server) = world().await;
    let want = "ERR_PROJECT_NOT_FOUND: project 'ghost' not found";
    let a = GetDocumentStructureArgs { mount_id: "ghost".into(), path: DOC.into(), marker: None };
    let e = err_text(server.fs_get_document_structure(Parameters(a)).await);
    assert!(e.contains(want), "got: {e}");
    let (status, _, body) =
        web_get(&f, &format!("/api/fs/ghost/image-file?path={DOC}&id=image-1")).await;
    assert_eq!(status.as_u16(), 404);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "ERR_PROJECT_NOT_FOUND");
    assert!(v["detail"].as_str().unwrap().contains(want), "{v}");
}

/// SPEC-0104/E2E-026: a document converted before the metadata existed (FR-NEW-007).
#[tokio::test]
async fn e2e_0104_026_a_document_converted_before_the_metadata_existed() {
    let f = fixture().await;
    seed(&f, "/old.pdf", b"%PDF-1.4").await;
    old_set(&f, "/old.pdf", "Old figure").await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    let before = footprint(&f, &server, "/old.pdf").await;
    let l = list(&server, "/old.pdf", None).await;
    assert_eq!(l["images"][0]["caption"], "Old figure");
    assert_eq!(l["images"][0].get("ocr_text"), Some(&Value::Null));
    assert_eq!(structure(&server, "/old.pdf", None).await, json!({"headings": [], "marker": null}));
    let std = view(&server, "/old.pdf", "standard", None, true).await;
    let full = view(&server, "/old.pdf", "full", None, true).await;
    assert_eq!(full["text"], std["text"]);
    assert_eq!(full["structure"], json!([]));
    assert_eq!(footprint(&f, &server, "/old.pdf").await, before);
}

/// SPEC-0104/E2E-030: a new conversion replaces what is read (FR-NEW-002, 006).
#[tokio::test]
async fn e2e_0104_030_a_new_conversion_replaces_what_is_read() {
    let (f, server) = report_world().await;
    let manifest = r#"{"version":2,"headings":[{"level":1,"title":"Summary","page":1}],"pictures":{"p1.png":{"ocr_text":"Q3 chart"}}}"#;
    let a = Answer {
        markdown: "# Summary\n\n![](p1.png)\n",
        files: vec![("p1.png", "PNG")],
        manifest: Some(manifest),
    };
    convert_exact(&f, &a, DOC).await;
    let l = list(&server, DOC, None).await;
    assert_eq!(l["images"].as_array().unwrap().len(), 1);
    assert_eq!(l["images"][0]["ocr_text"], "Q3 chart");
    assert_eq!(titled(&structure(&server, DOC, None).await), ["Summary"]);
    let t = text(&view(&server, DOC, "full", None, true).await);
    assert!(t.contains("[OCR image-1: Q3 chart]"), "got: {t}");
    assert!(!t.contains(REVENUE) && !t.contains("Costs"), "got: {t}");
}

/// SPEC-0104/E2E-035: a document converted by the built in reading (FR-NEW-007, 003).
#[tokio::test]
async fn e2e_0104_035_a_document_converted_by_the_built_in_reading() {
    let f = fixture().await;
    seed(&f, "/doc.docx", &docx_bytes()).await;
    crate::tools::archive_testkit::extract_text(&f, OWNER, "/doc.docx", true).await.unwrap();
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    assert_eq!(
        structure(&server, "/doc.docx", None).await,
        json!({"headings": [], "marker": null})
    );
    let std = view(&server, "/doc.docx", "standard", None, true).await;
    let full = view(&server, "/doc.docx", "full", None, true).await;
    assert_eq!(full["text"], std["text"]);
    assert_eq!(full["structure"], json!([]));
}

/// SPEC-0104/E2E-039: a download changes nothing (FR-NEW-009).
#[tokio::test]
async fn e2e_0104_039_a_download_changes_nothing() {
    let (f, server) = world().await;
    let before = footprint(&f, &server, DOC).await;
    let (status, _, _) = download(&f, DOC, "image-1").await;
    assert_eq!(status.as_u16(), 200);
    structure(&server, DOC, None).await;
    assert_eq!(footprint(&f, &server, DOC).await, before);
}
