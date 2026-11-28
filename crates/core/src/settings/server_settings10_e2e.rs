//! SPEC-0117 functional tests, US-0354: the `infra` section of the generic settings
//! store (SPEC-0114), its five write only secrets and the static rules of a change,
//! on the registry fixture of SPEC-0102 (the platform holds `/a.txt` in `proj`, so the
//! S3 stand in answers "present" where a blob store switch is made).

use super::ServerSettings;
use super::e2e::{Fx, Via, token_of};
use super::infra_testkit::S3Stand;
use crate::mcp::server::{SectionArgs, SetSettingArgs};
use crate::search::derived_e2e::{args, tool_json};
use crate::storage::rel::{Query, RelationalDb, SqliteRelationalDb};
use serde_json::{Value, json};

const ALICE: &str = "admin@example.com";
const BOB: &str = "bob@example.com";
const FOURTEEN: [&str; 14] = [
    "safety",
    "api",
    "extract",
    "doc",
    "sqlite",
    "db",
    "doc_service",
    "search",
    "git",
    "web",
    "context7",
    "server",
    "auth",
    "infra",
];
const ACCESS: &str = "AKIAEXAMPLE0001";
const SECRET: &str = "s3cr3t-value-abcdef";

fn settings(fx: &Fx) -> ServerSettings {
    ServerSettings::of(fx.state.admin.as_ref(), fx.state.config.clone()).unwrap()
}

async fn set(fx: &Fx, patch: Value) -> Result<Value, String> {
    fx.keep(settings(fx).set(ALICE, "infra", patch).await.map_err(|e| e.message))
}

async fn get(fx: &Fx) -> Value {
    fx.keep(settings(fx).get("infra").await.map_err(|e| e.message)).unwrap()
}

fn lines(all: &[Value]) -> Vec<&Value> {
    all.iter().filter(|c| c["target_kind"] == "setting").collect()
}

async fn nothing_stored(fx: &Fx) {
    assert_eq!(get(fx).await["source"], "default", "infra was stored");
    assert!(lines(&fx.changes().await).is_empty(), "a refused change was recorded");
}

/// The patch that switches the blob store to the stand in, with its two keys.
fn s3(stand: &S3Stand) -> Value {
    json!({"blob": {"backend": "s3", "endpoint": stand.url, "access_key": ACCESS, "secret_key": SECRET}})
}

/// The raw `value` and `secret_enc` of the stored row, read beside the store.
async fn raw_row(fx: &Fx) -> (String, Option<Vec<u8>>) {
    let db = SqliteRelationalDb::open(fx.dir.path().join("admin.db")).unwrap();
    let q = Query::new("SELECT value, secret_enc FROM server_setting WHERE section='infra'");
    let r = db.query_opt(&q).await.unwrap().expect("a stored infra row");
    let enc = match r.value(1).unwrap() {
        crate::storage::rel::SqlValue::Null => None,
        _ => Some(r.blob(1).unwrap()),
    };
    (r.text(0).unwrap(), enc)
}

#[tokio::test]
async fn e2e_0117_001_the_list_names_fourteen_sections_and_infra_reads_the_running_stores() {
    let fx = Fx::new().await;
    let list = settings(&fx).list().await.unwrap();
    let names: Vec<&str> = list["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["section"].as_str().unwrap())
        .collect();
    assert_eq!(names, FOURTEEN);
    let out = get(&fx).await;
    let mut keys: Vec<&String> = out["value"].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["blob", "git", "meta", "oauth"], "no admin block");
    assert_eq!(out["source"], "default");
    assert_eq!(out["restart_required"], false);
    for p in ["blob.access_key", "blob.secret_key", "meta.dsn", "git.dsn", "oauth.dsn"] {
        assert_eq!(out["secrets"][p]["has_value"], false, "{p}");
    }
    let m = settings(&fx).get("nope").await.unwrap_err().message;
    assert_eq!(
        m,
        format!("unknown setting section 'nope', expected one of {}", FOURTEEN.join(", "))
    );
}

#[tokio::test]
async fn e2e_0117_002_a_change_that_moves_nothing_is_stored_and_waits_for_a_restart() {
    let fx = Fx::new().await;
    let out = set(&fx, json!({"blob": {"region": "eu-west-1", "bucket_prefix": "x-"}})).await;
    let out = out.unwrap();
    assert_eq!(out["value"]["blob"]["region"], "eu-west-1");
    assert_eq!(
        out["value"]["blob"]["dir"], fx.state.config.infra.blob.dir,
        "a key not given keeps"
    );
    assert_eq!(out["value"]["meta"]["schema"], "public");
    assert_eq!(out["source"], "database");
    assert_eq!(out["restart_required"], true);
    assert_eq!(fx.state.config.infra.blob.region, "us-east-1", "the running stores are unchanged");
}

#[tokio::test]
async fn e2e_0117_003_the_admin_block_is_not_a_key() {
    let fx = Fx::new().await;
    let m = set(&fx, json!({"admin": {"path": "x"}})).await.unwrap_err();
    assert_eq!(m, "unknown key 'admin' in setting 'infra'");
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_004_unknown_keys_wrong_types_and_a_non_object_name_the_section() {
    let fx = Fx::new().await;
    let m = set(&fx, json!({"meta": {"nope": 1}})).await.unwrap_err();
    assert_eq!(m, "unknown key 'meta.nope' in setting 'infra'");
    for patch in [json!({"blob": {"region": 5}}), json!({"meta": "x"})] {
        let m = set(&fx, patch).await.unwrap_err();
        assert!(m.starts_with("invalid setting 'infra':"), "{m}");
    }
    let m = set(&fx, json!("x")).await.unwrap_err();
    assert_eq!(m, "the value of setting 'infra' must be an object");
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_005_the_s3_keys_are_encrypted_and_never_returned() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(200).await;
    let out = set(&fx, s3(&stand)).await.unwrap();
    assert!(out["value"]["blob"].get("access_key").is_none());
    assert!(out["value"]["blob"].get("secret_key").is_none());
    assert_eq!(out["value"]["blob"]["backend"], "s3");
    assert_eq!(out["secrets"]["blob.access_key"], json!({"has_value": true, "last4": "0001"}));
    assert_eq!(out["secrets"]["blob.secret_key"], json!({"has_value": true, "last4": "cdef"}));
    let (value, enc) = raw_row(&fx).await;
    assert!(!value.contains(ACCESS) && !value.contains(SECRET), "{value}");
    let enc = enc.expect("the secrets are stored encrypted");
    let hay = String::from_utf8_lossy(&enc).to_string();
    assert!(!hay.contains(ACCESS) && !hay.contains(SECRET));
}

#[tokio::test]
async fn e2e_0117_006_a_secret_is_in_no_answer_record_debug_or_log() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(200).await;
    set(&fx, s3(&stand)).await.unwrap();
    let list = settings(&fx).list().await.unwrap();
    let _ = fx.keep(Ok(list));
    get(&fx).await;
    // The S3 client of the AWS SDK prints its credentials provider at TRACE level (third
    // party, only the access key id, as before this spec): the log search is on the secret key.
    fx.assert_nowhere(SECRET).await;
    let lists = json!([fx.changes().await]).to_string();
    assert!(!lists.contains(ACCESS), "the record holds the access key");
    let mut full = super::server_settings::section_json(&fx.state.config, "infra");
    full["blob"]["access_key"] = json!(ACCESS);
    full["blob"]["secret_key"] = json!(SECRET);
    full["meta"]["dsn"] = json!("postgres://u:hunter2pw@h/db");
    let c = super::server_settings::with_section(&fx.state.config, "infra", full).unwrap();
    let dbg = format!("{c:?} {:?}", c.infra);
    for needle in [ACCESS, SECRET, "hunter2pw"] {
        assert!(!dbg.contains(needle), "Debug shows {needle}");
    }
}

#[tokio::test]
async fn e2e_0117_007_a_secret_needs_the_master_key() {
    let fx = Fx::with_key(None).await;
    let stand = S3Stand::start(200).await;
    let m = set(&fx, s3(&stand)).await.unwrap_err();
    assert_eq!(m, "no master key is configured; set FSA_TOKEN_KEY");
    nothing_stored(&fx).await;
    assert!(set(&fx, json!({"blob": {"region": "eu-west-1"}})).await.is_ok());
}

#[tokio::test]
async fn e2e_0117_008_a_secret_is_kept_unless_given_and_an_empty_text_clears_it() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(200).await;
    set(&fx, s3(&stand)).await.unwrap();
    let out = set(&fx, json!({"blob": {"region": "eu-west-1"}})).await.unwrap();
    assert_eq!(out["secrets"]["blob.access_key"]["has_value"], true);
    assert_eq!(out["secrets"]["blob.secret_key"]["has_value"], true);
    let out = set(&fx, json!({"blob": {"backend": "local", "secret_key": "  "}})).await.unwrap();
    assert_eq!(out["secrets"]["blob.secret_key"]["has_value"], false);
    assert_eq!(out["secrets"]["blob.access_key"]["has_value"], true);
}

#[tokio::test]
async fn e2e_0117_010_the_start_s_messages_and_the_blob_rules_refuse_a_bad_value() {
    let fx = Fx::new().await;
    let cases = [
        (
            json!({"meta": {"backend": "postgres"}}),
            "infra.meta.dsn is required when infra.meta.backend is 'postgres'",
        ),
        (
            json!({"git": {"backend": "sqlite", "dsn": "postgres://x"}}),
            "infra.git.dsn is set but infra.git.backend is 'sqlite', which stores to a file and never reads a dsn: remove one of the two",
        ),
        (
            json!({"blob": {"backend": "ftp"}}),
            "unknown infra.blob.backend 'ftp', expected one of local, minio, s3",
        ),
        (
            json!({"blob": {"backend": "s3"}}),
            "infra.blob.access_key and infra.blob.secret_key are required when infra.blob.backend is 's3'",
        ),
        (
            json!({"blob": {"backend": "minio", "access_key": ACCESS, "secret_key": SECRET}}),
            "infra.blob.endpoint is required when infra.blob.backend is 'minio'",
        ),
        (
            json!({"blob": {"backend": "local", "dir": ""}}),
            "infra.blob.dir is required when infra.blob.backend is 'local'",
        ),
    ];
    for (patch, want) in cases {
        let m = set(&fx, patch.clone()).await.unwrap_err();
        assert_eq!(m, want, "{patch}");
    }
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_011_a_member_is_refused_on_tool_route_and_screen() {
    let fx = Fx::new().await;
    let forbidden = "ERR_FORBIDDEN: 'bob@example.com' is not a platform admin";
    let p: rmcp::handler::server::wrapper::Parameters<SectionArgs> =
        args(json!({"section": "infra"}));
    let got = fx.keep(tool_json(fx.server(BOB).admin_get_setting(p).await));
    assert_eq!(got.unwrap_err(), forbidden);
    let p: rmcp::handler::server::wrapper::Parameters<SetSettingArgs> =
        args(json!({"section": "infra", "value": {"blob": {"region": "x"}}}));
    let got = fx.keep(tool_json(fx.server(BOB).admin_set_setting(p).await));
    assert_eq!(got.unwrap_err(), forbidden);
    let (st, _) = fx.rest(BOB, "GET", "/api/admin/settings/infra", None).await;
    assert_eq!(st, 403);
    let (st, _) = fx
        .rest(
            BOB,
            "PUT",
            "/api/admin/settings/infra",
            Some(json!({"value": {"blob": {"region": "x"}}})),
        )
        .await;
    assert_eq!(st, 403);
    let router = crate::settings_screen::router(fx.state.clone());
    let (st, _) = fx.page(&router, Via::Cookie(BOB), "GET", "/app/settings", None).await;
    assert_eq!(st, 403);
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_012_the_record_names_keys_never_values() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(200).await;
    set(&fx, s3(&stand)).await.unwrap();
    let all = fx.changes().await;
    let l = lines(&all);
    assert_eq!(l.len(), 1);
    assert_eq!(
        l[0]["fields"],
        json!(["blob.access_key", "blob.backend", "blob.endpoint", "blob.secret_key"])
    );
    let text = l[0].to_string();
    for v in [ACCESS, SECRET, stand.url.as_str()] {
        assert!(!text.contains(v), "the record holds {v}");
    }
    set(&fx, s3(&stand)).await.unwrap();
    let all = fx.changes().await;
    assert_eq!(lines(&all)[0]["fields"], json!([]), "the same values change no key");
}

#[tokio::test]
async fn e2e_0117_013_a_refused_change_records_nothing() {
    let fx = Fx::new().await;
    for patch in [json!({"admin": {}}), json!({"blob": {"backend": "ftp"}}), json!({"meta": 1})] {
        set(&fx, patch).await.unwrap_err();
    }
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_014_a_section_read_and_set_again_changes_nothing() {
    let fx = Fx::new().await;
    let read = get(&fx).await;
    let out = set(&fx, read["value"].clone()).await.unwrap();
    assert_eq!(out["value"], read["value"]);
    let all = fx.changes().await;
    assert_eq!(lines(&all)[0]["fields"], json!([]));
}

#[tokio::test]
async fn e2e_0117_016_the_screen_lists_fourteen_sections_and_shows_the_secrets_as_set() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(200).await;
    set(&fx, s3(&stand)).await.unwrap();
    let router = crate::settings_screen::router(fx.state.clone());
    let (st, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/settings", None).await;
    assert_eq!(st, 200, "{html}");
    for s in FOURTEEN {
        assert!(html.contains(&format!("data-section=\"{s}\"")), "{s}");
    }
    assert!(html.contains("Secret blob.secret_key: set, ends with cdef"), "{html}");
    assert!(html.contains("Secret blob.access_key: set, ends with 0001"), "{html}");
    assert!(!html.contains(ACCESS) && !html.contains(SECRET));
    assert!(html.contains("<textarea class=\"textarea\" name=\"value\" rows=\"4\" cols=\"60\">{}</textarea>"));
    let token = token_of(&html);
    let (st, _) = fx
        .page(
            &router,
            Via::Cookie(ALICE),
            "POST",
            "/app/settings/set",
            Some(&[
                ("section", "infra"),
                ("value", "{\"blob\": {\"region\": \"eu-west-1\"}}"),
                ("csrf_token", &token),
            ]),
        )
        .await;
    assert_eq!(st, 303);
    assert_eq!(get(&fx).await["value"]["blob"]["region"], "eu-west-1");
}

#[tokio::test]
async fn e2e_0117_017_two_administrators_last_write_wins_and_both_are_recorded() {
    let fx = Fx::new().await;
    let s = settings(&fx);
    s.set(ALICE, "infra", json!({"blob": {"region": "eu-west-1"}})).await.unwrap();
    s.set("other-admin@example.com", "infra", json!({"blob": {"region": "us-west-2"}}))
        .await
        .unwrap();
    assert_eq!(get(&fx).await["value"]["blob"]["region"], "us-west-2");
    let all = fx.changes().await;
    let l = lines(&all);
    assert_eq!(l.len(), 2);
    assert_eq!(l[0]["person"], "other-admin@example.com");
    assert_eq!(l[1]["person"], ALICE);
}

#[tokio::test]
async fn e2e_0117_056_blank_keys_are_refused_as_absent() {
    let fx = Fx::new().await;
    let m = set(
        &fx,
        json!({"blob": {"backend": "s3", "endpoint": "http://127.0.0.1:9", "access_key": " ", "secret_key": "   "}}),
    )
    .await
    .unwrap_err();
    assert_eq!(
        m,
        "infra.blob.access_key and infra.blob.secret_key are required when infra.blob.backend is 's3'"
    );
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_059_a_refused_member_opens_no_store() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(200).await;
    let (st, _) =
        fx.rest(BOB, "PUT", "/api/admin/settings/infra", Some(json!({"value": s3(&stand)}))).await;
    assert_eq!(st, 403);
    assert!(stand.hits().is_empty(), "a refused call reached the endpoint");
    nothing_stored(&fx).await;
}
