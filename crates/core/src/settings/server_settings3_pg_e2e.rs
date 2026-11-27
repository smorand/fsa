//! SPEC-0115 E2E-021, 035, 048: the git, web and context7 sections on a real
//! PostgreSQL admin store. They need `MCPFS_TEST_PG_DSN` and skip without it.
//! The clock is the real one (a paused clock jumps while a socket answer is in
//! flight and the pool times out).

use super::server_settings_boot::apply;
use super::server_settings_pg_e2e::Pg;
use crate::config::ServerConfig;
use crate::git::remote::{Provider, resolve_host, tests::lock_for_test, validate_hosts};
use serde_json::json;

const ADMIN: &str = "admin@example.com";
const KEY: [u8; 32] = [7u8; 32];

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn e2e_0115_021_the_three_sections_on_postgresql() {
    let Some(pg) = Pg::new().await else { return };
    let _hosts = lock_for_test();
    let s = pg.settings(ServerConfig::default());
    let out = s.set(ADMIN, "git", json!({"hosts": {"github.com": "github"}})).await.unwrap();
    assert_eq!(out["value"]["hosts"], json!({"github.com": "github"}));
    assert_eq!(out["source"], "database");
    assert_eq!(out["restart_required"], true);
    assert_eq!(s.get("git").await.unwrap(), out);
    let m = s
        .set(ADMIN, "git", json!({"hosts": [["a.example", "github"], ["a.example", "gitlab"]]}))
        .await
        .unwrap_err()
        .message;
    assert_eq!(m, "git.hosts contains duplicate host 'a.example'");
    let replaced = s.set(ADMIN, "git", json!({"hosts": {"b.example": "generic"}})).await.unwrap();
    assert_eq!(replaced["value"]["hosts"], json!({"b.example": "generic"}), "replaced, not merged");
    s.set(ADMIN, "web", json!({"black_list_url": ["*.ads.example"]})).await.unwrap();
    s.set(ADMIN, "context7", json!({"enabled": true})).await.unwrap();
    // The start overlay: the stored sections replace the file's, the map is published.
    validate_hosts(&crate::config::GitConfig::default()).unwrap();
    let mut yaml = ServerConfig { from_yaml_file: true, ..ServerConfig::default() };
    yaml.git.hosts.0 = vec![("yaml.example".into(), "github".into())];
    let started = apply(yaml, pg.admin.as_ref(), Some(KEY)).await.unwrap();
    assert_eq!(started.git.hosts.0, vec![("b.example".to_string(), "generic".to_string())]);
    assert_eq!(started.web.black_list_url, vec!["*.ads.example".to_string()]);
    assert!(started.context7.enabled);
    assert_eq!(resolve_host("b.example").unwrap(), Provider::Generic);
    assert!(resolve_host("yaml.example").is_err());
    pg.drop_schema().await;
}

#[tokio::test]
async fn e2e_0115_035_a_repeated_host_stores_and_records_nothing_on_postgresql() {
    let Some(pg) = Pg::new().await else { return };
    let s = pg.settings(ServerConfig::default());
    let m = s
        .set(ADMIN, "git", json!({"hosts": [["github.com", "github"], ["github.com", "gitlab"]]}))
        .await
        .unwrap_err()
        .message;
    assert_eq!(m, "git.hosts contains duplicate host 'github.com'");
    assert_eq!(s.get("git").await.unwrap()["source"], "default");
    assert_eq!(pg.setting_lines().await, 0);
    pg.drop_schema().await;
}

#[tokio::test]
async fn e2e_0115_048_a_wildcard_host_is_refused_on_postgresql() {
    let Some(pg) = Pg::new().await else { return };
    let s = pg.settings(ServerConfig::default());
    let m = s.set(ADMIN, "git", json!({"hosts": {"*.x": "github"}})).await.unwrap_err().message;
    assert_eq!(
        m,
        "git.hosts key '*.x' contains a wildcard character: host matching is exact only, one key \
         per hostname"
    );
    assert_eq!(s.get("git").await.unwrap()["source"], "default");
    assert_eq!(pg.setting_lines().await, 0);
    pg.drop_schema().await;
}
