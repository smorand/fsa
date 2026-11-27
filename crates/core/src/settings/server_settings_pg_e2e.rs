//! SPEC-0114 E2E-018: the settings store and the boot overlay on a real
//! PostgreSQL admin store. They need `MCPFS_TEST_PG_DSN` and skip without it.
//! The clock is the real one: a paused clock jumps while a socket answer is in
//! flight and the pool times out.

use super::ServerSettings;
use super::server_settings_boot::apply;
use crate::config::ServerConfig;
use crate::storage::admin::RelationalAdminStore;
use crate::storage::rel::{PoolSettings, PostgresRelationalDb, Query, RelationalDb};
use crate::storage::traits::AdminBackend;
use serde_json::json;
use std::sync::Arc;

const ADMIN: &str = "admin@example.com";
const TOKEN: &str = "tok-abcdef123456";
const KEY: [u8; 32] = [7u8; 32];

pub(super) struct Pg {
    pub(super) admin: Arc<RelationalAdminStore>,
    pub(super) db: Arc<PostgresRelationalDb>,
    schema: String,
}

impl Pg {
    pub(super) async fn new() -> Option<Self> {
        let dsn = std::env::var("MCPFS_TEST_PG_DSN").ok().filter(|v| !v.trim().is_empty())?;
        let id = uuid::Uuid::new_v4().to_string().replace('-', "");
        let schema = format!("fsa_p114_{}", &id[..12]);
        let db = Arc::new(
            PostgresRelationalDb::connect(&dsn, &schema, PoolSettings::default())
                .await
                .expect("MCPFS_TEST_PG_DSN is set but connection failed"),
        );
        let admin = Arc::new(RelationalAdminStore::new(db.clone()));
        admin.connect().await.unwrap();
        admin.set_master_key(Some(KEY));
        Some(Self { admin, db, schema })
    }

    pub(super) fn settings(&self, running: ServerConfig) -> ServerSettings {
        ServerSettings::of(self.admin.as_ref(), Arc::new(running)).unwrap()
    }

    /// How many lines of the settings record the store holds.
    pub(super) async fn setting_lines(&self) -> i64 {
        self.db
            .query_opt(&Query::new(
                "SELECT COUNT(*) FROM setting_change WHERE target_kind='setting'",
            ))
            .await
            .unwrap()
            .unwrap()
            .i64(0)
            .unwrap()
    }

    pub(super) async fn drop_schema(self) {
        let _ =
            self.db.execute(&Query::new(format!("DROP SCHEMA \"{}\" CASCADE", self.schema))).await;
    }
}

#[tokio::test]
async fn e2e_0114_018_the_store_on_postgresql() {
    let Some(pg) = Pg::new().await else { return };
    let s = pg.settings(ServerConfig::default());
    let out = s
        .set(ADMIN, "doc_service", json!({"api": {"auth_token": TOKEN, "url": "http://x/c"}}))
        .await
        .unwrap();
    assert_eq!(out["secrets"]["api.auth_token"], json!({"has_value": true, "last4": "3456"}));
    assert_eq!(out["source"], "database");
    assert_eq!(out["restart_required"], true);
    assert!(out["value"]["api"].get("auth_token").is_none());
    assert_eq!(s.get("doc_service").await.unwrap(), out);
    let again = s.set(ADMIN, "doc_service", out["value"].clone()).await.unwrap();
    assert_eq!(again["value"], out["value"]);
    assert_eq!(again["secrets"], out["secrets"], "a round trip keeps the secret");
    let m = s.set(ADMIN, "safety", json!({"max_diff_file_bytes": 0})).await.unwrap_err().message;
    assert_eq!(m, "invalid safety.max_diff_file_bytes: must be a whole number of bytes above zero");
    let list = s.list().await.unwrap();
    assert_eq!(list["settings"][6]["source"], "database");
    assert_eq!(list["settings"][0]["source"], "default");
    pg.drop_schema().await;
}

#[tokio::test]
async fn e2e_0114_018b_concurrent_sets_on_postgresql_are_both_recorded() {
    let Some(pg) = Pg::new().await else { return };
    let s = pg.settings(ServerConfig::default());
    // Seeded first, so both writers update a row that exists.
    s.set(ADMIN, "safety", json!({"max_read_lines": 5})).await.unwrap();
    let (a, b) = tokio::join!(
        s.set(ADMIN, "safety", json!({"max_read_lines": 11})),
        s.set("other@example.com", "safety", json!({"max_read_lines": 22})),
    );
    a.unwrap();
    b.unwrap();
    let v = s.get("safety").await.unwrap()["value"]["max_read_lines"].as_i64().unwrap();
    assert!(v == 11 || v == 22);
    let n = pg
        .db
        .query_opt(&Query::new("SELECT COUNT(*) FROM setting_change WHERE target_kind='setting'"))
        .await
        .unwrap()
        .unwrap()
        .i64(0)
        .unwrap();
    assert_eq!(n, 3, "the first set and both concurrent ones");
    pg.drop_schema().await;
}

#[tokio::test]
async fn e2e_0114_018c_the_boot_overlay_on_postgresql() {
    let Some(pg) = Pg::new().await else { return };
    // A YAML start seeds the sections, secret encrypted.
    let mut yaml = ServerConfig { from_yaml_file: true, ..ServerConfig::default() };
    yaml.safety.max_read_lines = 700;
    yaml.doc_service.api.auth_token = crate::config::Secret::new(TOKEN);
    let first = apply(yaml.clone(), pg.admin.as_ref(), Some(KEY)).await.unwrap();
    assert_eq!(first.safety.max_read_lines, 700);
    pg.settings(first).set(ADMIN, "safety", json!({"max_read_lines": 9})).await.unwrap();
    // A later start from an edited file: the database wins, the secret is read back.
    yaml.safety.max_read_lines = 900;
    let second = apply(yaml.clone(), pg.admin.as_ref(), Some(KEY)).await.unwrap();
    assert_eq!(second.safety.max_read_lines, 9);
    assert_eq!(second.doc_service.api.auth_token.expose(), TOKEN);
    // No key, or another key: the start stops naming the section.
    for key in [None, Some([8u8; 32])] {
        let e = apply(yaml.clone(), pg.admin.as_ref(), key).await.err().unwrap();
        assert!(
            e.to_string().starts_with("the stored setting 'doc_service' holds a secret"),
            "{e}"
        );
    }
    pg.drop_schema().await;
}
