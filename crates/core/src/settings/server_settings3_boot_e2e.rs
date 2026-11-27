//! SPEC-0115 functional tests, US-0346: the start of the server reads the
//! stored `git`, `web` and `context7` sections, publishes the git host map and
//! keeps the flags, the seed and the tokens as they were.

use super::ServerSettings;
use super::server_settings_e2e::{ALICE, KEY_A, boot, boot_config, set_on, yaml_for};
use super::server_settings_store::ServerSettingRow;
use crate::config::{GitConfig, ServerConfig};
use crate::git::remote::{Provider, resolve_host, tests::lock_for_test, validate_hosts};
use crate::mcp::server::MarkerArgs;
use crate::search::derived_e2e::{args, tool_json};
use crate::state::AppState;
use crate::tools::tool_surface_e2e::{call_tool, list_names};
use crate::tools::web_access::{NameAnswer, TestNet};
use crate::tools::web_access_e2e::{fetch, refusal_text, refused, served_text};
use crate::tools::web_testkit::{Script, stand_in};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

/// The YAML start of the settings tests, called while the host map lock is held.
fn load_yaml(path: &Path) -> ServerConfig {
    ServerConfig::load(path).unwrap()
}

fn token_for(root: &Path) -> String {
    crate::keys::mint_token_from_file(
        root.join("keys").join(crate::keys::PRIVATE_KEY_FILE),
        ALICE,
        crate::keys::DEFAULT_ISSUER,
        crate::keys::DEFAULT_CLAIM,
        3600,
    )
    .unwrap()
}

/// What a fresh process holds: no declared host.
fn forget_hosts() {
    validate_hosts(&GitConfig::default()).unwrap();
}

async fn changes_of(state: &Arc<AppState>) -> Vec<Value> {
    let mut out = Vec::new();
    let mut marker = String::new();
    loop {
        let p: rmcp::handler::server::wrapper::Parameters<MarkerArgs> =
            args(json!({"marker": marker}));
        let server = crate::mcp::server::McpServer::new(state.clone(), ALICE.into());
        let page = tool_json(server.admin_list_setting_changes(p).await).unwrap();
        out.extend(page["changes"].as_array().unwrap().iter().cloned());
        match page["marker"].as_str() {
            Some(m) => marker = m.to_string(),
            None => return out,
        }
    }
}

async fn read(state: &Arc<AppState>, section: &str) -> Value {
    ServerSettings::of(state.admin.as_ref(), state.config.clone())
        .unwrap()
        .get(section)
        .await
        .unwrap()
}

/// Writes a row as an operator editing the database would, with no check.
async fn put_raw(state: &Arc<AppState>, section: &str, value: &str) {
    let store = state.admin.settings().unwrap();
    // Read before the transaction opens: the store has one connection.
    let exists = store.all_server_settings().await.unwrap().iter().any(|r| r.section == section);
    let mut tx = store.begin().await.unwrap();
    let row = ServerSettingRow {
        section: section.into(),
        value: value.into(),
        secret_enc: None,
        updated_at: "2026-11-26T00:00:00Z".into(),
        updated_by: "operator".into(),
    };
    super::store::SettingsStore::put_server_setting(&mut tx, &row, exists).await.unwrap();
    tx.commit().await.unwrap();
}

async fn tool_set_token(router: &axum::Router, root: &Path, person: &str, host: &str, token: &str) {
    let jwt = crate::keys::mint_token_from_file(
        root.join("keys").join(crate::keys::PRIVATE_KEY_FILE),
        person,
        crate::keys::DEFAULT_ISSUER,
        crate::keys::DEFAULT_CLAIM,
        3600,
    )
    .unwrap();
    let out = call_tool(router, &jwt, "git.token_set", json!({"host": host, "token": token})).await;
    assert_ne!(out["result"]["isError"], json!(true), "{out}");
}

async fn held(state: &Arc<AppState>, person: &str) -> Vec<(String, String)> {
    let store = crate::tools::git_auth::token_store(&state.config, state.stores.relational())
        .await
        .unwrap();
    let mut v: Vec<(String, String)> = store
        .list_for_person(person)
        .into_iter()
        .map(|(h, s)| (h, s.access_token.clone()))
        .collect();
    v.sort();
    v
}

const GLPAT: &str = "glpat-AAAABBBBCCCCDDDDEEEE";

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_003_a_start_with_only_the_database_and_the_key_reads_the_three_sections() {
    let _hosts = lock_for_test();
    forget_hosts();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(
        &first,
        "git",
        json!({"enabled": true, "hosts": {"github.example.com": "github"}, "max_pack_size_mb": 64}),
    )
    .await;
    set_on(&first, "web", json!({"enabled": true, "black_list_url": ["*.ads.example"]})).await;
    set_on(&first, "context7", json!({"enabled": true})).await;
    drop(first);
    forget_hosts();
    let (router, second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert!(second.config.git.enabled);
    assert_eq!(second.config.git.max_pack_size_mb, 64);
    assert_eq!(resolve_host("github.example.com").unwrap(), Provider::Github);
    let token = token_for(d.path());
    let names = list_names(&router, &token).await;
    assert!(names.contains(&"context7.resolve_library_id".to_string()), "{names:?}");
    assert!(names.contains(&"git.remote_clone".to_string()));
    let v = fetch(&router, &token, "http://x.ads.example/").await;
    assert_eq!(refusal_text(&v), refused("x.ads.example"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0115_002b_a_stored_list_applies_after_the_restart_only() {
    let d = tempfile::tempdir().unwrap();
    let (port, _rec) = stand_in(vec![(
        "/",
        Script::Fixed {
            body: b"ads".to_vec(),
            content_type: Some("text/plain".into()),
            chunked: false,
            encoding: None,
        },
    )])
    .await;
    let seam = || Some(Arc::new(TestNet::default().with_site("x.ads.example", port)));
    let mut c = boot_config(d.path());
    c.web.test_net = seam();
    let (_r, first) = boot(c, Some(KEY_A)).await.unwrap();
    set_on(&first, "web", json!({"enabled": true})).await;
    drop(first);
    let mut c = boot_config(d.path());
    c.web.test_net = seam();
    let (router, second) = boot(c, Some(KEY_A)).await.unwrap();
    set_on(&second, "web", json!({"black_list_url": ["*.ads.example"]})).await;
    let token = token_for(d.path());
    let v = fetch(&router, &token, "http://x.ads.example/").await;
    assert_eq!(served_text(&v), "ads", "before the restart the list is not in force");
    drop(second);
    let mut c = boot_config(d.path());
    c.web.test_net = seam();
    let (router, _third) = boot(c, Some(KEY_A)).await.unwrap();
    let token = token_for(d.path());
    let v = fetch(&router, &token, "http://x.ads.example/").await;
    assert!(
        refusal_text(&v).ends_with("host 'x.ads.example' is not allowed by the web access lists"),
        "{v}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_004_a_new_host_list_leaves_the_stored_tokens_alone() {
    let _hosts = lock_for_test();
    forget_hosts();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(
        &first,
        "git",
        json!({"enabled": true, "hosts": {"github.com": "github", "gitlab.example.com": "gitlab"}}),
    )
    .await;
    drop(first);
    let (router, second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let who = "e2e0115-004@example.com";
    tool_set_token(&router, d.path(), who, "gitlab.example.com", GLPAT).await;
    let before = held(&second, who).await;
    assert_eq!(before, vec![("gitlab.example.com".to_string(), GLPAT.to_string())]);
    set_on(&second, "git", json!({"hosts": {"github.com": "github"}})).await;
    assert_eq!(read(&second, "git").await["value"]["hosts"], json!({"github.com": "github"}));
    let all = changes_of(&second).await;
    let line =
        all.iter().find(|c| c["target_id"] == "git" && c["action"] == "set_setting").unwrap();
    assert_eq!(line["fields"], json!(["hosts.gitlab.example.com"]));
    assert_eq!(held(&second, who).await, before, "the token of the removed host is untouched");
    drop(second);
    forget_hosts();
    let (_r, third) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert!(resolve_host("gitlab.example.com").is_err(), "the host is gone after the restart");
    assert_eq!(held(&third, who).await, before, "and its token is still held");
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_005_the_yaml_seeds_once_and_a_later_edit_does_not_override() {
    let _hosts = lock_for_test();
    let d = tempfile::tempdir().unwrap();
    let path = yaml_for(d.path(), "git:\n  hosts:\n    a.example.com: github\n");
    let (_r, first) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(read(&first, "git").await["value"]["hosts"], json!({"a.example.com": "github"}));
    drop(first);
    let path = yaml_for(d.path(), "git:\n  hosts:\n    b.example.com: github\n");
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        second.config.git.hosts.0,
        vec![("a.example.com".to_string(), "github".to_string())]
    );
    assert_eq!(resolve_host("a.example.com").unwrap(), Provider::Github);
    assert!(
        resolve_host("b.example.com").is_err(),
        "the edited file does not override the database"
    );
    let all = changes_of(&second).await;
    let seeds: Vec<&Value> = all.iter().filter(|c| c["action"] == "seed_setting").collect();
    assert_eq!(seeds.len(), 11, "the first start imports every section once");
    for s in ["git", "web", "context7"] {
        let n =
            seeds.iter().filter(|c| c["target_id"] == s && c["person"] == "system:yaml").count();
        assert_eq!(n, 1, "{s}");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_011_an_empty_database_starts_with_the_three_off() {
    let _hosts = lock_for_test();
    forget_hosts();
    let d = tempfile::tempdir().unwrap();
    let (router, state) = boot(boot_config(d.path()), None).await.unwrap();
    assert!(
        !state.config.git.enabled && !state.config.web.enabled && !state.config.context7.enabled
    );
    assert!(state.config.git.hosts.0.is_empty());
    assert!(
        state.config.web.black_list_url.is_empty() && state.config.web.white_list_url.is_empty()
    );
    let names = list_names(&router, &token_for(d.path())).await;
    for prefix in ["git.", "web.", "context7."] {
        assert!(!names.iter().any(|n| n.starts_with(prefix)), "{prefix}: {names:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_013_a_flag_switches_on_over_a_stored_false_and_is_never_stored() {
    // A stored git section publishes the process wide host map.
    let _hosts = lock_for_test();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "git", json!({"max_pack_size_mb": 512})).await;
    drop(first);
    let mut c = boot_config(d.path());
    c.force("git");
    let (_r, second) = boot(c, Some(KEY_A)).await.unwrap();
    assert!(second.config.git.enabled);
    let g = read(&second, "git").await;
    assert_eq!(g["value"]["enabled"], false);
    assert_eq!(g["restart_required"], false);
    set_on(&second, "git", json!({"max_pack_size_mb": 64})).await;
    assert_eq!(read(&second, "git").await["value"]["enabled"], false);
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_014_the_pack_limit_applies_after_the_restart() {
    // A stored git section publishes the process wide host map.
    let _hosts = lock_for_test();
    let d = tempfile::tempdir().unwrap();
    let mut c = boot_config(d.path());
    c.git.enabled = true;
    let (router, first) = boot(c, Some(KEY_A)).await.unwrap();
    first.admin.create_project("proj", ALICE).await.unwrap();
    set_on(&first, "git", json!({"enabled": true, "max_pack_size_mb": 64})).await;
    assert_eq!(first.config.git.max_pack_size_mb, 512, "a change is not live");
    assert_eq!(read(&first, "git").await["restart_required"], true);
    let token = token_for(d.path());
    let push = |router: axum::Router, mib: usize| {
        let token = token.clone();
        async move {
            use tower::ServiceExt;
            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/git/proj/git-receive-pack")
                .header("X-Forwarded-Authorization", format!("Bearer {token}"))
                .body(axum::body::Body::from(vec![0u8; mib * 1024 * 1024]))
                .unwrap();
            router.oneshot(req).await.unwrap().status()
        }
    };
    assert_ne!(push(router.clone(), 70).await, 413, "512 MB still runs");
    drop(first);
    let (router, second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(second.config.git.max_pack_size_mb, 64);
    assert_eq!(push(router.clone(), 70).await, 413);
    // A pack the limit allows is not refused for its size (the bytes are no pack).
    assert_ne!(push(router, 60).await, 413);
}

/// A start over a database holding one section the operator edited by hand.
async fn start_with_raw(section: &str, edit: impl Fn(&mut Value)) -> String {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut v = super::server_settings::section_json(&first.config, section);
    edit(&mut v);
    put_raw(&first, section, &v.to_string()).await;
    drop(first);
    boot(boot_config(d.path()), Some(KEY_A)).await.err().expect("the start is refused")
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0115_017_a_stored_section_the_checks_refuse_stops_the_start() {
    let e = start_with_raw("git", |v| v["hosts"] = json!({"*.x": "github"})).await;
    assert_eq!(
        e,
        "the stored settings are refused: git.hosts key '*.x' contains a wildcard character: \
         host matching is exact only, one key per hostname"
    );
    let e = start_with_raw("web", |v| v["white_list_url"] = json!(["not a site"])).await;
    assert_eq!(
        e,
        "the stored settings are refused: invalid entry 'not a site' in the web white list: \
         expected a host name, a *.host name, an IP address or an address range"
    );
    let e = start_with_raw("git", |v| *v = json!("github.com")).await;
    assert!(e.starts_with("the stored setting 'git' is invalid:"), "{e}");
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_020_a_start_from_the_yaml_alone_behaves_as_today() {
    let _hosts = lock_for_test();
    let d = tempfile::tempdir().unwrap();
    let path = yaml_for(
        d.path(),
        "git:\n  enabled: true\n  max_pack_size_mb: 32\n  hosts:\n    a.example.com: github\n    b.example.com: gitlab\nweb:\n  max_results: 7\n  black_list_url: [\"*.ads.example\"]\ncontext7:\n  request_timeout_secs: 9\n",
    );
    let yaml = load_yaml(&path);
    let (_r, state) = boot(yaml.clone(), Some(KEY_A)).await.unwrap();
    for s in ["git", "web", "context7"] {
        assert_eq!(
            super::server_settings::section_json(&state.config, s),
            super::server_settings::section_json(&yaml, s),
            "{s}"
        );
    }
    assert_eq!(resolve_host("b.example.com").unwrap(), Provider::Gitlab);
    drop(state);
    let (_r, again) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    for s in ["git", "web", "context7"] {
        assert_eq!(
            super::server_settings::section_json(&again.config, s),
            super::server_settings::section_json(&yaml, s),
            "{s} after the seed"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_029_a_refused_host_change_leaves_the_tokens_alone() {
    let _hosts = lock_for_test();
    forget_hosts();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "git", json!({"enabled": true, "hosts": {"gitlab.example.com": "gitlab"}}))
        .await;
    drop(first);
    let (router, second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let who = "e2e0115-029@example.com";
    tool_set_token(&router, d.path(), who, "gitlab.example.com", GLPAT).await;
    let before = held(&second, who).await;
    let s = ServerSettings::of(second.admin.as_ref(), second.config.clone()).unwrap();
    s.set(ALICE, "git", json!({"hosts": [["x.example", "github"], ["x.example", "gitlab"]]}))
        .await
        .unwrap_err();
    assert_eq!(held(&second, who).await, before);
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_030_a_stored_row_wins_and_the_missing_ones_are_seeded() {
    let _hosts = lock_for_test();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "git", json!({"hosts": {"db.example.com": "github"}})).await;
    drop(first);
    let path = yaml_for(
        d.path(),
        "git:\n  hosts:\n    yaml.example.com: github\nweb:\n  max_results: 7\ncontext7:\n  request_timeout_secs: 9\n",
    );
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        second.config.git.hosts.0,
        vec![("db.example.com".to_string(), "github".to_string())]
    );
    assert_eq!(second.config.web.max_results, 7);
    assert_eq!(second.config.context7.request_timeout_secs, 9);
    let all = changes_of(&second).await;
    let seeded: Vec<String> = all
        .iter()
        .filter(|c| c["action"] == "seed_setting")
        .map(|c| c["target_id"].as_str().unwrap().to_string())
        .collect();
    assert!(seeded.contains(&"web".into()) && seeded.contains(&"context7".into()));
    assert!(!seeded.contains(&"git".into()), "git had a row, so it is not imported");
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_031_three_flags_over_a_yaml_with_the_switches_off_store_off() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_for(
        d.path(),
        "git:\n  enabled: false\nweb:\n  enabled: false\ncontext7:\n  enabled: false\n",
    );
    let _hosts = lock_for_test();
    let mut c = load_yaml(&path);
    for n in ["git", "web", "context7"] {
        c.force(n);
    }
    let (_r, state) = boot(c, Some(KEY_A)).await.unwrap();
    assert!(state.config.git.enabled && state.config.web.enabled && state.config.context7.enabled);
    for s in ["git", "web", "context7"] {
        assert_eq!(read(&state, s).await["value"]["enabled"], false, "{s}");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_032_a_database_with_only_a_git_row_starts_the_others_off() {
    let _hosts = lock_for_test();
    forget_hosts();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "git", json!({"enabled": true, "hosts": {"only.example.com": "generic"}})).await;
    drop(first);
    forget_hosts();
    let (_r, second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(resolve_host("only.example.com").unwrap(), Provider::Generic);
    assert!(!second.config.web.enabled && !second.config.context7.enabled);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0115_038_the_lists_decide_alike_from_the_yaml_and_from_the_database() {
    let sites = ["http://ok.example/", "http://x.example/", "http://other.test/"];
    let (yaml_app, yaml_token, _s, _d) =
        crate::tools::web_access_e2e::served_with_lists(&["*.example"], &["ok.example"], None)
            .await;
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(
        &first,
        "web",
        json!({"enabled": true, "black_list_url": ["*.example"], "white_list_url": ["ok.example"]}),
    )
    .await;
    drop(first);
    let (db_app, _second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let db_token = token_for(d.path());
    for site in sites {
        let host = site.trim_start_matches("http://").trim_end_matches('/');
        let a = refusal_text(&fetch(&yaml_app, &yaml_token, site).await);
        let b = refusal_text(&fetch(&db_app, &db_token, site).await);
        assert_eq!(
            a,
            format!("ERR_FORBIDDEN: host '{host}' is not allowed by the web access lists")
        );
        assert_eq!(a, b, "{site}");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_042_a_stored_row_with_missing_keys_takes_defaults_not_the_yaml() {
    let _hosts = lock_for_test();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    put_raw(&first, "web", "{\"enabled\": true}").await;
    drop(first);
    let path = yaml_for(d.path(), "web:\n  max_results: 20\n");
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert!(second.config.web.enabled);
    assert_eq!(second.config.web.max_results, 10);
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_043_no_key_no_row_no_yaml_starts_and_declares_no_host() {
    let _hosts = lock_for_test();
    forget_hosts();
    let d = tempfile::tempdir().unwrap();
    let mut c = boot_config(d.path());
    c.force("git");
    let (router, state) = boot(c, None).await.unwrap();
    state.admin.create_project("proj", ALICE).await.unwrap();
    let token = token_for(d.path());
    let out = call_tool(
        &router,
        &token,
        "git.remote_clone",
        json!({"mount_id": "proj", "url": "https://github.com/o/r.git"}),
    )
    .await;
    let text = out["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
    assert!(
        text.ends_with(
            "ERR_INVALID_ARGUMENT: host 'github.com' is not declared in git.hosts; declare it \
             under git.hosts before cloning from this host"
        ),
        "{out}"
    );
    let names = list_names(&router, &token).await;
    assert!(!names.iter().any(|n| n.starts_with("web.")));
    drop(state);
    let (router, _s) = boot(boot_config(d.path()), None).await.unwrap();
    // The keys were written again by the new configuration: a token of the new ones.
    let token = token_for(d.path());
    let names = list_names(&router, &token).await;
    assert!(!names.iter().any(|n| n.starts_with("git.")), "without the flag no git tool");
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_044_an_emptied_host_list_leaves_the_tokens_alone() {
    let _hosts = lock_for_test();
    forget_hosts();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "git", json!({"enabled": true, "hosts": {"gitlab.example.com": "gitlab"}}))
        .await;
    drop(first);
    let (router, second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let who = "e2e0115-044@example.com";
    tool_set_token(&router, d.path(), who, "gitlab.example.com", GLPAT).await;
    let before = held(&second, who).await;
    set_on(&second, "git", json!({"hosts": {}})).await;
    assert_eq!(held(&second, who).await, before);
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_045_a_yaml_with_a_bad_list_stops_the_start_and_imports_nothing() {
    let _hosts = lock_for_test();
    let d = tempfile::tempdir().unwrap();
    let path = yaml_for(d.path(), "web:\n  white_list_url: [\"not a site\"]\n");
    let e = ServerConfig::load(&path).unwrap_err().message;
    assert!(
        e.contains(
            "invalid entry 'not a site' in the web white list: expected a host name, a \
             *.host name, an IP address or an address range"
        ),
        "{e}"
    );
    assert!(!d.path().join("admin.db").exists(), "nothing was opened, nothing imported");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0115_046_a_forced_web_switch_is_not_persisted_by_a_later_change() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "web", json!({"max_results": 10})).await;
    drop(first);
    let mut c = boot_config(d.path());
    c.force("web");
    let (_r, second) = boot(c, Some(KEY_A)).await.unwrap();
    set_on(&second, "web", json!({"max_results": 20})).await;
    assert_eq!(read(&second, "web").await["value"]["enabled"], false);
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_049_a_host_key_with_capitals_is_never_matched_by_a_remote_operation() {
    let _hosts = lock_for_test();
    forget_hosts();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "git", json!({"enabled": true, "hosts": {"Corp.Example": "github"}})).await;
    drop(first);
    forget_hosts();
    let (router, second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(read(&second, "git").await["value"]["hosts"], json!({"Corp.Example": "github"}));
    second.admin.create_project("proj", ALICE).await.unwrap();
    let out = call_tool(
        &router,
        &token_for(d.path()),
        "git.remote_clone",
        json!({"mount_id": "proj", "url": "https://Corp.Example/o/r.git"}),
    )
    .await;
    let text = out["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
    assert!(
        text.ends_with(
            "ERR_INVALID_ARGUMENT: host 'corp.example' is not declared in git.hosts; declare it \
             under git.hosts before cloning from this host"
        ),
        "{out}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_050_stored_hosts_survive_a_restart_as_typed() {
    let _hosts = lock_for_test();
    forget_hosts();
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "git", json!({"hosts": {"GitHub.com": "github"}})).await;
    drop(first);
    let (_r, second) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(read(&second, "git").await["value"]["hosts"], json!({"GitHub.com": "github"}));
    assert_eq!(second.config.git.hosts.0, vec![("GitHub.com".to_string(), "github".to_string())]);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0115_051_a_white_list_set_by_the_database_admits_its_site_only() {
    let d = tempfile::tempdir().unwrap();
    let (port, _rec) = stand_in(vec![(
        "/",
        Script::Fixed {
            body: b"fine".to_vec(),
            content_type: Some("text/plain".into()),
            chunked: false,
            encoding: None,
        },
    )])
    .await;
    let seam = || {
        Some(Arc::new(TestNet::default().with_site("ok.example", port).with_name(
            "ok.example",
            vec![NameAnswer::Addrs(vec!["93.184.216.35".parse().unwrap()])],
        )))
    };
    let mut c = boot_config(d.path());
    c.web.test_net = seam();
    let (_r, first) = boot(c, Some(KEY_A)).await.unwrap();
    set_on(
        &first,
        "web",
        json!({"enabled": true, "black_list_url": ["*.ads.example"], "white_list_url": ["ok.example"]}),
    )
    .await;
    drop(first);
    let mut c = boot_config(d.path());
    c.web.test_net = seam();
    let (router, _second) = boot(c, Some(KEY_A)).await.unwrap();
    let token = token_for(d.path());
    assert_eq!(served_text(&fetch(&router, &token, "http://ok.example/").await), "fine");
    for site in ["x.ads.example", "other.test"] {
        let v = fetch(&router, &token, &format!("http://{site}/")).await;
        assert!(
            refusal_text(&v)
                .ends_with(&format!("host '{site}' is not allowed by the web access lists")),
            "{site}: {v}"
        );
    }
}

/// Review finding of SPEC-0115: `hosts:` with no value was an empty map for the
/// YAML reader before the host map also read pairs.
#[test]
fn e2e_0115_055_a_yaml_hosts_key_with_no_value_loads_as_no_host() {
    let _hosts = lock_for_test();
    for yaml in ["git:\n  hosts:\n", "git:\n  hosts: ~\n", "git:\n  hosts: {}\n"] {
        let c = ServerConfig::from_yaml(yaml).unwrap_or_else(|e| panic!("{yaml:?}: {}", e.message));
        assert!(c.git.hosts.0.is_empty(), "{yaml:?}");
    }
}

/// A start whose stored hosts equal the ones the configuration already holds
/// republishes nothing: the map was published when the file was read, and a
/// second publication would race the tests that hold the map's lock.
#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_056_a_stored_host_list_equal_to_the_yaml_publishes_nothing_more() {
    let _hosts = lock_for_test();
    let d = tempfile::tempdir().unwrap();
    let path = yaml_for(d.path(), "git:\n  hosts:\n    a.example.com: github\n");
    let (_r, first) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    drop(first);
    // Something else published a different map meanwhile.
    let mut other = GitConfig::default();
    other.hosts.0 = vec![("sentinel.example".into(), "generic".into())];
    validate_hosts(&other).unwrap();
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(second.config.git.hosts.0.len(), 1);
    // The YAML load of the second start published the file's map; publish the
    // sentinel again, then boot from the database over the same hosts.
    validate_hosts(&other).unwrap();
    let mut c = ServerConfig::default();
    c.git.hosts.0 = vec![("a.example.com".into(), "github".into())];
    c.auth = second.config.auth.clone();
    c.infra = second.config.infra.clone();
    let (_r, third) = boot(c, Some(KEY_A)).await.unwrap();
    assert_eq!(third.config.git.hosts.0, vec![("a.example.com".to_string(), "github".to_string())]);
    assert_eq!(resolve_host("sentinel.example").unwrap(), Provider::Generic, "nothing republished");
}
