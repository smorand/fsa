//! SPEC-0110 functional tests of `search.documents` and of the honest keyword
//! mode, on the SPEC-0109 fixture of the parent module. Names are
//! `e2e_0110_NNN_slug`.

use super::*;
use crate::search::{
    IndexStats, IndexedChunk, Near, SearchBackend, SearchResult, VectorKind, VectorPage,
};

/// The real engine with a full text list that always fails.
struct NoKeyword(Arc<dyn SearchBackend>);

#[async_trait]
impl SearchBackend for NoKeyword {
    async fn index_chunks_checked(
        &self,
        v: &str,
        p: &str,
        c: &[IndexedChunk],
        f: &(dyn Fn() -> bool + Send + Sync),
    ) -> ToolResult<Option<usize>> {
        self.0.index_chunks_checked(v, p, c, f).await
    }
    async fn delete_path(&self, v: &str, p: &str) -> ToolResult<usize> {
        self.0.delete_path(v, p).await
    }
    async fn delete_all(&self, v: &str) -> ToolResult<usize> {
        self.0.delete_all(v).await
    }
    async fn query_bm25(&self, _v: &str, _q: &str, _k: usize) -> ToolResult<Vec<SearchResult>> {
        Err(ToolError::internal("the full text engine is down"))
    }
    async fn query_vector(&self, v: &str, q: &str, k: usize) -> ToolResult<Vec<SearchResult>> {
        self.0.query_vector(v, q, k).await
    }
    async fn stats(&self, v: &str) -> ToolResult<IndexStats> {
        self.0.stats(v).await
    }
    async fn vectors_of(
        &self,
        v: &str,
        p: &str,
        k: VectorKind,
        a: u32,
        l: usize,
    ) -> ToolResult<VectorPage> {
        self.0.vectors_of(v, p, k, a, l).await
    }
    async fn embed_input(&self, v: &str, k: VectorKind, i: &str) -> ToolResult<Vec<f32>> {
        self.0.embed_input(v, k, i).await
    }
    async fn nearest(&self, v: &str, k: VectorKind, x: &[f32], l: usize) -> ToolResult<Vec<Near>> {
        self.0.nearest(v, k, x, l).await
    }
    async fn similarities(
        &self,
        v: &str,
        k: VectorKind,
        x: &[f32],
        e: &[(String, u32)],
    ) -> ToolResult<Vec<Option<f32>>> {
        self.0.similarities(v, k, x, e).await
    }
    fn supported_modes(&self) -> Vec<&'static str> {
        self.0.supported_modes()
    }
}

const WARNING: &str = "the keyword list failed, the answer holds the vector list only";

/// E2E-013: a failing keyword list degrades the answer to `rag`, says so, and
/// keeps the vector lines; a healthy call carries no warning.
#[tokio::test]
async fn e2e_0110_013_a_failing_keyword_list_is_reported() {
    let mut w = zoo().await;
    let healthy = w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap();
    assert_eq!(healthy["mode_used"], "both");
    assert!(healthy.get("warning").is_none(), "{healthy}");
    let real = w.fx.state.search.clone().unwrap();
    w.set_search(Arc::new(NoKeyword(real)));
    let out = w.chunks(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap();
    assert_eq!(out["mode_used"], "rag", "{out}");
    assert_eq!(out["warning"], WARNING);
    assert_eq!(docs(&out, 0)[..2], ["/c.md", "/a.md"], "the vector list still answers: {out}");
    let out = w.documents(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap();
    assert_eq!(out["mode_used"], "rag", "{out}");
    assert_eq!(out["warning"], WARNING);
    assert_eq!(names(&out)[..2], ["/c.md", "/a.md"], "{out}");
}

use crate::mcp::server::SearchDocumentsArgs;

impl World {
    async fn documents(
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
        let p: rmcp::handler::server::wrapper::Parameters<SearchDocumentsArgs> = args(a);
        self.fx.keep(tool_json(self.server(who).search_documents(p).await))
    }
}

fn names(out: &Value) -> Vec<String> {
    out["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["document"].as_str().unwrap().into())
        .collect()
}

fn doc<'a>(out: &'a Value, path: &str) -> &'a Value {
    out["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["document"] == path)
        .unwrap_or_else(|| panic!("{path} is not answered: {out}"))
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-6, "{a} is not {b}");
}

const FOUR: &str = "ocelot gazelle sapphire granite";

/// E2E-001: one text, documents best first, the best chunk as proof.
#[tokio::test]
async fn e2e_0110_001_one_text_documents_with_proof() {
    let w = zoo().await;
    let out = w.documents(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap();
    assert_eq!(out["mode_used"], "both");
    assert_eq!((out["floor"].as_f64(), out["boost"].as_f64()), (Some(0.5), Some(0.5)));
    assert_eq!(out["weights"], json!([1.0]));
    assert!(out.get("warning").is_none(), "{out}");
    assert_eq!(names(&out)[..2], ["/c.md", "/a.md"], "{out}");
    assert_eq!(names(&out).len(), 4, "every document has a chunk with a similarity");
    let c = doc(&out, "/c.md");
    let i = &c["inputs"][0];
    assert_eq!((i["label"].as_str(), i["counted"].as_i64()), (Some("S1"), Some(1)));
    assert_eq!(i["best"]["chunk"], 1);
    assert_eq!(i["best"]["text"], "ocelot ocelot violet");
    assert!(i["best"]["heading_path"].is_array());
    let m = i["best"]["similarity"].as_f64().unwrap();
    near(i["score"].as_f64().unwrap(), m);
    near(c["score"].as_f64().unwrap(), m);
    let two =
        w.documents(BOB, PROJ, json!([{"text": "ocelot"}]), json!({"top_k": 2})).await.unwrap();
    assert_eq!(names(&two), ["/c.md", "/a.md"]);
}

/// A platform whose project has vectors only, so the pool is the 50 nearest
/// chunks and nothing else: `/a.md`, `/b.md` and 55 identical fillers.
async fn ragzoo() -> World {
    let w = World::in_mode("both", &[("mk", "text_embedding", 8)]).await;
    w.stub.set(Answer::Keyed);
    w.mode(PROJ, "rag").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("mk").await})).await;
    w.write(PROJ, "/a.md", "ocelot sapphire").await;
    w.write(PROJ, "/b.md", "gazelle granite").await;
    for n in 1..=55 {
        w.write(PROJ, &format!("/f{n:02}.md"), "harbor lantern meadow").await;
    }
    w
}

/// E2E-002: two texts and an image; an input a document is not among the 50
/// nearest chunks of scores 0 for it.
#[tokio::test]
async fn e2e_0110_002_a_missed_input_scores_zero() {
    let mut w = ragzoo().await;
    w.set_converter(Some(Named::of(&[("chart.png", "harbor lantern meadow")])));
    w.png(PROJ, "/chart.png", b"png-bytes").await;
    let out = w
        .documents(
            BOB,
            PROJ,
            json!([{"text": "ocelot sapphire"}, {"text": "gazelle granite"}, {"image": "/chart.png"}]),
            json!({"top_k": 50, "weights": [3, 3, 1]}),
        )
        .await
        .unwrap();
    assert_eq!(out["mode_used"], "rag");
    assert_eq!(out["weights"], json!([3.0, 3.0, 1.0]));
    let labels: Vec<&str> = doc(&out, "/a.md")["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["S1", "S2", "I1"]);
    // The picture reads as the filler text: 50 fillers fill its pool, a and b are out of it.
    for p in ["/a.md", "/b.md"] {
        let d = doc(&out, p);
        let i = &d["inputs"][2];
        assert_eq!((i["score"].as_f64(), i["counted"].as_i64()), (Some(0.0), Some(0)), "{d}");
        assert!(i["best"].is_null(), "{d}");
        let w = [3.0, 3.0, 1.0];
        let sum: f64 = (0..3).map(|k| w[k] * d["inputs"][k]["score"].as_f64().unwrap()).sum();
        near(d["score"].as_f64().unwrap(), sum / 7.0);
    }
    assert!(doc(&out, "/a.md")["inputs"][0]["score"].as_f64().unwrap() > 0.99);
    let f01 = doc(&out, "/f01.md");
    near(f01["inputs"][2]["score"].as_f64().unwrap(), 1.0);
    assert_eq!(f01["inputs"][2]["best"]["text"], "harbor lantern meadow");
    // A filler past the 50 nearest of every input is not answered at all.
    assert!(!names(&out).contains(&"/f55.md".to_string()), "{out}");
}

/// E2E-003: floor, boost and weights change the ranking.
#[tokio::test]
async fn e2e_0110_003_floor_boost_and_weights() {
    let w = zoo().await;
    w.put_long().await;
    let q = json!([{"text": FOUR}]);
    let pos = |out: &Value, p: &str| names(out).iter().position(|n| n == p).unwrap();
    // Each part of /long.md is about 0.5 close to the four words; a and b about 0.71.
    let high = w.documents(BOB, PROJ, q.clone(), json!({"floor": 0.55})).await.unwrap();
    assert!(pos(&high, "/long.md") > pos(&high, "/a.md"), "{high}");
    assert_eq!(doc(&high, "/long.md")["inputs"][0]["counted"], 1);
    let low = w.documents(BOB, PROJ, q.clone(), json!({"floor": 0.45})).await.unwrap();
    assert_eq!(names(&low)[0], "/long.md", "four parts at the floor lift it: {low}");
    assert_eq!(doc(&low, "/long.md")["inputs"][0]["counted"], 4);
    let flat = w.documents(BOB, PROJ, q.clone(), json!({"floor": 0.45, "boost": 0})).await.unwrap();
    assert!(pos(&flat, "/long.md") > pos(&flat, "/a.md"), "{flat}");
    let two = json!([{"text": "violet"}, {"text": "harbor"}]);
    let first = w.documents(BOB, PROJ, two.clone(), json!({"weights": [1, 0]})).await.unwrap();
    assert_eq!(names(&first)[0], "/c.md", "{first}");
    assert_eq!(first["weights"], json!([1.0, 0.0]));
    let second = w.documents(BOB, PROJ, two, json!({"weights": [0, 5]})).await.unwrap();
    assert_eq!(names(&second)[0], "/d.md", "{second}");
}

/// The formula as the spec words it, written apart from the production code.
fn expected(
    chunk_answers: &[Value],
    floor: f64,
    boost: f64,
    weights: &[f64],
) -> HashMap<String, f64> {
    let mut by_doc: HashMap<String, Vec<f64>> = HashMap::new();
    for (q, answer) in chunk_answers.iter().enumerate() {
        let mut sims: HashMap<String, Vec<f64>> = HashMap::new();
        for l in lines(answer, 0) {
            if let Some(s) = l["similarity"].as_f64() {
                sims.entry(l["document"].as_str().unwrap().into()).or_default().push(s);
            }
        }
        for (d, mut v) in sims {
            v.sort_by(|a, b| b.partial_cmp(a).unwrap());
            let m = v[0];
            let miss: f64 = v[1..].iter().filter(|s| **s >= floor).map(|s| 1.0 - s).product();
            let s_q = m + boost * (1.0 - m) * (1.0 - miss);
            let e = by_doc.entry(d).or_insert_with(|| vec![0.0; chunk_answers.len()]);
            e[q] = s_q;
        }
    }
    let total: f64 = weights.iter().sum();
    by_doc
        .into_iter()
        .map(|(d, v)| (d, v.iter().zip(weights).map(|(s, w)| s * w).sum::<f64>() / total))
        .collect()
}

/// E2E-004: a score is the formula on the chunk similarities `search.chunks`
/// shows for the same inputs.
#[tokio::test]
async fn e2e_0110_004_a_score_equals_the_formula_on_chunk_similarities() {
    let w = zoo().await;
    w.put_long().await;
    let inputs = [FOUR, "ocelot"];
    let mut answers = Vec::new();
    for t in inputs {
        answers
            .push(w.chunks(BOB, PROJ, json!([{"text": t}]), json!({"top_k": 50})).await.unwrap());
    }
    let (floor, boost, weights) = (0.3, 0.7, [1.0, 3.0]);
    let want = expected(&answers, floor, boost, &weights);
    let out = w
        .documents(
            BOB,
            PROJ,
            json!([{"text": FOUR}, {"text": "ocelot"}]),
            json!({"top_k": 50, "floor": floor, "boost": boost, "weights": weights}),
        )
        .await
        .unwrap();
    assert_eq!(names(&out).len(), want.len(), "{out}");
    for d in out["documents"].as_array().unwrap() {
        near(d["score"].as_f64().unwrap(), want[d["document"].as_str().unwrap()]);
    }
    let long = doc(&out, "/long.md");
    assert!(long["inputs"][0]["counted"].as_i64().unwrap() >= 4, "several chunks counted: {long}");
}

/// E2E-005: parameters and weights refused before any call.
#[tokio::test]
async fn e2e_0110_005_invalid_parameters() {
    let w = zoo().await;
    let before = w.stub.calls.lock().unwrap().len();
    let one = json!([{"text": "ocelot"}]);
    let two = json!([{"text": "violet"}, {"text": "harbor"}]);
    let cases: Vec<(Value, Value, &str)> = vec![
        (one.clone(), json!({"floor": -0.1}), "floor must be between 0 and 1"),
        (one.clone(), json!({"floor": 1.5}), "floor must be between 0 and 1"),
        (one.clone(), json!({"boost": 2}), "boost must be between 0 and 1"),
        (one.clone(), json!({"boost": -1}), "boost must be between 0 and 1"),
        (
            one.clone(),
            json!({"weights": [1, 1]}),
            "weights must hold one number per input: 2 given for 1 inputs",
        ),
        (
            two.clone(),
            json!({"weights": []}),
            "weights must hold one number per input: 0 given for 2",
        ),
        (two.clone(), json!({"weights": [1, -1]}), "weights must be numbers of 0 or more"),
        (two.clone(), json!({"weights": [0, 0]}), "weights must not all be 0"),
        (json!([]), json!({}), "inputs must hold 1 to 10 entries"),
        (json!([{}]), json!({}), "input 1 must hold exactly one of text and image"),
        (json!([{"text": "  "}]), json!({}), "input 1 text is empty"),
        (one.clone(), json!({"top_k": 0}), "top_k must be between 1 and 50"),
        (one.clone(), json!({"top_k": 51}), "top_k must be between 1 and 50"),
    ];
    for (inputs, extra, text) in cases {
        let e = w.documents(BOB, PROJ, inputs.clone(), extra.clone()).await.unwrap_err();
        assert!(e.contains("ERR_INVALID_ARGUMENT") && e.contains(text), "{inputs} {extra}: {e}");
    }
    assert_eq!(w.stub.calls.lock().unwrap().len(), before, "no embedding call was made");
    assert!(w.documents(BOB, PROJ, two, json!({"weights": [0, 0.5]})).await.is_ok());
}

/// E2E-006: a missing converter, a failing one, an empty conversion, a service down.
#[tokio::test]
async fn e2e_0110_006_missing_dependencies() {
    let mut w = zoo().await;
    w.png(PROJ, "/chart.png", b"png-bytes").await;
    let img = json!([{"image": "/chart.png"}]);
    let e = w.documents(BOB, PROJ, img.clone(), json!({})).await.unwrap_err();
    assert!(
        e.contains("ERR_NOT_SUPPORTED")
            && e.contains("no document converter is set up, an image cannot be turned into text"),
        "{e}"
    );
    w.set_converter(Some(Arc::new(StubDocService::failing(ToolError::internal(
        "converter exploded",
    )))));
    let e = w.documents(BOB, PROJ, img.clone(), json!({})).await.unwrap_err();
    assert!(e.contains("converter exploded"), "{e}");
    w.set_converter(Some(Named::of(&[("chart.png", " \n ")])));
    let e = w.documents(BOB, PROJ, img, json!({})).await.unwrap_err();
    assert!(e.contains("the converter found no text in /chart.png"), "{e}");
    w.stub.set(Answer::Down);
    let e = w.documents(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap_err();
    assert!(e.contains("ERR_INTERNAL_ERROR"), "{e}");
    w.stub.set(Answer::Keyed);
    assert!(w.documents(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.is_ok());
}

/// E2E-007: a mode without vectors, search off, an outsider, an administrator.
#[tokio::test]
async fn e2e_0110_007_mode_search_off_and_membership() {
    let w = zoo().await;
    for who in [CAROL, ALICE] {
        let e = w.documents(who, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap_err();
        assert!(e.contains("ERR_FORBIDDEN"), "{who}: {e}");
    }
    w.mode(PROJ, "none").await;
    let e = w.documents(BOB, PROJ, json!([{"text": "ocelot"}]), json!({})).await.unwrap_err();
    assert!(
        e.contains("ERR_NOT_SUPPORTED")
            && e.contains("the index mode of the project has no vectors"),
        "{e}"
    );
    let off = crate::settings::e2e::Fx::new().await;
    let p: rmcp::handler::server::wrapper::Parameters<SearchDocumentsArgs> =
        args(json!({"mount_id": PROJ, "inputs": [{"text": "a"}]}));
    let e = tool_json(off.server(BOB).search_documents(p).await).unwrap_err();
    assert!(e.contains("ERR_NOT_SUPPORTED") && e.contains("search is not enabled"), "{e}");
}

/// E2E-008: twenty repeats and eight concurrent calls give one answer, nothing is written.
#[tokio::test]
async fn e2e_0110_008_repeat_and_concurrency() {
    let w = zoo().await;
    w.put_long().await;
    let stored = w.vectors(BOB, PROJ, "/long.md", json!({"limit": 100})).await.unwrap();
    let inputs = json!([{"text": FOUR}, {"text": "ocelot"}]);
    let first = w.documents(BOB, PROJ, inputs.clone(), json!({})).await.unwrap();
    for _ in 0..20 {
        assert_eq!(w.documents(BOB, PROJ, inputs.clone(), json!({})).await.unwrap(), first);
    }
    let calls: Vec<_> = (0..8).map(|_| w.documents(BOB, PROJ, inputs.clone(), json!({}))).collect();
    for done in futures::future::join_all(calls).await {
        assert_eq!(done.unwrap(), first);
    }
    assert_eq!(w.vectors(BOB, PROJ, "/long.md", json!({"limit": 100})).await.unwrap(), stored);
}

/// E2E-009: floor 0 counts every chunk with a similarity, floor 1 only perfect ones.
#[tokio::test]
async fn e2e_0110_009_floor_zero_and_one() {
    let w = zoo().await;
    w.put_long().await;
    let q = json!([{"text": FOUR}]);
    let zero = w.documents(BOB, PROJ, q.clone(), json!({"floor": 0})).await.unwrap();
    assert_eq!(doc(&zero, "/long.md")["inputs"][0]["counted"], 4, "{zero}");
    let one = w.documents(BOB, PROJ, q, json!({"floor": 1})).await.unwrap();
    assert_eq!(doc(&one, "/long.md")["inputs"][0]["counted"], 1, "{one}");
    for d in one["documents"].as_array().unwrap() {
        let m = d["inputs"][0]["best"]["similarity"].as_f64().unwrap();
        near(d["score"].as_f64().unwrap(), m);
    }
}

/// E2E-010: with boost 0 a single input ranks as the best passage; one chunk
/// per document is its similarity whatever the boost.
#[tokio::test]
async fn e2e_0110_010_single_input_is_maxp() {
    let w = zoo().await;
    w.put_long().await;
    let q = json!([{"text": FOUR}]);
    let chunks = w.chunks(BOB, PROJ, q.clone(), json!({"top_k": 50})).await.unwrap();
    let mut best: HashMap<String, f64> = HashMap::new();
    for l in lines(&chunks, 0) {
        let e = best.entry(l["document"].as_str().unwrap().into()).or_insert(0.0);
        *e = e.max(l["similarity"].as_f64().unwrap());
    }
    let mut order: Vec<(String, f64)> = best.into_iter().collect();
    order.sort_by(|a, b| {
        ((b.1 * 1e6).round()).partial_cmp(&(a.1 * 1e6).round()).unwrap().then(a.0.cmp(&b.0))
    });
    let maxp = w.documents(BOB, PROJ, q.clone(), json!({"boost": 0, "top_k": 50})).await.unwrap();
    assert_eq!(names(&maxp), order.iter().map(|(d, _)| d.clone()).collect::<Vec<_>>(), "{maxp}");
    for (d, m) in &order {
        near(doc(&maxp, d)["score"].as_f64().unwrap(), *m);
    }
    for boost in [0.0, 0.5, 1.0] {
        let out = w.documents(BOB, PROJ, q.clone(), json!({"boost": boost})).await.unwrap();
        for p in ["/a.md", "/b.md", "/c.md", "/d.md"] {
            let d = doc(&out, p);
            near(
                d["score"].as_f64().unwrap(),
                d["inputs"][0]["best"]["similarity"].as_f64().unwrap(),
            );
        }
    }
}

/// E2E-011: equal scores order by path, on every repeat.
#[tokio::test]
async fn e2e_0110_011_ties_order_by_path() {
    let w = zoo().await;
    w.put(PROJ, &[("/z.md", "ocelot sapphire"), ("/a2.md", "ocelot sapphire")]).await;
    let q = json!([{"text": "ocelot sapphire"}]);
    let first = w.documents(BOB, PROJ, q.clone(), json!({})).await.unwrap();
    assert_eq!(names(&first)[..3], ["/a.md", "/a2.md", "/z.md"], "{first}");
    assert_eq!(doc(&first, "/a.md")["score"], doc(&first, "/z.md")["score"]);
    for _ in 0..20 {
        assert_eq!(w.documents(BOB, PROJ, q.clone(), json!({})).await.unwrap(), first);
    }
}

/// E2E-012: a chunk with no vector at the width in force is not counted.
#[tokio::test]
async fn e2e_0110_012_a_chunk_without_similarity_is_not_counted() {
    let w =
        World::in_mode("both", &[("mk", "text_embedding", 8), ("mk2", "text_embedding", 16)]).await;
    w.stub.set(Answer::Keyed);
    w.mode(PROJ, "both").await;
    w.profile(PROJ, json!({"text_model_id": w.model_id("mk").await})).await;
    w.put(PROJ, &[("/a.md", "ocelot sapphire"), ("/c.md", "ocelot ocelot violet")]).await;
    let q = json!([{"text": "ocelot"}]);
    assert_eq!(names(&w.documents(BOB, PROJ, q.clone(), json!({})).await.unwrap()).len(), 2);
    // The model changes, nothing is recalculated: the keyword list still finds both files.
    w.profile(PROJ, json!({"text_model_id": w.model_id("mk2").await})).await;
    let lines_now = w.chunks(BOB, PROJ, q.clone(), json!({})).await.unwrap();
    assert!(lines(&lines_now, 0).iter().all(|l| l["similarity"].is_null()), "{lines_now}");
    let none = w.documents(BOB, PROJ, q.clone(), json!({})).await.unwrap();
    assert_eq!(names(&none), Vec::<String>::new(), "{none}");
    w.put(PROJ, &[("/e.md", "ocelot ocelot ocelot")]).await;
    let some = w.documents(BOB, PROJ, q, json!({})).await.unwrap();
    assert_eq!(names(&some), ["/e.md"], "{some}");
    assert_eq!(doc(&some, "/e.md")["inputs"][0]["counted"], 1);
}

/// E2E-014: `search.documents` on PostgreSQL (pgvector), two widths side by side,
/// several chunks of a document counted.
#[tokio::test]
async fn e2e_0110_014_documents_on_postgres() {
    use crate::storage::rel::{PoolSettings, PostgresRelationalDb};
    let Some(dsn) = std::env::var("MCPFS_TEST_PG_DSN").ok().filter(|v| !v.trim().is_empty()) else {
        return;
    };
    let mut w = World::new(&[("m8", "text_embedding", 8), ("m16", "text_embedding", 16)]).await;
    let schema = format!("fsa_p110_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
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
    w.write(PROJ, "/a.md", "ocelot sapphire").await;
    w.write(PROJ, "/b.md", "gazelle granite").await;
    w.write(PROJ, "/long.md", &long_doc()).await;
    w.write("b", "/a.md", "ocelot violet").await;
    let v = w.vectors(BOB, PROJ, "/long.md", json!({"limit": 100})).await.unwrap();
    assert_eq!(v["vectors"].as_array().unwrap().len(), 4, "{v}");

    let out = w
        .documents(BOB, PROJ, json!([{"text": FOUR}]), json!({"floor": 0.45, "top_k": 50}))
        .await
        .unwrap();
    assert_eq!(out["mode_used"], "rag");
    assert_eq!(doc(&out, "/long.md")["inputs"][0]["counted"], 4, "{out}");
    let chunks = w.chunks(BOB, PROJ, json!([{"text": FOUR}]), json!({"top_k": 50})).await.unwrap();
    let want = expected(&[chunks], 0.45, 0.5, &[1.0]);
    for d in out["documents"].as_array().unwrap() {
        near(d["score"].as_f64().unwrap(), want[d["document"].as_str().unwrap()]);
    }
    let one =
        w.documents(BOB, PROJ, json!([{"text": "gazelle"}]), json!({"top_k": 1})).await.unwrap();
    assert_eq!(names(&one), ["/long.md"], "{one}");
    let other = w.documents(BOB, "b", json!([{"text": "ocelot"}]), json!({})).await.unwrap();
    assert_eq!(names(&other), ["/a.md"], "the other project, the other width: {other}");
    let drop = format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE");
    let _ = sqlx::query(sqlx::AssertSqlSafe(drop)).execute(db.pool()).await;
}
