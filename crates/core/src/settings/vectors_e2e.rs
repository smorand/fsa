//! SPEC-0108 functional tests of the vectors at the project dimension and of
//! their recalculation, driven through the real `McpServer` tools on the
//! SPEC-0102 fixture with `rag` search and a fake embedding service that
//! answers per model name. Names are `e2e_0108_NNN_slug`.

use super::e2e::{Fx, Searching};
use crate::mcp::server::{
    GetVectorsArgs, McpServer, ProjectIdArgs, RecalculateStartArgs, SetIndexingProfileArgs,
};
use crate::search::derived_e2e::{args, tool_json};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const ALICE: &str = "admin@example.com";
const BOB: &str = "bob@example.com";
const CAROL: &str = "carol@example.com";
const PROJ: &str = "proj";

/// How the stub answers.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Answer {
    Normal,
    /// This many values, whatever the model declares.
    Short(usize),
    Zero,
    /// A value too large for a 32 bit float.
    Huge,
    Down,
    /// The vector follows the words of the input (SPEC-0109): component `j` is
    /// 0.01 plus the number of times `KEYED_WORDS[j % 8]` occurs in it; a `data:`
    /// input is decoded first, so a picture is what its bytes say.
    Keyed,
}

/// The vocabulary of [`Answer::Keyed`].
pub(crate) const KEYED_WORDS: [&str; 8] =
    ["ocelot", "gazelle", "sapphire", "granite", "violet", "harbor", "lantern", "meadow"];

/// The vector of [`Answer::Keyed`] for `input`, `n` wide.
pub(crate) fn keyed_vector(input: &str, n: usize) -> Vec<f64> {
    let text = match input.strip_prefix("data:").and_then(|d| d.split_once("base64,")) {
        Some((_, b64)) => {
            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                .unwrap_or_default();
            String::from_utf8_lossy(&bytes).into_owned()
        }
        None => input.to_string(),
    };
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric()).collect();
    (0..n)
        .map(|j| 0.01 + words.iter().filter(|w| **w == KEYED_WORDS[j % 8]).count() as f64)
        .collect()
}

/// A fake `POST /v1/embeddings` service: the width of the answer follows the
/// model name of the request.
pub(crate) struct ModelStub {
    pub base: String,
    pub dims: Arc<Mutex<HashMap<String, usize>>>,
    pub mode: Arc<Mutex<Answer>>,
    /// (model, first input as text) of every call.
    pub calls: Arc<Mutex<Vec<(String, String)>>>,
    /// Delay of every answer in milliseconds.
    pub delay_ms: Arc<Mutex<u64>>,
}

impl ModelStub {
    pub async fn start(dims: &[(&str, usize)]) -> Self {
        type S = (
            Arc<Mutex<HashMap<String, usize>>>,
            Arc<Mutex<Answer>>,
            Arc<Mutex<Vec<(String, String)>>>,
            Arc<Mutex<u64>>,
        );
        async fn answer(
            axum::extract::State((dims, mode, calls, delay)): axum::extract::State<S>,
            axum::Json(body): axum::Json<Value>,
        ) -> (axum::http::StatusCode, axum::Json<Value>) {
            let model = body["model"].as_str().unwrap_or("").to_string();
            let input = body["input"][0].as_str().unwrap_or("").to_string();
            calls.lock().unwrap().push((model.clone(), input.clone()));
            let wait = *delay.lock().unwrap();
            if wait > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
            }
            let n = dims.lock().unwrap().get(&model).copied().unwrap_or(8);
            let v: Vec<f64> = match *mode.lock().unwrap() {
                Answer::Down => {
                    return (
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        axum::Json(json!({"error": "down"})),
                    );
                }
                Answer::Normal => (0..n).map(|i| (i + 1) as f64 / n as f64).collect(),
                Answer::Short(k) => (0..k).map(|i| (i + 1) as f64).collect(),
                Answer::Zero => vec![0.0; n],
                Answer::Huge => vec![1e39; n],
                Answer::Keyed => keyed_vector(&input, n),
            };
            (
                axum::http::StatusCode::OK,
                axum::Json(json!({"data": [{"embedding": v, "index": 0}]})),
            )
        }
        let state: S = (
            Arc::new(Mutex::new(dims.iter().map(|(k, v)| ((*k).to_string(), *v)).collect())),
            Arc::new(Mutex::new(Answer::Normal)),
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(Mutex::new(0)),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = axum::Router::new()
            .route("/v1/embeddings", axum::routing::post(answer))
            .with_state(state.clone());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            dims: state.0,
            mode: state.1,
            calls: state.2,
            delay_ms: state.3,
        }
    }

    pub fn set(&self, a: Answer) {
        *self.mode.lock().unwrap() = a;
    }

    pub fn models_called(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().map(|(m, _)| m.clone()).collect()
    }
}

/// The platform of the tests: `rag` search, the YAML model `yaml-model` at 1024,
/// a provider `Local` on the stub and the models of `dims` registered on it.
pub(crate) struct World {
    pub fx: Fx,
    pub stub: ModelStub,
    pub provider: String,
}

impl World {
    pub async fn new(models: &[(&str, &str, usize)]) -> Self {
        Self::in_mode("rag", models).await
    }

    /// The same platform running `search.mode` `mode` (SPEC-0109 needs `both`).
    pub async fn in_mode(mode: &'static str, models: &[(&str, &str, usize)]) -> Self {
        let mut dims: Vec<(&str, usize)> = vec![("yaml-model", 1024)];
        dims.extend(models.iter().map(|(n, _, d)| (*n, *d)));
        let stub = ModelStub::start(&dims).await;
        let fx = Fx::searching(Searching {
            mode,
            yaml_url: format!("{}/v1/embeddings", stub.base),
            rerank_url: None,
        })
        .await;
        let p = fx
            .create(json!({"name": "Local", "endpoint": format!("{}/v1", stub.base)}))
            .await
            .unwrap();
        let provider = p["provider_id"].as_str().unwrap().to_string();
        let w = Self { fx, stub, provider };
        for (name, kind, dim) in models {
            w.model(name, kind, *dim).await;
        }
        w
    }

    /// Registers a model on the provider; its id.
    pub async fn model(&self, name: &str, kind: &str, dim: usize) -> String {
        self.stub.dims.lock().unwrap().insert(name.to_string(), dim);
        let m = self
            .fx
            .model(json!({"provider_id": self.provider, "name": name, "kind": kind,
                "dimension": dim}))
            .await
            .unwrap();
        m["model_id"].as_str().unwrap().to_string()
    }

    pub async fn model_id(&self, name: &str) -> String {
        self.fx
            .models()
            .await
            .iter()
            .find(|m| m["name"] == name)
            .map(|m| m["model_id"].as_str().unwrap().to_string())
            .unwrap()
    }

    /// A second project owned by Bob, volume included.
    pub async fn project(&self, id: &str) {
        self.fx.state.admin.create_project(id, BOB).await.unwrap();
        self.fx.state.stores.provision_volume(id).await.unwrap();
    }

    pub fn server(&self, who: &str) -> McpServer {
        self.fx.server(who)
    }

    /// `admin.set_index_mode` by the administrator.
    pub async fn mode(&self, project: &str, mode: &str) {
        let a = args(json!({"project_id": project, "mode": mode}));
        tool_json(self.server(ALICE).admin_set_index_mode(a).await).expect("mode set");
    }

    /// `admin.set_indexing_profile` by Bob.
    pub async fn profile(&self, project: &str, a: Value) -> Value {
        let mut a = a;
        a["project_id"] = json!(project);
        let p: rmcp::handler::server::wrapper::Parameters<SetIndexingProfileArgs> = args(a);
        tool_json(self.server(BOB).admin_set_indexing_profile(p).await).expect("profile set")
    }

    /// `fs.write` by Bob, then waits until `get_vectors` shows the new text.
    pub async fn write(&self, project: &str, path: &str, text: &str) {
        let a = args(json!({"mount_id": project, "path": path, "content": text,
            "overwrite": true}));
        tool_json(self.server(BOB).fs_write(a).await).expect("write");
        let want = text.lines().next().unwrap_or("").to_string();
        for _ in 0..200 {
            if let Ok(v) = self.vectors(BOB, project, path, json!({})).await
                && v["vectors"]
                    .as_array()
                    .is_some_and(|l| l.iter().any(|e| e["text"].as_str().unwrap().contains(&want)))
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("the vectors of {project}{path} never showed {want:?}");
    }

    pub async fn vectors(
        &self,
        who: &str,
        project: &str,
        path: &str,
        extra: Value,
    ) -> Result<Value, String> {
        let mut a = json!({"mount_id": project, "path": path});
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        let p: rmcp::handler::server::wrapper::Parameters<GetVectorsArgs> = args(a);
        self.fx.keep(tool_json(self.server(who).fs_get_vectors(p).await))
    }

    pub async fn status_of(&self, project: &str) -> Value {
        let p: rmcp::handler::server::wrapper::Parameters<ProjectIdArgs> =
            args(json!({"project_id": project}));
        tool_json(self.server(BOB).admin_get_indexing_profile(p).await).unwrap()
    }

    pub async fn query(&self, project: &str, word: &str) -> Vec<String> {
        let a = args(json!({"mount_id": project, "query": word, "mode": "rag", "top_k": 10}));
        let v = tool_json(self.server(BOB).search_query(a).await).unwrap();
        v["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["path"].as_str().unwrap().into())
            .collect()
    }

    pub async fn stats(&self, project: &str) -> Value {
        let a = args(json!({"mount_id": project}));
        tool_json(self.server(BOB).search_status(a).await).unwrap()
    }
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

/// E2E-001: two projects, two widths, side by side.
#[tokio::test]
async fn e2e_0108_001_two_projects_two_widths() {
    let w = World::new(&[("m384", "text_embedding", 384), ("m1024", "text_embedding", 1024)]).await;
    w.project("b").await;
    w.mode(PROJ, "rag").await;
    w.mode("b", "rag").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m384").await})).await;
    w.profile("b", json!({"text_model_id": w.model_id("m1024").await})).await;
    w.write(PROJ, "/a.md", "alpha ocelot lives here").await;
    w.write("b", "/b.md", "beta gazelle runs there").await;

    let a = w.vectors(BOB, PROJ, "/a.md", json!({})).await.unwrap();
    assert_eq!(a["dimension"], 384);
    assert_eq!(floats(&a["vectors"][0]["vector"]).len(), 384);
    let b = w.vectors(BOB, "b", "/b.md", json!({})).await.unwrap();
    assert_eq!(b["dimension"], 1024);
    assert_eq!(floats(&b["vectors"][0]["vector"]).len(), 1024);
    assert_eq!(w.query(PROJ, "ocelot").await, vec!["/a.md".to_string()]);
    assert_eq!(w.query("b", "gazelle").await, vec!["/b.md".to_string()]);
    let called = w.stub.models_called();
    assert!(
        called.iter().any(|m| m == "m384") && called.iter().any(|m| m == "m1024"),
        "{called:?}"
    );
    assert!(!called.iter().any(|m| m == "yaml-model"), "the profile models only: {called:?}");
    // A query writes nothing: the vectors are as before.
    assert_eq!(w.vectors(BOB, PROJ, "/a.md", json!({})).await.unwrap(), a);
}

/// E2E-005: no profile, the server default model then the configuration.
#[tokio::test]
async fn e2e_0108_005_no_profile_falls_back() {
    let w = World::new(&[("dflt", "text_embedding", 1024)]).await;
    w.mode(PROJ, "rag").await;
    w.write(PROJ, "/a.md", "first page").await;
    assert_eq!(w.stub.models_called().last().map(String::as_str), Some("yaml-model"));
    let d = w.model_id("dflt").await;
    w.fx.set_default("text_embedding", &d).await.unwrap();
    w.write(PROJ, "/b.md", "second page").await;
    assert_eq!(w.stub.models_called().last().map(String::as_str), Some("dflt"));
    let v = w.vectors(BOB, PROJ, "/b.md", json!({})).await.unwrap();
    assert_eq!(v["dimension"], 1024);
}

/// E2E-009: an unusable answer is refused and nothing is stored for the file.
#[tokio::test]
async fn e2e_0108_009_unusable_vectors_are_refused() {
    let w = World::new(&[("m384", "text_embedding", 384)]).await;
    w.mode(PROJ, "rag").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m384").await})).await;
    for (answer, text) in [
        (Answer::Short(3), "embedding has 3 values, the model declares 384"),
        (Answer::Zero, "embedding is a zero vector"),
        (Answer::Huge, "embedding holds a value that is not a finite number"),
    ] {
        w.stub.set(answer);
        let job = w.fx.save("/bad.md", "some words").await;
        assert_eq!(job["state"], "failed", "{job}");
        assert!(job["reason"].as_str().unwrap().contains(text), "{job}");
        let v = w.vectors(BOB, PROJ, "/bad.md", json!({})).await.unwrap();
        assert_eq!(v["vectors"], json!([]), "nothing stored");
    }
}

/// E2E-010: reading vectors needs membership.
#[tokio::test]
async fn e2e_0108_010_reading_needs_membership() {
    let w = World::new(&[]).await;
    w.mode(PROJ, "rag").await;
    w.write(PROJ, "/a.md", "alpha").await;
    let e = w.vectors(CAROL, PROJ, "/a.md", json!({})).await.unwrap_err();
    assert!(e.contains("ERR_FORBIDDEN"), "{e}");
    assert!(w.vectors(ALICE, PROJ, "/a.md", json!({})).await.is_err(), "admin has no file access");
    assert!(w.vectors(BOB, PROJ, "/a.md", json!({})).await.is_ok());
}

/// E2E-011: no file, no vector, search off, a mode without vectors.
#[tokio::test]
async fn e2e_0108_011_get_vectors_refusals() {
    let w = World::new(&[]).await;
    w.mode(PROJ, "rag").await;
    let e = w.vectors(BOB, PROJ, "/missing.md", json!({})).await.unwrap_err();
    assert!(e.contains("ERR_NOT_FOUND"), "{e}");
    let client = w.fx.state.stores.client(PROJ).await.unwrap();
    client.write_text_atomic("/quiet.md", "never indexed").await.unwrap();
    let v = w.vectors(BOB, PROJ, "/quiet.md", json!({})).await.unwrap();
    assert_eq!(v["vectors"], json!([]));
    assert_eq!(v["dimension"], Value::Null);
    assert_eq!(v["next_after"], Value::Null);
    for (k, bad) in
        [("kind", json!("audio")), ("limit", json!(0)), ("limit", json!(101)), ("after", json!(-1))]
    {
        let e = w.vectors(BOB, PROJ, "/quiet.md", json!({k: bad})).await.unwrap_err();
        assert!(e.contains("ERR_INVALID_ARGUMENT"), "{k}: {e}");
    }
    w.mode(PROJ, "none").await;
    let e = w.vectors(BOB, PROJ, "/quiet.md", json!({})).await.unwrap_err();
    assert!(e.contains("ERR_NOT_SUPPORTED") && e.contains("no vectors"), "{e}");
    let off = Fx::new().await;
    let a = args(json!({"mount_id": PROJ, "path": "/a.txt"}));
    let e = tool_json(off.server(BOB).fs_get_vectors(a).await).unwrap_err();
    assert!(e.contains("ERR_NOT_SUPPORTED") && e.contains("search is not enabled"), "{e}");
}

/// E2E-020: the counts see every dimension and a dimension change leaves nothing behind.
#[tokio::test]
async fn e2e_0108_020_a_dimension_change_leaves_nothing_behind() {
    let w = World::new(&[("m384", "text_embedding", 384), ("m16", "text_embedding", 16)]).await;
    w.mode(PROJ, "rag").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m384").await})).await;
    w.write(PROJ, "/a.md", "alpha one").await;
    assert_eq!(w.stats(PROJ).await["vector_chunks"], 1);
    w.profile(PROJ, json!({"text_model_id": w.model_id("m16").await})).await;
    w.write(PROJ, "/a.md", "alpha two").await;
    let v = w.vectors(BOB, PROJ, "/a.md", json!({})).await.unwrap();
    assert_eq!(v["dimension"], 16);
    assert_eq!(v["vectors"].as_array().unwrap().len(), 1);
    assert_eq!(w.stats(PROJ).await["vector_chunks"], 1, "no copy at the old width");
}

/// E2E-002: paging of the vectors of a document.
#[tokio::test]
async fn e2e_0108_002_read_the_vectors_of_a_document() {
    let w = World::new(&[("m8", "text_embedding", 8)]).await;
    w.mode(PROJ, "rag").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m8").await})).await;
    let text = (1..=3)
        .map(|i| format!("# Part {i}\n\n{}\n", vec![format!("word{i}"); 400].join(" ")))
        .collect::<String>();
    w.write(PROJ, "/long.md", &text).await;
    let all = w.vectors(BOB, PROJ, "/long.md", json!({"limit": 100})).await.unwrap();
    let n = all["vectors"].as_array().unwrap().len();
    assert!(n >= 3, "one chunk per part at least: {n}");
    assert_eq!(all["next_after"], Value::Null, "the last page has no next position");
    let first = w.vectors(BOB, PROJ, "/long.md", json!({"limit": 1})).await.unwrap();
    assert_eq!(first["vectors"].as_array().unwrap().len(), 1);
    let last = first["vectors"][0]["number"].as_i64().unwrap();
    assert_eq!(first["next_after"], last);
    let second =
        w.vectors(BOB, PROJ, "/long.md", json!({"limit": 1, "after": last})).await.unwrap();
    assert!(second["vectors"][0]["number"].as_i64().unwrap() > last);
    let kinds = w.vectors(BOB, PROJ, "/long.md", json!({"kind": "image"})).await.unwrap();
    assert_eq!(kinds["vectors"], json!([]));
}

/// Sets every marker of the platform back to "not out of date", beside the
/// store, to observe what a later change marks.
async fn clear_markers(w: &World) {
    use crate::storage::rel::{Query, RelationalDb, SqliteRelationalDb};
    let db = SqliteRelationalDb::open(w.fx.dir.path().join("admin.db")).unwrap();
    db.execute(&Query::new("UPDATE project SET vectors_stale_at=NULL")).await.unwrap();
}

async fn stale(w: &World, project: &str) -> bool {
    w.status_of(project).await["vectors_out_of_date"].as_bool().unwrap()
}

/// E2E-012: editing the dimension or the name of a used model marks the
/// projects that use it, and only those.
#[tokio::test]
async fn e2e_0108_012_a_model_edit_marks_only_its_projects() {
    let w = World::new(&[
        ("used", "text_embedding", 384),
        ("also", "text_embedding", 16),
        ("img", "image_embedding", 8),
    ])
    .await;
    w.project("b").await;
    w.project("c").await;
    w.profile(
        PROJ,
        json!({"text_model_id": w.model_id("used").await,
        "image_model_id": w.model_id("img").await}),
    )
    .await;
    w.profile("b", json!({"text_model_id": w.model_id("also").await})).await;
    w.profile("c", json!({"chunk_strategy": "structural"})).await;
    clear_markers(&w).await;
    let used = w.model_id("used").await;
    let before = w.fx.changes().await.len();

    // The same value is no change, a token limit is no vector change.
    w.fx.update_model(json!({"model_id": used, "dimension": 384})).await.unwrap();
    w.fx.update_model(json!({"model_id": used, "max_input_tokens": 512})).await.unwrap();
    assert!(!stale(&w, PROJ).await, "nothing that changes a vector");
    assert_eq!(w.fx.changes().await.len(), before + 1, "only the token limit is recorded");

    // A model nobody uses changes freely.
    let spare = w.model("spare", "text_embedding", 32).await;
    w.fx.update_model(json!({"model_id": spare, "dimension": 64})).await.unwrap();
    assert!(!stale(&w, PROJ).await && !stale(&w, "b").await && !stale(&w, "c").await);

    // The dimension of a used model: its projects only.
    w.fx.update_model(json!({"model_id": used, "dimension": 400})).await.unwrap();
    assert!(stale(&w, PROJ).await);
    assert!(!stale(&w, "b").await && !stale(&w, "c").await);
    let since = w.status_of(PROJ).await["vectors_out_of_date_since"].clone();
    assert!(since.as_str().unwrap().ends_with('Z'));

    // The name of an image model used by a project.
    clear_markers(&w).await;
    w.fx.update_model(json!({"model_id": w.model_id("img").await, "name": "img2"})).await.unwrap();
    assert!(stale(&w, PROJ).await && !stale(&w, "b").await);
    // A refused edit marks nothing.
    clear_markers(&w).await;
    assert!(w.fx.update_model(json!({"model_id": used, "dimension": 0})).await.is_err());
    assert!(!stale(&w, PROJ).await);
}

impl World {
    pub async fn start(
        &self,
        who: &str,
        project: &str,
        path: Option<&str>,
    ) -> Result<Value, String> {
        let mut a = json!({"project_id": project});
        if let Some(p) = path {
            a["path"] = json!(p);
        }
        let p: rmcp::handler::server::wrapper::Parameters<RecalculateStartArgs> = args(a);
        self.fx.keep(tool_json(self.server(who).admin_recalculate_start(p).await))
    }

    pub async fn status(&self, who: &str, project: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<ProjectIdArgs> =
            args(json!({"project_id": project}));
        self.fx.keep(tool_json(self.server(who).admin_recalculate_status(p).await))
    }

    pub async fn cancel(&self, who: &str, project: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<ProjectIdArgs> =
            args(json!({"project_id": project}));
        self.fx.keep(tool_json(self.server(who).admin_recalculate_cancel(p).await))
    }

    /// Polls the status of `project` until the job is in one of `states`.
    pub async fn wait_for(&self, project: &str, states: &[&str]) -> Value {
        for _ in 0..400 {
            let s = self.status(BOB, project).await.unwrap();
            if let Some(state) = s["job"]["state"].as_str()
                && states.contains(&state)
            {
                return s;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        panic!("the recalculation of {project} never reached {states:?}");
    }

    /// Three indexed files `/a.md`, `/b.md`, `/c.md` of `proj` under `model`.
    pub async fn three_files(&self, model: &str) {
        self.mode(PROJ, "rag").await;
        self.profile(PROJ, json!({"text_model_id": self.model_id(model).await})).await;
        for (f, t) in [("a", "alpha text"), ("b", "bravo text"), ("c", "charlie text")] {
            self.write(PROJ, &format!("/{f}.md"), t).await;
        }
    }

    /// A converted document `path` of `proj` holding these pictures
    /// (caption, ocr text), committed as the conversion would.
    pub async fn seed_images(&self, path: &str, pictures: &[(&str, Option<&str>)]) {
        use crate::storage::meta::{NewArtifactSet, NewDocHeading, NewDocImage};
        let client = self.fx.state.stores.client(PROJ).await.unwrap();
        client.write_bytes_atomic(path, b"%PDF-1.4\n\xff\xfe").await.unwrap();
        let store = client.trash.clone().expect("a relational store");
        let node_rev = store.ensure_node_rev(path).await.unwrap().expect("a rev");
        let mut images = Vec::new();
        for (i, (caption, ocr)) in pictures.iter().enumerate() {
            let bytes = format!("pixels-{i}");
            let sha = crate::storage::VolumeClient::sha256_hex(bytes.as_bytes());
            client.blob.put(&sha, bytes.as_bytes()).await.unwrap();
            images.push(NewDocImage {
                seq: i as i64 + 1,
                caption: (*caption).into(),
                page: None,
                format: "png".into(),
                sha256: sha,
                size: bytes.len() as i64,
                line_start: 0,
                line_count: 1,
                ocr_text: ocr.map(Into::into),
            });
        }
        let set = NewArtifactSet {
            set_id: "set-doc".into(),
            path: path.into(),
            node_rev,
            text_sha: None,
            text_len: 0,
            report: format!("{} images, 0 tables", pictures.len()),
            note: None,
            images,
            tables: Vec::new(),
            headings: vec![NewDocHeading { seq: 1, level: 1, title: "T".into(), page: None }],
        };
        store.commit_artifact_set(&set).await.unwrap();
    }
}

/// E2E-003: a whole project recalculation rebuilds every file with the new
/// model, reports its progress and clears the marker.
#[tokio::test]
async fn e2e_0108_003_rebuild_a_project() {
    let w = World::new(&[("m384", "text_embedding", 384), ("m16", "text_embedding", 16)]).await;
    w.three_files("m384").await;
    assert_eq!(w.vectors(BOB, PROJ, "/a.md", json!({})).await.unwrap()["dimension"], 384);
    w.profile(PROJ, json!({"text_model_id": w.model_id("m16").await})).await;
    assert!(stale(&w, PROJ).await);
    let none = w.status(BOB, PROJ).await.unwrap();
    assert_eq!(none["job"], Value::Null);
    assert_eq!(none["vectors_out_of_date"], true);

    let started = w.start(BOB, PROJ, None).await.unwrap();
    assert_eq!(started["already_running"], false);
    assert_eq!(started["job"]["state"], "running");
    assert_eq!(started["job"]["path"], "/");
    assert_eq!(started["job"]["kind"], "vectors");
    let done = w.wait_for(PROJ, &["done", "failed"]).await;
    assert_eq!(done["job"]["state"], "done", "{done}");
    assert_eq!(done["job"]["progress"], json!({"done": 3, "total": 3}));
    assert!(done["job"]["ended_at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(done["vectors_out_of_date"], false);
    for f in ["a", "b", "c"] {
        let v = w.vectors(BOB, PROJ, &format!("/{f}.md"), json!({})).await.unwrap();
        assert_eq!(v["dimension"], 16, "{f}");
    }
    assert_eq!(w.stats(PROJ).await["vector_chunks"], 3, "no vector left at the old width");
    // The record of the profile is what the marker is read from, and an
    // administrator reads the same status.
    assert_eq!(w.status(ALICE, PROJ).await.unwrap()["job"]["state"], "done");
}

/// E2E-004: one document is rebuilt and the marker stays.
#[tokio::test]
async fn e2e_0108_004_rebuild_one_document() {
    let w = World::new(&[("m384", "text_embedding", 384), ("m16", "text_embedding", 16)]).await;
    w.three_files("m384").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m16").await})).await;
    let started = w.start(BOB, PROJ, Some("/a.md")).await.unwrap();
    assert_eq!(started["job"]["path"], "/a.md");
    let done = w.wait_for(PROJ, &["done", "failed"]).await;
    assert_eq!(done["job"]["state"], "done", "{done}");
    assert_eq!(done["job"]["progress"], json!({"done": 1, "total": 1}));
    assert_eq!(done["vectors_out_of_date"], true, "one document does not make the project current");
    assert_eq!(w.vectors(BOB, PROJ, "/a.md", json!({})).await.unwrap()["dimension"], 16);
    assert_eq!(w.vectors(BOB, PROJ, "/b.md", json!({})).await.unwrap()["dimension"], 384);
}

/// E2E-002 (images): the pictures of a converted document are embedded with the
/// image model by a recalculation and read as kind image.
#[tokio::test]
async fn e2e_0108_002_the_images_of_a_document() {
    let w = World::new(&[("m8", "text_embedding", 8), ("img4", "image_embedding", 4)]).await;
    w.mode(PROJ, "rag").await;
    w.profile(
        PROJ,
        json!({"text_model_id": w.model_id("m8").await,
        "image_model_id": w.model_id("img4").await}),
    )
    .await;
    w.seed_images("/doc.pdf", &[("Figure 1", Some("Revenue 4.2")), ("Figure 2", None)]).await;
    w.start(BOB, PROJ, Some("/doc.pdf")).await.unwrap();
    let done = w.wait_for(PROJ, &["done", "failed"]).await;
    assert_eq!(done["job"]["state"], "done", "{done}");
    let v = w.vectors(BOB, PROJ, "/doc.pdf", json!({"kind": "image"})).await.unwrap();
    assert_eq!(v["kind"], "image");
    assert_eq!(v["dimension"], 4);
    let list = v["vectors"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["number"], 1);
    assert_eq!(list[0]["text"], "Figure 1\nRevenue 4.2");
    assert_eq!(list[1]["text"], "Figure 2");
    assert_eq!(floats(&list[0]["vector"]).len(), 4);
    let calls = w.stub.calls.lock().unwrap().clone();
    let sent: Vec<&(String, String)> = calls.iter().filter(|(m, _)| m == "img4").collect();
    assert_eq!(sent.len(), 2);
    assert!(sent[0].1.starts_with("data:image/png;base64,"), "{}", sent[0].1);
    // The text vectors of a document with no text stay empty.
    assert_eq!(w.vectors(BOB, PROJ, "/doc.pdf", json!({})).await.unwrap()["vectors"], json!([]));
    // Deleting the document removes the image vectors with the rest (edge case).
    let a = args(json!({"mount_id": PROJ, "path": "/doc.pdf"}));
    tool_json(w.server(BOB).fs_delete(a).await).unwrap();
    let engine = w.fx.state.search.as_ref().unwrap();
    for _ in 0..100 {
        let page = engine
            .vectors_of(PROJ, "/doc.pdf", crate::search::VectorKind::Image, 0, 10)
            .await
            .unwrap();
        if page.vectors.is_empty() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the image vectors outlived their document");
}

/// E2E-006: shape refusals of the three tools.
#[tokio::test]
async fn e2e_0108_006_the_tools_refuse_bad_arguments() {
    let w = World::new(&[]).await;
    w.mode(PROJ, "rag").await;
    w.write(PROJ, "/a.md", "alpha").await;
    let client = w.fx.state.stores.client(PROJ).await.unwrap();
    client.mkdir("/dir").await.unwrap();
    for (project, path, code) in [
        ("nope", None, "ERR_PROJECT_NOT_FOUND"),
        (PROJ, Some("/missing.md"), "ERR_NOT_FOUND"),
        (PROJ, Some("/dir"), "ERR_NOT_FOUND"),
        (PROJ, Some("/../../etc/passwd"), "ERR_"),
    ] {
        let e = w.start(ALICE, project, path).await.unwrap_err();
        assert!(e.contains(code), "{project} {path:?}: {e}");
    }
    assert!(w.status(ALICE, "nope").await.unwrap_err().contains("ERR_PROJECT_NOT_FOUND"));
    assert!(w.cancel(ALICE, "nope").await.unwrap_err().contains("ERR_PROJECT_NOT_FOUND"));
    assert_eq!(w.status(BOB, PROJ).await.unwrap()["job"], Value::Null, "nothing started");
}

/// E2E-007: the service is down; the job fails, the marker stays and the old
/// vectors survive. A mode without vectors and search off are refused.
#[tokio::test]
async fn e2e_0108_007_a_service_that_is_down_fails_the_job() {
    let w = World::new(&[("m384", "text_embedding", 384), ("m16", "text_embedding", 16)]).await;
    w.three_files("m384").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m16").await})).await;
    w.stub.set(Answer::Down);
    w.start(BOB, PROJ, None).await.unwrap();
    let s = w.wait_for(PROJ, &["done", "failed", "cancelled"]).await;
    assert_eq!(s["job"]["state"], "failed", "{s}");
    assert_eq!(s["job"]["reason"], "embedding service unreachable");
    assert_eq!(s["vectors_out_of_date"], true);
    assert_eq!(w.vectors(BOB, PROJ, "/a.md", json!({})).await.unwrap()["dimension"], 384);
    // A later start, the service back, rebuilds.
    w.stub.set(Answer::Normal);
    w.start(BOB, PROJ, None).await.unwrap();
    let s = w.wait_for(PROJ, &["done", "failed"]).await;
    assert_eq!(s["job"]["state"], "done", "{s}");
    assert_eq!(s["vectors_out_of_date"], false);
    // Modes without vectors.
    for mode in ["bm25", "none"] {
        w.mode(PROJ, mode).await;
        let e = w.start(BOB, PROJ, None).await.unwrap_err();
        assert!(e.contains("ERR_NOT_SUPPORTED") && e.contains("no vectors"), "{mode}: {e}");
    }
    let off = Fx::new().await;
    let a = args(json!({"project_id": PROJ}));
    let e = tool_json(off.server(BOB).admin_recalculate_start(a).await).unwrap_err();
    assert!(e.contains("ERR_NOT_SUPPORTED") && e.contains("search is not enabled"), "{e}");
}

/// E2E-008 and E2E-017: a second start answers the running job, another scope is
/// refused, a cancel stops after the current file and leaves the marker, a second
/// cancel and a cancel with none change nothing.
#[tokio::test]
async fn e2e_0108_008_repeat_other_scope_and_cancel() {
    let w = World::new(&[("m384", "text_embedding", 384), ("m16", "text_embedding", 16)]).await;
    w.three_files("m384").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m16").await})).await;
    assert_eq!(w.cancel(BOB, PROJ).await.unwrap()["cancelled"], false, "nothing runs");
    *w.stub.delay_ms.lock().unwrap() = 300;
    let before = w.stub.calls.lock().unwrap().len();
    let first = w.start(BOB, PROJ, None).await.unwrap();
    // The job is inside the embedding of its first file, so a cancel finds it running.
    for _ in 0..200 {
        if w.stub.calls.lock().unwrap().len() > before {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let again = w.start(BOB, PROJ, None).await.unwrap();
    assert_eq!(again["already_running"], true);
    assert_eq!(again["job"]["job_id"], first["job"]["job_id"]);
    let e = w.start(BOB, PROJ, Some("/a.md")).await.unwrap_err();
    assert!(e.contains("ERR_INVALID_ARGUMENT") && e.contains("already running"), "{e}");
    assert_eq!(w.cancel(BOB, PROJ).await.unwrap()["cancelled"], true);
    assert_eq!(w.cancel(BOB, PROJ).await.unwrap()["cancelled"], true, "repeat while it ends");
    let s = w.wait_for(PROJ, &["done", "failed", "cancelled"]).await;
    assert_eq!(s["job"]["state"], "cancelled", "{s}");
    assert!(s["job"]["progress"]["done"].as_i64().unwrap() < 3, "{s}");
    assert_eq!(s["vectors_out_of_date"], true, "a cancel leaves the marker");
    assert_eq!(w.cancel(BOB, PROJ).await.unwrap()["cancelled"], false);
    // A cancelled job can be started again and runs to its end.
    *w.stub.delay_ms.lock().unwrap() = 0;
    w.start(BOB, PROJ, None).await.unwrap();
    let s = w.wait_for(PROJ, &["done", "failed"]).await;
    assert_eq!(s["job"]["state"], "done", "{s}");
    assert_eq!(s["vectors_out_of_date"], false);
}

/// E2E-010 (recalculation): who may start, read and cancel.
#[tokio::test]
async fn e2e_0108_010_recalculation_access() {
    let w = World::new(&[]).await;
    w.mode(PROJ, "rag").await;
    w.write(PROJ, "/a.md", "alpha").await;
    for who in [CAROL] {
        assert!(w.start(who, PROJ, None).await.unwrap_err().contains("ERR_FORBIDDEN"));
        assert!(w.cancel(who, PROJ).await.unwrap_err().contains("ERR_FORBIDDEN"));
        assert!(w.status(who, PROJ).await.unwrap_err().contains("ERR_FORBIDDEN"));
    }
    // A member who does not own the project reads, but neither starts nor cancels.
    w.fx.state.admin.add_member(PROJ, "dave@example.com", BOB).await.unwrap();
    let dave = "dave@example.com";
    assert!(w.status(dave, PROJ).await.is_ok());
    assert!(w.start(dave, PROJ, None).await.unwrap_err().contains("ERR_FORBIDDEN"));
    assert!(w.cancel(dave, PROJ).await.unwrap_err().contains("ERR_FORBIDDEN"));
    assert!(w.start(ALICE, PROJ, None).await.is_ok(), "a platform administrator may");
}

/// E2E-014: a file changed while the job reads it is indexed from its new content.
#[tokio::test]
async fn e2e_0108_014_a_file_edited_during_the_job() {
    let w = World::new(&[("m384", "text_embedding", 384), ("m16", "text_embedding", 16)]).await;
    w.three_files("m384").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m16").await})).await;
    *w.stub.delay_ms.lock().unwrap() = 1500;
    let before = w.stub.calls.lock().unwrap().len();
    w.start(BOB, PROJ, None).await.unwrap();
    // The job is inside the embedding of /a.md, the first file, which will take 1.5 s.
    for _ in 0..200 {
        if w.stub.calls.lock().unwrap().len() > before {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    // The service is fast again: the rebuild of the edit commits long before the
    // job's old embedding comes back.
    *w.stub.delay_ms.lock().unwrap() = 0;
    let a = args(json!({"mount_id": PROJ, "path": "/a.md", "content": "fresh content now",
        "overwrite": true}));
    tool_json(w.server(BOB).fs_write(a).await).unwrap();
    let s = w.wait_for(PROJ, &["done", "failed"]).await;
    assert_eq!(s["job"]["state"], "done", "{s}");
    let v = w.vectors(BOB, PROJ, "/a.md", json!({})).await.unwrap();
    assert!(
        v["vectors"][0]["text"].as_str().unwrap().contains("fresh content now"),
        "the old content won over the edit: {v}"
    );
    assert_eq!(v["dimension"], 16);
}

/// E2E-016: a restart ends the running job failed and starts it again.
#[tokio::test]
async fn e2e_0108_016_a_restart_resumes_the_recalculation() {
    use crate::search::derived::{Job, JobCause, JobKind, JobState};
    let w = World::new(&[("m384", "text_embedding", 384), ("m16", "text_embedding", 16)]).await;
    w.three_files("m384").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m16").await})).await;
    // What a crashed process left: a running row and a whole project scope.
    let jobs = w.fx.state.admin.derived_jobs().unwrap();
    let mut row = Job::new_waiting(PROJ, "/", JobKind::Vectors, JobCause::Manual);
    row.state = JobState::Running;
    row.done = Some(1);
    row.total = Some(3);
    jobs.insert(&row).await.unwrap();
    assert_eq!(w.status(BOB, PROJ).await.unwrap()["job"]["state"], "running");
    // A server of the same files recovers.
    let scheduler = w.fx.state.search.as_ref().unwrap().scheduler().unwrap();
    scheduler.recover().await.unwrap();
    let old = jobs.latest_vectors(PROJ).await.unwrap().unwrap();
    assert_eq!((old.job_id.as_str(), old.state), (row.job_id.as_str(), JobState::Failed));
    assert_eq!(old.reason.as_deref(), Some("interrupted by a server restart"));
    scheduler.resume_vectors().await;
    let s = w.wait_for(PROJ, &["done", "failed"]).await;
    assert_eq!(s["job"]["state"], "done", "{s}");
    assert_ne!(s["job"]["job_id"], json!(row.job_id), "a new job ran");
    assert_eq!(s["job"]["cause"], "change");
    assert_eq!(s["vectors_out_of_date"], false);
    assert_eq!(w.vectors(BOB, PROJ, "/c.md", json!({})).await.unwrap()["dimension"], 16);
    // Recovery of a project that no longer indexes vectors starts nothing.
    w.mode(PROJ, "bm25").await;
    let mut row2 = Job::new_waiting(PROJ, "/", JobKind::Vectors, JobCause::Manual);
    row2.state = JobState::Running;
    jobs.insert(&row2).await.unwrap();
    scheduler.recover().await.unwrap();
    scheduler.resume_vectors().await;
    let after = jobs.latest_vectors(PROJ).await.unwrap().unwrap();
    assert_eq!(after.state, JobState::Failed);
}

/// E2E-021: a project switched to a mode without vectors while a job runs gets no
/// vector back from it.
#[tokio::test]
async fn e2e_0108_021_a_mode_change_stops_the_job() {
    let w = World::new(&[("m384", "text_embedding", 384), ("m16", "text_embedding", 16)]).await;
    w.three_files("m384").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("m16").await})).await;
    *w.stub.delay_ms.lock().unwrap() = 600;
    let before = w.stub.calls.lock().unwrap().len();
    w.start(BOB, PROJ, None).await.unwrap();
    for _ in 0..200 {
        if w.stub.calls.lock().unwrap().len() > before {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    // The job is inside the embedding of its first file when the index is wiped.
    w.mode(PROJ, "none").await;
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert_eq!(w.stats(PROJ).await["vector_chunks"], 0, "the job wrote its vectors back");
    let calls_then = w.stub.calls.lock().unwrap().len();
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    assert_eq!(
        w.stub.calls.lock().unwrap().len(),
        calls_then,
        "the job went on with the next file"
    );
    // The project is free again: a start under the new mode is refused for its mode, not as running.
    let e = w.start(BOB, PROJ, None).await.unwrap_err();
    assert!(e.contains("no vectors"), "{e}");
}
