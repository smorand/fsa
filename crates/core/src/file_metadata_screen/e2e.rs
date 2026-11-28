//! SPEC-0111 acceptance tests of the `/app/file-metadata` screen.
//!
//! Every test drives the real router the way a browser does (session cookie,
//! form post, the `csrf_token` read back from the page) on [`World`]: the
//! `search::derived_e2e::Env` of the recalculation tests (search on, the
//! scheduler wrapped around the engine) with an identity that verifies the
//! cookies this module mints. Names are `e2e_0111_<id>_<slug>`.

use crate::chunking::{Chunk, Set};
use crate::docs::manifest_e2e::{Answer, program};
use crate::search::derived_e2e::{ALICE, DAVE, Env, OTHER, Options, PROJ};
use crate::state::AppState;
use axum::Router;
use axum::http::StatusCode;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt as _;

const REPORT: &str = "/report.pdf";

/// How a request is authenticated.
#[derive(Clone, Copy)]
enum Via {
    Cookie(&'static str),
    Bearer(&'static str),
    Nobody,
}

struct World {
    env: Env,
    state: Arc<AppState>,
    router: Router,
    key: PathBuf,
    _keys: tempfile::TempDir,
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn form_body(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&")
}

/// The first `csrf_token` value of a page.
fn token_of(html: &str) -> String {
    let at = html.find("name=\"csrf_token\" value=\"").expect("a token in the page");
    let rest = &html[at + "name=\"csrf_token\" value=\"".len()..];
    rest[..rest.find('"').unwrap()].to_string()
}

/// The id of the job a recalculation answered, from the page.
fn answered_job(html: &str) -> String {
    let key = "data-answer-job=\"";
    let at = html.find(key).expect("the job answer in the page");
    let rest = &html[at + key.len()..];
    rest[..rest.find('"').unwrap()].to_string()
}

fn chunk(number: u32, text: String) -> Chunk {
    Chunk {
        number,
        unit: number,
        text,
        heading_path: vec!["Report".into(), format!("Part {number}")],
        page_start: Some(1),
        page_end: Some(1),
        token_count: 5,
        figures: vec![],
        document_title: "Report".into(),
    }
}

impl World {
    async fn new() -> Self {
        let env = Env::new(Options::default()).await;
        let keys = tempfile::tempdir().unwrap();
        let (key, public) = crate::keys::write_keypair(keys.path()).unwrap();
        let mut config = (*env.state.config).clone();
        config.auth.jwt.public_key_path = public.display().to_string();
        let s = &env.state;
        let state = Arc::new(AppState {
            config: Arc::new(config.clone()),
            admin: s.admin.clone(),
            stores: s.stores.clone(),
            safety: s.safety.clone(),
            identity: Arc::new(crate::identity::IdentityResolver::new(&config.auth)),
            editors: s.editors.clone(),
            doc_service: None,
            search: s.search.clone(),
        });
        let router = crate::file_metadata_screen::router(state.clone());
        Self { env, state, router, key, _keys: keys }
    }

    fn jwt(&self, who: &str) -> String {
        crate::keys::mint_token_from_file(
            &self.key,
            who,
            crate::keys::DEFAULT_ISSUER,
            crate::keys::DEFAULT_CLAIM,
            3600,
        )
        .unwrap()
    }

    async fn call(
        &self,
        via: Via,
        method: &str,
        uri: &str,
        form: Option<&[(&str, &str)]>,
    ) -> (StatusCode, String) {
        let mut req = axum::http::Request::builder().method(method).uri(uri);
        req = match via {
            Via::Cookie(who) => req.header("Cookie", format!("mcpfs_token={}", self.jwt(who))),
            Via::Bearer(who) => req.header("Authorization", format!("Bearer {}", self.jwt(who))),
            Via::Nobody => req,
        };
        let body = match form {
            Some(f) => {
                req = req.header("Content-Type", "application/x-www-form-urlencoded");
                axum::body::Body::from(form_body(f))
            }
            None => axum::body::Body::empty(),
        };
        let res = self.router.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    /// The page of `path` as `who` sees it in a browser.
    async fn open(
        &self,
        who: &'static str,
        mount: &str,
        path: &str,
        extra: &str,
    ) -> (StatusCode, String) {
        let uri = format!("/app/file-metadata?mount_id={}&path={}{extra}", enc(mount), enc(path));
        self.call(Via::Cookie(who), "GET", &uri, None).await
    }

    /// A file converted by a stand in converter at `path`.
    async fn convert(&self, path: &str, answer: &Answer) {
        let c = self.state.stores.client(PROJ).await.unwrap();
        c.write_bytes_atomic(path, b"%PDF-1.4").await.unwrap();
        let home = tempfile::tempdir().unwrap();
        let svc = program(home.path(), answer);
        self.state.authorize(PROJ, ALICE).await.unwrap();
        crate::core::fs_ops::documentize(
            &c,
            &self.state.safety,
            Some(&svc),
            ALICE,
            PROJ,
            path,
            true,
        )
        .await
        .expect("the conversion succeeds");
    }

    /// A chunk set of `n` chunks on `path`, whose text is `text(number)`.
    async fn put_set(
        &self,
        path: &str,
        set_id: &str,
        is_default: bool,
        n: u32,
        text: impl Fn(u32) -> String,
    ) {
        let client = self.state.stores.client(PROJ).await.unwrap();
        let sha = client.meta.get(path).await.unwrap().unwrap().sha256.unwrap_or_default();
        let store = client.meta.chunk_store().expect("a chunk store");
        let set = Set {
            set_id: set_id.into(),
            path: path.into(),
            request_key: format!(r#"{{"strategy":"structural","set":"{set_id}"}}"#),
            strategy: "structural".into(),
            settings: r#"{"heading_cut_depth":2,"max_tokens":64}"#.into(),
            requested_strategy: "structural".into(),
            requested_settings: "{}".into(),
            is_default,
            fallback_from: None,
            chunk_count: 0,
            token_count: 0,
            source_sha: sha,
            built_at: "2026-11-26T10:00:00Z".into(),
        };
        let chunks: Vec<Chunk> = (1..=n).map(|i| chunk(i, text(i))).collect();
        store.insert_set(&set, &chunks).await.unwrap();
    }
}

/// Document D: two pictures (the first with OCR text), the table `Costs`, the
/// headings `Report` and `Costs`.
fn document_d() -> Answer {
    Answer {
        markdown: "# Report\n\n![](p1.png)\n\n![](p2.png)\n\n## Costs\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n",
        files: vec![("p1.png", "PNG"), ("p2.png", "PNG")],
        manifest: Some(
            r#"{"version":2,"pictures":{"p1.png":{"caption":"Figure 1","page":1,"ocr_text":"Revenue 4.2 M EUR"},"p2.png":{"caption":"Figure 2","page":2}},"tables":[{"caption":"Costs table"}],"headings":[{"level":1,"title":"Report"},{"level":2,"title":"Costs"}]}"#,
        ),
    }
}

/// A document whose every stored text is a stored XSS payload.
fn hostile() -> Answer {
    Answer {
        markdown: "# <b>bold</b>\n\n<script>alert('text')</script>\n\n![](p1.png)\n",
        files: vec![("p1.png", "PNG")],
        manifest: Some(
            r#"{"version":2,"pictures":{"p1.png":{"caption":"<script>alert(1)</script>","page":1,"ocr_text":"\"><svg onload=x>"}},"headings":[{"level":1,"title":"<b>bold</b>"}]}"#,
        ),
    }
}

const NAME_XSS: &str = "/x<img src=x onerror=alert(1)>.pdf";

#[tokio::test]
async fn e2e_0111_001_a_member_opens_a_file_and_sees_its_metadata() {
    let w = World::new().await;
    w.convert(REPORT, &document_d()).await;
    w.put_set(REPORT, "set-aaaa0001", true, 3, |i| format!("Chunk number {i} body")).await;
    let (status, html) = w.open(ALICE, PROJ, REPORT, "").await;
    assert_eq!(status, StatusCode::OK, "{html}");
    for want in [
        "Revenue 4.2 M EUR",
        "Figure 1",
        "Figure 2",
        "image-1",
        "Costs table",
        "table-1",
        "Report",
        "set-aaaa0001",
        "structural",
        "current",
        "data-section=\"text\"",
        "data-section=\"images\"",
        "data-section=\"tables\"",
        "data-section=\"headings\"",
        "data-section=\"chunk-sets\"",
    ] {
        assert!(html.contains(want), "page lacks {want}\n{html}");
    }
    // The picker, without a project, lists the caller's projects only.
    let (s, picker) = w.call(Via::Cookie(ALICE), "GET", "/app/file-metadata", None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(picker.contains(PROJ) && !picker.contains(OTHER) && !picker.contains("Revenue"));
}

#[tokio::test]
async fn e2e_0111_002_a_member_steps_through_the_chunks() {
    let w = World::new().await;
    w.convert(REPORT, &document_d()).await;
    w.put_set(REPORT, "set-aaaa0001", true, 3, |i| format!("Chunk number {i} body")).await;
    let at = |n: u32| format!("&set_id=set-aaaa0001&chunk={n}");
    let (s, two) = w.open(ALICE, PROJ, REPORT, &at(2)).await;
    assert_eq!(s, StatusCode::OK, "{two}");
    assert!(two.contains("Chunk number 2 body") && two.contains("Part 2"));
    assert!(two.contains("data-nav=\"previous\"") && two.contains("data-nav=\"next\""));
    assert!(two.contains("chunk=1\"") && two.contains("chunk=3\""));
    let (_, one) = w.open(ALICE, PROJ, REPORT, &at(1)).await;
    assert!(one.contains("Chunk number 1 body"));
    assert!(!one.contains("data-nav=\"previous\"") && one.contains("data-nav=\"next\""));
    let (_, three) = w.open(ALICE, PROJ, REPORT, &at(3)).await;
    assert!(three.contains("Chunk number 3 body"));
    assert!(three.contains("data-nav=\"previous\"") && !three.contains("data-nav=\"next\""));
}

#[tokio::test]
async fn e2e_0111_004_a_caller_who_is_not_a_member_is_refused() {
    let w = World::new().await;
    w.convert(REPORT, &document_d()).await;
    w.put_set(REPORT, "set-aaaa0001", true, 3, |i| format!("Chunk number {i} body")).await;
    let (status, html) = w.open(DAVE, PROJ, REPORT, "").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{html}");
    for secret in ["Revenue", "Figure 1", "set-aaaa0001", "Chunk number"] {
        assert!(!html.contains(secret), "a refused page leaked {secret}");
    }
}

#[tokio::test]
async fn e2e_0111_005_an_unknown_file_gives_a_clear_page() {
    let w = World::new().await;
    let (status, html) = w.open(ALICE, PROJ, "/missing.pdf", "").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{html}");
    assert!(html.contains("file not found"));
    assert!(!html.contains("data-section="), "no section for a file that does not exist");
}

#[tokio::test]
async fn e2e_0111_008_no_identity_is_unauthenticated() {
    let w = World::new().await;
    let uri = format!("/app/file-metadata?mount_id={PROJ}&path={}", enc(REPORT));
    let (status, body) = w.call(Via::Nobody, "GET", &uri, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["error"], "ERR_UNAUTHENTICATED");
}

#[tokio::test]
async fn e2e_0111_009_every_stored_value_is_escaped() {
    let w = World::new().await;
    w.convert(NAME_XSS, &hostile()).await;
    w.put_set(NAME_XSS, "set-xss00001", true, 1, |_| "<iframe src=x></iframe> body".into()).await;
    let (s, html) = w.open(ALICE, PROJ, NAME_XSS, "&set_id=set-xss00001&chunk=1").await;
    assert_eq!(s, StatusCode::OK, "{html}");
    for raw in
        ["<script>", "<img src=x", "<svg onload", "<iframe", "<b>bold</b>", "onerror=alert(1)>"]
    {
        assert!(!html.contains(raw), "unescaped {raw} in the page\n{html}");
    }
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(html.contains("&lt;iframe src=x&gt;"));
    assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    // The path in the listing of the jobs and in links is escaped as well.
    let (_, picker) = w
        .call(
            Via::Cookie(ALICE),
            "GET",
            &format!("/app/file-metadata?mount_id={PROJ}&path={}", enc(NAME_XSS)),
            None,
        )
        .await;
    assert!(!picker.contains("<img"));
}

#[tokio::test]
async fn e2e_0111_010_ten_thousand_chunks_are_paged() {
    let w = World::new().await;
    w.convert(REPORT, &document_d()).await;
    w.put_set(REPORT, "set-big00001", true, 10_000, |i| format!("Body of chunk {i}")).await;
    let (s, first) = w.open(ALICE, PROJ, REPORT, "&set_id=set-big00001").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(first.matches("data-chunk=\"").count(), 100, "one page of rows");
    assert!(first.contains("data-chunk=\"1\"") && first.contains("data-chunk=\"100\""));
    assert!(!first.contains("data-chunk=\"101\"") && !first.contains("Body of chunk 5000"));
    assert!(first.len() < 200_000, "the page is {} bytes", first.len());
    let at = first.find("data-nav=\"more-chunks\" href=\"").expect("a link to the next page");
    let rest = &first[at + "data-nav=\"more-chunks\" href=\"".len()..];
    let href = rest[..rest.find('"').unwrap()].replace("&amp;", "&");
    let (s, second) = w.call(Via::Cookie(ALICE), "GET", &href, None).await;
    assert_eq!(s, StatusCode::OK, "{second}");
    assert!(second.contains("data-chunk=\"101\"") && second.contains("data-chunk=\"200\""));
    assert!(!second.contains("data-chunk=\"100\""));
}

#[tokio::test]
async fn e2e_0111_011_a_file_with_no_conversion_shows_empty_states() {
    let w = World::new().await;
    w.env.seed("/notes.md", "# Notes\n\nSome words here.\n").await;
    let (s, html) = w.open(ALICE, PROJ, "/notes.md", "").await;
    assert_eq!(s, StatusCode::OK, "{html}");
    assert!(html.matches("No conversion yet").count() >= 4, "text, images, tables, headings");
    assert!(!html.contains("No chunk set"), "the default set of a seeded file is listed");
    let c = w.state.stores.client(PROJ).await.unwrap();
    c.write_text_atomic("/bare.txt", "plain").await.unwrap();
    let (s, bare) = w.open(ALICE, PROJ, "/bare.txt", "").await;
    assert_eq!(s, StatusCode::OK, "{bare}");
    assert!(bare.contains("No chunk set") && bare.contains("No conversion yet"));
    assert!(bare.contains("No job"));
}

const RECALCULATE: &str = "/app/file-metadata/recalculate";
const NOTES: &str = "/notes.md";

impl World {
    /// A world whose rebuilds stay running (the service takes an hour), with
    /// `/notes.md` indexed.
    async fn busy() -> Self {
        let w = Self::new().await;
        w.env.stub().set_delay(3600);
        w.env.seed(NOTES, "# Notes\n\nSome words here.\n").await;
        w
    }

    /// One press of a recalculate button as a browser does: open the page,
    /// read its token, post the form of `kind`.
    async fn press(&self, kind: &str) -> (StatusCode, String) {
        let (_, page) = self.open(ALICE, PROJ, NOTES, "").await;
        let token = token_of(&page);
        let mut form =
            vec![("mount_id", PROJ), ("path", NOTES), ("kind", kind), ("csrf_token", &token)];
        if kind == "chunks" {
            let set =
                page.split("name=\"set_id\" value=\"").nth(1).and_then(|r| r.split('"').next());
            form.push(("set_id", set.expect("a set id in the chunks form")));
        }
        self.call(Via::Cookie(ALICE), "POST", RECALCULATE, Some(&form)).await
    }
}

#[tokio::test]
async fn e2e_0111_003_a_member_recalculates_and_sees_the_job() {
    let w = World::busy().await;
    let (s, html) = w.press("search_index").await;
    assert_eq!(s, StatusCode::OK, "{html}");
    assert!(html.contains("data-section=\"answer\"") && html.contains("search_index"));
    assert!(html.contains("running"));
    let id = answered_job(&html);
    let jobs = w.env.jobs(NOTES).await;
    assert!(jobs.iter().any(|j| j["job_id"] == id.as_str() && j["state"] == "running"), "{jobs:?}");
    // The jobs of the file are listed on the page the post answers.
    assert!(html.contains(&format!("data-job=\"{id}\"")));
    let (s, chunks) = w.press("chunks").await;
    assert_eq!(s, StatusCode::OK, "{chunks}");
    assert!(chunks.contains("data-section=\"answer\"") && chunks.contains("chunks"));
}

#[tokio::test]
async fn e2e_0111_006_a_post_without_a_valid_token_is_refused() {
    let w = World::busy().await;
    let (_, page) = w.open(ALICE, PROJ, NOTES, "").await;
    let token = token_of(&page);
    let base = [("mount_id", PROJ), ("path", NOTES), ("kind", "search_index")];
    let refused = |s: StatusCode, b: &str| {
        assert_eq!(s, StatusCode::FORBIDDEN, "{b}");
        assert!(b.contains("missing or invalid csrf_token"), "{b}");
    };
    let (s, b) = w.call(Via::Cookie(ALICE), "POST", RECALCULATE, Some(&base)).await;
    refused(s, &b);
    let mut bogus = base.to_vec();
    bogus.push(("csrf_token", "not-a-token"));
    let (s, b) = w.call(Via::Cookie(ALICE), "POST", RECALCULATE, Some(&bogus)).await;
    refused(s, &b);
    // A member's token is no use to somebody else, who is not a member anyway.
    let mut stolen = base.to_vec();
    stolen.push(("csrf_token", &token));
    let (s, _) = w.call(Via::Cookie(DAVE), "POST", RECALCULATE, Some(&stolen)).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert!(w.env.jobs(NOTES).await.is_empty(), "a refused post started a job");
    // The refusals did not burn the real token, and a bearer needs none.
    let mut good = base.to_vec();
    good.push(("csrf_token", &token));
    let (s, b) = w.call(Via::Cookie(ALICE), "POST", RECALCULATE, Some(&good)).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, b) = w.call(Via::Bearer(ALICE), "POST", RECALCULATE, Some(&base)).await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

#[tokio::test]
async fn e2e_0111_007_a_file_already_queued_answers_its_queued_job() {
    let w = World::busy().await;
    let (_, first) = w.press("search_index").await;
    assert!(first.contains("running"));
    let (s, second) = w.press("search_index").await;
    assert_eq!(s, StatusCode::OK, "{second}");
    let queued = answered_job(&second);
    let (_, third) = w.press("search_index").await;
    let replaced = answered_job(&third);
    // One running job and at most one waiting job, whatever the presses.
    let jobs = w.env.jobs(NOTES).await;
    assert_eq!(jobs.len(), 2, "{jobs:?}");
    let waiting: Vec<&Value> = jobs.iter().filter(|j| j["state"] == "waiting").collect();
    assert_eq!(waiting.len(), 1, "{jobs:?}");
    assert_eq!(waiting[0]["job_id"], replaced.as_str(), "the answer is the job the tool holds");
    assert_ne!(queued, "", "the second press answered a job");
}

#[tokio::test]
async fn e2e_0111_012_a_token_works_once() {
    let w = World::busy().await;
    let (_, page) = w.open(ALICE, PROJ, NOTES, "").await;
    let token = token_of(&page);
    let form =
        [("mount_id", PROJ), ("path", NOTES), ("kind", "search_index"), ("csrf_token", &token)];
    let (s, b) = w.call(Via::Cookie(ALICE), "POST", RECALCULATE, Some(&form)).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, b) = w.call(Via::Cookie(ALICE), "POST", RECALCULATE, Some(&form)).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{b}");
    assert_eq!(w.env.jobs(NOTES).await.len(), 1, "the replay started no second job");
}
