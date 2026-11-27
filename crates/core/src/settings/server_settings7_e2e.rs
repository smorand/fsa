//! SPEC-0116 functional tests, US-0351: the start of the server reads the stored
//! `server` and `auth` sections, listens where they say, refuses to serve with no key,
//! and judges a YAML file and a stored section by the rules of FR-NEW-020.

use super::key_fixtures::RSA_1024;
use super::server_settings_e2e::{ALICE, KEY_A, boot, boot_config, load_yaml, set_on};
use super::server_settings4_e2e::BOB;
use super::server_settings5_e2e::kp;
use super::server_settings6_e2e::{capture_logs, logged};
use crate::config::ServerConfig;
use crate::state::AppState;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use tower::ServiceExt;

const NOKEY_START: &str = "no auth key is configured: store auth.jwt.public_keys or set auth.jwt.public_key_path (a first start from a YAML file imports them)";
const STORED: &str = "the stored settings are refused: ";

fn form(i: usize) -> String {
    format!(
        "auth.jwt.public_keys[{i}] is not a public key in PEM form (an RSA public key is expected)"
    )
}

fn cannot_serve(p: &str) -> String {
    format!("server.mcp_path '{p}' cannot be served: choose another path")
}

/// A first start over `root`, the stored `section` replaced by `edit(current)` as an
/// operator editing the database would, then the start that reads it: its error text.
async fn start_with_raw(section: &str, edit: impl FnOnce(&mut Value)) -> String {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut v = super::server_settings::section_json(&first.config, section);
    edit(&mut v);
    put_raw_state(&first, section, &v.to_string()).await;
    drop((_r, first));
    boot(boot_config(d.path()), Some(KEY_A)).await.err().expect("the start is refused")
}

/// A stored value written as text, no check, no parse.
async fn put_raw_state(state: &Arc<AppState>, section: &str, text: &str) {
    let store = state.admin.settings().unwrap();
    let exists = store.all_server_settings().await.unwrap().iter().any(|r| r.section == section);
    let mut tx = store.begin().await.unwrap();
    let row = super::server_settings_store::ServerSettingRow {
        section: section.into(),
        value: text.into(),
        secret_enc: None,
        updated_at: "2026-11-27T00:00:00Z".into(),
        updated_by: "operator".into(),
    };
    super::store::SettingsStore::put_server_setting(&mut tx, &row, exists).await.unwrap();
    tx.commit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_032_a_stored_key_that_is_not_a_key_stops_the_start() {
    let e = start_with_raw("auth", |v| v["jwt"]["public_keys"] = json!(["not a key"])).await;
    assert_eq!(e, format!("{STORED}{}", form(0)));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_033_a_stored_server_value_the_rules_refuse_stops_the_start() {
    let e = start_with_raw("server", |v| v["mcp_path"] = json!("mcp")).await;
    assert_eq!(e, format!("{STORED}server.mcp_path must start with '/' (got 'mcp')"));
    let e = start_with_raw("server", |v| v["host"] = json!("bad host")).await;
    assert_eq!(
        e,
        format!("{STORED}invalid server.host 'bad host': expected an IPv4 address or a host name")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_034_a_stored_value_of_the_wrong_shape_stops_the_start() {
    let e = start_with_raw("auth", |v| *v = json!("x")).await;
    assert_eq!(e, "the stored setting 'auth' is invalid: the value has the wrong type");
    let e = start_with_raw("server", |v| *v = json!([])).await;
    assert_eq!(e, "the stored setting 'server' is invalid: the value has the wrong type");
    let e = start_with_raw("server", |v| v["port"] = json!("80")).await;
    assert_eq!(e, "the stored setting 'server' is invalid: the value of 'port' has the wrong type");
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    put_raw_state(&first, "auth", "x").await;
    drop((_r, first));
    let e = boot(boot_config(d.path()), Some(KEY_A)).await.err().unwrap();
    assert_eq!(e, "the stored setting 'auth' is unreadable");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_103_a_stored_port_outside_the_range_stops_the_start_and_zero_does_not() {
    for p in [json!(70000), json!(-1), json!(3.5)] {
        let e = start_with_raw("server", |v| v["port"] = p).await;
        assert_eq!(
            e,
            "the stored setting 'server' is invalid: the value of 'port' has the wrong type"
        );
    }
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut v = super::server_settings::section_json(&first.config, "server");
    v["port"] = json!(0);
    put_raw_state(&first, "server", &v.to_string()).await;
    drop((_r, first));
    let _booted = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_124_the_first_stored_section_of_the_list_is_named() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut v = super::server_settings::section_json(&first.config, "server");
    v["mcp_path"] = json!("mcp");
    put_raw_state(&first, "server", &v.to_string()).await;
    put_raw_state(&first, "auth", "x").await;
    drop((_r, first));
    let e = boot(boot_config(d.path()), Some(KEY_A)).await.err().unwrap();
    assert_eq!(e, format!("{STORED}server.mcp_path must start with '/' (got 'mcp')"));
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    put_raw_state(&first, "auth", "x").await;
    drop((_r, first));
    let e = boot(boot_config(d.path()), Some(KEY_A)).await.err().unwrap();
    assert_eq!(e, "the stored setting 'auth' is unreadable");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_126_two_unreadable_sections_name_the_first() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    put_raw_state(&first, "server", "x").await;
    put_raw_state(&first, "auth", "y").await;
    drop((_r, first));
    let e = boot(boot_config(d.path()), Some(KEY_A)).await.err().unwrap();
    assert_eq!(e, "the stored setting 'server' is unreadable");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_127_a_wrong_typed_server_is_named_before_a_bad_key() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut v = super::server_settings::section_json(&first.config, "server");
    v["port"] = json!("80");
    put_raw_state(&first, "server", &v.to_string()).await;
    let mut a = super::server_settings::section_json(&first.config, "auth");
    a["jwt"]["public_keys"] = json!(["not a key"]);
    put_raw_state(&first, "auth", &a.to_string()).await;
    drop((_r, first));
    let e = boot(boot_config(d.path()), Some(KEY_A)).await.err().unwrap();
    assert!(e.starts_with("the stored setting 'server' is invalid:"), "{e}");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_173_a_stored_root_path_stops_the_start_without_a_panic() {
    let e = start_with_raw("server", |v| v["mcp_path"] = json!("/")).await;
    assert_eq!(e, format!("{STORED}{}", cannot_serve("/")));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_178_a_stored_path_of_a_route_stops_the_start_without_a_panic() {
    let e = start_with_raw("server", |v| v["mcp_path"] = json!("/exports")).await;
    assert_eq!(e, format!("{STORED}{}", cannot_serve("/exports")));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_195_a_stored_path_shadowing_a_route_stops_the_start() {
    let e = start_with_raw("server", |v| v["mcp_path"] = json!("/git/foo")).await;
    assert_eq!(e, format!("{STORED}{}", cannot_serve("/git/foo")));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_069_a_stored_auth_with_no_administrator_and_no_claim_still_starts() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut a = super::server_settings::section_json(&first.config, "auth");
    a["admins"] = json!([]);
    a["jwt"]["username_claim"] = json!("");
    put_raw_state(&first, "auth", &a.to_string()).await;
    drop((_r, first));
    let _booted = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_081_a_yaml_host_and_base_url_are_not_checked_a_stored_one_is() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(
        d.path(),
        json!({"host": "my_host", "public_base_url": "files.example.com"}),
        None,
    );
    let _booted = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    let e = start_with_raw("server", |v| v["host"] = json!("my_host")).await;
    assert_eq!(
        e,
        format!("{STORED}invalid server.host 'my_host': expected an IPv4 address or a host name")
    );
    let e = start_with_raw("server", |v| v["public_base_url"] = json!("files.example.com")).await;
    assert_eq!(
        e,
        format!(
            "{STORED}invalid server.public_base_url 'files.example.com': must start with http:// or https://"
        )
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_198_the_scheme_is_case_sensitive_and_the_scheme_alone_is_accepted() {
    let e = start_with_raw("server", |v| v["public_base_url"] = json!("HTTPS://files.example.com"))
        .await;
    assert_eq!(
        e,
        format!(
            "{STORED}invalid server.public_base_url 'HTTPS://files.example.com': must start with http:// or https://"
        )
    );
    for ok in ["http://", "https://files.example.com/"] {
        let d = tempfile::tempdir().unwrap();
        let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
        let mut v = super::server_settings::section_json(&first.config, "server");
        v["public_base_url"] = json!(ok);
        put_raw_state(&first, "server", &v.to_string()).await;
        drop((_r, first));
        let _booted = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    }
}

// ---- a YAML file --------------------------------------------------------------

/// A YAML (JSON is YAML) file for `root`: the key file of `boot_config`, the
/// administrator, the infra paths, plus `server` and `auth` overrides.
fn yaml_json(root: &Path, server: Value, auth: Option<Value>) -> std::path::PathBuf {
    let (_k, public) = crate::keys::write_keypair(root.join("keys")).unwrap();
    let mut a =
        json!({"jwt": {"public_key_path": public.display().to_string()}, "admins": [ALICE]});
    if let Some(over) = auth {
        a = over;
    }
    let doc = json!({
        "auth": a, "server": server,
        "infra": {
            "meta": {"dir": root.join("volumes").display().to_string()},
            "blob": {"dir": root.join("blobs").display().to_string()},
            "admin": {"path": root.join("admin.db").display().to_string()},
        },
    });
    let p = root.join("config.yaml");
    std::fs::write(&p, doc.to_string()).unwrap();
    p
}

/// Every change record line, read straight from the registry (a platform with no
/// administrator cannot ask the tool).
async fn changes_of(state: &Arc<AppState>) -> Vec<Value> {
    let reg = crate::settings::Registry::of(state.admin.as_ref(), 1536).unwrap();
    let all = reg.list_changes_of_kinds(&["setting"], 500).await.unwrap();
    all.iter().map(|c| serde_json::to_value(c).unwrap()).collect()
}

fn seeds_of(all: &[Value]) -> Vec<String> {
    all.iter()
        .filter(|c| c["action"] == "seed_setting")
        .map(|c| c["target_id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_040_the_yaml_seeds_server_and_auth_once() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(d.path(), json!({"port": 47833}), None);
    let (_r, first) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(first.config.server.port, 47833);
    drop((_r, first));
    let path = yaml_json(d.path(), json!({"port": 47834}), None);
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(second.config.server.port, 47833);
    let all = changes_of(&second).await;
    let seeds = seeds_of(&all);
    assert_eq!(seeds.len(), 14, "{seeds:?}");
    for s in ["server", "auth"] {
        let n = all
            .iter()
            .filter(|c| {
                c["action"] == "seed_setting" && c["target_id"] == s && c["person"] == "system:yaml"
            })
            .count();
        assert_eq!(n, 1, "{s}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_132_a_stored_port_wins_over_an_edited_yaml() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(d.path(), json!({"port": 47833}), None);
    let (_r, first) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    set_on(&first, "server", json!({"port": 47836})).await;
    drop((_r, first));
    let path = yaml_json(d.path(), json!({"port": 47834}), None);
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(second.config.server.port, 47836);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_041_a_configuration_without_a_key_list_prints_as_before() {
    let c = ServerConfig::from_yaml("auth:\n  jwt:\n    public_key_path: .keys/jwt.pub\n").unwrap();
    assert!(c.auth.jwt.public_keys.is_empty());
    let printed = format!("{:?}", c.auth.jwt);
    assert!(!printed.contains("public_keys"), "{printed}");
    let mut with = c.auth.jwt.clone();
    with.public_keys = vec!["x".into()];
    let printed = format!("{with:?}");
    assert!(printed.contains("public_keys: 1 key(s)"), "{printed}");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_042_a_yaml_list_of_keys_is_imported_and_verifies() {
    let d = tempfile::tempdir().unwrap();
    let k1 = &kp()[0];
    let auth = json!({"jwt": {"public_keys": [k1.public_spki]}, "admins": [ALICE]});
    let path = yaml_json(d.path(), json!({}), Some(auth));
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    let t = tok(&k1.private_pkcs8, ALICE);
    assert_eq!(state.identity.verify(&t).unwrap(), ALICE);
    let row = super::ServerSettings::of(state.admin.as_ref(), state.config.clone())
        .unwrap()
        .get("auth")
        .await
        .unwrap();
    assert_eq!(row["source"], "database");
    assert_eq!(row["value"]["jwt"]["public_keys"], json!([k1.public_spki]));
}

fn tok(private: &str, email: &str) -> String {
    let claims =
        json!({"iss": "web-a2a", "email": email, "exp": chrono::Utc::now().timestamp() + 3600});
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(private.as_bytes()).unwrap();
    jsonwebtoken::encode(&jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256), &claims, &key)
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_043_a_yaml_key_that_is_not_a_key_stops_the_start_with_the_bare_message() {
    let d = tempfile::tempdir().unwrap();
    let auth = json!({"jwt": {"public_keys": ["not a key"]}, "admins": [ALICE]});
    let path = yaml_json(d.path(), json!({}), Some(auth));
    let e = boot(load_yaml(&path), Some(KEY_A)).await.err().unwrap();
    assert_eq!(e, form(0));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_108_a_yaml_key_list_is_checked_for_form_and_count_not_for_size() {
    let k = &kp()[0];
    let nine = vec![k.public_spki.as_str(); 9];
    for (keys, want) in [
        (json!(nine), "auth.jwt.public_keys holds more than 8 keys".to_string()),
        (
            json!([k.private_pkcs1]),
            "auth.jwt.public_keys[0] holds a private key: only a public key is accepted"
                .to_string(),
        ),
    ] {
        let d = tempfile::tempdir().unwrap();
        let path = yaml_json(
            d.path(),
            json!({}),
            Some(json!({"jwt": {"public_keys": keys}, "admins": [ALICE]})),
        );
        assert_eq!(boot(load_yaml(&path), Some(KEY_A)).await.err().unwrap(), want);
    }
    let d = tempfile::tempdir().unwrap();
    let auth = json!({"jwt": {"public_keys": [RSA_1024]}, "admins": [ALICE]});
    let path = yaml_json(d.path(), json!({}), Some(auth));
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    // The key starts but the token library cannot verify with it: no usable key is left,
    // as before this release.
    let t = crate::keys::mint_token_from_file(
        d.path().join("keys").join(crate::keys::PRIVATE_KEY_FILE),
        ALICE,
        crate::keys::DEFAULT_ISSUER,
        crate::keys::DEFAULT_CLAIM,
        3600,
    )
    .unwrap();
    assert_eq!(state.identity.verify(&t).unwrap_err().message, "no JWT public key configured");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_067_a_yaml_port_zero_starts_twice() {
    let d = tempfile::tempdir().unwrap();
    for _ in 0..2 {
        let path = yaml_json(d.path(), json!({"host": "127.0.0.1", "port": 0}), None);
        let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
        assert_eq!(state.config.server.port, 0);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_174_a_yaml_health_path_stops_the_start_with_the_bare_message() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(d.path(), json!({"mcp_path": "/health"}), None);
    assert_eq!(boot(load_yaml(&path), Some(KEY_A)).await.err().unwrap(), cannot_serve("/health"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_179_a_yaml_path_of_a_screen_stops_the_start_with_the_bare_message() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(d.path(), json!({"mcp_path": "/app/settings"}), None);
    assert_eq!(
        boot(load_yaml(&path), Some(KEY_A)).await.err().unwrap(),
        cannot_serve("/app/settings")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_193_a_yaml_path_with_a_space_stops_the_start_with_the_bare_message() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(d.path(), json!({"mcp_path": "/a b"}), None);
    assert_eq!(boot(load_yaml(&path), Some(KEY_A)).await.err().unwrap(), cannot_serve("/a b"));
}

// ---- seeds that are skipped ---------------------------------------------------

const SKIP_SERVER: &str = "the YAML section 'server' is not imported: invalid server.public_base_url 'files.example.com': must start with http:// or https://";
const SKIP_AUTH: &str = "the YAML section 'auth' is not imported: it holds no key";

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_110_a_yaml_server_section_with_a_bad_base_url_is_not_imported() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let server = json!({"host": "127.0.0.1", "port": 0, "public_base_url": "files.example.com"});
    let mut last = None;
    for _ in 0..2 {
        logs.lock().unwrap().clear();
        let path = yaml_json(d.path(), server.clone(), None);
        let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
        assert_eq!(logged(&logs).matches(SKIP_SERVER).count(), 1);
        let all = changes_of(&state).await;
        let seeds = seeds_of(&all);
        assert!(seeds.contains(&"auth".to_string()) && !seeds.contains(&"server".to_string()));
        last = Some(state);
    }
    let state = last.unwrap();
    let s = super::ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap();
    assert_eq!(s.get("server").await.unwrap()["source"], "yaml");
    let m = s.set(ALICE, "server", json!({"port": 47850})).await.unwrap_err().message;
    assert_eq!(
        m,
        "invalid server.public_base_url 'files.example.com': must start with http:// or https://"
    );
    let out = s.set(ALICE, "server", json!({"port": 47850, "public_base_url": ""})).await.unwrap();
    assert_eq!(out["source"], "database");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_111_a_yaml_server_section_with_a_good_base_url_is_imported_quietly() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let path = yaml_json(d.path(), json!({"public_base_url": "https://files.example.com"}), None);
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert!(seeds_of(&changes_of(&state).await).contains(&"server".to_string()));
    assert!(!logged(&logs).contains("is not imported"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_112_a_yaml_host_that_is_not_a_name_starts_twice_the_same() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(d.path(), json!({"host": "my_host"}), None);
    let (_r, first) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    let banner = crate::app::banner_line("v", &first.config, None);
    let all = changes_of(&first).await;
    assert!(
        !seeds_of(&all).contains(&"server".to_string()) || first.config.server.host == "my_host"
    );
    drop((_r, first));
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(crate::app::banner_line("v", &second.config, None), banner);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_120_a_yaml_auth_with_no_key_is_not_imported_and_cannot_serve() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let auth = json!({"admins": [ALICE]});
    let path = yaml_json(d.path(), json!({"host": "127.0.0.1", "port": 0}), Some(auth));
    let config = load_yaml(&path);
    let (_r, state) = boot(config.clone(), Some(KEY_A)).await.unwrap();
    assert_eq!(logged(&logs).matches(SKIP_AUTH).count(), 1);
    assert!(!seeds_of(&changes_of(&state).await).contains(&"auth".to_string()));
    let s = super::ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap();
    assert_eq!(s.get("auth").await.unwrap()["source"], "yaml");
    drop((_r, state));
    let e = crate::app::serve_with_env(config, env_of(KEY_A), None).await.unwrap_err();
    assert_eq!(e.to_string(), NOKEY_START);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_121_a_yaml_edited_to_hold_a_key_is_imported_at_the_next_start() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(d.path(), json!({}), Some(json!({"admins": [ALICE]})));
    let (_r, first) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    drop((_r, first));
    let path = yaml_json(d.path(), json!({}), None);
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    let all = changes_of(&second).await;
    assert!(all.iter().any(|c| c["action"] == "seed_setting"
        && c["target_id"] == "auth"
        && c["person"] == "system:yaml"));
    let t = crate::keys::mint_token_from_file(
        d.path().join("keys").join(crate::keys::PRIVATE_KEY_FILE),
        ALICE,
        crate::keys::DEFAULT_ISSUER,
        crate::keys::DEFAULT_CLAIM,
        3600,
    )
    .unwrap();
    assert_eq!(second.identity.verify(&t).unwrap(), ALICE);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_123_a_yaml_with_no_auth_block_imports_nothing_for_auth() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let p = d.path().join("config.yaml");
    std::fs::write(
        &p,
        json!({"infra": {"meta": {"dir": d.path().join("v").display().to_string()},
            "blob": {"dir": d.path().join("b").display().to_string()},
            "admin": {"path": d.path().join("a.db").display().to_string()}}})
        .to_string(),
    )
    .unwrap();
    let (_r, state) = boot(load_yaml(&p), Some(KEY_A)).await.unwrap();
    assert_eq!(logged(&logs).matches(SKIP_AUTH).count(), 1);
    assert!(!seeds_of(&changes_of(&state).await).contains(&"auth".to_string()));
}

// ---- the address and the path the server serves ---------------------------------

fn env_of(key: [u8; 32]) -> crate::app::EnvFn {
    use base64::Engine;
    Arc::new(move |n| {
        (n == "FSA_TOKEN_KEY").then(|| base64::engine::general_purpose::STANDARD.encode(key))
    })
}

/// A free loopback port: bound, read, released.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

async fn answers(port: u16, path: &str) -> Option<u16> {
    let url = format!("http://127.0.0.1:{port}{path}");
    let client =
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(2)).build().unwrap();
    client.get(url).send().await.ok().map(|r| r.status().as_u16())
}

async fn wait_up(port: u16) {
    for _ in 0..100 {
        if answers(port, "/health").await == Some(200) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the server did not come up on {port}");
}

async fn stop(h: tokio::task::JoinHandle<anyhow::Result<()>>) {
    h.abort();
    let _ = h.await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
}

fn serving(config: ServerConfig) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    tokio::spawn(crate::app::serve_with_env(config, env_of(KEY_A), None))
}

fn file_token(root: &Path, who: &str) -> String {
    crate::keys::mint_token_from_file(
        root.join("keys").join(crate::keys::PRIVATE_KEY_FILE),
        who,
        crate::keys::DEFAULT_ISSUER,
        crate::keys::DEFAULT_CLAIM,
        3600,
    )
    .unwrap()
}

async fn put_setting(port: u16, bearer: &str, section: &str, value: Value) -> (u16, Value) {
    let client = reqwest::Client::new();
    let res = client
        .put(format!("http://127.0.0.1:{port}/api/admin/settings/{section}"))
        .header("Authorization", format!("Bearer {bearer}"))
        .json(&json!({"value": value}))
        .send()
        .await
        .unwrap();
    (res.status().as_u16(), res.json().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_008_a_stored_address_applies_at_the_next_start_only() {
    let d = tempfile::tempdir().unwrap();
    let (p1, p2) = (free_port(), free_port());
    let mut config = boot_config(d.path());
    config.server.host = "127.0.0.1".into();
    config.server.port = p1;
    let h = serving(config.clone());
    wait_up(p1).await;
    let t = file_token(d.path(), ALICE);
    let (st, out) = put_setting(p1, &t, "server", json!({"port": p2})).await;
    assert_eq!(st, 200, "{out}");
    assert_eq!(out["restart_required"], true);
    assert_eq!(answers(p1, "/health").await, Some(200));
    assert_eq!(answers(p2, "/health").await, None);
    stop(h).await;
    let h = serving(config);
    wait_up(p2).await;
    assert_eq!(answers(p1, "/health").await, None);
    stop(h).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_006_a_refused_port_leaves_the_running_server_where_it_is() {
    let d = tempfile::tempdir().unwrap();
    let p1 = free_port();
    let mut config = boot_config(d.path());
    config.server.host = "127.0.0.1".into();
    config.server.port = p1;
    let h = serving(config);
    wait_up(p1).await;
    let t = file_token(d.path(), ALICE);
    for p in [json!(70000), json!(0), json!(-1)] {
        let (st, out) = put_setting(p1, &t, "server", json!({"port": p})).await;
        assert_eq!(st, 400, "{out}");
    }
    let (st, out) = put_setting(p1, &t, "server", json!({"port": "80"})).await;
    assert_eq!(st, 400, "{out}");
    assert_eq!(answers(p1, "/health").await, Some(200));
    stop(h).await;
}

/// The `initialize` of the MCP route at `path`: the status.
async fn mcp_status(port: u16, path: &str, bearer: &str) -> u16 {
    let body = json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
        "protocolVersion": "2025-03-26", "capabilities": {},
        "clientInfo": {"name": "t", "version": "0"}}});
    let res = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}{path}"))
        .header("Authorization", format!("Bearer {bearer}"))
        .header("Accept", "application/json, text/event-stream")
        .json(&body)
        .send()
        .await
        .unwrap();
    res.status().as_u16()
}

async fn serve_at_path(path: &str) -> (u16, u16) {
    let d = tempfile::tempdir().unwrap();
    let port = free_port();
    let mut config = boot_config(d.path());
    config.server.host = "127.0.0.1".into();
    config.server.port = port;
    config.server.mcp_path = path.into();
    let h = serving(config);
    wait_up(port).await;
    let t = file_token(d.path(), ALICE);
    let at_path = mcp_status(port, path, &t).await;
    let at_default = mcp_status(port, "/mcp", &t).await;
    stop(h).await;
    (at_path, at_default)
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_009_the_stored_mcp_path_is_the_one_served() {
    let (at_path, at_default) = serve_at_path("/rpc").await;
    assert_eq!(at_path, 200);
    assert_eq!(at_default, 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_177_a_nested_path_is_served_and_health_still_answers() {
    let (at_path, _) = serve_at_path("/a/b").await;
    assert_eq!(at_path, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_192_a_path_with_punctuation_is_served() {
    let (at_path, _) = serve_at_path("/a-b_c.d~e/MCP").await;
    assert_eq!(at_path, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_196_a_path_next_to_a_route_prefix_is_served() {
    let (at_path, _) = serve_at_path("/gitx").await;
    assert_eq!(at_path, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_010_a_start_from_the_database_alone_serves_the_stored_auth_and_address() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let port = free_port();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(
        &first,
        "auth",
        json!({"jwt": {"public_keys": [&k[0].public_spki], "public_key_path": ""}, "admins": [ALICE]}),
    )
    .await;
    set_on(&first, "server", json!({"host": "127.0.0.1", "port": port})).await;
    drop((_r, first));
    // Only the admin database URL and the master key, as `--database-url` gives.
    let mut config = ServerConfig::default();
    config.infra.meta.dir = d.path().join("volumes").display().to_string();
    config.infra.blob.dir = d.path().join("blobs").display().to_string();
    config.set_database_url(&format!("sqlite:{}", d.path().join("admin.db").display())).unwrap();
    let h = serving(config);
    wait_up(port).await;
    let ok = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/api/admin/settings"))
        .header("Authorization", format!("Bearer {}", tok(&k[0].private_pkcs8, ALICE)))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status().as_u16(), 200);
    let other = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/api/admin/settings"))
        .header("Authorization", format!("Bearer {}", tok(&k[1].private_pkcs8, ALICE)))
        .send()
        .await
        .unwrap();
    assert_eq!(other.status().as_u16(), 401);
    stop(h).await;
}

fn claims_token(private: &str, claims: Value) -> String {
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(private.as_bytes()).unwrap();
    jsonwebtoken::encode(&jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256), &claims, &key)
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_027_a_stored_issuer_audience_and_claim_are_the_ones_in_force() {
    let d = tempfile::tempdir().unwrap();
    let k = &kp()[0];
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(
        &first,
        "auth",
        json!({"jwt": {"public_keys": [&k.public_spki], "issuer": "corp", "audience": "fsa", "username_claim": "sub"}}),
    )
    .await;
    drop((_r, first));
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let exp = chrono::Utc::now().timestamp() + 3600;
    let good = claims_token(
        &k.private_pkcs8,
        json!({"iss": "corp", "aud": "fsa", "sub": ALICE, "exp": exp}),
    );
    assert_eq!(state.identity.verify(&good).unwrap(), ALICE);
    let wrong_iss = claims_token(
        &k.private_pkcs8,
        json!({"iss": "web-a2a", "aud": "fsa", "sub": ALICE, "exp": exp}),
    );
    assert!(state.identity.verify(&wrong_iss).is_err());
    let other_aud = claims_token(
        &k.private_pkcs8,
        json!({"iss": "corp", "aud": "other", "sub": ALICE, "exp": exp}),
    );
    assert!(state.identity.verify(&other_aud).is_err());
    let email_only = claims_token(
        &k.private_pkcs8,
        json!({"iss": "corp", "aud": "fsa", "email": ALICE, "exp": exp}),
    );
    assert!(state.identity.verify(&email_only).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_028_the_stored_administrators_are_the_ones_in_force() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "auth", json!({"admins": ["boss@example.com"]})).await;
    drop((_r, first));
    let (router, _s) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let get = |who: &str| {
        let req = axum::http::Request::builder()
            .uri("/api/admin/settings")
            .header("Authorization", format!("Bearer {}", file_token(d.path(), who)))
            .body(axum::body::Body::empty())
            .unwrap();
        let router = router.clone();
        async move { router.oneshot(req).await.unwrap().status().as_u16() }
    };
    assert_eq!(get("boss@example.com").await, 200);
    assert_eq!(get(ALICE).await, 403);
    let _ = BOB;
}

// ---- no key anywhere ------------------------------------------------------------

fn keyless(root: &Path) -> ServerConfig {
    let mut c = boot_config(root);
    c.auth.jwt.public_key_path = String::new();
    c
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_029_serving_with_no_key_anywhere_is_refused_and_binds_nothing() {
    let d = tempfile::tempdir().unwrap();
    let port = free_port();
    let mut config = keyless(d.path());
    config.server.host = "127.0.0.1".into();
    config.server.port = port;
    let e = crate::app::serve_with_env(config, env_of(KEY_A), None).await.unwrap_err();
    assert_eq!(e.to_string(), NOKEY_START);
    assert_eq!(answers(port, "/health").await, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_030_a_stored_empty_key_list_and_path_refuse_to_serve() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut a = super::server_settings::section_json(&first.config, "auth");
    a["jwt"]["public_key_path"] = json!("");
    a["jwt"]["public_keys"] = json!([]);
    put_raw_state(&first, "auth", &a.to_string()).await;
    drop((_r, first));
    let e =
        crate::app::serve_with_env(boot_config(d.path()), env_of(KEY_A), None).await.unwrap_err();
    assert_eq!(e.to_string(), NOKEY_START);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_031_the_build_without_a_key_is_unchanged_and_a_missing_file_still_serves() {
    let d = tempfile::tempdir().unwrap();
    let _booted = boot(keyless(d.path()), Some(KEY_A)).await.unwrap();
    let port = free_port();
    let mut config = boot_config(d.path());
    config.auth.jwt.public_key_path = "/nonexistent/jwt.pub".into();
    config.server.host = "127.0.0.1".into();
    config.server.port = port;
    let h = serving(config);
    wait_up(port).await;
    let res = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/api/admin/settings"))
        .header("Authorization", format!("Bearer {}", file_token(d.path(), ALICE)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 401);
    stop(h).await;
}

// ---- the banner ---------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_068_the_banner_names_the_stored_port_and_the_file() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(d.path(), json!({"port": 47839}), None);
    let (_r, first) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    set_on(&first, "server", json!({"port": 47840})).await;
    drop((_r, first));
    let path = yaml_json(d.path(), json!({"port": 47841}), None);
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    let line = crate::app::banner_line("9.9.9", &state.config, Some(&path.display().to_string()));
    assert_eq!(line, format!("Serving fsa 9.9.9 on 0.0.0.0:47840 (config={})", path.display()));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_070_the_banner_without_a_file_says_the_settings_come_from_the_database() {
    let d = tempfile::tempdir().unwrap();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&first, "server", json!({"host": "127.0.0.1", "port": 47842})).await;
    drop((_r, first));
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(
        crate::app::banner_line("9.9.9", &state.config, None),
        "Serving fsa 9.9.9 on 127.0.0.1:47842 (config=none, settings from the database)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_071_the_banner_of_a_file_without_a_stored_server_names_its_host() {
    let d = tempfile::tempdir().unwrap();
    let path = yaml_json(d.path(), json!({"host": "10.1.2.3"}), None);
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        crate::app::banner_line("9.9.9", &state.config, Some(&path.display().to_string())),
        format!("Serving fsa 9.9.9 on 10.1.2.3:5002 (config={})", path.display())
    );
}

// ---- the route table against the real router (design D23, D24) ----------------------

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn e2e_0116_d23_every_path_the_real_router_cannot_mount_is_refused() {
    use axum::body::Body;
    let d = tempfile::tempdir().unwrap();
    let mut config = boot_config(d.path());
    config.git.enabled = true;
    let _hosts = crate::git::remote::tests::lock_for_test();
    let (router, _s) = boot(config, Some(KEY_A)).await.unwrap();
    let mut probes: Vec<String> = vec!["/".into(), "/health".into(), "/app".into()];
    for route in crate::mcp_mount::ROUTES {
        let segs: Vec<String> = route
            .trim_start_matches('/')
            .split('/')
            .map(|s| if s.starts_with('{') { "x".to_string() } else { s.to_string() })
            .collect();
        for n in 1..=segs.len() {
            probes.push(format!("/{}", segs[..n].join("/")));
        }
    }
    let mut collisions = 0;
    for path in probes {
        if path.ends_with('/') && path != "/" {
            continue;
        }
        let r = router.clone();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let svc = tower::service_fn(|_req: axum::http::Request<Body>| async {
                Ok::<_, std::convert::Infallible>(axum::response::Response::new(Body::empty()))
            });
            let _ = r.nest_service(&path, svc);
        }))
        .is_err();
        if panicked {
            collisions += 1;
            assert!(
                crate::mcp_mount::refusal(&path).is_some(),
                "{path} panics the router and is not refused"
            );
        }
    }
    assert!(collisions > 20, "the probe found only {collisions} collisions");
}
