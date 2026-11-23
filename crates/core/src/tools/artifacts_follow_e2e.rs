//! SPEC-0105 end to end tests of "document artifacts follow their document":
//! the real `McpServer` tool methods on documents converted by the SPEC-0103
//! stand in converters (`docs::manifest_e2e`). Every move test runs the
//! artifact sweep and reads again: a set that kept an old path or revision
//! would pass the first read and vanish at the sweep.

use crate::docs::manifest_e2e::{Answer, Counting, DOC, fixture, leak, pics, program};
use crate::mcp::server::{
    CopyArgs, DocumentViewArgs, GetDocumentStructureArgs, ListImagesArgs, ListTablesArgs,
    McpServer, MkdirArgs, MoveArgs,
};
use crate::tools::admin::test_support::Fixture;
use crate::tools::archive_testkit::{MOUNT, OWNER, seed};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value, json};

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

/// Document D of the spec: two pictures (the first with OCR text), the table
/// `Costs`, the headings `Report` and `Costs`.
fn document_d() -> Answer {
    Answer {
        markdown: "# Report\n\n![](p1.png)\n\n![](p2.png)\n\n## Costs\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n",
        files: vec![("p1.png", "PNG"), ("p2.png", "PNG")],
        manifest: Some(
            r#"{"version":2,"pictures":{"p1.png":{"caption":"Figure 1","page":1,"ocr_text":"Revenue 4.2 M EUR"},"p2.png":{"caption":"Figure 2","page":2}},"tables":[{"caption":"Costs"}],"headings":[{"level":1,"title":"Report"},{"level":2,"title":"Costs"}]}"#,
        ),
    }
}

/// A document holding `n` pictures captioned `<prefix><k>`.
fn captioned(n: usize, prefix: &str) -> Answer {
    let entries: Vec<String> =
        (1..=n).map(|k| format!(r#""p{k}.png":{{"caption":"{prefix}{k}","page":1}}"#)).collect();
    let mut a = pics(n, &entries.join(","));
    a.markdown = leak(format!("# Title\n\n{}", a.markdown));
    a
}

/// A fixture (one member, Alice) and her tool server.
async fn world() -> (Fixture, McpServer) {
    let f = fixture().await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    (f, server)
}

/// Convert the document at `doc` through the converter behind `svc`.
async fn convert_via(f: &Fixture, svc: &dyn crate::docs::service::DocService, doc: &str) {
    seed(f, doc, b"%PDF-1.4").await;
    f.state.authorize(MOUNT, OWNER).await.unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    crate::core::fs_ops::documentize(&c, &f.state.safety, Some(svc), OWNER, MOUNT, doc, true)
        .await
        .expect("the conversion succeeds");
}

async fn convert_answer(f: &Fixture, a: &Answer, doc: &str) {
    let home = tempfile::tempdir().unwrap();
    let svc = program(home.path(), a);
    convert_via(f, &svc, doc).await;
}

/// Everything Alice can read of the artifacts at `path`.
async fn snapshot(server: &McpServer, path: &str) -> Value {
    let images = ok_json(
        server
            .fs_list_images(Parameters(ListImagesArgs {
                mount_id: MOUNT.into(),
                path: path.into(),
                marker: None,
            }))
            .await,
    );
    let tables = ok_json(
        server
            .fs_list_tables(Parameters(ListTablesArgs {
                mount_id: MOUNT.into(),
                path: path.into(),
                marker: None,
            }))
            .await,
    );
    let structure = ok_json(
        server
            .fs_get_document_structure(Parameters(GetDocumentStructureArgs {
                mount_id: MOUNT.into(),
                path: path.into(),
                marker: None,
            }))
            .await,
    );
    let view = ok_json(
        server
            .fs_get_document_view(Parameters(DocumentViewArgs {
                mount_id: MOUNT.into(),
                path: path.into(),
                captions: true,
                table_mode: None,
                view: Some("full".into()),
            }))
            .await,
    );
    // The view names the path it was read at; every other field must not change.
    let mut view = view;
    view["path"] = Value::Null;
    json!({"images": images, "tables": tables, "structure": structure, "view": view})
}

/// Image listing only, for the cases that count captions.
async fn images_of(server: &McpServer, path: &str) -> Value {
    let a = ListImagesArgs { mount_id: MOUNT.into(), path: path.into(), marker: None };
    ok_json(server.fs_list_images(Parameters(a)).await)
}

fn captions(list: &Value) -> Vec<String> {
    list["images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["caption"].as_str().unwrap().into())
        .collect()
}

async fn mv(
    server: &McpServer,
    source: &str,
    destination: &str,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let a = MoveArgs {
        mount_id: MOUNT.into(),
        source: source.into(),
        destination: destination.into(),
        overwrite: false,
    };
    server.fs_move(Parameters(a)).await
}

async fn move_ok(server: &McpServer, source: &str, destination: &str) {
    ok_json(mv(server, source, destination).await);
}

/// The old path no longer names a file.
async fn assert_gone(server: &McpServer, path: &str) {
    let a = ListImagesArgs { mount_id: MOUNT.into(), path: path.into(), marker: None };
    let err = err_text(server.fs_list_images(Parameters(a)).await);
    assert!(err.contains(&format!("ERR_NOT_FOUND: not a file: {path}")), "got: {err}");
}

async fn assert_no_artifacts(server: &McpServer, path: &str) {
    let a = ListImagesArgs { mount_id: MOUNT.into(), path: path.into(), marker: None };
    let err = err_text(server.fs_list_images(Parameters(a)).await);
    assert!(err.contains("ERR_NOT_FOUND: No artifacts: document not extracted"), "got: {err}");
}

/// Run the artifact sweep well past its grace period and return the number of
/// sets it deleted: a carried set must survive it.
async fn sweep(f: &Fixture) -> usize {
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let store = c.trash.as_deref().expect("a relational volume");
    let now = crate::util::now_unix() as i64 + 10_000_000;
    store.sweep_stale_artifact_sets(now, 600).await.unwrap().0
}

/// The artifacts at `path` are `before`, now and after the sweep.
async fn assert_follows(f: &Fixture, server: &McpServer, path: &str, before: &Value) {
    assert_eq!(&snapshot(server, path).await, before, "before the sweep");
    assert_eq!(sweep(f).await, 0, "the sweep deleted a carried set");
    assert_eq!(&snapshot(server, path).await, before, "after the sweep");
}

#[tokio::test]
async fn e2e_0105_001_a_converted_document_is_moved() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    assert_eq!(captions(&before["images"]), ["Figure 1", "Figure 2"]);
    move_ok(&server, DOC, "/archive/report.pdf").await;
    assert_follows(&f, &server, "/archive/report.pdf", &before).await;
    assert_gone(&server, DOC).await;
}

#[tokio::test]
async fn e2e_0105_002_a_converted_document_is_renamed_in_place() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    move_ok(&server, DOC, "/report-2026.pdf").await;
    assert_follows(&f, &server, "/report-2026.pdf", &before).await;
    assert_gone(&server, DOC).await;
}

#[tokio::test]
async fn e2e_0105_003_a_folder_holding_converted_documents_is_moved() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), "/docs/report.pdf").await;
    convert_answer(&f, &captioned(1, "Chart"), "/docs/sub/other.pdf").await;
    let (d, d2) = (
        snapshot(&server, "/docs/report.pdf").await,
        snapshot(&server, "/docs/sub/other.pdf").await,
    );
    move_ok(&server, "/docs", "/library").await;
    assert_follows(&f, &server, "/library/report.pdf", &d).await;
    assert_follows(&f, &server, "/library/sub/other.pdf", &d2).await;
    assert_gone(&server, "/docs/report.pdf").await;
    assert_gone(&server, "/docs/sub/other.pdf").await;
}

#[tokio::test]
async fn e2e_0105_004_the_conversion_text_follows_without_a_converter_call() {
    let (f, server) = world().await;
    let home = tempfile::tempdir().unwrap();
    let svc = program(home.path(), &document_d());
    let counting = Counting::new(&svc);
    convert_via(&f, &counting, DOC).await;
    assert_eq!(counting.calls(), 1);
    let before = snapshot(&server, DOC).await;
    move_ok(&server, DOC, "/archive/report.pdf").await;
    assert_follows(&f, &server, "/archive/report.pdf", &before).await;
    assert_eq!(counting.calls(), 1);
}

#[tokio::test]
async fn e2e_0105_013_a_move_onto_an_existing_folder_without_overwrite() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    let a = MkdirArgs {
        mount_id: MOUNT.into(),
        path: "/archive".into(),
        parents: true,
        exist_ok: true,
    };
    ok_json(server.fs_mkdir(Parameters(a)).await);
    let err = err_text(mv(&server, DOC, "/archive").await);
    assert!(err.contains("ERR_NO_CLOBBER: '/archive' exists (pass overwrite=true)"), "got: {err}");
    assert_follows(&f, &server, DOC, &before).await;
}

#[tokio::test]
async fn e2e_0105_014_a_move_of_a_document_that_does_not_exist() {
    let (f, server) = world().await;
    let err = err_text(mv(&server, "/docs/ghost.pdf", "/archive/ghost.pdf").await);
    assert!(err.contains("ERR_NOT_FOUND: '/docs/ghost.pdf' does not exist"), "got: {err}");
    assert_gone(&server, "/archive/ghost.pdf").await;
    assert_eq!(sweep(&f).await, 0);
}

#[tokio::test]
async fn e2e_0105_026_a_move_under_a_plain_file() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    seed(&f, "/flat.txt", b"plain").await;
    let before = snapshot(&server, DOC).await;
    let err = err_text(mv(&server, DOC, "/flat.txt/report.pdf").await);
    assert!(
        err.contains("ERR_NO_CLOBBER: '/flat.txt' already exists and is not a directory"),
        "got: {err}"
    );
    assert_follows(&f, &server, DOC, &before).await;
}

#[tokio::test]
async fn e2e_0105_027_the_same_move_twice() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    move_ok(&server, DOC, "/archive/report.pdf").await;
    let err = err_text(mv(&server, DOC, "/archive/report.pdf").await);
    assert!(err.contains("ERR_NOT_FOUND: '/report.pdf' does not exist"), "got: {err}");
    assert_follows(&f, &server, "/archive/report.pdf", &before).await;
}

#[tokio::test]
async fn e2e_0105_031_a_plain_file_moved_and_copied() {
    let (f, server) = world().await;
    seed(&f, "/notes.txt", b"plain").await;
    move_ok(&server, "/notes.txt", "/n.txt").await;
    let a = CopyArgs {
        mount_id: MOUNT.into(),
        source: "/n.txt".into(),
        destination: "/m.txt".into(),
        overwrite: false,
        recursive: false,
    };
    ok_json(server.fs_copy(Parameters(a)).await);
    assert_no_artifacts(&server, "/n.txt").await;
    assert_no_artifacts(&server, "/m.txt").await;
    assert_eq!(sweep(&f).await, 0);
}

#[tokio::test]
async fn e2e_0105_032_an_empty_folder_moved() {
    let (f, server) = world().await;
    let a =
        MkdirArgs { mount_id: MOUNT.into(), path: "/empty".into(), parents: true, exist_ok: true };
    ok_json(server.fs_mkdir(Parameters(a)).await);
    move_ok(&server, "/empty", "/vide").await;
    assert_eq!(sweep(&f).await, 0);
}

#[tokio::test]
async fn e2e_0105_033_nested_folders_with_three_converted_documents() {
    let (f, server) = world().await;
    let home = tempfile::tempdir().unwrap();
    let docs = [("/a/x.pdf", 1), ("/a/b/y.pdf", 2), ("/a/b/c/z.pdf", 3)];
    for (path, n) in docs {
        let svc = program(home.path(), &captioned(n, "Pic"));
        convert_via(&f, &svc, path).await;
    }
    move_ok(&server, "/a", "/z").await;
    for (path, n) in [("/z/x.pdf", 1), ("/z/b/y.pdf", 2), ("/z/b/c/z.pdf", 3)] {
        let expect: Vec<String> = (1..=n).map(|k| format!("Pic{k}")).collect();
        assert_eq!(captions(&images_of(&server, path).await), expect);
    }
    assert_eq!(sweep(&f).await, 0);
    for (path, n) in [("/z/x.pdf", 1), ("/z/b/y.pdf", 2), ("/z/b/c/z.pdf", 3)] {
        assert_eq!(captions(&images_of(&server, path).await).len(), n);
    }
}

#[tokio::test]
async fn e2e_0105_034_a_folder_mixing_converted_and_plain_files() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), "/mix/a.pdf").await;
    seed(&f, "/mix/b.txt", b"plain").await;
    let before = snapshot(&server, "/mix/a.pdf").await;
    move_ok(&server, "/mix", "/mixed").await;
    assert_follows(&f, &server, "/mixed/a.pdf", &before).await;
    assert_no_artifacts(&server, "/mixed/b.txt").await;
}

#[tokio::test]
async fn e2e_0105_035_a_document_moved_back_to_its_original_path() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    move_ok(&server, DOC, "/archive/report.pdf").await;
    move_ok(&server, "/archive/report.pdf", DOC).await;
    assert_follows(&f, &server, DOC, &before).await;
}

#[tokio::test]
async fn e2e_0105_038_a_hundred_converted_documents_in_one_folder() {
    let (f, server) = world().await;
    let home = tempfile::tempdir().unwrap();
    for k in 1..=100 {
        let svc = program(home.path(), &captioned(1, &format!("n{k}x")));
        convert_via(&f, &svc, &format!("/many/d{k}.pdf")).await;
    }
    let started = std::time::Instant::now();
    move_ok(&server, "/many", "/lots").await;
    assert!(started.elapsed() < std::time::Duration::from_secs(10), "{:?}", started.elapsed());
    assert_eq!(sweep(&f).await, 0);
    for k in 1..=100 {
        let list = images_of(&server, &format!("/lots/d{k}.pdf")).await;
        assert_eq!(captions(&list), [format!("n{k}x1")]);
    }
}

#[tokio::test]
async fn e2e_0105_039_two_moves_in_a_row() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    move_ok(&server, DOC, "/a.pdf").await;
    move_ok(&server, "/a.pdf", "/b.pdf").await;
    assert_follows(&f, &server, "/b.pdf", &before).await;
    assert_gone(&server, "/a.pdf").await;
    assert_gone(&server, DOC).await;
}

#[tokio::test]
async fn e2e_0105_048_the_images_keep_their_ids_and_the_table_its_caption() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    move_ok(&server, DOC, "/archive/report.pdf").await;
    assert_eq!(sweep(&f).await, 0);
    let images = images_of(&server, "/archive/report.pdf").await;
    let ids: Vec<&str> =
        images["images"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["image-1", "image-2"]);
    let tables = snapshot(&server, "/archive/report.pdf").await["tables"].clone();
    assert_eq!(tables["tables"][0]["id"], "table-1");
    assert_eq!(tables["tables"][0]["caption"], "Costs");
}

#[tokio::test]
async fn e2e_0105_071_a_rename_under_a_plain_file() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    seed(&f, "/flat.txt", b"plain").await;
    let before = snapshot(&server, DOC).await;
    let err = err_text(mv(&server, DOC, "/flat.txt/report.pdf").await);
    assert!(
        err.contains("ERR_NO_CLOBBER: '/flat.txt' already exists and is not a directory"),
        "got: {err}"
    );
    assert_follows(&f, &server, DOC, &before).await;
}

#[tokio::test]
async fn e2e_0105_mod001_a_move_no_longer_discards() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    move_ok(&server, DOC, "/b.pdf").await;
    assert_follows(&f, &server, "/b.pdf", &before).await;
    assert_gone(&server, DOC).await;
}

#[tokio::test]
async fn e2e_0105_mod004_a_folder_move_keeps_the_artifacts() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), "/q1/report.pdf").await;
    let before = snapshot(&server, "/q1/report.pdf").await;
    move_ok(&server, "/q1", "/archive").await;
    assert_follows(&f, &server, "/archive/report.pdf", &before).await;
    assert_gone(&server, "/q1/report.pdf").await;
}

#[tokio::test]
async fn e2e_0105_mod006_moving_away_and_back_keeps_the_artifacts() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    move_ok(&server, DOC, "/b.pdf").await;
    move_ok(&server, "/b.pdf", DOC).await;
    assert_follows(&f, &server, DOC, &before).await;
}

/// A rewrite discards the artifacts (FR-MOD-001); a later move must not bring
/// the stale set back under the node's new revision.
#[tokio::test]
async fn e2e_0105_072_a_rewritten_then_moved_document_stays_without_artifacts() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    seed(&f, DOC, b"%PDF-1.4 rewritten").await;
    assert_no_artifacts(&server, DOC).await;
    move_ok(&server, DOC, "/archive/report.pdf").await;
    assert_no_artifacts(&server, "/archive/report.pdf").await;
}

async fn mv_over(
    server: &McpServer,
    source: &str,
    destination: &str,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let a = MoveArgs {
        mount_id: MOUNT.into(),
        source: source.into(),
        destination: destination.into(),
        overwrite: true,
    };
    server.fs_move(Parameters(a)).await
}

const D: &str = "/docs/report.pdf";
const OLD: &str = "/docs/old.pdf";

#[tokio::test]
async fn e2e_0105_009_overwrite_by_move_replaces_the_destination_artifacts() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), D).await;
    convert_answer(&f, &captioned(1, "Old"), OLD).await;
    let before = snapshot(&server, D).await;
    ok_json(mv_over(&server, D, OLD).await);
    // The overwritten set keeps its old revision: the sweep drops it, never D's.
    assert_eq!(snapshot(&server, OLD).await, before, "before the sweep");
    assert_eq!(sweep(&f).await, 1, "only the overwritten set is swept");
    assert_eq!(snapshot(&server, OLD).await, before, "after the sweep");
    let list = images_of(&server, OLD).await;
    assert_eq!(captions(&list), ["Figure 1", "Figure 2"]);
    assert!(!list.to_string().contains("Old"));
    assert_gone(&server, D).await;
}

#[tokio::test]
async fn e2e_0105_015_a_folder_moved_into_itself() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    let err = err_text(mv(&server, "/docs", "/docs/inner").await);
    assert!(
        err.contains("ERR_INVALID_ARGUMENT: '/docs' cannot be moved onto or into itself"),
        "got: {err}"
    );
    assert_follows(&f, &server, D, &before).await;
}

#[tokio::test]
async fn e2e_0105_020_a_move_while_the_document_is_being_converted() {
    for before_artifacts in [false, true] {
        let (f, server) = world().await;
        if before_artifacts {
            convert_answer(&f, &document_d(), D).await;
        } else {
            seed(&f, D, b"%PDF-1.4").await;
        }
        let home = tempfile::tempdir().unwrap();
        let inner = program(home.path(), &captioned(1, "Late"));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let gate = Gated {
            inner: &inner,
            started: std::sync::Mutex::new(Some(started_tx)),
            release: tokio::sync::Mutex::new(Some(release_rx)),
        };
        let before = if before_artifacts { Some(snapshot(&server, D).await) } else { None };
        let c = f.state.stores.client(MOUNT).await.unwrap();
        let convert = crate::core::fs_ops::documentize(
            &c,
            &f.state.safety,
            Some(&gate),
            OWNER,
            MOUNT,
            D,
            true,
        );
        let drive = async {
            started_rx.await.unwrap();
            move_ok(&server, D, "/archive/report.pdf").await;
            release_tx.send(()).unwrap();
        };
        let (r, ()) = tokio::join!(convert, drive);
        let e = r.expect_err("the conversion is refused");
        assert!(e.message.contains("Document changed during conversion"), "{}", e.message);
        match before {
            Some(b) => assert_follows(&f, &server, "/archive/report.pdf", &b).await,
            None => {
                assert_no_artifacts(&server, "/archive/report.pdf").await;
                assert_gone(&server, D).await;
            }
        }
    }
}

/// Holds the conversion open until the test lets it go.
struct Gated<'a> {
    inner: &'a dyn crate::docs::service::DocService,
    started: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

#[async_trait::async_trait]
impl crate::docs::service::DocService for Gated<'_> {
    async fn convert(
        &self,
        bytes: &[u8],
        file_name: &str,
    ) -> crate::errors::Result<crate::docs::service::Conversion> {
        if let Some(tx) = self.started.lock().unwrap().take() {
            tx.send(()).unwrap();
        }
        let rx = self.release.lock().await.take();
        if let Some(rx) = rx {
            rx.await.unwrap();
        }
        self.inner.convert(bytes, file_name).await
    }

    fn accepted_extensions(&self) -> &[String] {
        self.inner.accepted_extensions()
    }

    fn max_input_bytes(&self) -> u64 {
        self.inner.max_input_bytes()
    }
}

#[tokio::test]
async fn e2e_0105_022_a_refused_move_onto_an_existing_document_keeps_both_sets() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), D).await;
    convert_answer(&f, &captioned(1, "Old"), OLD).await;
    let (d, old) = (snapshot(&server, D).await, snapshot(&server, OLD).await);
    let err = err_text(mv(&server, D, OLD).await);
    assert!(err.contains("ERR_NO_CLOBBER: '/docs/old.pdf' exists (pass overwrite=true)"), "{err}");
    assert_follows(&f, &server, D, &d).await;
    assert_follows(&f, &server, OLD, &old).await;
}

#[tokio::test]
async fn e2e_0105_029_a_second_member_moves_a_converted_document() {
    let (f, server) = world().await;
    let bob = crate::tools::archive_testkit::add_second_member(&f).await;
    let bob_server = McpServer::new(f.state.clone(), bob.to_string());
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    move_ok(&bob_server, D, "/archive/report.pdf").await;
    assert_follows(&f, &server, "/archive/report.pdf", &before).await;
    assert_eq!(snapshot(&bob_server, "/archive/report.pdf").await, before);
}

#[tokio::test]
async fn e2e_0105_036_a_converted_document_overwritten_by_a_plain_file() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), D).await;
    seed(&f, "/docs/plain.txt", b"plain text").await;
    ok_json(mv_over(&server, "/docs/plain.txt", D).await);
    assert_no_artifacts(&server, D).await;
    assert_eq!(sweep(&f).await, 1, "the overwritten set is swept");
    assert_no_artifacts(&server, D).await;
    let c = f.state.stores.client(MOUNT).await.unwrap();
    assert_eq!(c.read_bytes(D).await.unwrap(), b"plain text");
}

/// FR-NEW-012: a move still carries a chunk set. (The search entry rule is
/// pinned by the SPEC-0100 tests, which this story leaves untouched.)
#[tokio::test]
async fn e2e_0105_047_chunk_sets_behave_as_before() {
    use crate::mcp::server::ListChunkSetsArgs;
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), D).await;
    let a = crate::mcp::server::CreateChunkSetArgs {
        mount_id: MOUNT.into(),
        path: D.into(),
        strategy: "fixed".into(),
        settings: serde_json::Map::new(),
    };
    ok_json(server.fs_create_chunk_set(Parameters(a)).await);
    let list = |p: &str| ListChunkSetsArgs { mount_id: MOUNT.into(), path: p.into() };
    let before = ok_json(server.fs_list_chunk_sets(Parameters(list(D))).await);
    assert_eq!(before["sets"].as_array().unwrap().len(), 1);
    move_ok(&server, D, "/archive/report.pdf").await;
    let after = ok_json(server.fs_list_chunk_sets(Parameters(list("/archive/report.pdf"))).await);
    assert_eq!(after["sets"], before["sets"]);
}

/// The refusal runs before the overwrite deletes anything.
async fn refused_self_move(source: &str, destination: &str, overwrite: bool, doc: &str) {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), doc).await;
    let before = snapshot(&server, doc).await;
    let r = if overwrite {
        mv_over(&server, source, destination).await
    } else {
        mv(&server, source, destination).await
    };
    let err = err_text(r);
    let want = format!("ERR_INVALID_ARGUMENT: '{source}' cannot be moved onto or into itself");
    assert!(err.contains(&want), "got: {err}");
    assert_follows(&f, &server, doc, &before).await;
}

#[tokio::test]
async fn e2e_0105_060_a_document_moved_onto_itself_with_overwrite() {
    refused_self_move(D, D, true, D).await;
}

#[tokio::test]
async fn e2e_0105_061_a_folder_moved_onto_its_ancestor_with_overwrite() {
    refused_self_move("/docs/sub", "/docs", true, "/docs/sub/other.pdf").await;
}

#[tokio::test]
async fn e2e_0105_065_a_move_onto_itself_without_overwrite() {
    refused_self_move(D, D, false, D).await;
}

#[tokio::test]
async fn e2e_0105_066_a_folder_moved_onto_its_ancestor_without_overwrite() {
    refused_self_move("/docs/sub", "/docs", false, "/docs/sub/other.pdf").await;
}

// US-0304: trash and restore.

use crate::mcp::server::{DeleteArgs, TrashListArgs, TrashRestoreArgs};

/// Delete `path` into the trash; the trash path of the item.
async fn trash(server: &McpServer, path: &str, recursive: bool) -> String {
    let a = DeleteArgs { mount_id: MOUNT.into(), path: path.into(), recursive, trash: true };
    let out = ok_json(server.fs_delete(Parameters(a)).await);
    out["trash_path"].as_str().expect("a trash path").to_string()
}

async fn restore(server: &McpServer, trash_path: &str) -> Result<CallToolResult, rmcp::ErrorData> {
    let a = TrashRestoreArgs { mount_id: MOUNT.into(), trash_path: trash_path.into() };
    server.fs_trash_restore(Parameters(a)).await
}

async fn restored_path(server: &McpServer, trash_path: &str) -> String {
    let out = ok_json(restore(server, trash_path).await);
    out["restored_path"].as_str().unwrap().to_string()
}

/// Run the retention sweep (one day retention) two days from now: every file
/// outside the trash is stale and goes to the trash.
async fn retention_sweep(f: &Fixture) -> usize {
    use crate::storage::traits::PurgeConfig;
    f.state
        .admin
        .set_purge_config(
            MOUNT,
            PurgeConfig {
                autopurge_enabled: true,
                use_internal_purge: true,
                file_retention_days: Some(1),
                project_retention_days: None,
            },
        )
        .await
        .unwrap();
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let now = crate::util::now_unix() + 2.0 * 86_400.0;
    crate::purge::sweep_project_files_at(now, &c, &*f.state.admin, &f.state.safety, MOUNT, None)
        .await
        .unwrap()
}

/// The trash path of the item deleted from `original`.
async fn trash_path_of(server: &McpServer, original: &str) -> String {
    let a =
        TrashListArgs { mount_id: MOUNT.into(), path_prefix: String::new(), limit: 50, offset: 0 };
    let out = ok_json(server.fs_trash_list(Parameters(a)).await);
    let found: Vec<_> = out["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["original_path"] == original)
        .collect();
    assert_eq!(found.len(), 1, "one trashed item from {original}");
    found[0]["trash_path"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn e2e_0105_007_trash_then_restore() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    let t = trash(&server, DOC, false).await;
    assert_gone(&server, DOC).await;
    assert_eq!(restored_path(&server, &t).await, DOC);
    assert_follows(&f, &server, DOC, &before).await;
}

#[tokio::test]
async fn e2e_0105_008_restore_under_a_new_name_after_a_collision() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    let t = trash(&server, DOC, false).await;
    seed(&f, DOC, b"new").await;
    let name = restored_path(&server, &t).await;
    assert_eq!(name, format!("{DOC}_restored"));
    assert_follows(&f, &server, &name, &before).await;
    assert_no_artifacts(&server, DOC).await;
}

#[tokio::test]
async fn e2e_0105_016_restore_of_an_unknown_trash_item() {
    let (_f, server) = world().await;
    let err = err_text(restore(&server, "/.mcp_trash/missing").await);
    assert!(err.contains("ERR_NOT_FOUND: '/.mcp_trash/missing' is not a trashed entry"), "{err}");
}

#[tokio::test]
async fn e2e_0105_021_restoring_the_same_item_twice() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    let t = trash(&server, DOC, false).await;
    restored_path(&server, &t).await;
    let err = err_text(restore(&server, &t).await);
    assert!(err.contains(&format!("ERR_NOT_FOUND: '{t}' is not a trashed entry")), "{err}");
    assert_follows(&f, &server, DOC, &before).await;
}

#[tokio::test]
async fn e2e_0105_023_reading_the_old_path_after_a_trash() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let t = trash(&server, DOC, false).await;
    assert_gone(&server, DOC).await;
    for err in [
        err_text(
            server
                .fs_list_tables(Parameters(ListTablesArgs {
                    mount_id: MOUNT.into(),
                    path: DOC.into(),
                    marker: None,
                }))
                .await,
        ),
        err_text(
            server
                .fs_get_document_structure(Parameters(GetDocumentStructureArgs {
                    mount_id: MOUNT.into(),
                    path: DOC.into(),
                    marker: None,
                }))
                .await,
        ),
        err_text(
            server
                .fs_get_document_view(Parameters(DocumentViewArgs {
                    mount_id: MOUNT.into(),
                    path: DOC.into(),
                    captions: true,
                    table_mode: None,
                    view: Some("full".into()),
                }))
                .await,
        ),
    ] {
        assert!(err.contains(&format!("ERR_NOT_FOUND: not a file: {DOC}")), "{err}");
    }
    // Readable only after the restore: at the trash location it is refused.
    assert_no_artifacts(&server, &t).await;
    let a = ListTablesArgs { mount_id: MOUNT.into(), path: t.clone(), marker: None };
    let err = err_text(server.fs_list_tables(Parameters(a)).await);
    assert!(err.contains("ERR_NOT_FOUND: No artifacts: document not extracted"), "{err}");
}

#[tokio::test]
async fn e2e_0105_025_a_restore_whose_folder_was_replaced_by_a_file() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), "/docs/report.pdf").await;
    let before = snapshot(&server, "/docs/report.pdf").await;
    let t = trash(&server, "/docs/report.pdf", false).await;
    let c = f.state.stores.client(MOUNT).await.unwrap();
    c.delete_tree("/docs").await.unwrap();
    seed(&f, "/docs", b"plain").await;
    let err = err_text(restore(&server, &t).await);
    assert!(err.contains("ERR_NO_CLOBBER: '/docs' already exists and is not a directory"), "{err}");
    // Still in the trash with its set: clear the way and restore.
    c.delete_file("/docs").await.unwrap();
    assert_eq!(restored_path(&server, &t).await, "/docs/report.pdf");
    assert_follows(&f, &server, "/docs/report.pdf", &before).await;
}

#[tokio::test]
async fn e2e_0105_028_a_non_member_cannot_read_a_trashed_document() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    trash(&server, DOC, false).await;
    let eve = McpServer::new(f.state.clone(), "eve@example.com".to_string());
    let a = ListImagesArgs { mount_id: MOUNT.into(), path: DOC.into(), marker: None };
    let err = err_text(eve.fs_list_images(Parameters(a)).await);
    assert!(err.contains("ERR_FORBIDDEN: 'eve@example.com' is not a member of 'atlas'"), "{err}");
}

#[tokio::test]
async fn e2e_0105_042_another_file_at_the_old_path_of_a_trashed_document() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    let t = trash(&server, DOC, false).await;
    seed(&f, DOC, b"plain").await;
    assert_no_artifacts(&server, DOC).await;
    let name = restored_path(&server, &t).await;
    assert_follows(&f, &server, &name, &before).await;
}

#[tokio::test]
async fn e2e_0105_043_restore_after_the_original_path_was_reused() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    let t = trash(&server, DOC, false).await;
    convert_answer(&f, &captioned(1, "Other"), DOC).await;
    let name = restored_path(&server, &t).await;
    assert_eq!(name, format!("{DOC}_restored"));
    assert_eq!(captions(&images_of(&server, &name).await).len(), 2);
    assert_follows(&f, &server, &name, &before).await;
    assert_eq!(captions(&images_of(&server, DOC).await), ["Other1"]);
}

#[tokio::test]
async fn e2e_0105_044_a_folder_trashed_and_restored() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), "/docs/report.pdf").await;
    convert_answer(&f, &captioned(1, "Chart"), "/docs/sub/other.pdf").await;
    let d = snapshot(&server, "/docs/report.pdf").await;
    let d2 = snapshot(&server, "/docs/sub/other.pdf").await;
    let t = trash(&server, "/docs", true).await;
    assert_eq!(restored_path(&server, &t).await, "/docs");
    assert_follows(&f, &server, "/docs/report.pdf", &d).await;
    assert_follows(&f, &server, "/docs/sub/other.pdf", &d2).await;
}

#[tokio::test]
async fn e2e_0105_045_a_document_trashed_by_the_retention_sweep() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    // The companion text file is stale too, so the count is not pinned.
    assert!(retention_sweep(&f).await >= 1);
    assert_gone(&server, DOC).await;
    let t = trash_path_of(&server, DOC).await;
    assert_eq!(restored_path(&server, &t).await, DOC);
    assert_follows(&f, &server, DOC, &before).await;
}

#[tokio::test]
async fn e2e_0105_053_the_trash_listing_is_unchanged() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    trash(&server, DOC, false).await;
    let a =
        TrashListArgs { mount_id: MOUNT.into(), path_prefix: String::new(), limit: 50, offset: 0 };
    let out = ok_json(server.fs_trash_list(Parameters(a)).await);
    let mut keys: Vec<&String> = out["entries"][0].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "deleted_at",
            "deleted_by",
            "kind",
            "original_path",
            "purge_in_days",
            "size",
            "trash_path"
        ]
    );
    assert_eq!(out["entries"][0]["original_path"], DOC);
}

#[tokio::test]
async fn e2e_0105_064_a_sweep_trash_and_a_member_delete_are_alike() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    convert_answer(&f, &captioned(1, "Chart"), "/docs/other.pdf").await;
    let d = snapshot(&server, DOC).await;
    let d2 = snapshot(&server, "/docs/other.pdf").await;
    trash(&server, DOC, false).await;
    assert!(retention_sweep(&f).await >= 1);
    for original in [DOC, "/docs/other.pdf"] {
        let t = trash_path_of(&server, original).await;
        assert_eq!(restored_path(&server, &t).await, original);
    }
    assert_follows(&f, &server, DOC, &d).await;
    assert_follows(&f, &server, "/docs/other.pdf", &d2).await;
}

/// SPEC-0019/E2E-090 under the new rule (DEC-014): trash then restore keeps
/// the artifacts; while trashed the old path is not a file.
#[tokio::test]
async fn e2e_0105_mod003_trash_and_restore_keep_the_artifacts() {
    let (f, server) = world().await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    let t = trash(&server, DOC, false).await;
    assert_gone(&server, DOC).await;
    restored_path(&server, &t).await;
    assert_follows(&f, &server, DOC, &before).await;
}

// US-0305: removal for good and the rules that still discard.

use crate::mcp::server::WriteArgs;

/// Like `world`, with the server option `allow_hard_delete` as given and the
/// read guard off, so an edit is one `fs.write` with `overwrite`.
async fn world_with(hard: bool) -> (Fixture, McpServer) {
    let f = Fixture::with_config(move |c| {
        c.safety.write_quota_bytes = 1_000_000_000;
        c.safety.allow_hard_delete = hard;
        c.safety.read_guard = false;
    })
    .await;
    f.seed_project(MOUNT, OWNER).await;
    let c = f.state.stores.client(MOUNT).await.unwrap();
    c.makedirs("/docs", true).await.unwrap();
    seed(&f, DOC, b"%PDF-1.4").await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    (f, server)
}

async fn hard_delete(
    server: &McpServer,
    path: &str,
    recursive: bool,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let a = DeleteArgs { mount_id: MOUNT.into(), path: path.into(), recursive, trash: false };
    server.fs_delete(Parameters(a)).await
}

async fn write_over(
    server: &McpServer,
    path: &str,
    content: &str,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let a = WriteArgs {
        mount_id: MOUNT.into(),
        path: path.into(),
        content: content.into(),
        overwrite: true,
        create_parents: true,
    };
    server.fs_write(Parameters(a)).await
}

/// The purge's own artifact sweep at the real clock: a set whose node is gone
/// goes at once, without waiting for the grace period (DEC-005).
async fn purge_sweep(f: &Fixture) -> usize {
    let c = f.state.stores.client(MOUNT).await.unwrap();
    crate::purge::sweep_project_artifacts_at(crate::util::now_unix() as i64, &c).await.unwrap()
}

/// The stand in converter's picture bytes are the only blob of that content.
async fn picture_blob_exists(f: &Fixture) -> bool {
    let c = f.state.stores.client(MOUNT).await.unwrap();
    c.blob.exists(&crate::storage::volume::VolumeClient::sha256_hex(b"PNG")).await.unwrap()
}

#[tokio::test]
async fn e2e_0105_011_a_hard_deleted_document_loses_its_artifacts() {
    let (f, server) = world_with(true).await;
    convert_answer(&f, &document_d(), DOC).await;
    assert!(picture_blob_exists(&f).await);
    ok_json(hard_delete(&server, DOC, false).await);
    // The set outlives the node until the sweep, but nothing can read it.
    assert_gone(&server, DOC).await;
    assert_eq!(purge_sweep(&f).await, 1, "the set of the deleted node is swept without grace");
    assert!(!picture_blob_exists(&f).await, "its picture blobs are released");
    assert_eq!(sweep(&f).await, 0);
    let a =
        TrashListArgs { mount_id: MOUNT.into(), path_prefix: String::new(), limit: 50, offset: 0 };
    assert!(
        ok_json(server.fs_trash_list(Parameters(a)).await)["entries"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    seed(&f, DOC, b"new plain file").await;
    assert_no_artifacts(&server, DOC).await;
}

/// A document trashed then hard deleted at its trash path: its set sat at the
/// trash path, so the sweep must find it there.
#[tokio::test]
async fn e2e_0105_011_a_trashed_document_hard_deleted_at_its_trash_path() {
    let (f, server) = world_with(true).await;
    convert_answer(&f, &document_d(), DOC).await;
    let t = trash(&server, DOC, false).await;
    assert_eq!(purge_sweep(&f).await, 0, "a trashed document keeps its set");
    assert!(picture_blob_exists(&f).await);
    ok_json(hard_delete(&server, &t, false).await);
    assert_eq!(purge_sweep(&f).await, 1, "swept at once, no grace");
    assert!(!picture_blob_exists(&f).await, "blobs released");
    assert_eq!(purge_sweep(&f).await, 0);
}

#[tokio::test]
async fn e2e_0105_011_a_folder_hard_deleted_with_two_documents() {
    let (f, server) = world_with(true).await;
    convert_answer(&f, &document_d(), "/docs/report.pdf").await;
    convert_answer(&f, &captioned(1, "Chart"), "/docs/sub/other.pdf").await;
    ok_json(hard_delete(&server, "/docs", true).await);
    assert_eq!(purge_sweep(&f).await, 2);
    assert!(!picture_blob_exists(&f).await);
    // A trashed folder hard deleted at its trash path goes the same way.
    convert_answer(&f, &document_d(), "/docs/report.pdf").await;
    let t = trash(&server, "/docs", true).await;
    ok_json(hard_delete(&server, &t, true).await);
    assert_eq!(purge_sweep(&f).await, 1);
    assert!(!picture_blob_exists(&f).await);
}

#[tokio::test]
async fn e2e_0105_040_an_edit_after_a_move_discards_as_before() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), DOC).await;
    move_ok(&server, DOC, "/archive/report.pdf").await;
    ok_json(write_over(&server, "/archive/report.pdf", "a new version").await);
    assert_no_artifacts(&server, "/archive/report.pdf").await;
    assert_eq!(sweep(&f).await, 1, "the edited document's set is swept");
    assert_no_artifacts(&server, "/archive/report.pdf").await;
}

#[tokio::test]
async fn e2e_0105_046_a_project_soft_deleted_and_restored() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    assert!(f.state.admin.soft_delete_project(MOUNT).await.unwrap());
    f.state.admin.undelete_project(MOUNT).await.unwrap();
    assert_follows(&f, &server, DOC, &before).await;
}

#[tokio::test]
async fn e2e_0105_058_removing_the_project_removes_the_artifacts() {
    let (f, _server) = world_with(false).await;
    convert_answer(&f, &document_d(), DOC).await;
    assert!(f.state.admin.soft_delete_project(MOUNT).await.unwrap());
    let past_grace = crate::util::now_unix()
        + (f.state.config.safety.project_purge_grace_days as f64 + 1.0) * 86_400.0;
    let removed = crate::purge::sweep_grace_period_at(
        past_grace,
        &f.state.stores,
        f.state.admin.as_ref(),
        &f.state.config,
        None,
    )
    .await
    .unwrap();
    assert_eq!(removed, 1);
    f.seed_project(MOUNT, OWNER).await;
    seed(&f, DOC, b"%PDF-1.4").await;
    let server = McpServer::new(f.state.clone(), OWNER.to_string());
    assert_no_artifacts(&server, DOC).await;
    assert_eq!(sweep(&f).await, 0, "nothing of the removed project remains");
    assert!(!picture_blob_exists(&f).await);
}

#[tokio::test]
async fn e2e_0105_063_a_hard_delete_the_server_refuses_keeps_everything() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    let err = err_text(hard_delete(&server, DOC, false).await);
    assert!(
        err.contains(
            "ERR_NOT_SUPPORTED: hard delete disabled (server started without allow_hard_delete)"
        ),
        "{err}"
    );
    assert_follows(&f, &server, DOC, &before).await;
}

#[tokio::test]
async fn e2e_0105_070_an_edit_refused_for_quota_keeps_the_artifacts() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), DOC).await;
    let before = snapshot(&server, DOC).await;
    let left = f.state.safety.session_remaining(OWNER, MOUNT);
    f.state.safety.charge_write(OWNER, MOUNT, left - 10).unwrap();
    let err = err_text(write_over(&server, DOC, &"x".repeat(1000)).await);
    assert!(err.contains("ERR_WRITE_QUOTA_EXCEEDED: session write quota of"), "{err}");
    assert!(err.contains("bytes exceeded"), "{err}");
    assert_follows(&f, &server, DOC, &before).await;
}

// ---------------------------------------------------------------- US-0306: copy

/// What D's artifacts weigh (SPEC-0105 DEC-007): two pictures of 3 bytes with
/// the captions `Figure 1` and `Figure 2` and the OCR text `Revenue 4.2 M EUR`,
/// the table `Costs` (CSV 7, Markdown 33, caption 5), the headings `Report` and
/// `Costs`, and the conversion text (81 bytes as stored, measured: the
/// converter's Markdown is 80).
fn weight_of_d() -> i64 {
    let images = (3 + 8 + 17) + (3 + 8);
    let table = 7 + 33 + 5;
    let headings = 6 + 5;
    images + table + headings + 81
}

/// The size of the file behind every document of these tests.
const FILE_BYTES: i64 = 8;

async fn cp(
    server: &McpServer,
    source: &str,
    destination: &str,
    overwrite: bool,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let a = CopyArgs {
        mount_id: MOUNT.into(),
        source: source.into(),
        destination: destination.into(),
        overwrite,
        recursive: false,
    };
    server.fs_copy(Parameters(a)).await
}

async fn copy_ok(server: &McpServer, source: &str, destination: &str, overwrite: bool) {
    ok_json(cp(server, source, destination, overwrite).await);
}

/// Leave exactly `left` bytes of the session write allowance.
fn pin_allowance(f: &Fixture, left: i64) {
    let now = f.state.safety.session_remaining(OWNER, MOUNT);
    f.state.safety.charge_write(OWNER, MOUNT, now - left).unwrap();
}

fn allowance(f: &Fixture) -> i64 {
    f.state.safety.session_remaining(OWNER, MOUNT)
}

/// Writes one more byte: refused exactly when the allowance is spent.
async fn one_more_byte(server: &McpServer) -> Result<CallToolResult, rmcp::ErrorData> {
    write_over(server, "/one.txt", "x").await
}

#[tokio::test]
async fn e2e_0105_005_a_converted_document_is_copied() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    pin_allowance(&f, 100_000);
    copy_ok(&server, D, "/docs/report-copy.pdf", false).await;
    assert_follows(&f, &server, "/docs/report-copy.pdf", &before).await;
    assert_follows(&f, &server, D, &before).await;
    assert_eq!(allowance(&f), 100_000 - FILE_BYTES - weight_of_d());
}

#[tokio::test]
async fn e2e_0105_010_overwrite_by_copy() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    convert_answer(&f, &captioned(1, "Old"), OLD).await;
    let before = snapshot(&server, D).await;
    copy_ok(&server, D, OLD, true).await;
    assert_eq!(snapshot(&server, OLD).await, before, "before the sweep");
    assert_eq!(sweep(&f).await, 1, "only the replaced set is swept");
    assert_eq!(snapshot(&server, OLD).await, before, "after the sweep");
    assert_follows(&f, &server, D, &before).await;
    assert!(!images_of(&server, OLD).await.to_string().contains("Old"));
}

#[tokio::test]
async fn e2e_0105_012_the_source_of_a_copy_keeps_its_artifacts() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    copy_ok(&server, D, "/docs/c.pdf", false).await;
    ok_json(write_over(&server, "/docs/c.pdf", "edited").await);
    assert_no_artifacts(&server, "/docs/c.pdf").await;
    assert_eq!(snapshot(&server, D).await, before, "before the sweep");
    assert_eq!(sweep(&f).await, 1, "only the edited copy's set is swept");
    assert_eq!(snapshot(&server, D).await, before, "after the sweep");
}

#[tokio::test]
async fn e2e_0105_018_a_copy_that_does_not_fit_the_session_allowance() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    let left = FILE_BYTES + weight_of_d() - 1;
    pin_allowance(&f, left);
    let err = err_text(cp(&server, D, "/docs/c.pdf", false).await);
    assert!(err.contains("ERR_WRITE_QUOTA_EXCEEDED: session write quota of"), "{err}");
    assert!(err.contains("bytes exceeded"), "{err}");
    assert_gone(&server, "/docs/c.pdf").await;
    assert_follows(&f, &server, D, &before).await;
    assert_eq!(allowance(&f), left);
}

#[tokio::test]
async fn e2e_0105_024_a_copy_into_a_folder_that_does_not_exist_yet() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    pin_allowance(&f, 100_000);
    copy_ok(&server, D, "/nowhere/c.pdf", false).await;
    assert_follows(&f, &server, "/nowhere/c.pdf", &before).await;
    assert_eq!(allowance(&f), 100_000 - FILE_BYTES - weight_of_d());
}

#[tokio::test]
async fn e2e_0105_037_names_with_accents_and_spaces() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    move_ok(&server, D, "/Archives été/Rapport n° 1.pdf").await;
    copy_ok(&server, "/Archives été/Rapport n° 1.pdf", "/Archives été/Copie 🌐.pdf", false).await;
    assert_follows(&f, &server, "/Archives été/Rapport n° 1.pdf", &before).await;
    assert_follows(&f, &server, "/Archives été/Copie 🌐.pdf", &before).await;
}

#[tokio::test]
async fn e2e_0105_041_a_converted_copy_converts_again_on_its_own() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    copy_ok(&server, D, "/c.pdf", false).await;
    convert_answer(&f, &captioned(1, "New"), "/c.pdf").await;
    assert_eq!(captions(&images_of(&server, "/c.pdf").await), ["New1"]);
    assert_eq!(snapshot(&server, D).await, before, "before the sweep");
    assert_eq!(sweep(&f).await, 1, "only the replaced set of the copy is swept");
    assert_eq!(snapshot(&server, D).await, before, "after the sweep");
    assert_eq!(captions(&images_of(&server, "/c.pdf").await), ["New1"]);
}

#[tokio::test]
async fn e2e_0105_049_ocr_text_and_headings_follow() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    move_ok(&server, D, "/m.pdf").await;
    copy_ok(&server, "/m.pdf", "/c.pdf", false).await;
    for path in ["/m.pdf", "/c.pdf"] {
        let s = snapshot(&server, path).await.to_string();
        assert!(s.contains("Revenue 4.2 M EUR"), "{path}: {s}");
        assert!(s.contains("Report") && s.contains("Costs"), "{path}: {s}");
    }
    assert_eq!(sweep(&f).await, 0);
}

#[tokio::test]
async fn e2e_0105_050_a_second_copy_of_a_copy() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    pin_allowance(&f, 100_000);
    copy_ok(&server, D, "/c1.pdf", false).await;
    copy_ok(&server, "/c1.pdf", "/c2.pdf", false).await;
    assert_follows(&f, &server, "/c2.pdf", &before).await;
    assert_eq!(allowance(&f), 100_000 - 2 * (FILE_BYTES + weight_of_d()));
}

#[tokio::test]
async fn e2e_0105_051_a_copy_is_charged_to_the_allowance_a_move_is_not() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    pin_allowance(&f, 100_000);
    move_ok(&server, D, "/docs/moved.pdf").await;
    assert_eq!(allowance(&f), 100_000);
    copy_ok(&server, "/docs/moved.pdf", "/docs/c.pdf", false).await;
    assert_eq!(allowance(&f), 100_000 - FILE_BYTES - weight_of_d());
}

#[tokio::test]
async fn e2e_0105_052_project_storage_is_not_charged_for_artifacts() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let used = c.meta.used_bytes().await.unwrap();
    copy_ok(&server, D, "/docs/c.pdf", false).await;
    assert_eq!(c.meta.used_bytes().await.unwrap(), used + FILE_BYTES);
}

#[tokio::test]
async fn e2e_0105_059_a_copy_that_fits_the_allowance_exactly() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    pin_allowance(&f, FILE_BYTES + weight_of_d());
    copy_ok(&server, D, "/docs/c.pdf", false).await;
    assert_follows(&f, &server, "/docs/c.pdf", &before).await;
    let err = err_text(one_more_byte(&server).await);
    assert!(err.contains("ERR_WRITE_QUOTA_EXCEEDED"), "{err}");
}

#[tokio::test]
async fn e2e_0105_067_a_file_copied_onto_itself_with_overwrite() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    pin_allowance(&f, 100_000);
    copy_ok(&server, D, D, true).await;
    assert_eq!(snapshot(&server, D).await, before, "before the sweep");
    assert_eq!(sweep(&f).await, 1, "the set the copy replaced is swept");
    assert_eq!(snapshot(&server, D).await, before, "after the sweep");
    assert_eq!(allowance(&f), 100_000 - FILE_BYTES - weight_of_d());
}

#[tokio::test]
async fn e2e_0105_069_a_copy_onto_an_existing_document_without_overwrite() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    convert_answer(&f, &captioned(1, "Old"), OLD).await;
    let (d, old) = (snapshot(&server, D).await, snapshot(&server, OLD).await);
    pin_allowance(&f, 100_000);
    let err = err_text(cp(&server, D, OLD, false).await);
    assert!(err.contains("ERR_NO_CLOBBER: '/docs/old.pdf' exists (pass overwrite=true)"), "{err}");
    assert_eq!(allowance(&f), 100_000);
    assert_follows(&f, &server, D, &d).await;
    assert_follows(&f, &server, OLD, &old).await;
}

#[tokio::test]
async fn e2e_0105_mod002_a_copy_no_longer_starts_with_none() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    copy_ok(&server, D, "/docs/copy.pdf", false).await;
    assert_eq!(snapshot(&server, "/docs/copy.pdf").await, before);
    assert_eq!(snapshot(&server, D).await, before);
}

#[tokio::test]
async fn e2e_0105_mod005_a_copy_over_a_document_replaces_its_artifacts() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    convert_answer(&f, &captioned(1, "Old"), OLD).await;
    copy_ok(&server, D, OLD, true).await;
    assert_eq!(captions(&images_of(&server, OLD).await), ["Figure 1", "Figure 2"]);
}

#[tokio::test]
async fn e2e_0105_mod007_a_copy_is_charged_the_file_and_its_artifacts() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let left = allowance(&f);
    copy_ok(&server, D, "/docs/copy.pdf", false).await;
    assert_eq!(left - allowance(&f), FILE_BYTES + weight_of_d());
}

#[tokio::test]
async fn e2e_0105_072b_a_copy_over_a_document_from_a_plain_file_leaves_none() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    seed(&f, "/plain.pdf", b"%PDF-1.4").await;
    copy_ok(&server, "/plain.pdf", D, true).await;
    assert_no_artifacts(&server, D).await;
    assert_eq!(sweep(&f).await, 1);
}

// ------------------------------------------------------- US-0307: copy a folder

const D2: &str = "/docs/sub/other.pdf";

async fn cp_tree(
    server: &McpServer,
    source: &str,
    destination: &str,
    overwrite: bool,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let a = CopyArgs {
        mount_id: MOUNT.into(),
        source: source.into(),
        destination: destination.into(),
        overwrite,
        recursive: true,
    };
    server.fs_copy(Parameters(a)).await
}

/// `/docs` holding D and D2 (a document of its own, one captioned picture),
/// both converted; returns the weight of D2's artifacts as measured.
async fn docs_world() -> (Fixture, McpServer, i64) {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    convert_answer(&f, &captioned(1, "Two"), D2).await;
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let w2 = c.trash.as_deref().unwrap().artifact_set_weight(D2).await.unwrap();
    assert!(w2 > 0 && w2 < weight_of_d());
    (f, server, w2)
}

#[tokio::test]
async fn e2e_0105_006_a_folder_holding_converted_documents_is_copied() {
    let (f, server, _) = docs_world().await;
    let (d, d2) = (snapshot(&server, D).await, snapshot(&server, D2).await);
    pin_allowance(&f, 100_000);
    ok_json(cp_tree(&server, "/docs", "/docs-copy", false).await);
    assert_follows(&f, &server, "/docs-copy/report.pdf", &d).await;
    assert_follows(&f, &server, "/docs-copy/sub/other.pdf", &d2).await;
    assert_follows(&f, &server, D, &d).await;
    assert_follows(&f, &server, D2, &d2).await;
}

#[tokio::test]
async fn e2e_0105_019_a_folder_copy_where_the_last_documents_artifacts_do_not_fit() {
    let (f, server, w2) = docs_world().await;
    let (d, d2) = (snapshot(&server, D).await, snapshot(&server, D2).await);
    // Room for D alone: the whole copy is refused, not D's half.
    let left = weight_of_d() + w2 - 1;
    pin_allowance(&f, left);
    let err = err_text(cp_tree(&server, "/docs", "/docs-copy", false).await);
    assert!(err.contains("ERR_WRITE_QUOTA_EXCEEDED: session write quota of"), "{err}");
    assert!(err.contains("bytes exceeded"), "{err}");
    let c = f.state.stores.client(MOUNT).await.unwrap();
    assert!(!c.exists("/docs-copy").await.unwrap());
    assert_follows(&f, &server, D, &d).await;
    assert_follows(&f, &server, D2, &d2).await;
    assert_eq!(allowance(&f), left);
}

#[tokio::test]
async fn e2e_0105_057_a_folder_copy_charges_the_artifacts_of_each_document() {
    let (f, server, w2) = docs_world().await;
    let (d, d2) = (snapshot(&server, D).await, snapshot(&server, D2).await);
    pin_allowance(&f, 100_000);
    ok_json(cp_tree(&server, "/docs", "/docs-copy", false).await);
    assert_eq!(allowance(&f), 100_000 - weight_of_d() - w2);
    assert_follows(&f, &server, D, &d).await;
    assert_follows(&f, &server, D2, &d2).await;
}

#[tokio::test]
async fn e2e_0105_062_a_folder_copied_onto_an_existing_folder_with_overwrite() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), "/src/report.pdf").await;
    convert_answer(&f, &captioned(1, "Old"), "/dst/report.pdf").await;
    convert_answer(&f, &captioned(1, "Keep"), "/dst/keep.pdf").await;
    let (d, keep) =
        (snapshot(&server, "/src/report.pdf").await, snapshot(&server, "/dst/keep.pdf").await);
    ok_json(cp_tree(&server, "/src", "/dst", true).await);
    assert_eq!(snapshot(&server, "/dst/report.pdf").await, d);
    assert!(!images_of(&server, "/dst/report.pdf").await.to_string().contains("Old"));
    assert_eq!(snapshot(&server, "/dst/keep.pdf").await, keep);
    assert!(images_of(&server, "/dst/keep.pdf").await.to_string().contains("Keep"));
    assert_eq!(snapshot(&server, "/src/report.pdf").await, d);
}

#[tokio::test]
async fn e2e_0105_068_a_folder_copied_into_itself() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    pin_allowance(&f, 100_000);
    ok_json(cp_tree(&server, "/docs", "/docs/inner", true).await);
    assert_follows(&f, &server, "/docs/inner/report.pdf", &before).await;
    assert_follows(&f, &server, D, &before).await;
    // One copy of D, charged once; no copy of the copy.
    assert_eq!(allowance(&f), 100_000 - weight_of_d());
    let c = f.state.stores.client(MOUNT).await.unwrap();
    assert!(!c.exists("/docs/inner/inner").await.unwrap());
}

#[tokio::test]
async fn e2e_0105_068b_a_folder_copied_onto_itself_keeps_its_artifacts() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let before = snapshot(&server, D).await;
    pin_allowance(&f, 100_000);
    ok_json(cp_tree(&server, "/docs", "/docs", true).await);
    assert_follows(&f, &server, D, &before).await;
    // Charged as a copy, nothing rewritten.
    assert_eq!(allowance(&f), 100_000 - weight_of_d());
}

// ------------------------------------------- US-0308: no partial result, no converter

const EVE: &str = "eve@example.com";

fn ops_logged(f: &Fixture) -> Vec<String> {
    f.state.safety.audit(OWNER, MOUNT).into_iter().map(|e| e.op).collect()
}

#[tokio::test]
async fn e2e_0105_017_a_non_member_cannot_move_copy_or_restore() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    convert_answer(&f, &captioned(1, "Gone"), OLD).await;
    let (live, gone) = (snapshot(&server, D).await, snapshot(&server, OLD).await);
    let t = trash(&server, OLD, false).await;
    let eve = McpServer::new(f.state.clone(), EVE.to_string());
    let want = format!("ERR_FORBIDDEN: '{EVE}' is not a member of '{MOUNT}'");
    assert!(err_text(mv(&eve, D, "/docs/m.pdf").await).contains(&want));
    assert!(err_text(cp(&eve, D, "/docs/c.pdf", false).await).contains(&want));
    assert!(err_text(restore(&eve, &t).await).contains(&want));
    assert_follows(&f, &server, D, &live).await;
    assert_gone(&server, "/docs/m.pdf").await;
    assert_gone(&server, "/docs/c.pdf").await;
    assert_eq!(restored_path(&server, &t).await, OLD);
    assert_follows(&f, &server, OLD, &gone).await;
}

#[tokio::test]
async fn e2e_0105_030_an_unknown_project() {
    let (_f, server) = world().await;
    let a = MoveArgs {
        mount_id: "ghost".into(),
        source: DOC.into(),
        destination: "/x.pdf".into(),
        overwrite: false,
    };
    let err = err_text(server.fs_move(Parameters(a)).await);
    assert!(err.contains("ERR_PROJECT_NOT_FOUND: project 'ghost' not found"), "{err}");
}

#[tokio::test]
async fn e2e_0105_054_no_converter_call_on_any_of_these_operations() {
    let (f, server) = world_with(false).await;
    let home = tempfile::tempdir().unwrap();
    let inner = program(home.path(), &document_d());
    let counting = Counting::new(&inner);
    convert_via(&f, &counting, D).await;
    assert_eq!(counting.calls(), 1);
    let before = snapshot(&server, D).await;
    move_ok(&server, D, "/docs/a.pdf").await;
    copy_ok(&server, "/docs/a.pdf", "/docs/b.pdf", false).await;
    let t = trash(&server, "/docs/a.pdf", false).await;
    restored_path(&server, &t).await;
    assert_eq!(counting.calls(), 1, "carrying artifacts never converts");
    assert_follows(&f, &server, "/docs/a.pdf", &before).await;
    assert_follows(&f, &server, "/docs/b.pdf", &before).await;
}

#[tokio::test]
async fn e2e_0105_055_the_audit_log_keeps_its_entries() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let base = ops_logged(&f);
    move_ok(&server, D, "/docs/a.pdf").await;
    copy_ok(&server, "/docs/a.pdf", "/docs/b.pdf", false).await;
    let t = trash(&server, "/docs/a.pdf", false).await;
    restored_path(&server, &t).await;
    assert_eq!(ops_logged(&f)[base.len()..], ["move", "copy", "delete"]);
    assert_eq!(ops_logged(&f)[..base.len()], base[..]);
}

#[tokio::test]
async fn e2e_0105_056_a_refused_copy_changes_nothing_the_operator_sees() {
    let (f, server) = world_with(false).await;
    convert_answer(&f, &document_d(), D).await;
    let c = f.state.stores.client(MOUNT).await.unwrap();
    let used = c.meta.used_bytes().await.unwrap();
    let base = ops_logged(&f);
    pin_allowance(&f, FILE_BYTES + weight_of_d() - 1);
    let err = err_text(cp(&server, D, "/docs/c.pdf", false).await);
    assert!(err.contains("ERR_WRITE_QUOTA_EXCEEDED"), "{err}");
    assert_eq!(ops_logged(&f), base);
    assert_eq!(c.meta.used_bytes().await.unwrap(), used);
}
