//! SPEC-0115 functional tests: the `git`, `web` and `context7` sections of the
//! generic settings store (SPEC-0114), on the registry fixture of SPEC-0102.
//! US-0345 covers the rules (set, get, list, refusals, record, screen).

use super::ServerSettings;
use super::e2e::{Fx, Via, token_of};
use crate::mcp::server::{SectionArgs, SetSettingArgs};
use crate::search::derived_e2e::{args, tool_json};
use serde_json::{Value, json};

const ALICE: &str = "admin@example.com";
const BOB: &str = "bob@example.com";
const BARE: &str = "git.hosts key '";
const ALL_THIRTEEN: &str = "safety, api, extract, doc, sqlite, db, doc_service, search, git, web, context7, server, auth, infra";

fn settings(fx: &Fx) -> ServerSettings {
    ServerSettings::of(fx.state.admin.as_ref(), fx.state.config.clone()).unwrap()
}

async fn set_as(fx: &Fx, who: &str, section: &str, patch: Value) -> Result<Value, String> {
    fx.keep(settings(fx).set(who, section, patch).await.map_err(|e| e.message))
}

async fn set(fx: &Fx, section: &str, patch: Value) -> Result<Value, String> {
    set_as(fx, ALICE, section, patch).await
}

async fn get(fx: &Fx, section: &str) -> Value {
    fx.keep(settings(fx).get(section).await.map_err(|e| e.message)).unwrap()
}

fn lines(all: &[Value]) -> Vec<&Value> {
    all.iter().filter(|c| c["target_kind"] == "setting").collect()
}

async fn nothing_stored(fx: &Fx, section: &str) {
    assert_eq!(get(fx, section).await["source"], "default", "{section} was stored");
    assert!(lines(&fx.changes().await).is_empty(), "a refused change was recorded");
}

#[tokio::test]
async fn e2e_0115_001_a_host_is_declared_and_waits_for_a_restart() {
    let fx = Fx::new().await;
    let out = set(&fx, "git", json!({"hosts": {"github.example.com": "github"}})).await.unwrap();
    assert_eq!(out["value"]["hosts"], json!({"github.example.com": "github"}));
    assert_eq!(out["restart_required"], true);
    assert_eq!(out["source"], "database");
    assert_eq!(get(&fx, "git").await, out);
    assert_eq!(out["value"]["max_pack_size_mb"], 512, "a key not given keeps its value");
}

#[tokio::test]
async fn e2e_0115_002_a_black_list_is_stored_not_applied() {
    let fx = Fx::new().await;
    let out = set(&fx, "web", json!({"black_list_url": ["*.ads.example"]})).await.unwrap();
    assert_eq!(out["value"]["black_list_url"], json!(["*.ads.example"]));
    assert_eq!(out["value"]["max_results"], 10);
    assert_eq!(out["restart_required"], true);
    assert!(fx.state.config.web.black_list_url.is_empty(), "the running server has no list");
}

#[tokio::test]
async fn e2e_0115_006_a_repeated_host_is_refused_as_at_the_start() {
    let fx = Fx::new().await;
    let m = set(&fx, "git", json!({"hosts": [["github.com", "github"], ["github.com", "gitlab"]]}))
        .await
        .unwrap_err();
    assert_eq!(m, "git.hosts contains duplicate host 'github.com'");
    nothing_stored(&fx, "git").await;
}

#[tokio::test]
async fn e2e_0115_007_a_white_list_entry_is_refused_as_at_the_start() {
    let fx = Fx::new().await;
    let m = set(&fx, "web", json!({"white_list_url": ["not a site"]})).await.unwrap_err();
    assert_eq!(
        m,
        "invalid entry 'not a site' in the web white list: expected a host name, a *.host \
         name, an IP address or an address range"
    );
    nothing_stored(&fx, "web").await;
}

#[tokio::test]
async fn e2e_0115_008_a_bad_host_or_provider_is_refused_with_the_start_s_words() {
    let fx = Fx::new().await;
    for host in ["https://github.com", "github.com/org", "github.com:443"] {
        let m = set(&fx, "git", json!({"hosts": {host: "github"}})).await.unwrap_err();
        let lead =
            format!("{BARE}{host}' is not a bare hostname (no scheme, path, or port allowed)");
        assert!(m.starts_with(&lead), "{m}");
    }
    let m = set(&fx, "git", json!({"hosts": {"*.github.com": "github"}})).await.unwrap_err();
    assert_eq!(
        m,
        "git.hosts key '*.github.com' contains a wildcard character: host matching is exact \
         only, one key per hostname"
    );
    let m = set(&fx, "git", json!({"hosts": {"github.com": "githib"}})).await.unwrap_err();
    assert_eq!(
        m,
        "git.hosts entry 'github.com' has unknown provider 'githib', expected one of github, \
         gitlab, generic, anonymous"
    );
    nothing_stored(&fx, "git").await;
}

#[tokio::test]
async fn e2e_0115_009_unknown_keys_and_wrong_types_name_the_section() {
    let fx = Fx::new().await;
    let m = set(&fx, "git", json!({"github_client_secret": "x"})).await.unwrap_err();
    assert_eq!(m, "unknown key 'github_client_secret' in setting 'git'");
    let m = set(&fx, "context7", json!({"nope": 1})).await.unwrap_err();
    assert_eq!(m, "unknown key 'nope' in setting 'context7'");
    let m = set(&fx, "git", json!({"max_pack_size_mb": "big"})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'git':"), "{m}");
    let m = set(&fx, "web", json!({"enabled": "yes"})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'web':"), "{m}");
    for s in ["git", "web", "context7"] {
        assert_eq!(get(&fx, s).await["secrets"], json!({}), "{s}");
    }
    assert!(lines(&fx.changes().await).is_empty());
}

#[tokio::test]
async fn e2e_0115_010_a_member_is_refused_on_tool_route_and_screen() {
    let fx = Fx::new().await;
    let forbidden = "ERR_FORBIDDEN: 'bob@example.com' is not a platform admin";
    for s in ["git", "web", "context7"] {
        let p: rmcp::handler::server::wrapper::Parameters<SectionArgs> =
            args(json!({"section": s}));
        let got = fx.keep(tool_json(fx.server(BOB).admin_get_setting(p).await));
        assert_eq!(got.unwrap_err(), forbidden, "{s}");
        let p: rmcp::handler::server::wrapper::Parameters<SetSettingArgs> =
            args(json!({"section": s, "value": {}}));
        let got = fx.keep(tool_json(fx.server(BOB).admin_set_setting(p).await));
        assert_eq!(got.unwrap_err(), forbidden, "{s}");
        let (st, _) = fx.rest(BOB, "GET", &format!("/api/admin/settings/{s}"), None).await;
        assert_eq!(st, 403, "{s}");
        let (st, _) = fx
            .rest(BOB, "PUT", &format!("/api/admin/settings/{s}"), Some(json!({"value": {}})))
            .await;
        assert_eq!(st, 403, "{s}");
    }
    let router = crate::settings_screen::router(fx.state.clone());
    let (st, _) = fx.page(&router, Via::Cookie(BOB), "GET", "/app/settings", None).await;
    assert_eq!(st, 403);
    nothing_stored(&fx, "git").await;
}

#[tokio::test]
async fn e2e_0115_012_an_empty_host_list_removes_every_host() {
    let fx = Fx::new().await;
    let two = json!({"hosts": {"a.example": "github", "b.example": "gitlab"}});
    set(&fx, "git", two.clone()).await.unwrap();
    let out = set(&fx, "git", json!({"hosts": {}})).await.unwrap();
    assert_eq!(out["value"]["hosts"], json!({}));
    let all = fx.changes().await;
    let last = lines(&all)[0];
    assert_eq!(last["fields"], json!(["hosts.a.example", "hosts.b.example"]));
    set(&fx, "git", two.clone()).await.unwrap();
    let before = lines(&fx.changes().await).len();
    set(&fx, "git", two).await.unwrap();
    let all = fx.changes().await;
    let now = lines(&all);
    assert_eq!(now.len(), before + 1, "an accepted change adds one line");
    assert_eq!(now[0]["fields"], json!([]), "that line holds no key");
}

#[tokio::test]
async fn e2e_0115_015_the_record_holds_key_names_never_values() {
    let fx = Fx::new().await;
    set(&fx, "git", json!({"hosts": {"corp.example.com": "github"}, "github_client_id": "abc"}))
        .await
        .unwrap();
    let all = fx.changes().await;
    let l = lines(&all)[0];
    assert_eq!(l["fields"], json!(["github_client_id", "hosts.corp.example.com"]));
    let text = l.to_string();
    assert!(!text.contains("abc") && !text.contains("\"github\""), "{text}");
}

#[tokio::test]
async fn e2e_0115_016_each_section_read_then_set_to_itself_changes_nothing() {
    let fx = Fx::new().await;
    for s in ["git", "web", "context7"] {
        let before = get(&fx, s).await;
        let again = set(&fx, s, before["value"].clone()).await.unwrap();
        assert_eq!(again["value"], before["value"], "{s}");
        assert_eq!(again["restart_required"], false, "{s}");
    }
    assert!(lines(&fx.changes().await).iter().all(|c| c["fields"] == json!([])));
}

#[tokio::test]
async fn e2e_0115_018_the_list_names_eleven_sections_and_an_unknown_one_lists_them() {
    let fx = Fx::new().await;
    let list = settings(&fx).list().await.unwrap();
    let names: Vec<&str> = list["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["section"].as_str().unwrap())
        .collect();
    assert_eq!(names.join(", "), ALL_THIRTEEN);
    for s in &list["settings"].as_array().unwrap()[8..] {
        assert_eq!(s["source"], "default");
    }
    let m = set(&fx, "nope", json!({})).await.unwrap_err();
    assert_eq!(m, format!("unknown setting section 'nope', expected one of {ALL_THIRTEEN}"));
}

async fn screen_post(fx: &Fx, section: &str, value: &str) -> (axum::http::StatusCode, String) {
    let router = crate::settings_screen::router(fx.state.clone());
    let (_, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/settings", None).await;
    let token = token_of(&html);
    fx.page(
        &router,
        Via::Cookie(ALICE),
        "POST",
        "/app/settings/set",
        Some(&[("section", section), ("value", value), ("csrf_token", &token)]),
    )
    .await
}

#[tokio::test]
async fn e2e_0115_019_the_screen_lists_eleven_sections_and_applies_a_hosts_patch() {
    let fx = Fx::new().await;
    let router = crate::settings_screen::router(fx.state.clone());
    let (st, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/settings", None).await;
    assert_eq!(st, 200, "{html}");
    for s in super::server_settings::SECTIONS {
        assert!(html.contains(&format!("data-section=\"{s}\"")), "{s}");
    }
    assert!(html.contains("data-section=\"context7\""));
    let (st, _) =
        screen_post(&fx, "git", "{\"hosts\": {\"github.example.com\": \"github\"}}").await;
    assert_eq!(st, 303);
    let (_, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/settings", None).await;
    assert!(html.contains("github.example.com"), "{html}");
    assert!(html.contains("Source: database (restart required)"), "{html}");
}

#[tokio::test]
async fn e2e_0115_022_two_administrators_last_write_wins_and_both_are_recorded() {
    let fx = Fx::new().await;
    set_as(&fx, ALICE, "git", json!({"hosts": {"a.example": "github"}})).await.unwrap();
    set_as(&fx, "other-admin@example.com", "git", json!({"hosts": {"b.example": "gitlab"}}))
        .await
        .unwrap();
    assert_eq!(get(&fx, "git").await["value"]["hosts"], json!({"b.example": "gitlab"}));
    let all = fx.changes().await;
    let l = lines(&all);
    assert_eq!(l.len(), 2);
    assert_eq!(l[0]["person"], "other-admin@example.com");
    assert_eq!(l[1]["person"], ALICE);
    // Truly at once: one of the two values stays, both are recorded.
    let fx = Fx::new().await;
    let s = settings(&fx);
    let (a, b) = tokio::join!(
        s.set(ALICE, "git", json!({"hosts": {"a.example": "github"}})),
        s.set("other-admin@example.com", "git", json!({"hosts": {"b.example": "gitlab"}})),
    );
    a.unwrap();
    b.unwrap();
    let h = get(&fx, "git").await["value"]["hosts"].clone();
    assert!(h == json!({"a.example": "github"}) || h == json!({"b.example": "gitlab"}), "{h}");
    assert_eq!(lines(&fx.changes().await).len(), 2);
}

#[tokio::test]
async fn e2e_0115_023_a_section_given_as_a_text_is_refused() {
    let fx = Fx::new().await;
    let m = set(&fx, "git", json!("github.com")).await.unwrap_err();
    assert_eq!(m, "the value of setting 'git' must be an object");
    nothing_stored(&fx, "git").await;
}

#[tokio::test]
async fn e2e_0115_024_hosts_given_as_a_text_is_refused() {
    let fx = Fx::new().await;
    let m = set(&fx, "git", json!({"hosts": "github.com"})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'git':"), "{m}");
    nothing_stored(&fx, "git").await;
}

#[tokio::test]
async fn e2e_0115_025_a_list_given_as_a_text_is_refused() {
    let fx = Fx::new().await;
    let m = set(&fx, "web", json!({"black_list_url": "*.ads.example"})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'web':"), "{m}");
    nothing_stored(&fx, "web").await;
}

#[tokio::test]
async fn e2e_0115_026_a_host_key_keeps_the_case_it_was_typed_in() {
    let fx = Fx::new().await;
    let out = set(&fx, "git", json!({"hosts": {"GitHub.com": "github", "github.com": "gitlab"}}))
        .await
        .unwrap();
    assert_eq!(out["value"]["hosts"], json!({"GitHub.com": "github", "github.com": "gitlab"}));
    assert_eq!(out["restart_required"], true);
    let out = set(&fx, "git", json!({"hosts": {"GitHub.com": "github"}})).await.unwrap();
    assert_eq!(out["value"]["hosts"], json!({"GitHub.com": "github"}));
}

#[tokio::test]
async fn e2e_0115_027_a_pair_of_the_wrong_length_is_refused() {
    let fx = Fx::new().await;
    let m = set(&fx, "git", json!({"hosts": [["github.com", "github", "x"]]})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'git':"), "{m}");
    nothing_stored(&fx, "git").await;
}

#[tokio::test]
async fn e2e_0115_028_hosts_that_are_neither_an_object_nor_pairs_are_refused() {
    let fx = Fx::new().await;
    for bad in [Value::Null, json!(3)] {
        let m = set(&fx, "git", json!({"hosts": bad})).await.unwrap_err();
        assert!(m.starts_with("invalid setting 'git':"), "{m}");
    }
    nothing_stored(&fx, "git").await;
}

#[tokio::test]
async fn e2e_0115_033_a_refused_change_adds_no_record_line() {
    let fx = Fx::new().await;
    set(&fx, "git", json!({"max_pack_size_mb": 64})).await.unwrap();
    let before = lines(&fx.changes().await).len();
    set(&fx, "git", json!({"hosts": {"*.x": "github"}})).await.unwrap_err();
    set(&fx, "web", json!({"white_list_url": ["not a site"]})).await.unwrap_err();
    assert_eq!(lines(&fx.changes().await).len(), before);
}

#[tokio::test]
async fn e2e_0115_034_a_secret_env_name_is_a_plain_name_never_resolved() {
    let fx = Fx::new().await;
    // SAFETY of the test: a name nobody else reads.
    let out = set(&fx, "git", json!({"gitlab_client_secret_env": "FSA_SECRET_GITLAB_0115"}))
        .await
        .unwrap();
    assert_eq!(out["value"]["gitlab_client_secret_env"], "FSA_SECRET_GITLAB_0115");
    assert_eq!(out["secrets"], json!({}));
}

#[tokio::test]
async fn e2e_0115_036_a_refused_patch_on_the_screen_shows_the_message() {
    let fx = Fx::new().await;
    let (st, page) = screen_post(
        &fx,
        "git",
        "{\"hosts\": [[\"github.com\", \"github\"], [\"github.com\", \"gitlab\"]]}",
    )
    .await;
    assert_eq!(st, 400, "{page}");
    assert!(
        page.contains("git.hosts contains duplicate host &#39;github.com&#39;")
            || page.contains("git.hosts contains duplicate host 'github.com'"),
        "{page}"
    );
    nothing_stored(&fx, "git").await;
}

#[tokio::test]
async fn e2e_0115_037_a_section_in_capitals_is_unknown() {
    let fx = Fx::new().await;
    let m = set(&fx, "GIT", json!({})).await.unwrap_err();
    assert_eq!(m, format!("unknown setting section 'GIT', expected one of {ALL_THIRTEEN}"));
}

#[tokio::test]
async fn e2e_0115_039_the_hosts_read_back_in_byte_order() {
    let fx = Fx::new().await;
    let out = set(
        &fx,
        "git",
        json!({"hosts": {"b.example": "generic", "a.example": "gitlab", "c.example": "github"}}),
    )
    .await
    .unwrap();
    let keys: Vec<&String> = out["value"]["hosts"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["a.example", "b.example", "c.example"]);
    let out = set(&fx, "git", json!({"hosts": {"b.example": "generic", "B.example": "generic"}}))
        .await
        .unwrap();
    let keys: Vec<&String> = out["value"]["hosts"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["B.example", "b.example"]);
    let got = get(&fx, "git").await;
    let keys: Vec<&String> = got["value"]["hosts"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["B.example", "b.example"]);
}

#[tokio::test]
async fn e2e_0115_040_pairs_replace_the_whole_list() {
    let fx = Fx::new().await;
    set(&fx, "git", json!({"hosts": {"a.example": "github", "b.example": "gitlab"}}))
        .await
        .unwrap();
    let out = set(&fx, "git", json!({"hosts": [["c.example", "github"]]})).await.unwrap();
    assert_eq!(out["value"]["hosts"], json!({"c.example": "github"}));
}

#[tokio::test]
async fn e2e_0115_041_a_secret_env_name_given_as_a_number_is_refused() {
    let fx = Fx::new().await;
    let m = set(&fx, "git", json!({"github_client_secret_env": 5})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'git':"), "{m}");
    nothing_stored(&fx, "git").await;
}

#[tokio::test]
async fn e2e_0115_047_the_record_names_the_list_key_and_no_entry() {
    let fx = Fx::new().await;
    set(&fx, "web", json!({"black_list_url": ["a.example", "b.example"]})).await.unwrap();
    let all = fx.changes().await;
    let l = lines(&all)[0];
    assert_eq!(l["fields"], json!(["black_list_url"]));
    let t = l.to_string();
    assert!(!t.contains("a.example") && !t.contains("b.example"), "{t}");
}

#[tokio::test]
async fn e2e_0115_052_a_bad_black_list_entry_is_refused() {
    let fx = Fx::new().await;
    let m =
        set(&fx, "web", json!({"black_list_url": ["ok.example", "not a site"]})).await.unwrap_err();
    assert_eq!(
        m,
        "invalid entry 'not a site' in the web black list: expected a host name, a *.host \
         name, an IP address or an address range"
    );
    nothing_stored(&fx, "web").await;
}

#[tokio::test]
async fn e2e_0115_053_a_refused_change_leaves_restart_required_false() {
    let fx = Fx::new().await;
    set(&fx, "git", json!({"hosts": {"*.x": "github"}})).await.unwrap_err();
    let g = get(&fx, "git").await;
    assert_eq!(g["restart_required"], false);
    assert_eq!(g["source"], "default");
}

#[tokio::test]
async fn e2e_0115_054_context7_reads_its_defaults_on_a_server_never_set() {
    let fx = Fx::new().await;
    let g = get(&fx, "context7").await;
    assert_eq!(g["value"]["api_url"], "https://context7.com/api");
    assert_eq!(g["value"]["request_timeout_secs"], 30);
    assert_eq!(g["value"]["enabled"], false);
    assert_eq!(g["source"], "default");
}
