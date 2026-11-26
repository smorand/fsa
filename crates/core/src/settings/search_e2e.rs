//! SPEC-0109 functional tests of `search.chunks` and `search.images`, driven
//! through the real `McpServer` tools on the SPEC-0108 `World` fixture (`both`
//! search, SQLite) with a fake embedding service whose vector follows the words
//! of the input, so every similarity and rank is known. Names are
//! `e2e_0109_NNN_slug`.

use super::vectors_e2e::{Answer, World};
use crate::docs::DocService;
use crate::docs::service::{Conversion, StubDocService};
use crate::errors::{Result as ToolResult, ToolError};
use crate::mcp::server::{SearchChunksArgs, SearchImagesArgs};
use crate::search::derived_e2e::{args, tool_json};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

const ALICE: &str = "admin@example.com";
const BOB: &str = "bob@example.com";
const CAROL: &str = "carol@example.com";
const PROJ: &str = "proj";

/// A converter that answers a fixed text per file name.
struct Named {
    texts: HashMap<String, String>,
    exts: Vec<String>,
}

impl Named {
    fn of(pairs: &[(&str, &str)]) -> Arc<dyn DocService> {
        Arc::new(Self {
            texts: pairs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect(),
            exts: vec![".png".into()],
        })
    }
}

#[async_trait]
impl DocService for Named {
    async fn convert(&self, _bytes: &[u8], file_name: &str) -> ToolResult<Conversion> {
        self.texts
            .get(file_name)
            .map(|t| Conversion::text(t.clone()))
            .ok_or_else(|| ToolError::internal(format!("no text for {file_name}")))
    }
    fn accepted_extensions(&self) -> &[String] {
        &self.exts
    }
    fn max_input_bytes(&self) -> u64 {
        1 << 20
    }
}

impl World {
    /// The same platform with `doc` as the configuration file converter.
    fn set_converter(&mut self, doc: Option<Arc<dyn DocService>>) {
        let s = &self.fx.state;
        self.fx.state = Arc::new(crate::state::AppState {
            config: s.config.clone(),
            admin: s.admin.clone(),
            stores: s.stores.clone(),
            safety: s.safety.clone(),
            identity: s.identity.clone(),
            editors: s.editors.clone(),
            doc_service: doc,
            search: s.search.clone(),
        });
    }

    /// The same platform served by `search` (the PostgreSQL engine of E2E-018).
    fn set_search(&mut self, search: Arc<dyn crate::search::SearchBackend>) {
        let s = &self.fx.state;
        self.fx.state = Arc::new(crate::state::AppState {
            config: s.config.clone(),
            admin: s.admin.clone(),
            stores: s.stores.clone(),
            safety: s.safety.clone(),
            identity: s.identity.clone(),
            editors: s.editors.clone(),
            doc_service: s.doc_service.clone(),
            search: Some(search),
        });
    }

    async fn chunks(
        &self,
        who: &str,
        project: &str,
        inputs: Value,
        extra: Value,
    ) -> Result<Value, String> {
        let mut a = json!({"mount_id": project, "inputs": inputs});
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        let p: rmcp::handler::server::wrapper::Parameters<SearchChunksArgs> = args(a);
        self.fx.keep(tool_json(self.server(who).search_chunks(p).await))
    }

    async fn images(
        &self,
        who: &str,
        project: &str,
        inputs: Value,
        extra: Value,
    ) -> Result<Value, String> {
        let mut a = json!({"mount_id": project, "inputs": inputs});
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        let p: rmcp::handler::server::wrapper::Parameters<SearchImagesArgs> = args(a);
        self.fx.keep(tool_json(self.server(who).search_images(p).await))
    }

    /// Stores image vectors for `path`: (number, caption, picture bytes), with the
    /// image model of the project.
    async fn index_pictures(&self, path: &str, pictures: &[(u32, &str, &str)]) {
        let images: Vec<crate::search::ImageInput> = pictures
            .iter()
            .map(|(number, text, bytes)| crate::search::ImageInput {
                number: *number,
                text: (*text).into(),
                data_uri: format!(
                    "data:image/png;base64,{}",
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
                ),
            })
            .collect();
        let search = self.fx.state.search.as_ref().unwrap();
        let n = search.index_images_checked(PROJ, path, &images, &|| true).await.unwrap();
        assert_eq!(n, Some(pictures.len()));
    }

    /// Writes `files` and waits until the vectors and the full text index show them.
    async fn put(&self, project: &str, files: &[(&str, &str)]) {
        for (path, text) in files {
            self.write(project, path, text).await;
            let word = text.split(|c: char| !c.is_alphanumeric()).find(|w| !w.is_empty()).unwrap();
            let search = self.fx.state.search.as_ref().unwrap();
            let mut seen = false;
            for _ in 0..200 {
                if let Ok(hits) = search.query_bm25(project, word, 50).await
                    && hits.iter().any(|h| h.path == *path)
                {
                    seen = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            assert!(seen, "the full text index never showed {path}");
        }
    }

    /// Writes one file and waits for its vectors only (a `rag` project has no full text).
    async fn put_vectors_only(&self, project: &str, path: &str, text: &str) {
        self.write(project, path, text).await;
    }

    async fn png(&self, project: &str, path: &str, bytes: &[u8]) {
        let client = self.fx.state.stores.client(project).await.unwrap();
        client.write_bytes_atomic(path, bytes).await.unwrap();
    }
}

/// A `both` platform with one 8 wide text model on `proj`, keyed vectors and the
/// four short files every test searches.
async fn zoo() -> World {
    let w = World::in_mode("both", &[("mk", "text_embedding", 8)]).await;
    w.stub.set(Answer::Keyed);
    w.mode(PROJ, "both").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("mk").await})).await;
    w.put(
        PROJ,
        &[
            ("/a.md", "ocelot sapphire"),
            ("/b.md", "gazelle granite"),
            ("/c.md", "ocelot ocelot violet"),
            ("/d.md", "meadow harbor lantern"),
        ],
    )
    .await;
    w
}

/// Four sections of 400 words each, one word per section, so chunk neighbours
/// have known vectors (each top level section is a chunk).
fn long_doc() -> String {
    let part = |n: u32, w: &str| format!("# Part {n}\n\n{}\n\n", format!("{w} ").repeat(400));
    format!(
        "{}{}{}{}",
        part(1, "ocelot"),
        part(2, "gazelle"),
        part(3, "sapphire"),
        part(4, "granite")
    )
}

impl World {
    /// Writes `/long.md` and waits for its entries.
    async fn put_long(&self) {
        self.put(PROJ, &[("/long.md", &long_doc())]).await;
        let v = self.vectors(BOB, PROJ, "/long.md", json!({"limit": 100})).await.unwrap();
        assert_eq!(v["vectors"].as_array().unwrap().len(), 4, "one chunk per part: {v}");
    }
}

fn lines(out: &Value, group: usize) -> Vec<Value> {
    out["inputs"][group]["lines"].as_array().unwrap().clone()
}

fn docs(out: &Value, group: usize) -> Vec<String> {
    lines(out, group).iter().map(|l| l["document"].as_str().unwrap().to_string()).collect()
}

/// E2E-001: one text input finds its chunks, best first, with the line shape.
#[tokio::test]
async fn e2e_0109_001_one_text_input() {
    let w = zoo().await;
    let out = w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap();
    assert_eq!(out["mode_used"], "both");
    assert_eq!(out["inputs"].as_array().unwrap().len(), 1);
    assert_eq!(out["inputs"][0]["label"], "S1");
    assert_eq!(out["inputs"][0]["kind"], "text");
    let l = lines(&out, 0);
    assert_eq!(&docs(&out, 0)[..2], ["/c.md", "/a.md"], "{out}");
    assert_eq!(l[0]["chunk"], 1);
    assert_eq!(l[0]["text"], "ocelot ocelot violet", "the chunk text without its header line");
    assert!(l[0]["heading_path"].is_array());
    let s0 = l[0]["similarity"].as_f64().unwrap();
    let s1 = l[1]["similarity"].as_f64().unwrap();
    assert!(s0 > s1 && s1 > 0.5 && s0 < 1.0, "{s0} {s1}");
    assert!(l[0].get("context").is_none_or(|c| c == &json!([])), "no context asked");
    let two = w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({"top_k": 2})).await.unwrap();
    assert_eq!(lines(&two, 0).len(), 2);
    assert_eq!(l.len(), 4, "every chunk has a vector, the default top_k is 10");
}

/// E2E-002: two texts and an image, grouped S1, S2, I1; the image becomes text.
#[tokio::test]
async fn e2e_0109_002_texts_and_an_image() {
    let mut w = zoo().await;
    w.set_converter(Some(Named::of(&[("chart.png", "# Chart\n\ngazelle granite sales")])));
    w.png(PROJ, "/chart.png", b"png-bytes").await;
    let before = w.stub.calls.lock().unwrap().len();
    let out = w
        .chunks(
            BOB,
            PROJ,
            json!([{"text": "ocelot"}, {"text": "violet"}, {"image": "/chart.png"}]),
            json!({"top_k": 3}),
        )
        .await
        .unwrap();
    let labels: Vec<_> =
        out["inputs"].as_array().unwrap().iter().map(|g| g["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["S1", "S2", "I1"]);
    assert_eq!(out["inputs"][2]["kind"], "image");
    assert_eq!(out["inputs"][2]["query_text"], "# Chart\n\ngazelle granite sales");
    assert_eq!(docs(&out, 2)[0], "/b.md", "the picture reads as its text: {out}");
    assert_eq!(docs(&out, 1)[0], "/c.md");
    let sent: Vec<String> =
        w.stub.calls.lock().unwrap()[before..].iter().map(|(_, i)| i.clone()).collect();
    assert!(sent.contains(&"# Chart\n\ngazelle granite sales".to_string()), "{sent:?}");
    assert!(out["inputs"][0].get("query_text").is_none(), "only an image shows its text");
}

/// E2E-003: chunks before and after, marked as context, each with a similarity.
#[tokio::test]
async fn e2e_0109_003_context_before_and_after() {
    let w = zoo().await;
    w.put_long().await;
    let out = w
        .chunks(
            BOB,
            PROJ,
            json!([{"text": "gazelle"}]),
            json!({"top_k": 1, "context_before": 1, "context_after": 1}),
        )
        .await
        .unwrap();
    let hit = &lines(&out, 0)[0];
    assert_eq!(hit["document"], "/long.md", "{out}");
    assert!(hit["text"].as_str().unwrap().contains("gazelle"), "{hit}");
    let n = hit["chunk"].as_i64().unwrap();
    let ctx = hit["context"].as_array().unwrap();
    assert_eq!(ctx.len(), 2, "{hit}");
    assert_eq!(
        (ctx[0]["position"].as_str(), ctx[0]["chunk"].as_i64()),
        (Some("before"), Some(n - 1))
    );
    assert_eq!(
        (ctx[1]["position"].as_str(), ctx[1]["chunk"].as_i64()),
        (Some("after"), Some(n + 1))
    );
    assert!(ctx[0]["text"].as_str().unwrap().contains("ocelot"), "{hit}");
    assert!(ctx[1]["text"].as_str().unwrap().contains("sapphire"), "{hit}");
    for c in ctx {
        assert_eq!(c["role"], "context");
        let s = c["similarity"].as_f64().unwrap();
        assert!((0.0..hit["similarity"].as_f64().unwrap()).contains(&s), "{c} against {hit}");
    }
    // Two chunks around: the first chunk has nothing before it.
    let first = w
        .chunks(
            BOB,
            PROJ,
            json!([{"text": "ocelot ocelot ocelot ocelot ocelot"}]),
            json!({"top_k": 4, "context_before": 2}),
        )
        .await
        .unwrap();
    let long: Vec<_> =
        lines(&first, 0).into_iter().filter(|l| l["document"] == "/long.md").collect();
    assert!(!long.is_empty());
}

/// E2E-005: invalid arguments of `search.chunks`, refused before any call.
#[tokio::test]
async fn e2e_0109_005_invalid_arguments() {
    let w = zoo().await;
    let before = w.stub.calls.lock().unwrap().len();
    let long = "x".repeat(8001);
    let cases: Vec<(Value, Value, &str)> = vec![
        (json!([]), json!({}), "inputs must hold 1 to 10 entries"),
        (
            json!((0..11).map(|_| json!({"text": "a"})).collect::<Vec<_>>()),
            json!({}),
            "inputs must hold 1 to 10 entries",
        ),
        (json!([{}]), json!({}), "input 1 must hold exactly one of text and image"),
        (
            json!([{"text": "a", "image": "/x.png"}]),
            json!({}),
            "input 1 must hold exactly one of text and image",
        ),
        (json!([{"text": "a"}, {"text": "   "}]), json!({}), "input 2 text is empty"),
        (json!([{"text": long}]), json!({}), "input 1 text is over 8000 characters"),
        (json!([{"text": "a"}]), json!({"top_k": 0}), "top_k must be between 1 and 50"),
        (json!([{"text": "a"}]), json!({"top_k": 51}), "top_k must be between 1 and 50"),
        (
            json!([{"text": "a"}]),
            json!({"context_before": 6}),
            "context_before must be between 0 and 5",
        ),
        (
            json!([{"text": "a"}]),
            json!({"context_after": -1}),
            "context_after must be between 0 and 5",
        ),
    ];
    for (inputs, extra, text) in cases {
        let e = w.chunks(BOB, PROJ, inputs.clone(), extra.clone()).await.unwrap_err();
        assert!(e.contains("ERR_INVALID_ARGUMENT") && e.contains(text), "{inputs} {extra}: {e}");
    }
    assert_eq!(w.stub.calls.lock().unwrap().len(), before, "no embedding call was made");
    let image_cases: Vec<(Value, Value, &str)> = vec![
        (json!([]), json!({}), "inputs must hold 1 to 10 entries"),
        (
            json!([{"text": "a", "image": "/x.png"}]),
            json!({}),
            "input 1 must hold exactly one of text and image",
        ),
        (json!([{"text": "  "}]), json!({}), "input 1 text is empty"),
        (json!([{"text": "a"}]), json!({"top_k": 0}), "top_k must be between 1 and 50"),
        (json!([{"text": "a"}]), json!({"top_k": 51}), "top_k must be between 1 and 50"),
    ];
    for (inputs, extra, text) in image_cases {
        let e = w.images(BOB, PROJ, inputs.clone(), extra.clone()).await.unwrap_err();
        assert!(e.contains("ERR_INVALID_ARGUMENT") && e.contains(text), "{inputs} {extra}: {e}");
    }
    assert_eq!(w.stub.calls.lock().unwrap().len(), before, "no embedding call was made");
}

/// E2E-006: a missing converter, a failing one, an empty conversion, a service down.
#[tokio::test]
async fn e2e_0109_006_missing_dependencies() {
    let mut w = zoo().await;
    w.png(PROJ, "/chart.png", b"png-bytes").await;
    let img = json!([{"image": "/chart.png"}]);
    let e = w.chunks(BOB, PROJ, img.clone(), json!({})).await.unwrap_err();
    assert!(e.contains("ERR_NOT_SUPPORTED"), "{e}");
    assert!(
        e.contains("no document converter is set up, an image cannot be turned into text"),
        "{e}"
    );
    w.set_converter(Some(Arc::new(StubDocService::failing(ToolError::internal(
        "converter exploded",
    )))));
    let e = w.chunks(BOB, PROJ, img.clone(), json!({})).await.unwrap_err();
    assert!(e.contains("converter exploded"), "{e}");
    w.set_converter(Some(Named::of(&[("chart.png", "  \n ")])));
    let e = w.chunks(BOB, PROJ, img.clone(), json!({})).await.unwrap_err();
    assert!(
        e.contains("ERR_INVALID_ARGUMENT")
            && e.contains("the converter found no text in /chart.png"),
        "{e}"
    );
    w.stub.set(Answer::Down);
    let e = w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap_err();
    assert!(e.contains("ERR_INTERNAL_ERROR"), "{e}");
    w.stub.set(Answer::Keyed);
    assert!(
        w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.is_ok(),
        "nothing broke"
    );
}

/// E2E-007: a mode without vectors and a platform without search are refused.
#[tokio::test]
async fn e2e_0109_007_mode_and_search_off() {
    let w = zoo().await;
    w.mode(PROJ, "none").await;
    let e = w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap_err();
    assert!(
        e.contains("ERR_NOT_SUPPORTED")
            && e.contains("the index mode of the project has no vectors"),
        "{e}"
    );
    let e = w.images(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap_err();
    assert!(
        e.contains("ERR_NOT_SUPPORTED")
            && e.contains("the index mode of the project has no vectors"),
        "{e}"
    );
    let off = crate::settings::e2e::Fx::new().await;
    let p: rmcp::handler::server::wrapper::Parameters<SearchChunksArgs> =
        args(json!({"mount_id": PROJ, "inputs": [{"text": "a"}]}));
    let e = tool_json(off.server(BOB).search_chunks(p).await).unwrap_err();
    assert!(e.contains("ERR_NOT_SUPPORTED") && e.contains("search is not enabled"), "{e}");
    let p: rmcp::handler::server::wrapper::Parameters<SearchImagesArgs> =
        args(json!({"mount_id": PROJ, "inputs": [{"text": "a"}]}));
    let e = tool_json(off.server(BOB).search_images(p).await).unwrap_err();
    assert!(e.contains("ERR_NOT_SUPPORTED") && e.contains("search is not enabled"), "{e}");
}

/// E2E-008: members only, administrators included.
#[tokio::test]
async fn e2e_0109_008_membership() {
    let w = zoo().await;
    for who in [CAROL, ALICE] {
        let e = w.chunks(who, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap_err();
        assert!(e.contains("ERR_FORBIDDEN"), "{who}: {e}");
        let e = w.images(who, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap_err();
        assert!(e.contains("ERR_FORBIDDEN"), "{who}: {e}");
    }
    assert!(w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.is_ok());
}

/// E2E-009: an image path that is missing, a folder, not an image, too big.
#[tokio::test]
async fn e2e_0109_009_image_files() {
    let mut w = zoo().await;
    w.set_converter(Some(Named::of(&[("ok.png", "ocelot")])));
    w.png(PROJ, "/ok.png", b"x").await;
    w.png(PROJ, "/big.png", &vec![0u8; 10 * 1024 * 1024 + 1]).await;
    let client = w.fx.state.stores.client(PROJ).await.unwrap();
    client.write_text_atomic("/dir/note.md", "hello").await.unwrap();
    let before = w.stub.calls.lock().unwrap().len();
    for (path, code, text) in [
        ("/missing.png", "ERR_NOT_FOUND", "not a file: /missing.png"),
        (
            "/dir",
            "ERR_INVALID_ARGUMENT",
            "input 2 image must be a png, jpg, jpeg, gif, webp or bmp file",
        ),
        (
            "/a.md",
            "ERR_INVALID_ARGUMENT",
            "input 2 image must be a png, jpg, jpeg, gif, webp or bmp file",
        ),
        ("/big.png", "ERR_INVALID_ARGUMENT", "input 2 image is over 10485760 bytes"),
    ] {
        let e = w
            .chunks(BOB, PROJ, json!([{"image": "/ok.png"}, {"image": path}]), json!({}))
            .await
            .unwrap_err();
        assert!(e.contains(code) && e.contains(text), "{path}: {e}");
    }
    assert_eq!(w.stub.calls.lock().unwrap().len(), before, "the first image was not embedded");
}

/// E2E-010: the same call twenty times, ties included.
#[tokio::test]
async fn e2e_0109_010_ties_are_deterministic() {
    let w = zoo().await;
    w.put(
        PROJ,
        &[("/t1.md", "meadow meadow"), ("/t2.md", "meadow meadow"), ("/t3.md", "meadow meadow")],
    )
    .await;
    let first = w.chunks(BOB, PROJ, json!([{"text": "meadow"}]), json!({})).await.unwrap();
    let order = docs(&first, 0);
    let ties: Vec<&String> = order.iter().filter(|d| d.starts_with("/t")).collect();
    assert_eq!(ties.len(), 3);
    for _ in 0..20 {
        let again = w.chunks(BOB, PROJ, json!([{"text": "meadow"}]), json!({})).await.unwrap();
        assert_eq!(again, first);
    }
}

/// E2E-011: a chunk found by both lists is one line.
#[tokio::test]
async fn e2e_0109_011_one_line_per_chunk() {
    let w = zoo().await;
    let out = w.chunks(BOB, PROJ, json!([{"text": "ocelot violet"}]), json!({})).await.unwrap();
    let mut keys: Vec<(String, i64)> = lines(&out, 0)
        .iter()
        .map(|l| (l["document"].as_str().unwrap().to_string(), l["chunk"].as_i64().unwrap()))
        .collect();
    let n = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), n, "{out}");
    assert_eq!(n, 4);
}

/// E2E-012: the keyword list answers alone when the dense list is empty.
#[tokio::test]
async fn e2e_0109_012_keyword_list_alone() {
    let w = zoo().await;
    let m16 = w.model("m16", "text_embedding", 16).await;
    w.profile(PROJ, json!({"text_model_id": m16})).await;
    let out = w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap();
    let mut found = docs(&out, 0);
    found.sort();
    assert_eq!(found, ["/a.md", "/c.md"], "only the keyword hits: {out}");
    for l in lines(&out, 0) {
        assert_eq!(l["similarity"], Value::Null, "no vector at that width: {l}");
    }
}

/// E2E-013: every similarity reads between 0 and 1.
#[tokio::test]
async fn e2e_0109_013_similarities_in_range() {
    let w = zoo().await;
    w.put_long().await;
    let out = w
        .chunks(
            BOB,
            PROJ,
            json!([{"text": "ocelot"}, {"text": "nothing in common qqq"}]),
            json!({"context_before": 2, "context_after": 2}),
        )
        .await
        .unwrap();
    let mut n = 0;
    for g in 0..2 {
        for l in lines(&out, g) {
            let all = std::iter::once(l.clone())
                .chain(l["context"].as_array().cloned().unwrap_or_default());
            for x in all {
                let s = x["similarity"].as_f64().unwrap();
                assert!((0.0..=1.0).contains(&s), "{x}");
                n += 1;
            }
        }
    }
    assert!(n > 10, "{n} {out}");
}

/// E2E-014: a context chunk is never a line and never context twice.
#[tokio::test]
async fn e2e_0109_014_context_never_repeats() {
    let w = zoo().await;
    w.put_long().await;
    for query in ["gazelle sapphire", "gazelle granite"] {
        let out = w
            .chunks(
                BOB,
                PROJ,
                json!([{"text": query}]),
                json!({"top_k": 2, "context_before": 1, "context_after": 1}),
            )
            .await
            .unwrap();
        let mut seen = Vec::new();
        for l in lines(&out, 0) {
            seen.push((l["document"].as_str().unwrap().to_string(), l["chunk"].as_i64().unwrap()));
            for c in l["context"].as_array().unwrap() {
                seen.push((
                    l["document"].as_str().unwrap().to_string(),
                    c["chunk"].as_i64().unwrap(),
                ));
            }
        }
        let n = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), n, "{query}: {out}");
    }
}

/// E2E-015: a project in mode `bm25` is refused; in mode `rag` only the dense list runs.
#[tokio::test]
async fn e2e_0109_015_project_modes() {
    let w = zoo().await;
    w.mode(PROJ, "bm25").await;
    let e = w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap_err();
    assert!(
        e.contains("ERR_NOT_SUPPORTED")
            && e.contains("the index mode of the project has no vectors"),
        "{e}"
    );
    w.mode(PROJ, "rag").await;
    w.put_vectors_only(PROJ, "/r.md", "granite granite").await;
    let out = w.chunks(BOB, PROJ, json!([{"text": "granite"}]), json!({})).await.unwrap();
    assert_eq!(out["mode_used"], "rag");
    assert_eq!(docs(&out, 0)[0], "/r.md", "{out}");
}

/// A `both` platform with a text model and an 8 wide image model, two pictures stored.
async fn gallery() -> World {
    let w =
        World::in_mode("both", &[("mk", "text_embedding", 8), ("imk", "image_embedding", 8)]).await;
    w.stub.set(Answer::Keyed);
    w.mode(PROJ, "both").await;
    w.profile(
        PROJ,
        json!({"text_model_id": w.model_id("mk").await, "image_model_id": w.model_id("imk").await}),
    )
    .await;
    w.png(PROJ, "/doc.pdf", b"%PDF-1.4").await;
    w.png(PROJ, "/other.pdf", b"%PDF-1.4").await;
    w.index_pictures(
        "/doc.pdf",
        &[(1, "Revenue chart", "ocelot"), (2, "Gazelle photo", "gazelle gazelle")],
    )
    .await;
    w.index_pictures("/other.pdf", &[(1, "Granite wall", "granite")]).await;
    w
}

/// E2E-004: search the images by a picture and by a text.
#[tokio::test]
async fn e2e_0109_004_search_images() {
    let w = gallery().await;
    w.png(PROJ, "/query.png", b"gazelle").await;
    let before = w.stub.calls.lock().unwrap().len();
    let out = w
        .images(
            BOB,
            PROJ,
            json!([{"image": "/query.png"}, {"text": "ocelot"}]),
            json!({"top_k": 2}),
        )
        .await
        .unwrap();
    let labels: Vec<_> =
        out["inputs"].as_array().unwrap().iter().map(|g| g["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["I1", "S1"], "a picture is I, a text is S, each counted in its kind");
    let by_picture = lines(&out, 0);
    assert_eq!(by_picture.len(), 2, "{out}");
    assert_eq!(by_picture[0]["document"], "/doc.pdf");
    assert_eq!(by_picture[0]["image"], 2);
    assert_eq!(by_picture[0]["text"], "Gazelle photo");
    assert!(by_picture[0]["similarity"].as_f64().unwrap() > 0.99, "{out}");
    assert!(by_picture[0]["similarity"].as_f64() > by_picture[1]["similarity"].as_f64());
    let by_text = lines(&out, 1);
    assert_eq!(
        (by_text[0]["document"].as_str(), by_text[0]["image"].as_i64()),
        (Some("/doc.pdf"), Some(1))
    );
    // The picture went to the image model as a data URI, the text as it is.
    let calls = w.stub.calls.lock().unwrap()[before..].to_vec();
    assert!(calls.iter().all(|(m, _)| m == "imk"), "{calls:?}");
    assert!(calls[0].1.starts_with("data:image/png;base64,"), "{calls:?}");
    assert_eq!(calls[1].1, "ocelot");
    // A call writes nothing: the stored images are as they were.
    let v = w.vectors(BOB, PROJ, "/doc.pdf", json!({"kind": "image"})).await.unwrap();
    assert_eq!(v["vectors"].as_array().unwrap().len(), 2);
}

/// What `search.query` answered on the code of v0.13.0 for the same corpus
/// (the four files of `zoo`), recorded before this spec changed anything.
const RECORDED_QUERY: [(&str, &str); 3] = [
    (
        "bm25",
        r#"{"results":[{"path":"/c.md","score":2.0537588596343994,"chunk":"ocelot ocelot violet","rank":1,"chunk_number":1,"heading_path":[]},{"path":"/a.md","score":0.7361700534820557,"chunk":"ocelot sapphire","rank":2,"chunk_number":1,"heading_path":[]}],"mode_used":"bm25","warning":null}"#,
    ),
    (
        "rag",
        r#"{"results":[{"path":"/c.md","score":0.0,"chunk":"ocelot ocelot violet","rank":1,"chunk_number":1,"heading_path":[]},{"path":"/a.md","score":-0.4142135381698608,"chunk":"ocelot sapphire","rank":2,"chunk_number":1,"heading_path":[]},{"path":"/b.md","score":-1.0,"chunk":"gazelle granite","rank":3,"chunk_number":1,"heading_path":[]},{"path":"/d.md","score":-1.2360680103302002,"chunk":"meadow harbor lantern","rank":4,"chunk_number":1,"heading_path":[]}],"mode_used":"rag","warning":null}"#,
    ),
    (
        "both",
        r#"{"results":[{"path":"/c.md","score":0.032786883413791656,"chunk":"ocelot ocelot violet","rank":1,"chunk_number":1,"heading_path":[]},{"path":"/a.md","score":0.032258063554763794,"chunk":"ocelot sapphire","rank":2,"chunk_number":1,"heading_path":[]},{"path":"/b.md","score":0.01587301678955555,"chunk":"gazelle granite","rank":3,"chunk_number":1,"heading_path":[]},{"path":"/d.md","score":0.015625,"chunk":"meadow harbor lantern","rank":4,"chunk_number":1,"heading_path":[]}],"mode_used":"both","warning":null}"#,
    ),
];

/// E2E-016: `search.query` answers byte for byte what it answered before this
/// spec, in each mode. The scores are the engines' own, odd as they are (the
/// sqlite-vec one is `1 - L2 distance`): this spec must not touch them.
#[tokio::test]
async fn e2e_0109_016_search_query_is_byte_identical() {
    let w = zoo().await;
    for (mode, recorded) in RECORDED_QUERY {
        let p: rmcp::handler::server::wrapper::Parameters<crate::mcp::server::SearchQueryArgs> =
            args(json!({"mount_id": PROJ, "query": "ocelot violet", "mode": mode,
                "top_k": 10, "rerank": false}));
        let out = tool_json(w.server(BOB).search_query(p).await).unwrap();
        assert_eq!(serde_json::to_string(&out).unwrap(), recorded, "mode {mode}");
    }
}

/// E2E-018: both tools on PostgreSQL (pgvector), two widths side by side.
#[tokio::test]
async fn e2e_0109_018_the_tools_on_postgres() {
    use crate::storage::rel::{PoolSettings, PostgresRelationalDb};
    use sqlx::Row;
    let Some(dsn) = std::env::var("MCPFS_TEST_PG_DSN").ok().filter(|v| !v.trim().is_empty()) else {
        return;
    };
    let mut w = World::new(&[
        ("m8", "text_embedding", 8),
        ("m16", "text_embedding", 16),
        ("img8", "image_embedding", 8),
    ])
    .await;
    let schema = format!("fsa_p109_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
    let db = PostgresRelationalDb::connect(&dsn, &schema, PoolSettings::default()).await.unwrap();
    let targets = Arc::new(crate::settings::RegistryTargets {
        admin: w.fx.state.admin.clone(),
        fallback: w.fx.state.config.search.embedding.clone(),
    });
    let engine = crate::search::vector_pg::PostgresVectorBackend::new(
        db.pool().clone(),
        1024,
        Arc::new(reqwest::Client::new()),
        targets,
    )
    .await
    .unwrap();
    let sched = crate::search::ScheduledBackend::new(
        Arc::new(engine),
        w.fx.state.admin.derived_jobs().unwrap(),
        w.fx.state.stores.clone(),
        0,
        w.fx.state.config.search.chunking(),
    );
    w.set_search(Arc::new(sched));
    w.stub.set(Answer::Keyed);
    w.project("b").await;
    for (p, model) in [(PROJ, "m8"), ("b", "m16")] {
        w.mode(p, "rag").await;
        w.profile(p, json!({"text_model_id": w.model_id(model).await})).await;
    }
    w.profile(PROJ, json!({"image_model_id": w.model_id("img8").await})).await;
    w.write(PROJ, "/a.md", "ocelot sapphire").await;
    w.write(PROJ, "/b.md", "gazelle granite").await;
    w.write("b", "/a.md", "ocelot violet").await;

    let out = w.chunks(BOB, PROJ, json!([{"text": "gazelle"}]), json!({"top_k": 2})).await.unwrap();
    assert_eq!(out["mode_used"], "rag");
    let l = lines(&out, 0);
    assert_eq!(l[0]["document"], "/b.md");
    assert!(l[0]["similarity"].as_f64().unwrap() > l[1]["similarity"].as_f64().unwrap());
    let other = w.chunks(BOB, "b", json!([{"text": "ocelot"}]), json!({})).await.unwrap();
    assert_eq!(docs(&other, 0), ["/a.md"], "the other project, the other width: {other}");

    w.png(PROJ, "/doc.pdf", b"%PDF-1.4").await;
    w.index_pictures(
        "/doc.pdf",
        &[(1, "Revenue chart", "ocelot"), (2, "Gazelle photo", "gazelle")],
    )
    .await;
    let imgs = w.images(BOB, PROJ, json!([{"text": "gazelle"}]), json!({})).await.unwrap();
    let l = lines(&imgs, 0);
    assert_eq!((l[0]["document"].as_str(), l[0]["image"].as_i64()), (Some("/doc.pdf"), Some(2)));
    assert!(l[0]["similarity"].as_f64().unwrap() > 0.99, "{imgs}");

    let rows = sqlx::query(sqlx::AssertSqlSafe(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = current_schema() \
         AND table_name LIKE 'search\\_%' ORDER BY table_name",
    ))
    .fetch_all(db.pool())
    .await
    .unwrap();
    let names: Vec<String> = rows.iter().map(|r| r.get("table_name")).collect();
    assert_eq!(names, ["search_chunks", "search_chunks_16", "search_chunks_8", "search_images_8"]);
    let drop = format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE");
    let _ = sqlx::query(sqlx::AssertSqlSafe(drop)).execute(db.pool()).await;
}
