//! SPEC-0108 E2E-019: the profile of SPEC-0107 and the marker rules of SPEC-0108
//! on a real PostgreSQL admin store, through the registry. They need
//! `MCPFS_TEST_PG_DSN` (pgvector or plain PostgreSQL) and skip without it.

use super::{ModelInput, ProfileChange, ProviderInput, Registry, SecretText};
use crate::chunking::ChunkDefaults;
use crate::errors::Result;
use crate::storage::admin::RelationalAdminStore;
use crate::storage::rel::{PoolSettings, PostgresRelationalDb, Query, RelationalDb};
use crate::storage::traits::AdminBackend;
use std::sync::Arc;

const ADMIN: &str = "admin@example.com";
const BOB: &str = "bob@example.com";

struct Pg {
    reg: Registry,
    db: Arc<PostgresRelationalDb>,
    schema: String,
}

impl Pg {
    async fn new() -> Option<Self> {
        let dsn = std::env::var("MCPFS_TEST_PG_DSN").ok().filter(|v| !v.trim().is_empty())?;
        let id = uuid::Uuid::new_v4().to_string().replace('-', "");
        let schema = format!("fsa_p108_{}", &id[..12]);
        let db = Arc::new(
            PostgresRelationalDb::connect(&dsn, &schema, PoolSettings::default())
                .await
                .expect("MCPFS_TEST_PG_DSN is set but connection failed"),
        );
        let admin = Arc::new(RelationalAdminStore::new(db.clone()));
        admin.connect().await.unwrap();
        admin.set_master_key(Some([7u8; 32]));
        admin.create_project("proj", BOB).await.unwrap();
        admin.create_project("other", BOB).await.unwrap();
        let reg = Registry::of(admin.as_ref(), 1024).unwrap();
        Some(Self { reg, db, schema })
    }

    async fn model(&self, name: &str, kind: &str, dim: i64) -> String {
        let p = self
            .reg
            .create_provider(
                ADMIN,
                ProviderInput {
                    name: format!("prov-{name}"),
                    endpoint: "http://localhost:9010/v1".into(),
                    auth_scheme: "none".into(),
                    secret: SecretText::new(String::new()),
                },
            )
            .await
            .unwrap();
        self.reg
            .create_model(
                ADMIN,
                ModelInput {
                    provider_id: p.provider_id,
                    name: name.into(),
                    kind: kind.into(),
                    dimension: Some(dim),
                    max_input_tokens: None,
                },
            )
            .await
            .unwrap()
            .model_id
    }

    async fn set(&self, project: &str, c: ProfileChange) -> Result<super::ProfileRow> {
        self.reg.set_project_profile(BOB, project, c, &ChunkDefaults::default()).await
    }

    async fn clear_markers(&self) {
        self.db.execute(&Query::new("UPDATE project SET vectors_stale_at=NULL")).await.unwrap();
    }

    async fn drop_schema(self) {
        let sql = format!("DROP SCHEMA IF EXISTS \"{}\" CASCADE", self.schema);
        let _ = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(self.db.pool()).await;
    }
}

fn models(text: &str, image: &str) -> ProfileChange {
    ProfileChange {
        text_model_id: Some(text.into()),
        image_model_id: Some(image.into()),
        ..Default::default()
    }
}

/// The profile round trip, the marker, the record and the clearing, on PostgreSQL.
#[tokio::test]
async fn e2e_0108_019_the_profile_and_its_marker_on_postgres() {
    let Some(pg) = Pg::new().await else { return };
    let t = pg.model("t384", "text_embedding", 384).await;
    let i = pg.model("i8", "image_embedding", 8).await;
    let row = pg.set("proj", models(&t, &i)).await.unwrap();
    assert_eq!(row.text_model_id.as_deref(), Some(t.as_str()));
    assert!(row.vectors_stale_at.is_some());
    assert_eq!(pg.reg.project_profile("proj").await.unwrap(), row);
    assert_eq!(pg.reg.project_profile("other").await.unwrap(), Default::default());

    // The same models again change and record nothing.
    let lines = pg.reg.list_changes_of_kinds(&["project_profile"], 50).await.unwrap().len();
    assert_eq!(pg.set("proj", models(&t, &i)).await.unwrap(), row);
    assert_eq!(pg.reg.list_changes_of_kinds(&["project_profile"], 50).await.unwrap().len(), lines);

    // A used model refuses its deletion; a dimension edit marks its project only.
    let e = pg.reg.delete_model(ADMIN, &t).await.unwrap_err();
    assert!(e.to_string().contains("is used by 1 project"), "{e}");
    pg.clear_markers().await;
    pg.reg
        .update_model(ADMIN, &t, super::ModelUpdate { dimension: Some(400), ..Default::default() })
        .await
        .unwrap();
    assert!(pg.reg.project_profile("proj").await.unwrap().vectors_stale_at.is_some());
    assert!(pg.reg.project_profile("other").await.unwrap().vectors_stale_at.is_none());

    // Clearing the field lets the model go.
    let cleared = pg
        .set("proj", ProfileChange { text_model_id: Some(String::new()), ..Default::default() })
        .await
        .unwrap();
    assert_eq!(cleared.text_model_id, None);
    pg.reg.delete_model(ADMIN, &t).await.unwrap();
    pg.drop_schema().await;
}

/// Two concurrent sets of different fields both survive on PostgreSQL, and two
/// sets of the same fields leave one whole request.
#[tokio::test]
async fn e2e_0108_019_concurrent_sets_on_postgres() {
    let Some(pg) = Pg::new().await else { return };
    let (t1, t2) =
        (pg.model("t1", "text_embedding", 16).await, pg.model("t2", "text_embedding", 32).await);
    let (i1, i2) =
        (pg.model("i1", "image_embedding", 8).await, pg.model("i2", "image_embedding", 12).await);
    for round in 0..10 {
        let a = pg.set(
            "proj",
            ProfileChange { chunk_strategy: Some("fixed".into()), ..Default::default() },
        );
        let b =
            pg.set("proj", ProfileChange { text_model_id: Some(t1.clone()), ..Default::default() });
        let (a, b) = tokio::join!(a, b);
        a.unwrap_or_else(|e| panic!("round {round}: {e}"));
        b.unwrap_or_else(|e| panic!("round {round}: {e}"));
        let row = pg.reg.project_profile("proj").await.unwrap();
        assert_eq!(row.chunk_strategy.as_deref(), Some("fixed"), "round {round}");
        assert_eq!(row.text_model_id.as_deref(), Some(t1.as_str()), "round {round}");
        pg.set(
            "proj",
            ProfileChange {
                chunk_strategy: Some(String::new()),
                text_model_id: Some(String::new()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }
    let (x, y) = tokio::join!(pg.set("other", models(&t1, &i1)), pg.set("other", models(&t2, &i2)));
    x.unwrap();
    y.unwrap();
    let row = pg.reg.project_profile("other").await.unwrap();
    let pair = (row.text_model_id.as_deref(), row.image_model_id.as_deref());
    assert!(
        pair == (Some(t1.as_str()), Some(i1.as_str()))
            || pair == (Some(t2.as_str()), Some(i2.as_str())),
        "a mix of two requests: {pair:?}"
    );
    pg.drop_schema().await;
}
