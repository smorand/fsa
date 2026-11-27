//! SPEC-0116 functional tests: the `server` and `auth` sections of the generic
//! settings store (SPEC-0114), on the registry fixture of SPEC-0102.
//! US-0348 covers the section list and the rules of a change to `server`.

use super::ServerSettings;
use super::e2e::Fx;
use serde_json::{Value, json};

pub(super) const ALICE: &str = "admin@example.com";
pub(super) const BOB: &str = "bob@example.com";
pub(super) const ALL_THIRTEEN: &str =
    "safety, api, extract, doc, sqlite, db, doc_service, search, git, web, context7, server, auth";

pub(super) fn settings(fx: &Fx) -> ServerSettings {
    ServerSettings::of(fx.state.admin.as_ref(), fx.state.config.clone()).unwrap()
}

pub(super) async fn set(fx: &Fx, section: &str, patch: Value) -> Result<Value, String> {
    fx.keep(settings(fx).set(ALICE, section, patch).await.map_err(|e| e.message))
}

pub(super) async fn get(fx: &Fx, section: &str) -> Value {
    fx.keep(settings(fx).get(section).await.map_err(|e| e.message)).unwrap()
}

pub(super) fn lines(all: &[Value]) -> Vec<&Value> {
    all.iter().filter(|c| c["target_kind"] == "setting").collect()
}

/// Nothing was stored for `section` and nothing was recorded.
pub(super) async fn nothing_stored(fx: &Fx, section: &str) {
    assert_eq!(get(fx, section).await["source"], "default", "{section} was stored");
    assert!(lines(&fx.changes().await).is_empty(), "a refused change was recorded");
}

fn host_msg(h: &str) -> String {
    format!("invalid server.host '{h}': expected an IPv4 address or a host name")
}

fn mount_msg(p: &str) -> String {
    format!("server.mcp_path '{p}' cannot be served: choose another path")
}

#[tokio::test]
async fn e2e_0116_001_the_listen_address_is_stored_and_waits_for_a_restart() {
    let fx = Fx::new().await;
    let out = set(&fx, "server", json!({"host": "127.0.0.1", "port": 47831})).await.unwrap();
    assert_eq!(out["value"]["host"], "127.0.0.1");
    assert_eq!(out["value"]["port"], 47831);
    assert_eq!(out["value"]["mcp_path"], "/mcp", "a key not given keeps its value");
    assert_eq!(out["source"], "database");
    assert_eq!(out["restart_required"], true);
    assert_eq!(get(&fx, "server").await, out);
    assert_eq!(fx.state.config.server.port, 5002, "the running server keeps its address");
    assert_eq!(fx.state.config.server.host, "0.0.0.0");
}

#[tokio::test]
async fn e2e_0116_002_the_list_names_thirteen_sections_and_an_unknown_one_lists_them() {
    let fx = Fx::new().await;
    let list = settings(&fx).list().await.unwrap();
    let names: Vec<&str> = list["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["section"].as_str().unwrap())
        .collect();
    assert_eq!(names.join(", "), ALL_THIRTEEN);
    for s in &list["settings"].as_array().unwrap()[11..] {
        assert_eq!(s["source"], "default");
    }
    let server = get(&fx, "server").await;
    assert_eq!(server["value"]["port"], 5002);
    let auth = get(&fx, "auth").await;
    assert_eq!(auth["source"], "default");
    assert_eq!(auth["value"]["admins"], json!([ALICE]));
    let m = set(&fx, "nope", json!({})).await.unwrap_err();
    assert_eq!(m, format!("unknown setting section 'nope', expected one of {ALL_THIRTEEN}"));
}

#[tokio::test]
async fn e2e_0116_003_a_section_name_in_another_case_is_refused() {
    let fx = Fx::new().await;
    for s in ["SERVER", "Auth"] {
        let m = set(&fx, s, json!({})).await.unwrap_err();
        assert_eq!(m, format!("unknown setting section '{s}', expected one of {ALL_THIRTEEN}"));
    }
}

#[tokio::test]
async fn e2e_0116_004_a_host_that_is_not_an_address_or_a_name_is_refused() {
    let fx = Fx::new().await;
    for h in ["bad host", "a_b", "-x", "x-", "::1", ""] {
        let m = set(&fx, "server", json!({"host": h})).await.unwrap_err();
        assert_eq!(m, host_msg(h));
    }
    nothing_stored(&fx, "server").await;
    for h in ["0.0.0.0", "localhost", "files.example.com", "10.0.0.5"] {
        set(&fx, "server", json!({"host": h})).await.unwrap();
    }
}

#[tokio::test]
async fn e2e_0116_005_an_mcp_path_without_a_leading_slash_is_refused() {
    let fx = Fx::new().await;
    let m = set(&fx, "server", json!({"mcp_path": "mcp"})).await.unwrap_err();
    assert_eq!(m, "server.mcp_path must start with '/' (got 'mcp')");
    let m = set(&fx, "server", json!({"mcp_path": ""})).await.unwrap_err();
    assert_eq!(m, "server.mcp_path must start with '/' (got '')");
    nothing_stored(&fx, "server").await;
    set(&fx, "server", json!({"mcp_path": "/rpc"})).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_007_the_running_values_set_again_need_no_restart() {
    let fx = Fx::new().await;
    let out = set(&fx, "server", json!({"host": "0.0.0.0", "port": 5002})).await.unwrap();
    assert_eq!(out["restart_required"], false);
    let all = fx.changes().await;
    let l = lines(&all);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["fields"], json!([]));
    let out = set(&fx, "server", json!({"port": 5003})).await.unwrap();
    assert_eq!(out["restart_required"], true);
    let out = set(&fx, "server", json!({"port": 5002})).await.unwrap();
    assert_eq!(out["restart_required"], false);
}

#[tokio::test]
async fn e2e_0116_038_a_refused_change_is_not_recorded_and_an_unchanged_one_is() {
    let fx = Fx::new().await;
    set(&fx, "server", json!({"host": "bad host"})).await.unwrap_err();
    assert!(lines(&fx.changes().await).is_empty());
    set(&fx, "server", json!({"port": 5002})).await.unwrap();
    let all = fx.changes().await;
    let l = lines(&all);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["fields"], json!([]));
}

#[tokio::test]
async fn e2e_0116_039_the_record_of_a_server_change_names_keys_only() {
    let fx = Fx::new().await;
    set(&fx, "server", json!({"host": "127.0.0.1", "port": 47832, "mcp_path": "/rpc"}))
        .await
        .unwrap();
    let all = fx.changes().await;
    let l = lines(&all)[0];
    assert_eq!(l["fields"], json!(["host", "mcp_path", "port"]));
    let text = l.to_string();
    assert!(!text.contains("127.0.0.1") && !text.contains("47832") && !text.contains("/rpc"));
}

#[tokio::test]
async fn e2e_0116_066_the_host_grammar_is_spelled() {
    let fx = Fx::new().await;
    let l64 = format!("{}.example", "a".repeat(64));
    let n254 =
        format!("{}.{}.{}.{}", "a".repeat(63), "b".repeat(63), "c".repeat(63), "d".repeat(62));
    assert_eq!(n254.len(), 254);
    for h in ["a..b", ".a", "a.", "a.-b", "-a.b", "é.example", "a_b.c", l64.as_str(), n254.as_str()]
    {
        let m = set(&fx, "server", json!({"host": h})).await.unwrap_err();
        assert_eq!(m, host_msg(h));
    }
    nothing_stored(&fx, "server").await;
    let l63 = format!("{}.example", "a".repeat(63));
    let n253 =
        format!("{}.{}.{}.{}", "a".repeat(63), "b".repeat(63), "c".repeat(63), "d".repeat(61));
    assert_eq!(n253.len(), 253);
    for h in ["a.b-c", "999.1.1.1", l63.as_str(), n253.as_str()] {
        set(&fx, "server", json!({"host": h})).await.unwrap();
    }
}

#[tokio::test]
async fn e2e_0116_139_the_port_rule_and_its_place_in_the_order() {
    let fx = Fx::new().await;
    let range = "invalid setting 'server': server.port must be a whole number from 1 to 65535";
    for p in [json!(70000), json!(-1), json!(0)] {
        assert_eq!(set(&fx, "server", json!({"port": p})).await.unwrap_err(), range);
    }
    for p in [json!(3.5), json!("80")] {
        let m = set(&fx, "server", json!({"port": p})).await.unwrap_err();
        assert_eq!(m, "invalid setting 'server': the value of 'port' has the wrong type");
    }
    let m = set(&fx, "server", json!({"host": "bad host", "port": 70000})).await.unwrap_err();
    assert_eq!(m, host_msg("bad host"));
    nothing_stored(&fx, "server").await;
    for p in [1, 65535] {
        set(&fx, "server", json!({"port": p})).await.unwrap();
    }
}

#[tokio::test]
async fn e2e_0116_172_the_root_and_the_health_path_cannot_be_served() {
    let fx = Fx::new().await;
    for p in ["/", "/health"] {
        assert_eq!(set(&fx, "server", json!({"mcp_path": p})).await.unwrap_err(), mount_msg(p));
    }
    nothing_stored(&fx, "server").await;
    set(&fx, "server", json!({"mcp_path": "/rpc"})).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_175_a_path_that_a_route_of_the_server_holds_cannot_be_served() {
    let fx = Fx::new().await;
    for p in
        ["/exports", "/git", "/api/docs", "/app/settings", "/app/settings/set", "/api/fs/roots"]
    {
        assert_eq!(set(&fx, "server", json!({"mcp_path": p})).await.unwrap_err(), mount_msg(p));
    }
    assert_eq!(
        set(&fx, "server", json!({"mcp_path": "/mcp/"})).await.unwrap_err(),
        mount_msg("/mcp/")
    );
    nothing_stored(&fx, "server").await;
}

#[tokio::test]
async fn e2e_0116_176_a_path_with_route_syntax_cannot_be_served() {
    let fx = Fx::new().await;
    for p in ["/{", "/a}", "/:x", "/*x", "/*", "/{*x}", "/{x}"] {
        assert_eq!(set(&fx, "server", json!({"mcp_path": p})).await.unwrap_err(), mount_msg(p));
    }
    nothing_stored(&fx, "server").await;
}

#[tokio::test]
async fn e2e_0116_191_a_path_with_odd_characters_cannot_be_served() {
    let fx = Fx::new().await;
    for p in ["/a b", "/a?b", "/a#b", "/a//b", "/a/../b", "/é"] {
        assert_eq!(set(&fx, "server", json!({"mcp_path": p})).await.unwrap_err(), mount_msg(p));
    }
    nothing_stored(&fx, "server").await;
}

#[tokio::test]
async fn e2e_0116_194_a_path_matched_by_a_route_with_a_variable_part_cannot_be_served() {
    let fx = Fx::new().await;
    for p in [
        "/git/foo",
        "/api/fs/foo",
        "/app/converters/default",
        "/app/converters/default/set",
        "/api/docs/x",
        "/exports/mcp",
    ] {
        assert_eq!(set(&fx, "server", json!({"mcp_path": p})).await.unwrap_err(), mount_msg(p));
    }
    nothing_stored(&fx, "server").await;
    for p in ["/gitx", "/app", "/a/b", "/api", "/a-b_c.d~e/MCP"] {
        set(&fx, "server", json!({"mcp_path": p})).await.unwrap();
    }
}

/// An operator editing the stored `server` value by hand, no check.
async fn put_server_raw(fx: &Fx, value: Value) {
    let store = fx.state.admin.settings().unwrap();
    let exists = store.all_server_settings().await.unwrap().iter().any(|r| r.section == "server");
    let mut tx = store.begin().await.unwrap();
    let row = super::server_settings_store::ServerSettingRow {
        section: "server".into(),
        value: value.to_string(),
        secret_enc: None,
        updated_at: "2026-11-27T00:00:00Z".into(),
        updated_by: "operator".into(),
    };
    super::store::SettingsStore::put_server_setting(&mut tx, &row, exists).await.unwrap();
    tx.commit().await.unwrap();
}

const REDIRECT: &str = "server.public_base_url cannot be changed through the settings: it prefixes download links that carry a credential, ask the operator to change it in the database";

#[tokio::test]
async fn e2e_0116_154_the_public_base_url_cannot_be_redirected_by_a_change() {
    let fx = Fx::new().await;
    let mut v = get(&fx, "server").await["value"].clone();
    v["public_base_url"] = json!("https://files.example");
    put_server_raw(&fx, v).await;
    let before = lines(&fx.changes().await).len();
    let m =
        set(&fx, "server", json!({"public_base_url": "https://evil.example"})).await.unwrap_err();
    assert_eq!(m, REDIRECT);
    assert_eq!(lines(&fx.changes().await).len(), before, "a refusal is not recorded");
    assert_eq!(get(&fx, "server").await["value"]["public_base_url"], "https://files.example");
}

#[tokio::test]
async fn e2e_0116_155_the_public_base_url_in_force_and_the_empty_text_are_accepted() {
    let fx = Fx::new().await;
    let mut v = get(&fx, "server").await["value"].clone();
    v["public_base_url"] = json!("https://files.example");
    put_server_raw(&fx, v).await;
    set(&fx, "server", json!({"public_base_url": "https://files.example"})).await.unwrap();
    let out = set(&fx, "server", json!({"public_base_url": ""})).await.unwrap();
    assert_eq!(out["value"]["public_base_url"], "");
}

#[tokio::test]
async fn e2e_0116_159_the_base_url_refusal_comes_before_the_host_one() {
    let fx = Fx::new().await;
    let m = set(&fx, "server", json!({"host": "bad host", "public_base_url": "ftp://x"}))
        .await
        .unwrap_err();
    assert_eq!(m, REDIRECT);
    nothing_stored(&fx, "server").await;
}
