//! SPEC-0108 tests of the vector stores themselves, one contract run on
//! sqlite-vec and on pgvector: a width per project, images, replacement at
//! another width, a read that never writes. The PostgreSQL runs need
//! `MCPFS_TEST_PG_DSN` (pgvector) and skip without it. Names `e2e_0108_NNN_`.

use crate::errors::Result;
use crate::search::embedding::{EmbedTarget, EmbedTargets, ProjectTarget};
use crate::search::{ImageInput, IndexedChunk, SearchBackend, VectorKind};
use crate::settings::vectors_e2e::ModelStub;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// What each project embeds with: (model name, width), and its image model.
struct PerProject {
    url: String,
    text: Mutex<HashMap<String, (String, u32)>>,
    image: Mutex<HashMap<String, (String, u32)>>,
}

impl PerProject {
    fn set(&self, project: &str, model: &str, dim: u32) {
        self.text.lock().unwrap().insert(project.into(), (model.into(), dim));
    }
    fn target(&self, model: &str) -> EmbedTarget {
        EmbedTarget { url: self.url.clone(), model: model.into(), bearer: None }
    }
}

#[async_trait::async_trait]
impl EmbedTargets for PerProject {
    async fn target(&self) -> Result<EmbedTarget> {
        Ok(self.target("m3"))
    }
    async fn target_for(&self, project: &str) -> Result<ProjectTarget> {
        let found = self.text.lock().unwrap().get(project).cloned();
        Ok(match found {
            Some((m, d)) => ProjectTarget { target: self.target(&m), dimension: Some(d) },
            None => ProjectTarget { target: self.target("m3"), dimension: None },
        })
    }
    async fn image_target_for(&self, project: &str) -> Result<Option<ProjectTarget>> {
        let found = self.image.lock().unwrap().get(project).cloned();
        Ok(found.map(|(m, d)| ProjectTarget { target: self.target(&m), dimension: Some(d) }))
    }
}

async fn world() -> (Arc<PerProject>, ModelStub) {
    let stub = ModelStub::start(&[("m3", 3), ("m5", 5), ("m7", 7), ("img4", 4)]).await;
    let t = Arc::new(PerProject {
        url: format!("{}/v1/embeddings", stub.base),
        text: Default::default(),
        image: Default::default(),
    });
    (t, stub)
}

fn unique_schema() -> String {
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    format!("fsa_v108_{}", &id[..12])
}

fn chunks(texts: &[&str]) -> Vec<IndexedChunk> {
    texts
        .iter()
        .zip(1..)
        .map(|(t, number)| IndexedChunk { number, text: (*t).to_string() })
        .collect()
}

/// The contract of FR-NEW-002, 004, 005, 006 on any engine whose platform width is 3.
async fn contract(b: &dyn SearchBackend, t: &PerProject) {
    t.set("p3", "m3", 3);
    t.set("p5", "m5", 5);
    let go = || true;
    // Two projects, one path, two widths.
    b.index_chunks_checked("p3", "/a.md", &chunks(&["alpha", "beta", "gamma"]), &go).await.unwrap();
    b.index_chunks_checked("p5", "/a.md", &chunks(&["delta"]), &go).await.unwrap();
    let a = b.vectors_of("p3", "/a.md", VectorKind::Text, 0, 10).await.unwrap();
    assert_eq!((a.dimension, a.vectors.len(), a.next_after), (Some(3), 3, None));
    assert!(a.vectors.iter().all(|v| v.vector.len() == 3));
    assert_eq!(a.vectors[1].text, "beta");
    let d = b.vectors_of("p5", "/a.md", VectorKind::Text, 0, 10).await.unwrap();
    assert_eq!((d.dimension, d.vectors.len()), (Some(5), 1));
    assert_eq!(d.vectors[0].vector.len(), 5);
    // Paging.
    let first = b.vectors_of("p3", "/a.md", VectorKind::Text, 0, 2).await.unwrap();
    assert_eq!((first.vectors.len(), first.next_after), (2, Some(2)));
    let rest = b.vectors_of("p3", "/a.md", VectorKind::Text, 2, 2).await.unwrap();
    assert_eq!((rest.vectors.len(), rest.next_after), (1, None));
    // Each project queries its own width and only its own rows.
    let r3 = b.query_vector("p3", "x", 10).await.unwrap();
    assert_eq!(r3.len(), 3);
    let r5 = b.query_vector("p5", "x", 10).await.unwrap();
    assert_eq!(r5.len(), 1);
    assert_eq!(b.stats("p3").await.unwrap().vector_chunks, 3);
    assert_eq!(b.stats("p5").await.unwrap().vector_chunks, 1);
    // A file with no vector, and a file of another project.
    let none = b.vectors_of("p3", "/none.md", VectorKind::Text, 0, 10).await.unwrap();
    assert_eq!((none.dimension, none.vectors.len()), (None, 0));
    assert_eq!(
        b.vectors_of("p3", "/a.md", VectorKind::Image, 0, 10).await.unwrap().vectors.len(),
        0
    );

    // A query at a width with no store finds nothing and changes nothing.
    t.set("p3", "m7", 7);
    assert!(b.query_vector("p3", "x", 10).await.unwrap().is_empty());
    assert_eq!(b.vectors_of("p3", "/a.md", VectorKind::Text, 0, 10).await.unwrap(), a);
    // Writing at the new width replaces every older vector of the path.
    b.index_chunks_checked("p3", "/a.md", &chunks(&["alpha two"]), &go).await.unwrap();
    let now = b.vectors_of("p3", "/a.md", VectorKind::Text, 0, 10).await.unwrap();
    assert_eq!((now.dimension, now.vectors.len()), (Some(7), 1));
    assert_eq!(b.stats("p3").await.unwrap().vector_chunks, 1, "nothing left at width 3");
    assert_eq!(b.stats("p5").await.unwrap().vector_chunks, 1, "another project is untouched");

    // A refused guard stores nothing.
    let refused = b.index_chunks_checked("p3", "/a.md", &chunks(&["x"]), &|| false).await.unwrap();
    assert_eq!(refused, None);
    assert_eq!(b.vectors_of("p3", "/a.md", VectorKind::Text, 0, 10).await.unwrap(), now);

    // Images: their own width, replaced as a set, cleared by an empty list.
    let img = |n: u32| ImageInput {
        number: n,
        text: format!("cap {n}"),
        data_uri: "data:image/png;base64,AAAA".into(),
    };
    assert_eq!(
        b.index_images_checked("p3", "/a.md", &[img(1)], &go).await.unwrap(),
        Some(0),
        "no image model, no vector"
    );
    t.image.lock().unwrap().insert("p3".into(), ("img4".into(), 4));
    assert_eq!(
        b.index_images_checked("p3", "/a.md", &[img(1), img(2)], &go).await.unwrap(),
        Some(2)
    );
    let i = b.vectors_of("p3", "/a.md", VectorKind::Image, 0, 10).await.unwrap();
    assert_eq!((i.dimension, i.vectors.len()), (Some(4), 2));
    assert_eq!(i.vectors[1].text, "cap 2");
    assert_eq!(b.stats("p3").await.unwrap().vector_chunks, 1, "images are not text chunks");
    assert_eq!(b.index_images_checked("p3", "/a.md", &[img(3)], &go).await.unwrap(), Some(1));
    let i = b.vectors_of("p3", "/a.md", VectorKind::Image, 0, 10).await.unwrap();
    assert_eq!(i.vectors.iter().map(|v| v.number).collect::<Vec<_>>(), [3]);

    // Delete clears text and images of the path; delete_all the whole volume.
    assert_eq!(b.delete_path("p3", "/a.md").await.unwrap(), 1);
    assert!(b.vectors_of("p3", "/a.md", VectorKind::Text, 0, 10).await.unwrap().vectors.is_empty());
    assert!(
        b.vectors_of("p3", "/a.md", VectorKind::Image, 0, 10).await.unwrap().vectors.is_empty()
    );
    assert_eq!(b.stats("p5").await.unwrap().vector_chunks, 1);
    assert_eq!(b.delete_all("p5").await.unwrap(), 1);
    assert_eq!(b.stats("p5").await.unwrap().vector_chunks, 0);
}

/// E2E-013: the stores agree on sqlite-vec.
#[tokio::test]
async fn e2e_0108_013_the_stores_agree_on_sqlite() {
    crate::storage::sqlite::register_sqlite_vec();
    let (t, _stub) = world().await;
    let dir = tempfile::tempdir().unwrap();
    let b = crate::search::vector_sqlite::SqliteVecBackend::new(
        3,
        dir.path().to_str().unwrap(),
        Arc::new(reqwest::Client::new()),
        t.clone(),
    );
    contract(&b, &t).await;
}

/// A pgvector backend on a throwaway schema, or `None` without a DSN.
async fn pg(
    schema: &str,
    t: Arc<PerProject>,
) -> Option<(crate::search::vector_pg::PostgresVectorBackend, sqlx::PgPool)> {
    let dsn = std::env::var("MCPFS_TEST_PG_DSN").ok().filter(|v| !v.trim().is_empty())?;
    use crate::storage::rel::{PoolSettings, PostgresRelationalDb};
    let db = PostgresRelationalDb::connect(&dsn, schema, PoolSettings::default())
        .await
        .expect("MCPFS_TEST_PG_DSN is set but connection failed");
    let pool = db.pool().clone();
    let b = crate::search::vector_pg::PostgresVectorBackend::new(
        pool.clone(),
        3,
        Arc::new(reqwest::Client::new()),
        t,
    )
    .await
    .unwrap();
    Some((b, pool))
}

async fn drop_schema(pool: &sqlx::PgPool, schema: &str) {
    let sql = format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE");
    let _ = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await;
}

/// E2E-013: the same contract on pgvector.
#[tokio::test]
async fn e2e_0108_013_the_stores_agree_on_postgres() {
    let (t, _stub) = world().await;
    let schema = unique_schema();
    let Some((b, pool)) = pg(&schema, t.clone()).await else { return };
    contract(&b, &t).await;
    drop_schema(&pool, &schema).await;
}

/// E2E-018: dimensions 384 and 1024 coexist on PostgreSQL, the platform width keeps
/// the legacy table, a width above the index limit is stored and read, and a table
/// name is only ever built from an integer.
#[tokio::test]
async fn e2e_0108_018_widths_coexist_on_postgres() {
    use sqlx::Row;
    let stub = ModelStub::start(&[("m384", 384), ("m1024", 1024), ("m2048", 2048)]).await;
    let t = Arc::new(PerProject {
        url: format!("{}/v1/embeddings", stub.base),
        text: Default::default(),
        image: Default::default(),
    });
    let schema = unique_schema();
    let dsn_set = std::env::var("MCPFS_TEST_PG_DSN").ok().filter(|v| !v.trim().is_empty());
    if dsn_set.is_none() {
        return;
    }
    // Platform width 1024 here: build the backend by hand.
    use crate::storage::rel::{PoolSettings, PostgresRelationalDb};
    let db = PostgresRelationalDb::connect(&dsn_set.unwrap(), &schema, PoolSettings::default())
        .await
        .unwrap();
    let pool = db.pool().clone();
    let b = crate::search::vector_pg::PostgresVectorBackend::new(
        pool.clone(),
        1024,
        Arc::new(reqwest::Client::new()),
        t.clone(),
    )
    .await
    .unwrap();
    for (p, m, d) in [("a", "m384", 384), ("b", "m1024", 1024), ("c", "m2048", 2048)] {
        t.set(p, m, d);
        b.index_chunks_checked(p, "/f.md", &chunks(&["some text"]), &|| true).await.unwrap();
        let page = b.vectors_of(p, "/f.md", VectorKind::Text, 0, 5).await.unwrap();
        assert_eq!(page.dimension, Some(d), "{p}");
        assert_eq!(page.vectors[0].vector.len(), d as usize, "{p}");
        assert_eq!(b.query_vector(p, "q", 5).await.unwrap().len(), 1, "{p}");
    }
    let rows = sqlx::query(sqlx::AssertSqlSafe(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = current_schema() \
         AND table_name LIKE 'search\\_chunks%' ORDER BY table_name",
    ))
    .fetch_all(&pool)
    .await
    .unwrap();
    let names: Vec<String> = rows.iter().map(|r| r.get("table_name")).collect();
    assert_eq!(names, ["search_chunks", "search_chunks_2048", "search_chunks_384"]);
    // The width above 2000 has no ivfflat index; the others do.
    let idx = sqlx::query(sqlx::AssertSqlSafe(
        "SELECT indexname FROM pg_indexes WHERE schemaname = current_schema() \
         AND indexname LIKE 'idx_search%' ORDER BY indexname",
    ))
    .fetch_all(&pool)
    .await
    .unwrap();
    let idx: Vec<String> = idx.iter().map(|r| r.get("indexname")).collect();
    assert_eq!(idx, ["idx_search_chunks_384_emb", "idx_search_chunks_emb"]);
    drop_schema(&pool, &schema).await;
}
