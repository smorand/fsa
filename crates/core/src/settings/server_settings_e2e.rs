//! SPEC-0114 functional tests of the generic settings store, on the registry
//! fixture of SPEC-0102 (a SQLite admin store reopened by a restart).

use super::ServerSettings;
use super::e2e::Fx;
use serde_json::{Value, json};

pub(super) const ALICE: &str = "admin@example.com";
const TOKEN: &str = "tok-abcdef123456";

fn settings(fx: &Fx) -> ServerSettings {
    ServerSettings::of(fx.state.admin.as_ref(), fx.state.config.clone()).unwrap()
}

async fn set(fx: &Fx, section: &str, patch: Value) -> Result<Value, String> {
    fx.keep(settings(fx).set(ALICE, section, patch).await.map_err(|e| e.message))
}

async fn get(fx: &Fx, section: &str) -> Value {
    fx.keep(settings(fx).get(section).await.map_err(|e| e.message)).unwrap()
}

fn setting_changes(all: &[Value]) -> Vec<&Value> {
    all.iter().filter(|c| c["target_kind"] == "setting").collect()
}

#[tokio::test]
async fn e2e_0114_001_set_one_scalar_flags_a_restart() {
    let fx = Fx::new().await;
    let out = set(&fx, "safety", json!({"max_read_lines": 500})).await.unwrap();
    assert_eq!(out["section"], "safety");
    assert_eq!(out["value"]["max_read_lines"], 500);
    assert_eq!(out["value"]["trash_dir"], ".mcp_trash", "a key not given keeps its value");
    assert_eq!(out["source"], "database");
    assert_eq!(out["restart_required"], true);
    assert_eq!(get(&fx, "safety").await, out);
    assert_eq!(fx.state.config.safety.max_read_lines, 2000, "the running server is not changed");
    let all = fx.changes().await;
    let lines = setting_changes(&all);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["action"], "set_setting");
    assert_eq!(lines[0]["target_id"], "safety");
    assert_eq!(lines[0]["fields"], json!(["max_read_lines"]));
    assert_eq!(lines[0]["person"], ALICE);
}

#[tokio::test]
async fn e2e_0114_002_a_secret_is_write_only() {
    let fx = Fx::new().await;
    let out = set(&fx, "doc_service", json!({"api": {"auth_token": TOKEN, "url": "http://x/c"}}))
        .await
        .unwrap();
    assert_eq!(out["secrets"]["api.auth_token"], json!({"has_value": true, "last4": "3456"}));
    assert!(out["value"]["api"].get("auth_token").is_none(), "{out}");
    assert_eq!(out["value"]["api"]["url"], "http://x/c");
    assert_eq!(get(&fx, "doc_service").await, out);
    // A short secret shows no characters.
    let short = set(&fx, "doc_service", json!({"api": {"auth_token": "abc"}})).await.unwrap();
    assert_eq!(short["secrets"]["api.auth_token"], json!({"has_value": true, "last4": ""}));
    // Absent keeps, an empty string clears.
    let kept = set(&fx, "doc_service", json!({"max_input_bytes": 1000})).await.unwrap();
    assert_eq!(kept["secrets"]["api.auth_token"]["has_value"], true);
    let cleared = set(&fx, "doc_service", json!({"api": {"auth_token": ""}})).await.unwrap();
    assert_eq!(cleared["secrets"]["api.auth_token"], json!({"has_value": false, "last4": ""}));
    // The change record names the key, never a value.
    let all = fx.changes().await;
    let first = setting_changes(&all).into_iter().last().unwrap();
    assert_eq!(first["fields"], json!(["api.auth_token", "api.url"]));
    fx.assert_nowhere(TOKEN).await;
    fx.assert_nowhere("abc\"").await;
}

#[tokio::test]
async fn e2e_0114_006_a_value_today_s_check_refuses_is_refused_alike() {
    let fx = Fx::new().await;
    let m = set(&fx, "safety", json!({"max_diff_file_bytes": 0})).await.unwrap_err();
    assert_eq!(m, "invalid safety.max_diff_file_bytes: must be a whole number of bytes above zero");
    let m = set(&fx, "search", json!({"settle_delay_seconds": "abc"})).await.unwrap_err();
    assert_eq!(m, "settle delay must be a whole number of seconds, 0 or more");
    let m = set(&fx, "search", json!({"enabled": true, "mode": "nope"})).await.unwrap_err();
    assert!(m.contains("search.mode"), "{m}");
    // Nothing was stored.
    assert_eq!(get(&fx, "safety").await["source"], "default");
    assert_eq!(get(&fx, "search").await["source"], "default");
    assert!(setting_changes(&fx.changes().await).is_empty());
}

#[tokio::test]
async fn e2e_0114_009_unknown_names_and_wrong_types_name_the_section() {
    let fx = Fx::new().await;
    let m = set(&fx, "nope", json!({})).await.unwrap_err();
    assert_eq!(
        m,
        "unknown setting section 'nope', expected one of safety, api, extract, doc, sqlite, db, doc_service, search, git, web, context7, server, auth, infra"
    );
    let m = settings(&fx).get("nope").await.unwrap_err().message;
    assert!(m.starts_with("unknown setting section 'nope'"), "{m}");
    let m = set(&fx, "safety", json!({"nope": 1})).await.unwrap_err();
    assert_eq!(m, "unknown key 'nope' in setting 'safety'");
    let m = set(&fx, "doc_service", json!({"api": {"nope": 1}})).await.unwrap_err();
    assert_eq!(m, "unknown key 'api.nope' in setting 'doc_service'");
    let m = set(&fx, "safety", json!({"max_read_lines": "many"})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'safety':"), "{m}");
    let m = set(&fx, "doc_service", json!({"api": 3})).await.unwrap_err();
    assert!(m.contains("'doc_service'") && m.contains("api"), "{m}");
    let m = set(&fx, "safety", json!([1])).await.unwrap_err();
    assert_eq!(m, "the value of setting 'safety' must be an object");
    let m = set(&fx, "doc_service", json!({"api": {"auth_token": 5}})).await.unwrap_err();
    assert_eq!(m, "api.auth_token of setting 'doc_service' must be a text");
    assert!(setting_changes(&fx.changes().await).is_empty());
}

#[tokio::test]
async fn e2e_0114_009b_a_secret_needs_the_master_key() {
    let fx = Fx::with_key(None).await;
    let m = set(&fx, "doc_service", json!({"api": {"auth_token": TOKEN}})).await.unwrap_err();
    assert_eq!(m, "no master key is configured; set FSA_TOKEN_KEY");
    // A section without a secret key needs none.
    set(&fx, "safety", json!({"max_read_lines": 10})).await.unwrap();
}

#[tokio::test]
async fn e2e_0114_010_list_and_get_shapes() {
    let fx = Fx::new().await;
    let list = settings(&fx).list().await.unwrap();
    let names: Vec<&str> = list["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["section"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
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
            "infra"
        ]
    );
    for s in list["settings"].as_array().unwrap() {
        assert_eq!(s["source"], "default");
        assert_eq!(s["restart_required"], false);
        assert_eq!(s["updated_at"], Value::Null);
    }
    set(&fx, "api", json!({"enabled": false})).await.unwrap();
    let list = settings(&fx).list().await.unwrap();
    let api = &list["settings"][1];
    assert_eq!(api["source"], "database");
    assert_eq!(api["restart_required"], true);
    assert_eq!(api["updated_by"], ALICE);
    assert!(api["updated_at"].as_str().unwrap().ends_with('Z'));
    let g = get(&fx, "sqlite").await;
    assert_eq!(
        g["value"],
        json!({"enabled": false, "max_result_rows": 1000, "statement_timeout_secs": 30})
    );
    assert_eq!(g["secrets"], json!({}));
}

#[tokio::test]
async fn e2e_0114_011_concurrent_sets_last_write_wins_and_both_are_recorded() {
    let fx = Fx::new().await;
    let s = settings(&fx);
    let (a, b) = tokio::join!(
        s.set(ALICE, "safety", json!({"max_read_lines": 11})),
        s.set("other-admin@example.com", "safety", json!({"max_read_lines": 22})),
    );
    a.unwrap();
    b.unwrap();
    let v = get(&fx, "safety").await["value"]["max_read_lines"].as_i64().unwrap();
    assert!(v == 11 || v == 22);
    let all = fx.changes().await;
    assert_eq!(setting_changes(&all).len(), 2);
}

#[tokio::test]
async fn e2e_0114_013_each_section_round_trips() {
    let fx = Fx::new().await;
    for section in super::server_settings::SECTIONS {
        let before = get(&fx, section).await;
        let again = set(&fx, section, before["value"].clone()).await.unwrap();
        assert_eq!(again["value"], before["value"], "{section}");
        assert_eq!(again["secrets"], before["secrets"], "{section}");
        assert_eq!(again["restart_required"], false, "{section}");
    }
    let all = fx.changes().await;
    assert!(setting_changes(&all).iter().all(|c| c["fields"] == json!([])));
}

// ---- US-0342: the boot overlay and the database URL bootstrap ----

use crate::config::ServerConfig;
use base64::Engine;

pub(super) const KEY_A: [u8; 32] = [42u8; 32];
const KEY_B: [u8; 32] = [43u8; 32];

fn env_of(key: Option<[u8; 32]>) -> impl Fn(&str) -> Option<String> + Sync {
    move |n| {
        (n == "FSA_TOKEN_KEY")
            .then(|| key.map(|k| base64::engine::general_purpose::STANDARD.encode(k)))
            .flatten()
    }
}

/// A configuration for a real start in `root`, with the admin store at
/// `root/admin.db`: the shape `--database-url sqlite:...` gives.
pub(super) fn boot_config(root: &std::path::Path) -> ServerConfig {
    let (_key, public) = crate::keys::write_keypair(root.join("keys")).unwrap();
    let mut c = ServerConfig::default();
    c.auth.jwt.public_key_path = public.display().to_string();
    c.auth.admins = vec![ALICE.into()];
    c.infra.meta.dir = root.join("volumes").display().to_string();
    c.infra.blob.dir = root.join("blobs").display().to_string();
    c.set_database_url(&format!("sqlite:{}", root.join("admin.db").display())).unwrap();
    c
}

pub(super) async fn boot(
    config: ServerConfig,
    key: Option<[u8; 32]>,
) -> Result<(axum::Router, std::sync::Arc<crate::state::AppState>), String> {
    crate::app::build_with_env(config, &env_of(key)).await.map_err(|e| e.to_string())
}

/// A YAML load publishes the process wide git host map, so it takes the lock
/// the tests of that map take; the guard is dropped before any await.
pub(super) fn load_yaml(path: &std::path::Path) -> ServerConfig {
    let _hosts = crate::git::remote::tests::lock_for_test();
    ServerConfig::load(path).unwrap()
}

pub(super) async fn set_on(state: &crate::state::AppState, section: &str, patch: Value) {
    ServerSettings::of(state.admin.as_ref(), std::sync::Arc::new(state.config.as_ref().clone()))
        .unwrap()
        .set(ALICE, section, patch)
        .await
        .unwrap();
}

#[tokio::test]
async fn e2e_0114_003_start_with_only_the_database_and_the_key_reads_the_settings() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "safety", json!({"max_read_lines": 500})).await;
    set_on(&first, "api", json!({"enabled": false})).await;
    set_on(&first, "doc_service", json!({"api": {"auth_token": TOKEN}})).await;
    assert_eq!(first.config.safety.max_read_lines, 2000, "a set is not live");
    drop(first);
    let (router, second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(second.config.safety.max_read_lines, 500);
    assert!(!second.config.api.enabled);
    assert_eq!(second.config.doc_service.api.auth_token.expose(), TOKEN);
    use tower::ServiceExt;
    let res = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/swagger.json")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "api.enabled false read from the database");
    let got = ServerSettings::of(second.admin.as_ref(), second.config.clone())
        .unwrap()
        .get("safety")
        .await
        .unwrap();
    assert_eq!(got["restart_required"], false, "the running server holds the stored value");
}

pub(super) fn yaml_for(root: &std::path::Path, extra: &str) -> std::path::PathBuf {
    let p = root.join("config.yaml");
    let (_k, public) = crate::keys::write_keypair(root.join("keys")).unwrap();
    std::fs::write(
        &p,
        format!(
            "auth:\n  jwt:\n    public_key_path: {}\n  admins: [{ALICE}]\ninfra:\n  meta:\n    dir: {}\n  blob:\n    dir: {}\n  admin:\n    path: {}\n{extra}",
            public.display(),
            root.join("volumes").display(),
            root.join("blobs").display(),
            root.join("admin.db").display(),
        ),
    )
    .unwrap();
    p
}

#[tokio::test]
async fn e2e_0114_004_the_yaml_seeds_once_and_a_later_edit_does_not_override() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_for(d.path(), "safety:\n  max_read_lines: 700\n");
    let (_r, first) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(first.config.safety.max_read_lines, 700);
    let s = ServerSettings::of(first.admin.as_ref(), first.config.clone()).unwrap();
    let list = s.list().await.unwrap();
    for e in list["settings"].as_array().unwrap() {
        assert_eq!(e["source"], "database", "{e}");
        assert_eq!(e["updated_by"], "system:yaml");
    }
    drop(first);
    let path = yaml_for(d.path(), "safety:\n  max_read_lines: 900\n");
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(second.config.safety.max_read_lines, 700, "the database wins");
    let again = ServerSettings::of(second.admin.as_ref(), second.config.clone()).unwrap();
    let changes = again.list().await.unwrap();
    assert_eq!(changes["settings"][0]["updated_by"], "system:yaml", "seeded once, not again");
}

#[tokio::test]
async fn e2e_0114_008_a_stored_secret_needs_the_key_it_was_stored_with() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "doc_service", json!({"api": {"auth_token": TOKEN}})).await;
    drop(first);
    for key in [None, Some(KEY_B)] {
        let e = boot(boot_config(d.path()), key).await.err().unwrap();
        assert_eq!(
            e,
            "the stored setting 'doc_service' holds a secret that cannot be read: set FSA_TOKEN_KEY to the key it was stored with"
        );
        assert!(!e.contains(TOKEN));
    }
    let _ = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    // A malformed key stops the start, as before.
    let bad = |n: &str| (n == "FSA_TOKEN_KEY").then(|| "not-a-key".to_string());
    let e = crate::app::build_with_env(boot_config(d.path()), &bad).await.err().unwrap();
    assert!(e.to_string().starts_with("FSA_TOKEN_KEY must be valid base64."), "{e}");
}

#[tokio::test]
async fn e2e_0114_008b_a_yaml_secret_without_a_key_stays_in_force_and_is_not_imported() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_for(
        d.path(),
        "doc_service:\n  mode: api\n  api:\n    url: http://x/c\n    auth_token: from-the-yaml-token\n",
    );
    let (_r, state) = boot(load_yaml(&path), None).await.unwrap();
    assert_eq!(state.config.doc_service.api.auth_token.expose(), "from-the-yaml-token");
    let s = ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap();
    let list = s.list().await.unwrap();
    let ds = list["settings"].as_array().unwrap().iter().find(|e| e["section"] == "doc_service");
    assert_eq!(ds.unwrap()["source"], "yaml", "not imported, still the file's");
}

#[tokio::test]
async fn e2e_0114_015_a_forced_switch_wins_over_a_stored_false() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "sqlite", json!({"enabled": false, "max_result_rows": 7})).await;
    drop(first);
    let mut c = boot_config(d.path());
    c.force("sqlite");
    let (_r, second) = boot(c, Some(KEY_A)).await.unwrap();
    assert!(second.config.sqlite.enabled, "the flag forces it on");
    assert_eq!(second.config.sqlite.max_result_rows, 7, "the rest is the stored value");
}

#[tokio::test]
async fn e2e_0114_016_today_s_yaml_alone_behaves_identically() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_for(
        d.path(),
        "safety:\n  max_read_lines: 123\n  max_diff_file_bytes: 4096\napi:\n  enabled: false\nsqlite:\n  enabled: true\n  max_result_rows: 9\ndoc:\n  pandoc_timeout_secs: 11\ndb:\n  max_file_bytes: 1000\nsearch:\n  settle_delay_seconds: 7\n  chunking:\n    max_tokens: 900\ndoc_service:\n  mode: api\n  max_input_bytes: 5000\n  api:\n    url: http://x/c\n    auth_token: yaml-token-123456\n",
    );
    let yaml = load_yaml(&path);
    let (_r, state) = boot(yaml.clone(), Some(KEY_A)).await.unwrap();
    for section in super::server_settings::SECTIONS {
        assert_eq!(
            super::server_settings::section_json(&state.config, section),
            super::server_settings::section_json(&yaml, section),
            "{section}"
        );
    }
    drop(state);
    let (_r, again) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    for section in super::server_settings::SECTIONS {
        assert_eq!(
            super::server_settings::section_json(&again.config, section),
            super::server_settings::section_json(&yaml, section),
            "{section} after the seed was read back"
        );
    }
}

#[tokio::test]
async fn e2e_0114_012_no_debug_log_or_record_holds_a_secret() {
    let fx = Fx::new().await;
    set(&fx, "doc_service", json!({"api": {"auth_token": TOKEN}})).await.unwrap();
    // The configuration a boot reads back prints redacted.
    let mut c = fx.state.config.as_ref().clone();
    c.doc_service.api.auth_token = crate::config::Secret::new(TOKEN);
    assert!(!format!("{c:?}").contains(TOKEN));
    assert!(!format!("{:?}", c.doc_service).contains(TOKEN));
    fx.assert_nowhere(TOKEN).await;
}

#[test]
fn e2e_0114_017_the_database_url_sets_the_admin_store() {
    let mut c = ServerConfig::default();
    c.set_database_url("sqlite:/tmp/x/admin.db").unwrap();
    assert_eq!(c.infra.admin.backend, "sqlite");
    assert_eq!(c.infra.admin.path, "/tmp/x/admin.db");
    assert!(c.infra.admin.dsn.is_empty());
    c.set_database_url("postgres://u:pw@h:5432/db").unwrap();
    assert_eq!(c.infra.admin.backend, "postgres");
    assert_eq!(c.infra.admin.dsn.expose(), "postgres://u:pw@h:5432/db");
    c.set_database_url("postgresql://u@h/db").unwrap();
    assert_eq!(c.infra.admin.backend, "postgres");
    c.set_database_url("sqlite:/y.db").unwrap();
    assert!(c.infra.admin.dsn.is_empty(), "a SQLite store holds no DSN");
    for bad in ["", "   ", "sqlite:", "mysql://u:secret@h/db", "/tmp/x.db"] {
        let e = c.set_database_url(bad).unwrap_err();
        assert_eq!(
            e.message,
            "the database URL must start with postgres://, postgresql:// or sqlite:PATH"
        );
        assert!(!e.message.contains("secret"));
    }
}

#[test]
fn e2e_0114_017b_serve_needs_a_file_or_a_url_and_the_url_wins() {
    // The host map is process wide: a start that succeeds publishes it.
    let _hosts = crate::git::remote::tests::lock_for_test();
    use crate::cli::load_serve_config_with;
    let d = tempfile::tempdir().unwrap();
    let none =
        |_: Option<&std::path::Path>| Err(vec![std::path::PathBuf::from("config/local.yaml")]);
    // No file and no URL keeps today's refusal, word for word.
    let e = load_serve_config_with(None, None, none).unwrap_err();
    assert_eq!(
        e.message,
        "no configuration file found, tried: config/local.yaml. Pass --config PATH, set FSA_CONFIG, or create one of those files"
    );
    assert!(load_serve_config_with(None, Some("  "), none).is_err(), "a blank URL is no URL");
    // A URL alone: the defaults, no file, nothing to seed from.
    let (c, p) = load_serve_config_with(None, Some("sqlite:/a.db"), none).unwrap();
    assert!(p.is_none());
    assert!(!c.from_yaml_file);
    assert_eq!(c.infra.admin.path, "/a.db");
    // A bad URL is refused, with or without a file.
    assert!(load_serve_config_with(None, Some("mysql://x"), none).is_err());
    // An explicit file that is absent is refused by name, URL or not.
    let missing = d.path().join("nope.yaml");
    let named = |p: Option<&std::path::Path>| Ok(p.unwrap().to_path_buf());
    let e = load_serve_config_with(Some(&missing), Some("sqlite:/a.db"), named).unwrap_err();
    assert!(e.message.contains("nope.yaml"), "{}", e.message);
    // A file and a URL: the URL replaces infra.admin, the file's other values stay.
    let path = yaml_for(d.path(), "safety:\n  max_read_lines: 5\n");
    let (c, p) = load_serve_config_with(Some(&path), Some("sqlite:/other.db"), named).unwrap();
    assert_eq!(c.infra.admin.path, "/other.db");
    assert_eq!(c.safety.max_read_lines, 5);
    assert_eq!(p.as_deref(), Some(path.as_path()));
    assert!(c.from_yaml_file);
}

// ---- US-0343: tools, REST routes, screen ----

use super::e2e::{Via, token_of};
use crate::mcp::server::{SectionArgs, SetSettingArgs};
use crate::search::derived_e2e::{args, tool_json};

const BOB: &str = "bob@example.com";

async fn tool_set(fx: &Fx, who: &str, section: &str, value: Value) -> Result<Value, String> {
    let p: rmcp::handler::server::wrapper::Parameters<SetSettingArgs> =
        args(json!({"section": section, "value": value}));
    fx.keep(tool_json(fx.server(who).admin_set_setting(p).await))
}

async fn tool_get(fx: &Fx, who: &str, section: &str) -> Result<Value, String> {
    let p: rmcp::handler::server::wrapper::Parameters<SectionArgs> =
        args(json!({"section": section}));
    fx.keep(tool_json(fx.server(who).admin_get_setting(p).await))
}

async fn tool_list(fx: &Fx, who: &str) -> Result<Value, String> {
    fx.keep(tool_json(fx.server(who).admin_list_settings().await))
}

#[tokio::test]
async fn e2e_0114_020_the_three_tools_answer_an_administrator() {
    let fx = Fx::new().await;
    let set = tool_set(&fx, ALICE, "safety", json!({"max_read_lines": 321})).await.unwrap();
    assert_eq!(set["value"]["max_read_lines"], 321);
    assert_eq!(tool_get(&fx, ALICE, "safety").await.unwrap(), set);
    let list = tool_list(&fx, ALICE).await.unwrap();
    assert_eq!(list["settings"][0]["source"], "database");
    // An empty or absent value is an object of no key, which changes nothing.
    let same = tool_set(&fx, ALICE, "safety", json!({})).await.unwrap();
    assert_eq!(same["value"], set["value"]);
}

#[tokio::test]
async fn e2e_0114_007_a_non_administrator_is_refused_everywhere() {
    let fx = Fx::new().await;
    tool_set(&fx, ALICE, "doc_service", json!({"api": {"auth_token": TOKEN}})).await.unwrap();
    let forbidden = "ERR_FORBIDDEN: 'bob@example.com' is not a platform admin";
    assert_eq!(tool_list(&fx, BOB).await.unwrap_err(), forbidden);
    assert_eq!(tool_get(&fx, BOB, "safety").await.unwrap_err(), forbidden);
    assert_eq!(tool_set(&fx, BOB, "safety", json!({})).await.unwrap_err(), forbidden);
    // The refusal comes before the shape check.
    assert_eq!(tool_get(&fx, BOB, "nope").await.unwrap_err(), forbidden);
    let (st, body) = fx.rest(BOB, "GET", "/api/admin/settings", None).await;
    assert_eq!(st, 403, "{body}");
    let (st, _) = fx.rest(BOB, "GET", "/api/admin/settings/safety", None).await;
    assert_eq!(st, 403);
    let (st, _) = fx
        .rest(
            BOB,
            "PUT",
            "/api/admin/settings/safety",
            Some(json!({"value": {"max_read_lines": 1}})),
        )
        .await;
    assert_eq!(st, 403);
    let router = crate::settings_screen::router(fx.state.clone());
    let (st, html) = fx.page(&router, Via::Cookie(BOB), "GET", "/app/settings", None).await;
    assert_eq!(st, 403, "{html}");
    let (st, _) = fx
        .page(
            &router,
            Via::Cookie(BOB),
            "POST",
            "/app/settings/set",
            Some(&[("section", "safety"), ("value", "{}"), ("csrf_token", "x")]),
        )
        .await;
    assert_eq!(st, 403);
    assert_eq!(get(&fx, "safety").await["source"], "default", "nothing changed");
    fx.assert_nowhere(TOKEN).await;
}

#[tokio::test]
async fn e2e_0114_019_the_rest_routes_equal_the_tools() {
    let fx = Fx::new().await;
    let (st, put) = fx
        .rest(
            ALICE,
            "PUT",
            "/api/admin/settings/doc_service",
            Some(json!({"value": {"api": {"auth_token": TOKEN, "url": "http://x/c"}}})),
        )
        .await;
    assert_eq!(st, 200, "{put}");
    let (st, got) = fx.rest(ALICE, "GET", "/api/admin/settings/doc_service", None).await;
    assert_eq!(st, 200);
    assert_eq!(got, put);
    assert_eq!(tool_get(&fx, ALICE, "doc_service").await.unwrap(), got);
    let (st, list) = fx.rest(ALICE, "GET", "/api/admin/settings", None).await;
    assert_eq!(st, 200);
    assert_eq!(list, tool_list(&fx, ALICE).await.unwrap());
    // Refusals carry the tool's words in the plane's envelope.
    let (st, e) = fx
        .rest(
            ALICE,
            "PUT",
            "/api/admin/settings/safety",
            Some(json!({"value": {"max_diff_file_bytes": 0}})),
        )
        .await;
    assert_eq!(st, 400);
    assert!(e.to_string().contains("max_diff_file_bytes"), "{e}");
    let (st, e) = fx.rest(ALICE, "GET", "/api/admin/settings/nope", None).await;
    assert_eq!(st, 400);
    assert!(e.to_string().contains("unknown setting section 'nope'"), "{e}");
    fx.assert_nowhere(TOKEN).await;
}

#[tokio::test]
async fn e2e_0114_005_the_screen_lists_edits_and_records() {
    let fx = Fx::new().await;
    tool_set(&fx, ALICE, "doc_service", json!({"api": {"auth_token": TOKEN}})).await.unwrap();
    let router = crate::settings_screen::router(fx.state.clone());
    let (st, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/settings", None).await;
    assert_eq!(st, 200, "{html}");
    for s in super::server_settings::SECTIONS {
        assert!(html.contains(&format!("data-section=\"{s}\"")), "{s}");
    }
    assert!(html.contains("Secret api.auth_token: set, ends with 3456"), "{html}");
    assert!(html.contains("Source: database (restart required)"));
    assert!(!html.contains(TOKEN));
    // Edit one section as a browser: token from the page, then the post.
    let token = token_of(&html);
    let (st, _) = fx
        .page(
            &router,
            Via::Cookie(ALICE),
            "POST",
            "/app/settings/set",
            Some(&[
                ("section", "safety"),
                ("value", "{\"max_read_lines\": 77}"),
                ("csrf_token", &token),
            ]),
        )
        .await;
    assert_eq!(st, 303);
    assert_eq!(get(&fx, "safety").await["value"]["max_read_lines"], 77);
    let (_, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/settings", None).await;
    assert!(html.contains("max_read_lines"), "{html}");
    assert!(html.contains("<td>set_setting</td>"), "the change is listed: {html}");
    // A refused value shows the refusal on the page, and the token is single use.
    let token = token_of(&html);
    let (st, page) = fx
        .page(
            &router,
            Via::Cookie(ALICE),
            "POST",
            "/app/settings/set",
            Some(&[
                ("section", "safety"),
                ("value", "{\"max_diff_file_bytes\": 0}"),
                ("csrf_token", &token),
            ]),
        )
        .await;
    assert_eq!(st, 400, "{page}");
    assert!(page.contains("max_diff_file_bytes"));
    let (st, _) = fx
        .page(
            &router,
            Via::Cookie(ALICE),
            "POST",
            "/app/settings/set",
            Some(&[("section", "safety"), ("value", "{}"), ("csrf_token", &token)]),
        )
        .await;
    assert_eq!(st, 403, "a used token is refused");
    let (st, _) = fx
        .page(
            &router,
            Via::Cookie(ALICE),
            "POST",
            "/app/settings/set",
            Some(&[("section", "safety")]),
        )
        .await;
    assert_eq!(st, 403, "no token");
    fx.assert_nowhere(TOKEN).await;
}

#[tokio::test]
async fn e2e_0114_021_the_purge_verb_reads_the_stored_settings() {
    let d = tempfile::tempdir().unwrap();
    // The file says 30 days of grace; the database (seeded from it, then changed) says 0.
    let path = yaml_for(d.path(), "safety:\n  project_purge_grace_days: 30\n");
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    set_on(&state, "safety", json!({"project_purge_grace_days": 0})).await;
    state.admin.create_project("gone", ALICE).await.unwrap();
    state.stores.provision_volume("gone").await.unwrap();
    assert!(state.admin.soft_delete_project("gone").await.unwrap());
    let admin = state.admin.clone();
    drop(state);
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let cli = crate::cli::Cli {
        command: Some(crate::cli::Command::Purge {
            config: Some(path.clone()),
            project: None,
            on_demand: false,
        }),
    };
    // The verb loads the YAML, which publishes the process wide git host map:
    // run it in its own thread and runtime, holding the lock the map's tests take.
    std::thread::spawn(move || {
        let _hosts = crate::git::remote::tests::lock_for_test();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(crate::cli::dispatch(cli)).unwrap();
    })
    .join()
    .unwrap();
    assert!(admin.get_project("gone").await.unwrap().is_none(), "removed with the stored 0 days");
}

#[tokio::test]
async fn e2e_0114_014_a_setting_survives_fsa_migrate() {
    let d = tempfile::tempdir().unwrap();
    let from = boot_config(&d.path().join("src"));
    let to = boot_config(&d.path().join("dst"));
    let (_r, first) = boot(from.clone(), Some(KEY_A)).await.unwrap();
    set_on(&first, "safety", json!({"max_read_lines": 321})).await;
    set_on(&first, "doc_service", json!({"api": {"auth_token": TOKEN}})).await;
    drop(first);
    let report = crate::migrate::migrate(&from, &to).await.unwrap();
    assert!(report.tables.iter().any(|t| t.table == "server_setting" && t.rows == 2), "{report:?}");
    let (_r, moved) = boot(to.clone(), Some(KEY_A)).await.unwrap();
    assert_eq!(moved.config.safety.max_read_lines, 321);
    assert_eq!(moved.config.doc_service.api.auth_token.expose(), TOKEN);
    drop(moved);
    // The ciphertext moved as it was: another key reads nothing and stops the start.
    let e = boot(to, Some(KEY_B)).await.err().unwrap();
    assert!(e.starts_with("the stored setting 'doc_service' holds a secret"), "{e}");
}

#[test]
fn e2e_0114_017c_the_serve_verb_takes_the_database_url_flag() {
    use clap::Parser;
    let cli = crate::cli::Cli::try_parse_from(["fsa", "serve", "--database-url", "sqlite:/x.db"])
        .unwrap();
    match cli.command {
        Some(crate::cli::Command::Serve { database_url, config, .. }) => {
            assert_eq!(database_url.as_deref(), Some("sqlite:/x.db"));
            assert!(config.is_none(), "the file is optional");
        }
        other => panic!("not serve: {other:?}"),
    }
    let cli = crate::cli::Cli::try_parse_from(["fsa", "serve"]).unwrap();
    assert!(matches!(cli.command, Some(crate::cli::Command::Serve { database_url: None, .. })));
}

#[tokio::test]
async fn e2e_0114_015b_a_forced_switch_is_never_stored() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_for(d.path(), "doc:\n  pandoc_timeout_secs: 12\n");
    // First start with --doc: the seed holds the file's value, not the flag's.
    let mut c = load_yaml(&path);
    c.force("doc");
    let (_r, first) = boot(c, Some(KEY_A)).await.unwrap();
    assert!(first.config.doc.enabled);
    let s = ServerSettings::of(first.admin.as_ref(), first.config.clone()).unwrap();
    let doc = s.get("doc").await.unwrap();
    assert_eq!(doc["value"]["enabled"], false, "the flag is not a setting");
    assert_eq!(doc["restart_required"], false, "a forced switch is not a pending change");
    // A change of another key must not store the flag either.
    set_on(&first, "doc", json!({"pandoc_timeout_secs": 13})).await;
    drop(first);
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert!(!second.config.doc.enabled, "no flag, no switch");
    assert_eq!(second.config.doc.pandoc_timeout_secs, 13);
    // A stored switch is the one the next start without the flag goes back to.
    set_on(&second, "doc", json!({"enabled": true})).await;
    drop(second);
    let mut c = load_yaml(&path);
    c.force("doc");
    let (_r, third) = boot(c, Some(KEY_A)).await.unwrap();
    assert!(third.config.doc.enabled);
    let s = ServerSettings::of(third.admin.as_ref(), third.config.clone()).unwrap();
    assert_eq!(s.get("doc").await.unwrap()["value"]["enabled"], true);
    assert_eq!(s.get("doc").await.unwrap()["restart_required"], false);
}
