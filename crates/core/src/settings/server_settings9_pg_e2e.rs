//! SPEC-0116 E2E-047, 048, 049: the `server` and `auth` sections on a real
//! PostgreSQL admin store. They need `MCPFS_TEST_PG_DSN` and skip without it. The
//! clock is the real one (a paused clock jumps while a socket answer is in flight).

use super::key_fixtures::RSA_1024;
use super::server_settings_boot::apply;
use super::server_settings_pg_e2e::Pg;
use super::server_settings5_e2e::kp;
use crate::config::ServerConfig;
use serde_json::json;

const ADMIN: &str = "admin@example.com";
const KEY: [u8; 32] = [7u8; 32];

fn running() -> ServerConfig {
    let mut c = ServerConfig::default();
    c.auth.jwt.public_key_path = "/k/jwt.pub".into();
    c.auth.admins = vec![ADMIN.into()];
    c
}

#[tokio::test]
async fn e2e_0116_047_the_two_sections_round_trip_on_postgresql() {
    let Some(pg) = Pg::new().await else { return };
    let s = pg.settings(running());
    let k = kp();
    let out = s.set(ADMIN, "server", json!({"host": "127.0.0.1", "port": 47831})).await.unwrap();
    assert_eq!(out["value"]["port"], 47831);
    assert_eq!(s.get("server").await.unwrap(), out);
    let auth = s
        .set(
            ADMIN,
            "auth",
            json!({"jwt": {"public_keys": [&k[0].public_spki, &k[1].public_spki]}, "admins": [ADMIN, "b@example.com"]}),
        )
        .await
        .unwrap();
    assert_eq!(auth["value"]["jwt"]["public_keys"].as_array().unwrap().len(), 2);
    assert_eq!(auth["secrets"], json!({}));
    let lines = pg.setting_lines().await;
    let m = s
        .set(ADMIN, "auth", json!({"jwt": {"public_keys": [&k[0].private_pkcs8]}}))
        .await
        .unwrap_err()
        .message;
    assert_eq!(m, "auth.jwt.public_keys[0] holds a private key: only a public key is accepted");
    assert_eq!(s.get("auth").await.unwrap(), auth, "nothing was stored");
    assert_eq!(pg.setting_lines().await, lines, "nothing was recorded");
    let m = s
        .set(ADMIN, "auth", json!({"jwt": {"public_keys": [RSA_1024]}}))
        .await
        .unwrap_err()
        .message;
    assert_eq!(m, "auth.jwt.public_keys[0] holds an RSA key shorter than 2048 bits");
    pg.drop_schema().await;
}

#[tokio::test]
async fn e2e_0116_048_a_start_on_postgresql_reads_the_stored_server_and_auth() {
    let Some(pg) = Pg::new().await else { return };
    let s = pg.settings(running());
    let k = kp();
    s.set(ADMIN, "server", json!({"host": "127.0.0.1", "port": 47832})).await.unwrap();
    s.set(
        ADMIN,
        "auth",
        json!({"jwt": {"public_keys": [&k[0].public_spki], "public_key_path": ""}, "admins": ["boss@example.com"]}),
    )
    .await
    .unwrap();
    let started = apply(ServerConfig::default(), pg.admin.as_ref(), Some(KEY)).await.unwrap();
    assert_eq!(started.server.port, 47832);
    assert_eq!(started.server.host, "127.0.0.1");
    assert_eq!(started.auth.jwt.public_keys, vec![k[0].public_spki.clone()]);
    assert_eq!(started.auth.admins, vec!["boss@example.com".to_string()]);
    let id = crate::identity::IdentityResolver::new(&started.auth);
    assert!(id.verify("not a token").is_err());
    pg.drop_schema().await;
}

#[tokio::test]
async fn e2e_0116_049_the_last_key_cannot_be_taken_away_on_postgresql() {
    let Some(pg) = Pg::new().await else { return };
    let s = pg.settings(running());
    let m =
        s.set(ADMIN, "auth", json!({"jwt": {"public_key_path": ""}})).await.unwrap_err().message;
    assert_eq!(m, "auth.jwt needs at least one public key: give public_keys or public_key_path");
    assert_eq!(s.get("auth").await.unwrap()["source"], "default");
    assert_eq!(pg.setting_lines().await, 0);
    pg.drop_schema().await;
}
