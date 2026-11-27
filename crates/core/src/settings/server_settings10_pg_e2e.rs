//! SPEC-0117 E2E-009, 027, 055, 060, 061, 062: the `infra` section with PostgreSQL, as
//! the admin store and as a meta store. They need `MCPFS_TEST_PG_DSN` (009 and 062 need
//! only the driver) and skip without it. The clock is the real one.

use super::ServerSettings;
use super::infra_testkit::S3Stand;
use super::server_settings_pg_e2e::Pg;
use crate::config::{ServerConfig, backend};
use crate::storage::admin::RelationalAdminStore;
use crate::storage::rel::{
    PoolSettings, PostgresRelationalDb, Query, RelationalDb, SqliteRelationalDb,
};
use crate::storage::traits::AdminBackend;
use crate::storage::{RelationalRegistry, StoreManager};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

const ADMIN: &str = "admin@example.com";
const KEY: [u8; 32] = [7u8; 32];

fn dsn() -> Option<String> {
    std::env::var("MCPFS_TEST_PG_DSN").ok().filter(|v| !v.trim().is_empty())
}

fn schema_name() -> String {
    format!("fsa_p117_{}", &uuid::Uuid::new_v4().to_string().replace('-', "")[..12])
}

/// The server's DSN with the password replaced by `pw`.
fn with_password(dsn: &str, pw: &str) -> String {
    let (scheme, rest) = dsn.split_once("://").unwrap();
    let (userinfo, host) = rest.split_once('@').unwrap();
    let user = userinfo.split(':').next().unwrap();
    format!("{scheme}://{user}:{pw}@{host}")
}

/// A platform whose meta store is a PostgreSQL schema, its admin store a SQLite file.
struct Platform {
    settings: ServerSettings,
    config: ServerConfig,
    admin: Arc<RelationalAdminStore>,
    schemas: Vec<String>,
}

impl Platform {
    async fn new(root: &Path, dsn: &str) -> Self {
        let schema = schema_name();
        let mut config = ServerConfig::default();
        config.infra.meta.backend = backend::POSTGRES.into();
        config.infra.meta.dsn = crate::config::Dsn::new(dsn);
        config.infra.meta.schema = schema.clone();
        config.infra.meta.dir = root.join("volumes").display().to_string();
        config.infra.blob.dir = root.join("blobs").display().to_string();
        config.infra.admin.path = root.join("admin.db").display().to_string();
        config.auth.admins = vec![ADMIN.into()];
        let db = Arc::new(SqliteRelationalDb::open(root.join("admin.db")).unwrap());
        let admin = Arc::new(RelationalAdminStore::new(db));
        admin.connect().await.unwrap();
        admin.set_master_key(Some(KEY));
        admin.create_project("p", ADMIN).await.unwrap();
        let stores = Arc::new(StoreManager::new(
            Arc::new(config.clone()),
            Arc::new(RelationalRegistry::new()),
        ));
        stores.provision_volume("p").await.unwrap();
        stores.client("p").await.unwrap().write_text_atomic("/a.txt", "pg data").await.unwrap();
        let settings = ServerSettings::of(admin.as_ref(), Arc::new(config.clone())).unwrap();
        Self { settings, config, admin, schemas: vec![schema] }
    }

    async fn drop_schemas(self, dsn: &str) {
        for s in &self.schemas {
            if let Ok(db) = PostgresRelationalDb::connect(dsn, s, PoolSettings::default()).await {
                let _ = db.execute(&Query::new(format!("DROP SCHEMA \"{s}\" CASCADE"))).await;
            }
        }
        let _ = &self.admin;
    }
}

#[tokio::test]
async fn e2e_0117_009_an_unreachable_dsn_is_refused_without_its_password() {
    let fx = super::e2e::Fx::new().await;
    let s = ServerSettings::of(fx.state.admin.as_ref(), fx.state.config.clone()).unwrap();
    let m = s
        .set(
            ADMIN,
            "infra",
            json!({"meta": {"backend": "postgres", "dsn": "postgres://u:hunter2pw@127.0.0.1:1/db"}}),
        )
        .await
        .unwrap_err()
        .message;
    assert!(m.starts_with("infra.meta cannot be reached:"), "{m}");
    assert!(!m.contains("hunter2pw") && !m.contains("127.0.0.1:1/db"), "{m}");
    assert_eq!(s.get("infra").await.unwrap()["source"], "default");
}

#[tokio::test]
async fn e2e_0117_027_a_wrong_password_is_refused_without_the_dsn() {
    let Some(dsn) = dsn() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = Platform::new(d.path(), &dsn).await;
    let bad = with_password(&dsn, "wrongpw");
    let m = p
        .settings
        .set(ADMIN, "infra", json!({"meta": {"dsn": bad, "schema": schema_name()}}))
        .await
        .unwrap_err()
        .message;
    assert!(m.starts_with("infra.meta cannot be reached:"), "{m}");
    assert!(!m.contains("wrongpw") && !m.contains(&bad), "{m}");
    p.drop_schemas(&dsn).await;
}

#[tokio::test]
async fn e2e_0117_055_a_dsn_that_differs_by_a_parameter_passes_when_the_data_is_the_same() {
    let Some(dsn) = dsn() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = Platform::new(d.path(), &dsn).await;
    let other =
        format!("{dsn}{}application_name=fsa-p117", if dsn.contains('?') { "&" } else { "?" });
    let out = p.settings.set(ADMIN, "infra", json!({"meta": {"dsn": other}})).await.unwrap();
    assert_eq!(out["restart_required"], true);
    assert_eq!(out["secrets"]["meta.dsn"], json!({"has_value": true, "last4": ""}));
    p.drop_schemas(&dsn).await;
}

#[tokio::test]
async fn e2e_0117_060_the_section_round_trips_on_a_postgresql_admin_store() {
    let Some(pg) = Pg::new().await else { return };
    let s = pg.settings(ServerConfig::default());
    let region = s.set(ADMIN, "infra", json!({"blob": {"region": "eu-west-1"}})).await.unwrap();
    assert_eq!(s.get("infra").await.unwrap(), region);
    let stand = S3Stand::start(404).await;
    let out = s
        .set(
            ADMIN,
            "infra",
            json!({"blob": {"backend": "s3", "endpoint": stand.url, "access_key": "AKIAPG000001", "secret_key": "pg-secret-abcd"}}),
        )
        .await
        .unwrap();
    assert_eq!(out["secrets"]["blob.secret_key"], json!({"has_value": true, "last4": "abcd"}));
    assert_eq!(out["secrets"]["blob.access_key"], json!({"has_value": true, "last4": "0001"}));
    assert!(out["value"]["blob"].get("secret_key").is_none());
    assert_eq!(out["value"]["blob"]["region"], "eu-west-1");
    assert_eq!(pg.setting_lines().await, 2);
    pg.drop_schema().await;
}

#[tokio::test]
async fn e2e_0117_061_a_postgresql_meta_store_moves_only_after_the_migration() {
    let Some(dsn) = dsn() else { return };
    let d = tempfile::tempdir().unwrap();
    let mut p = Platform::new(d.path(), &dsn).await;
    let target = schema_name();
    p.schemas.push(target.clone());
    let m = p
        .settings
        .set(ADMIN, "infra", json!({"meta": {"schema": target}}))
        .await
        .unwrap_err()
        .message;
    assert!(
        m.starts_with("infra.meta points at a store that does not hold the data of the current one (project 'p': nodes "),
        "{m}"
    );
    assert!(m.contains("fsa migrate --from"), "{m}");
    let mut to = p.config.clone();
    to.infra.meta.schema = target.clone();
    to.infra.admin.path = d.path().join("admin-to.db").display().to_string();
    crate::migrate::migrate(&p.config, &to).await.unwrap();
    let out = p.settings.set(ADMIN, "infra", json!({"meta": {"schema": target}})).await.unwrap();
    assert_eq!(out["value"]["meta"]["schema"], target);
    assert_eq!(out["restart_required"], true);
    p.drop_schemas(&dsn).await;
}

#[tokio::test]
async fn e2e_0117_062_a_refused_dsn_on_a_postgresql_admin_store_stores_and_records_nothing() {
    let Some(pg) = Pg::new().await else { return };
    let s = pg.settings(ServerConfig::default());
    let m = s
        .set(
            ADMIN,
            "infra",
            json!({"meta": {"backend": "postgres", "dsn": "postgres://u:hunter2pw@127.0.0.1:1/db"}}),
        )
        .await
        .unwrap_err()
        .message;
    assert!(m.starts_with("infra.meta cannot be reached:"), "{m}");
    assert!(!m.contains("hunter2pw"), "{m}");
    assert_eq!(s.get("infra").await.unwrap()["source"], "default");
    assert_eq!(pg.setting_lines().await, 0);
    pg.drop_schema().await;
}
