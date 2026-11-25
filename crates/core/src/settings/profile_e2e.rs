//! SPEC-0107 functional tests of the per project indexing profile, driven
//! through the real `McpServer` tools and the real models screen on the
//! registry fixture of SPEC-0102 (a SQLite admin store reopened by a restart).

use super::e2e::{Fx, KEY, Via};
use crate::mcp::server::{ModelIdArgs, ProjectIdArgs, SetIndexingProfileArgs};
use crate::search::derived_e2e::{args, tool_json};
use crate::storage::traits::AdminBackend;
use serde_json::{Value, json};

const ALICE: &str = "admin@example.com";
const BOB: &str = "bob@example.com";
const CAROL: &str = "carol@example.com";
const DAVE: &str = "dave@example.com";
const PROJ: &str = "proj";

async fn set_as(fx: &Fx, who: &str, a: Value) -> Result<Value, String> {
    let p: rmcp::handler::server::wrapper::Parameters<SetIndexingProfileArgs> = args(a);
    fx.keep(tool_json(fx.server(who).admin_set_indexing_profile(p).await))
}

async fn get_as(fx: &Fx, who: &str, project: &str) -> Result<Value, String> {
    let p: rmcp::handler::server::wrapper::Parameters<ProjectIdArgs> =
        args(json!({"project_id": project}));
    fx.keep(tool_json(fx.server(who).admin_get_indexing_profile(p).await))
}

/// The ids of a local provider's text and image models.
async fn models(fx: &Fx) -> (String, String) {
    let p = fx.local("Local").await;
    let pid = p["provider_id"].as_str().unwrap().to_string();
    let t = fx.model_of(&pid, "qwen3-embedding-0.6b", "text_embedding").await;
    let i = fx.model_of(&pid, "qwen3-vl-embedding-2b", "image_embedding").await;
    (t["model_id"].as_str().unwrap().into(), i["model_id"].as_str().unwrap().into())
}

fn profile_changes(all: &[Value]) -> Vec<&Value> {
    all.iter().filter(|c| c["action"] == "set_indexing_profile").collect()
}

#[tokio::test]
async fn e2e_0107_001_choose_the_models_of_a_project() {
    let fx = Fx::new().await;
    let (t, i) = models(&fx).await;
    let out =
        set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": t, "image_model_id": i}))
            .await
            .unwrap();
    assert_eq!(out["project_id"], PROJ);
    assert_eq!(out["text_model"]["model_id"], t);
    assert_eq!(out["text_model"]["name"], "qwen3-embedding-0.6b");
    assert_eq!(out["text_model"]["provider"], "Local");
    assert_eq!(out["text_model"]["kind"], "text_embedding");
    assert_eq!(out["text_model"]["dimension"], 1024);
    assert_eq!(out["text_model"]["source"], "project");
    assert_eq!(out["image_model"]["model_id"], i);
    assert_eq!(out["image_model"]["source"], "project");
    assert_eq!(out["vectors_out_of_date"], true);
    assert!(out["vectors_out_of_date_since"].as_str().unwrap().ends_with('Z'));
    assert_eq!(get_as(&fx, BOB, PROJ).await.unwrap(), out);
}

#[tokio::test]
async fn e2e_0107_002_choose_the_chunk_strategy() {
    let fx = Fx::new().await;
    let out = set_as(
        &fx,
        BOB,
        json!({"project_id": PROJ, "chunk_strategy": "fixed", "chunk_settings": {"size_tokens": 512}}),
    )
    .await
    .unwrap();
    assert_eq!(out["chunking"]["strategy"], "fixed");
    assert_eq!(out["chunking"]["settings"], json!({"size_tokens": 512}));
    assert_eq!(out["chunking"]["filled"], json!({"overlap_tokens": 0, "size_tokens": 512}));
    assert_eq!(out["chunking"]["source"], "project");
    assert_eq!(out["vectors_out_of_date"], false, "a chunk change leaves the marker");
    let all = fx.changes().await;
    let lines = profile_changes(&all);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["target_kind"], "project_profile");
    assert_eq!(lines[0]["target_id"], PROJ);
    assert_eq!(lines[0]["fields"], json!(["chunk_strategy", "chunk_settings"]));
}

#[tokio::test]
async fn e2e_0107_003_fall_back_and_clear() {
    // The default text model must match the vector width of the platform.
    let fx = Fx::with_dimension(1024).await;
    let (t, _) = models(&fx).await;
    fx.set_default("text_embedding", &t).await.unwrap();
    let out = get_as(&fx, BOB, PROJ).await.unwrap();
    assert_eq!(out["text_model"]["model_id"], t);
    assert_eq!(out["text_model"]["source"], "server_default");
    assert_eq!(out["chunking"]["strategy"], Value::Null);
    assert_eq!(out["chunking"]["source"], "server_default");
    assert_eq!(out["image_model"], Value::Null);
    assert_eq!(out["vectors_out_of_date"], false);
    let p = fx.local("Other").await;
    let other = fx.model_of(p["provider_id"].as_str().unwrap(), "other", "text_embedding").await;
    let o = other["model_id"].as_str().unwrap();
    let set = set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": o})).await.unwrap();
    assert_eq!(set["text_model"]["model_id"], o);
    assert_eq!(set["text_model"]["source"], "project");
    let cleared = set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": ""})).await.unwrap();
    assert_eq!(cleared["text_model"]["model_id"], t);
    assert_eq!(cleared["text_model"]["source"], "server_default");
}

#[tokio::test]
async fn e2e_0107_004_the_screen_shows_the_profile() {
    let fx = Fx::new().await;
    let p = fx.local("Local").await;
    let pid = p["provider_id"].as_str().unwrap();
    let t = fx.model_of(pid, "<b>\"x\"</b>", "text_embedding").await;
    set_as(
        &fx,
        BOB,
        json!({"project_id": PROJ, "text_model_id": t["model_id"], "chunk_strategy": "structural"}),
    )
    .await
    .unwrap();
    let router = crate::models_screen::router(fx.state.clone());
    let (s, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/models", None).await;
    assert_eq!(s, axum::http::StatusCode::OK);
    let at = html.find("data-project=\"proj\"").expect("a section for the project");
    let section = &html[at..];
    let section = &section[..section.find("</section>").unwrap()];
    assert!(section.contains("structural"), "{section}");
    assert!(section.contains("&lt;b&gt;"), "{section}");
    assert!(!html.contains("<b>\"x\"</b>"), "the model name is escaped");
    assert!(section.contains("vectors out of date"), "{section}");
}

#[tokio::test]
async fn e2e_0107_005_invalid_chunk_choices_change_nothing() {
    let fx = Fx::new().await;
    let before = get_as(&fx, BOB, PROJ).await.unwrap();
    let cases = [
        (json!({"chunk_strategy": "magic"}), "strategy must be 'fixed' or 'structural'"),
        (
            json!({"chunk_strategy": "fixed", "chunk_settings": {"size_tokens": 5}}),
            "size_tokens must be between 32 and 8192",
        ),
        (
            json!({"chunk_strategy": "fixed", "chunk_settings": {"heading_cut_depth": 2}}),
            "setting 'heading_cut_depth' does not apply to strategy 'fixed'",
        ),
        (json!({"chunk_settings": {"size_tokens": 64}}), "chunk_settings need a chunk_strategy"),
    ];
    for (mut a, text) in cases {
        a["project_id"] = json!(PROJ);
        let e = set_as(&fx, BOB, a).await.unwrap_err();
        assert!(e.contains("ERR_INVALID_ARGUMENT") && e.contains(text), "{e}");
    }
    assert_eq!(get_as(&fx, BOB, PROJ).await.unwrap(), before);
    assert!(profile_changes(&fx.changes().await).is_empty());
}

#[tokio::test]
async fn e2e_0107_006_unknown_model_and_unknown_project() {
    let fx = Fx::new().await;
    let before = get_as(&fx, BOB, PROJ).await.unwrap();
    let e = set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": "model-000000000000"}))
        .await
        .unwrap_err();
    assert!(
        e.contains("ERR_NOT_FOUND") && e.contains("model not found: model-000000000000"),
        "{e}"
    );
    let e =
        set_as(&fx, ALICE, json!({"project_id": "ghost", "image_model_id": ""})).await.unwrap_err();
    assert!(e.contains("ERR_PROJECT_NOT_FOUND"), "{e}");
    let e = get_as(&fx, ALICE, "ghost").await.unwrap_err();
    assert!(e.contains("ERR_PROJECT_NOT_FOUND"), "{e}");
    assert_eq!(get_as(&fx, BOB, PROJ).await.unwrap(), before);
}

#[tokio::test]
async fn e2e_0107_007_a_model_of_the_wrong_kind_is_refused() {
    let fx = Fx::new().await;
    let (t, i) = models(&fx).await;
    let p = fx.local("Rank").await;
    let r = fx.model_of(p["provider_id"].as_str().unwrap(), "rerank", "reranker").await;
    let r = r["model_id"].as_str().unwrap();
    let before = get_as(&fx, BOB, PROJ).await.unwrap();
    let e = set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": r})).await.unwrap_err();
    assert!(e.contains("ERR_INVALID_ARGUMENT"), "{e}");
    assert!(e.contains("model 'rerank' is not a text_embedding model"), "{e}");
    let e = set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": i})).await.unwrap_err();
    assert!(e.contains("model 'qwen3-vl-embedding-2b' is not a text_embedding model"), "{e}");
    let e = set_as(&fx, BOB, json!({"project_id": PROJ, "image_model_id": t})).await.unwrap_err();
    assert!(e.contains("model 'qwen3-embedding-0.6b' is not an image_embedding model"), "{e}");
    assert_eq!(get_as(&fx, BOB, PROJ).await.unwrap(), before);
}

#[tokio::test]
async fn e2e_0107_008_who_may_read_and_who_may_set() {
    let fx = Fx::new().await;
    fx.state.admin.add_member(PROJ, CAROL, BOB).await.unwrap();
    let set = json!({"project_id": PROJ, "chunk_strategy": "fixed"});
    let e = set_as(&fx, CAROL, set.clone()).await.unwrap_err();
    assert!(e.contains("ERR_FORBIDDEN"), "{e}");
    let e = set_as(&fx, DAVE, set.clone()).await.unwrap_err();
    assert!(e.contains("ERR_FORBIDDEN"), "{e}");
    let e = get_as(&fx, DAVE, PROJ).await.unwrap_err();
    assert!(e.contains("ERR_FORBIDDEN"), "{e}");
    assert_eq!(get_as(&fx, CAROL, PROJ).await.unwrap()["chunking"]["strategy"], Value::Null);
    set_as(&fx, ALICE, set).await.unwrap();
    assert_eq!(get_as(&fx, ALICE, PROJ).await.unwrap()["chunking"]["strategy"], "fixed");
    assert_eq!(get_as(&fx, CAROL, PROJ).await.unwrap()["chunking"]["strategy"], "fixed");
}

#[tokio::test]
async fn e2e_0107_009_two_concurrent_sets_leave_one_whole_profile() {
    let fx = Fx::new().await;
    let (t, i) = models(&fx).await;
    let p = fx.local("Other").await;
    let pid = p["provider_id"].as_str().unwrap();
    let t2 =
        fx.model_of(pid, "t2", "text_embedding").await["model_id"].as_str().unwrap().to_string();
    let i2 =
        fx.model_of(pid, "i2", "image_embedding").await["model_id"].as_str().unwrap().to_string();
    let a = json!({"project_id": PROJ, "text_model_id": t, "image_model_id": i, "chunk_strategy": "fixed"});
    let b = json!({"project_id": PROJ, "text_model_id": t2, "image_model_id": i2, "chunk_strategy": "structural"});
    let (ra, rb) = tokio::join!(set_as(&fx, BOB, a), set_as(&fx, ALICE, b));
    ra.unwrap();
    rb.unwrap();
    let end = get_as(&fx, BOB, PROJ).await.unwrap();
    let first = end["text_model"]["model_id"] == t;
    let (tm, im, st) = if first { (&t, &i, "fixed") } else { (&t2, &i2, "structural") };
    assert_eq!(end["text_model"]["model_id"], *tm);
    assert_eq!(end["image_model"]["model_id"], *im);
    assert_eq!(end["chunking"]["strategy"], st);
}

#[tokio::test]
async fn e2e_0107_010_a_set_leaves_other_projects_alone() {
    let fx = Fx::new().await;
    fx.state.admin.create_project("other", BOB).await.unwrap();
    let (t, _) = models(&fx).await;
    set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": t, "chunk_strategy": "fixed"}))
        .await
        .unwrap();
    let other = get_as(&fx, BOB, "other").await.unwrap();
    assert_eq!(other["text_model"], Value::Null);
    assert_eq!(other["chunking"]["strategy"], Value::Null);
    assert_eq!(other["vectors_out_of_date"], false);
}

fn config_of(root: &std::path::Path) -> crate::config::ServerConfig {
    let mut c = crate::config::ServerConfig::default();
    c.infra.meta.dir = root.join("volumes").display().to_string();
    c.infra.blob.dir = root.join("blobs").display().to_string();
    c.infra.admin.path = root.join("admin.db").display().to_string();
    c
}

#[tokio::test]
async fn e2e_0107_011_a_profile_survives_restart_migrate_and_an_old_database() {
    let mut fx = Fx::new().await;
    let (t, i) = models(&fx).await;
    let set = json!({"project_id": PROJ, "text_model_id": t, "image_model_id": i,
        "chunk_strategy": "structural", "chunk_settings": {"max_tokens": 256}});
    let want = set_as(&fx, BOB, set).await.unwrap();
    fx.restart(Some(KEY)).await;
    assert_eq!(get_as(&fx, BOB, PROJ).await.unwrap(), want);

    // An offline copy to another deployment carries the profile.
    let to_dir = tempfile::tempdir().unwrap();
    let (from, to) = (config_of(fx.dir.path()), config_of(to_dir.path()));
    crate::migrate::migrate(&from, &to).await.unwrap();
    let reg = crate::storage::RelationalRegistry::new();
    let admin = crate::storage::build_admin_store(&to, &reg).await.unwrap();
    admin.connect().await.unwrap();
    admin.set_master_key(Some(KEY));
    let r = super::Registry::of(admin.as_ref(), 1536).unwrap();
    let copied = r.project_profile(PROJ).await.unwrap();
    assert_eq!(copied.chunk_strategy.as_deref(), Some("structural"));
    assert_eq!(copied.text_model_id.as_deref(), Some(t.as_str()));
    assert_eq!(copied.image_model_id.as_deref(), Some(i.as_str()));
    assert!(copied.vectors_stale_at.is_some());

    // A database from before the columns reads an empty profile.
    use crate::storage::rel::{Query, RelationalDb, SqliteRelationalDb};
    let db = std::sync::Arc::new(SqliteRelationalDb::open_in_memory().unwrap());
    db.execute(&Query::new(
        "CREATE TABLE \"project\" (\"id\" TEXT NOT NULL, \"owner\" TEXT NOT NULL, \
         \"created_at\" TEXT NOT NULL, PRIMARY KEY (\"id\"))",
    ))
    .await
    .unwrap();
    db.execute(
        &Query::new("INSERT INTO project (id, owner, created_at) VALUES (?1, ?2, ?3)")
            .bind("legacy")
            .bind("o@t.c")
            .bind("2020-01-01T00:00:00Z"),
    )
    .await
    .unwrap();
    let old = crate::storage::admin::RelationalAdminStore::new(db);
    old.connect().await.unwrap();
    let r = super::Registry::of(&old, 1536).unwrap();
    let p = r.project_profile("legacy").await.unwrap();
    assert!(
        p.chunk_strategy.is_none() && p.text_model_id.is_none() && p.vectors_stale_at.is_none()
    );
}

#[tokio::test]
async fn e2e_0107_012_no_secret_reaches_an_answer_a_record_or_a_log() {
    let fx = Fx::new().await;
    let cloud = fx.cloud().await;
    let t = fx.model_of(cloud["provider_id"].as_str().unwrap(), "emb", "text_embedding").await;
    set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": t["model_id"]})).await.unwrap();
    get_as(&fx, BOB, PROJ).await.unwrap();
    let router = crate::models_screen::router(fx.state.clone());
    fx.page(&router, Via::Cookie(ALICE), "GET", "/app/models", None).await;
    fx.assert_nowhere("sk-test-123").await;
}

#[tokio::test]
async fn e2e_0107_013_a_used_model_cannot_be_deleted() {
    let fx = Fx::new().await;
    let (t, _) = models(&fx).await;
    set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": t})).await.unwrap();
    let e = fx.delete_model_as(ALICE, &t).await.unwrap_err();
    assert!(e.contains("ERR_INVALID_ARGUMENT"), "{e}");
    assert!(e.contains("model 'qwen3-embedding-0.6b' is used by 1 project(s)"), "{e}");
    set_as(&fx, BOB, json!({"project_id": PROJ, "text_model_id": ""})).await.unwrap();
    fx.delete_model_as(ALICE, &t).await.unwrap();
    let _ = ModelIdArgs { model_id: String::new() };
}

#[tokio::test]
async fn e2e_0107_014_the_text_model_falls_back_to_the_configuration() {
    let fx = Fx::new().await;
    let none = get_as(&fx, BOB, PROJ).await.unwrap();
    assert_eq!(none["text_model"], Value::Null, "no default and no endpoint");
    let mut config = (*fx.state.config).clone();
    config.search.embedding.endpoint = "http://localhost:9010/v1/embeddings".into();
    config.search.embedding.model = "yaml-model".into();
    config.search.embedding.dimensions = 768;
    let s = &fx.state;
    let state = crate::state::AppState {
        config: std::sync::Arc::new(config),
        admin: s.admin.clone(),
        stores: s.stores.clone(),
        safety: s.safety.clone(),
        identity: s.identity.clone(),
        editors: s.editors.clone(),
        doc_service: None,
        search: None,
    };
    let server = crate::mcp::server::McpServer::new(std::sync::Arc::new(state), BOB.to_string());
    let p: rmcp::handler::server::wrapper::Parameters<ProjectIdArgs> =
        args(json!({"project_id": PROJ}));
    let out = tool_json(server.admin_get_indexing_profile(p).await).unwrap();
    assert_eq!(out["text_model"]["source"], "configuration");
    assert_eq!(out["text_model"]["name"], "yaml-model");
    assert_eq!(out["text_model"]["model_id"], Value::Null);
    assert_eq!(out["text_model"]["dimension"], 768);
}

#[tokio::test]
async fn e2e_0107_015_setting_the_same_values_again_changes_and_records_nothing() {
    let fx = Fx::new().await;
    let (t, _) = models(&fx).await;
    let a = json!({"project_id": PROJ, "text_model_id": t, "chunk_strategy": "fixed",
        "chunk_settings": {"size_tokens": 256}});
    let first = set_as(&fx, BOB, a.clone()).await.unwrap();
    let again = set_as(&fx, BOB, a).await.unwrap();
    assert_eq!(first, again);
    assert_eq!(
        again["vectors_out_of_date_since"], first["vectors_out_of_date_since"],
        "the first marker time stays"
    );
    assert_eq!(profile_changes(&fx.changes().await).len(), 1);
}

#[tokio::test]
async fn e2e_0107_016_concurrent_sets_of_different_fields_both_survive() {
    let fx = Fx::new().await;
    let (t, i) = models(&fx).await;
    let a = json!({"project_id": PROJ, "text_model_id": t});
    let b = json!({"project_id": PROJ, "image_model_id": i});
    let (ra, rb) = tokio::join!(set_as(&fx, BOB, a), set_as(&fx, ALICE, b));
    ra.unwrap();
    rb.unwrap();
    let end = get_as(&fx, BOB, PROJ).await.unwrap();
    assert_eq!(end["text_model"]["model_id"], t);
    assert_eq!(end["image_model"]["model_id"], i);
    assert_eq!(end["vectors_out_of_date"], true);
}

#[tokio::test]
async fn e2e_0107_017_a_stored_choice_the_limits_no_longer_accept_still_reads() {
    let fx = Fx::new().await;
    set_as(&fx, BOB, json!({"project_id": PROJ, "chunk_strategy": "fixed"})).await.unwrap();
    // A value out of today's range, as an older version could have stored.
    let db = crate::storage::rel::SqliteRelationalDb::open(fx.dir.path().join("admin.db")).unwrap();
    use crate::storage::rel::{Query, RelationalDb};
    db.execute(
        &Query::new("UPDATE project SET chunk_settings=?1 WHERE id=?2")
            .bind("{\"size_tokens\":1}")
            .bind(PROJ),
    )
    .await
    .unwrap();
    let out = get_as(&fx, BOB, PROJ).await.unwrap();
    assert_eq!(out["chunking"]["settings"], json!({"size_tokens": 1}));
    assert_eq!(out["chunking"]["filled"], Value::Null);
    let router = crate::models_screen::router(fx.state.clone());
    let (s, _) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/models", None).await;
    assert_eq!(s, axum::http::StatusCode::OK);
}
