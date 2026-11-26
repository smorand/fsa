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

// ---- SPEC-0109: reads by vector -------------------------------------------

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    (dot / (na * nb)).clamp(0.0, 1.0)
}

/// The contract of SPEC-0109 FR-NEW-003 and 009 on any engine: embed an input at
/// the project width, nearest entries of a kind, similarity of given entries.
async fn contract_0109(b: &dyn SearchBackend, t: &PerProject, stub: &ModelStub) {
    use crate::errors::code;
    stub.set(crate::settings::vectors_e2e::Answer::Keyed);
    t.set("p", "mk", 8);
    t.set("q", "mk", 8);
    let go = || true;
    b.index_chunks_checked(
        "p",
        "/a.md",
        &chunks(&["ocelot sapphire", "gazelle granite", "ocelot ocelot violet"]),
        &go,
    )
    .await
    .unwrap();
    b.index_chunks_checked("p", "/b.md", &chunks(&["meadow harbor"]), &go).await.unwrap();
    b.index_chunks_checked("q", "/a.md", &chunks(&["ocelot"]), &go).await.unwrap();

    let v = b.embed_input("p", VectorKind::Text, "ocelot").await.unwrap();
    assert_eq!(v.len(), 8);
    let near = b.nearest("p", VectorKind::Text, &v, 50).await.unwrap();
    assert_eq!(near.len(), 4, "only the rows of the project");
    // Every similarity is the cosine of the stored vector, best first, ties by path then number.
    let page = b.vectors_of("p", "/a.md", VectorKind::Text, 0, 10).await.unwrap();
    for hit in near.iter().filter(|h| h.path == "/a.md") {
        let stored = &page.vectors.iter().find(|s| s.number == hit.number).unwrap().vector;
        assert!((hit.similarity - cosine(&v, stored)).abs() < 1e-4, "{hit:?}");
        let stored_text = &page.vectors.iter().find(|s| s.number == hit.number).unwrap().text;
        assert_eq!(&hit.text, stored_text);
    }
    assert!(near.windows(2).all(|w| w[0].similarity >= w[1].similarity), "{near:?}");
    assert_eq!((near[0].path.as_str(), near[0].number), ("/a.md", 3), "{near:?}");
    assert!(near.iter().all(|h| (0.0..=1.0).contains(&h.similarity)));
    let two = b.nearest("p", VectorKind::Text, &v, 2).await.unwrap();
    assert_eq!(two, near[..2].to_vec());

    // The similarity of given entries, in the order asked, None for an absent one.
    let asked = [("/a.md".to_string(), 2), ("/zz.md".to_string(), 1), ("/b.md".to_string(), 1)];
    let sims = b.similarities("p", VectorKind::Text, &v, &asked).await.unwrap();
    assert_eq!(sims.len(), 3);
    assert!(sims[0].is_some() && sims[1].is_none() && sims[2].is_some(), "{sims:?}");
    let by_nearest = near.iter().find(|h| h.path == "/b.md").unwrap().similarity;
    assert!((sims[2].unwrap() - by_nearest).abs() < 1e-5);

    // A width with no store finds nothing and changes nothing.
    let other = b.nearest("p", VectorKind::Text, &[1.0, 2.0, 3.0], 50).await.unwrap();
    assert!(other.is_empty());
    assert_eq!(b.vectors_of("p", "/a.md", VectorKind::Text, 0, 10).await.unwrap(), page);

    // Images: their model, their store; a text input goes to the image model as is.
    let e = b.embed_input("p", VectorKind::Image, "ocelot").await.unwrap_err();
    assert_eq!(e.code, code::NOT_SUPPORTED);
    assert!(e.to_string().contains("the project has no image model"), "{e}");
    t.image.lock().unwrap().insert("p".into(), ("mk".into(), 8));
    let uri = |s: &str| {
        format!(
            "data:image/png;base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, s)
        )
    };
    let imgs = vec![
        ImageInput { number: 1, text: "a gazelle".into(), data_uri: uri("gazelle gazelle") },
        ImageInput { number: 2, text: "an ocelot".into(), data_uri: uri("ocelot") },
    ];
    b.index_images_checked("p", "/a.md", &imgs, &go).await.unwrap();
    let iv = b.embed_input("p", VectorKind::Image, &uri("ocelot")).await.unwrap();
    let inear = b.nearest("p", VectorKind::Image, &iv, 50).await.unwrap();
    assert_eq!(
        (inear[0].path.as_str(), inear[0].number, inear[0].text.as_str()),
        ("/a.md", 2, "an ocelot")
    );
    assert!(inear[0].similarity > 0.99, "{inear:?}");
    // Text and images never meet: the text store has no image row.
    assert_eq!(b.nearest("p", VectorKind::Text, &v, 50).await.unwrap().len(), 4);

    // An unusable answer is refused, nothing is written.
    stub.set(crate::settings::vectors_e2e::Answer::Zero);
    let e = b.embed_input("p", VectorKind::Text, "ocelot").await.unwrap_err();
    assert!(e.to_string().contains("embedding is a zero vector"), "{e}");
    stub.set(crate::settings::vectors_e2e::Answer::Keyed);
    assert_eq!(b.vectors_of("p", "/a.md", VectorKind::Text, 0, 10).await.unwrap(), page);
}

async fn world_keyed() -> (Arc<PerProject>, ModelStub) {
    let stub = ModelStub::start(&[("m3", 3), ("mk", 8)]).await;
    let t = Arc::new(PerProject {
        url: format!("{}/v1/embeddings", stub.base),
        text: Default::default(),
        image: Default::default(),
    });
    (t, stub)
}

/// E2E-017: the reads by vector on sqlite-vec.
#[tokio::test]
async fn e2e_0109_017_reads_by_vector_on_sqlite() {
    crate::storage::sqlite::register_sqlite_vec();
    let (t, stub) = world_keyed().await;
    let dir = tempfile::tempdir().unwrap();
    let b = crate::search::vector_sqlite::SqliteVecBackend::new(
        3,
        dir.path().to_str().unwrap(),
        Arc::new(reqwest::Client::new()),
        t.clone(),
    );
    contract_0109(&b, &t, &stub).await;
}

/// E2E-017: the same contract on pgvector.
#[tokio::test]
async fn e2e_0109_017_reads_by_vector_on_postgres() {
    let (t, stub) = world_keyed().await;
    let schema = unique_schema();
    let Some((b, pool)) = pg(&schema, t.clone()).await else { return };
    contract_0109(&b, &t, &stub).await;
    // A read at a width with no store creates no table (FR-NEW-011).
    use sqlx::Row;
    let tables = || async {
        let rows = sqlx::query(sqlx::AssertSqlSafe(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = current_schema() ORDER BY table_name",
        ))
        .fetch_all(&pool)
        .await
        .unwrap();
        rows.iter().map(|r| r.get::<String, _>("table_name")).collect::<Vec<_>>()
    };
    let before = tables().await;
    let v = vec![0.5f32; 11];
    assert!(b.nearest("p", VectorKind::Text, &v, 5).await.unwrap().is_empty());
    assert!(b.nearest("p", VectorKind::Image, &v, 5).await.unwrap().is_empty());
    let none = b.similarities("p", VectorKind::Text, &v, &[("/a.md".into(), 1)]).await.unwrap();
    assert_eq!(none, vec![None]);
    assert_eq!(tables().await, before, "a read must not create a table");
    drop_schema(&pool, &schema).await;
}
