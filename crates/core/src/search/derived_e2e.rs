//! SPEC-0100 acceptance tests: derived data rebuilds after the file settles.
//!
//! Every test drives the real `McpServer` handlers, the path a client calls, on
//! [`Env`]: an in memory admin store (which also holds the `derived_job` table),
//! a local volume and blob store on a temp dir, and a Tantivy BM25 backend when
//! search is enabled. Tests run on a paused tokio clock
//! (`#[tokio::test(start_paused = true)]`), so later stories step the settle
//! delay instead of sleeping. Names are `e2e_0100_<id>_<slug>`, so
//! `cargo test -p fsa-core --lib -- e2e_0100_` runs the whole spec.

use crate::config::ServerConfig;
use crate::mcp::server::McpServer;
use crate::safety::SafetyManager;
use crate::search::SearchBackend;
use crate::search::derived::{Job, JobCause, JobKind, JobState, JobStore};
use crate::state::AppState;
use crate::storage::traits::{AdminBackend, IndexMode};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

pub(crate) const ALICE: &str = "alice@example.com";
pub(crate) const DAVE: &str = "dave@example.com";
/// Alice owns it.
pub(crate) const PROJ: &str = "proj";
/// Dave owns it; Alice is not a member.
pub(crate) const OTHER: &str = "other";

/// What a test changes from the default environment.
#[derive(Clone)]
pub(crate) struct Options {
    /// `search.enabled`; a Tantivy BM25 backend behind [`Stub`] is wired when true.
    pub search: bool,
    /// Wrap the engine in the scheduler, as `app.rs` does. Off only for the
    /// tests that seed waiting and running rows straight into the table.
    pub scheduled: bool,
    /// `search.settle_delay_seconds`.
    pub delay: u64,
    /// A configured embedding endpoint, which `admin.set_index_mode` demands
    /// for `rag` and `both`. Never called: the engine is the in process stub.
    pub embedding: bool,
    /// Any other change to the server configuration, applied last.
    pub config: fn(&mut ServerConfig),
    /// Admin store and volume metadata on a fresh PostgreSQL schema of
    /// `MCPFS_TEST_PG_DSN` (SPEC-0112 E2E-018); the test checks the variable
    /// first and calls [`Env::drop_schema`] at its end.
    pub pg: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            search: true,
            scheduled: true,
            delay: 30,
            embedding: false,
            config: |_| {},
            pg: false,
        }
    }
}

/// The embedding service seen by a rebuild, in process: a rebuild (an index
/// call made while the scheduler runs a job of that path) first waits the
/// service delay and is recorded; any index call fails with the text of
/// `search::embedding::embed` when the service is down, else stores in
/// Tantivy. Other index calls (full index, move, `search.index`) never wait,
/// as they did when they went through the unguarded `index_path`. Process local rather than an HTTP server, because the default
/// build has no embedding client (feature `rag`) and real sockets let a paused
/// clock jump while a request is in flight. Seeding goes through the inner
/// engine and never waits.
pub(crate) struct Stub {
    inner: crate::search::bm25_sqlite::TantivyBm25Backend,
    /// Delays of the next rebuilds, in order; then `delay`.
    queue: std::sync::Mutex<std::collections::VecDeque<u64>>,
    delay: std::sync::atomic::AtomicU64,
    down: std::sync::atomic::AtomicBool,
    running: std::sync::atomic::AtomicUsize,
    max_running: std::sync::atomic::AtomicUsize,
    /// (path, instant) of every rebuild start and end.
    starts: std::sync::Mutex<Vec<(String, tokio::time::Instant)>>,
    ends: std::sync::Mutex<Vec<(String, tokio::time::Instant)>>,
    /// The scheduler whose running jobs mark an index call as a rebuild.
    scheduler: std::sync::Mutex<std::sync::Weak<crate::search::derived::Scheduler>>,
}

impl Stub {
    fn new(dir: &str) -> Self {
        Self {
            inner: crate::search::bm25_sqlite::TantivyBm25Backend::new(dir),
            queue: Default::default(),
            delay: Default::default(),
            down: Default::default(),
            running: Default::default(),
            max_running: Default::default(),
            starts: Default::default(),
            ends: Default::default(),
            scheduler: Default::default(),
        }
    }
    fn watch(&self, scheduler: &Arc<crate::search::derived::Scheduler>) {
        *self.scheduler.lock().unwrap() = Arc::downgrade(scheduler);
    }
    /// True when the scheduler runs a rebuild of `path` right now.
    async fn rebuilding(&self, volume_id: &str, path: &str) -> bool {
        let scheduler = self.scheduler.lock().unwrap().upgrade();
        match scheduler {
            Some(s) => s
                .list(volume_id, &|p| p == path)
                .await
                .iter()
                .any(|l| l.job.state == JobState::Running),
            None => false,
        }
    }
    pub fn set_delay(&self, secs: u64) {
        self.delay.store(secs, Ordering::SeqCst);
    }
    pub fn then_delays(&self, secs: &[u64]) {
        self.queue.lock().unwrap().extend(secs);
    }
    pub fn set_down(&self, down: bool) {
        self.down.store(down, Ordering::SeqCst);
    }
    pub fn starts(&self) -> Vec<(String, tokio::time::Instant)> {
        self.starts.lock().unwrap().clone()
    }
    pub fn ends(&self) -> Vec<(String, tokio::time::Instant)> {
        self.ends.lock().unwrap().clone()
    }
    pub fn max_running(&self) -> usize {
        self.max_running.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl SearchBackend for Stub {
    async fn index_chunks_checked(
        &self,
        volume_id: &str,
        path: &str,
        chunks: &[crate::search::IndexedChunk],
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> crate::errors::Result<Option<usize>> {
        if !self.rebuilding(volume_id, path).await {
            if self.down.load(Ordering::SeqCst) {
                return Err(crate::errors::ToolError::internal(
                    "embedding request failed: error sending request: connection refused",
                ));
            }
            return self.inner.index_chunks_checked(volume_id, path, chunks, still_current).await;
        }
        self.starts.lock().unwrap().push((path.to_string(), tokio::time::Instant::now()));
        let n = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_running.fetch_max(n, Ordering::SeqCst);
        let secs = self.queue.lock().unwrap().pop_front();
        let secs = secs.unwrap_or_else(|| self.delay.load(Ordering::SeqCst));
        tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
        let out = if self.down.load(Ordering::SeqCst) {
            Err(crate::errors::ToolError::internal(
                "embedding request failed: error sending request: connection refused",
            ))
        } else {
            self.inner.index_chunks_checked(volume_id, path, chunks, still_current).await
        };
        self.running.fetch_sub(1, Ordering::SeqCst);
        self.ends.lock().unwrap().push((path.to_string(), tokio::time::Instant::now()));
        out
    }

    async fn delete_path(&self, volume_id: &str, path: &str) -> crate::errors::Result<usize> {
        self.inner.delete_path(volume_id, path).await
    }

    async fn delete_all(&self, volume_id: &str) -> crate::errors::Result<usize> {
        self.inner.delete_all(volume_id).await
    }

    async fn query_bm25(
        &self,
        volume_id: &str,
        query: &str,
        top_k: usize,
    ) -> crate::errors::Result<Vec<crate::search::SearchResult>> {
        self.inner.query_bm25(volume_id, query, top_k).await
    }

    async fn query_vector(
        &self,
        volume_id: &str,
        query: &str,
        top_k: usize,
    ) -> crate::errors::Result<Vec<crate::search::SearchResult>> {
        self.inner.query_vector(volume_id, query, top_k).await
    }

    async fn stats(&self, volume_id: &str) -> crate::errors::Result<crate::search::IndexStats> {
        self.inner.stats(volume_id).await
    }

    fn supported_modes(&self) -> Vec<&'static str> {
        self.inner.supported_modes()
    }
}

#[cfg(feature = "postgres")]
async fn pg_admin(schema: &str) -> Arc<crate::storage::admin::RelationalAdminStore> {
    use crate::storage::rel::{PoolSettings, PostgresRelationalDb};
    let dsn = std::env::var("MCPFS_TEST_PG_DSN").unwrap();
    let db = Arc::new(
        PostgresRelationalDb::connect(&dsn, schema, PoolSettings::default())
            .await
            .expect("MCPFS_TEST_PG_DSN is set but connection failed"),
    );
    let admin = Arc::new(crate::storage::admin::RelationalAdminStore::new(db));
    admin.connect().await.unwrap();
    admin
}

#[cfg(not(feature = "postgres"))]
async fn pg_admin(_schema: &str) -> Arc<crate::storage::admin::RelationalAdminStore> {
    panic!("a pg Env needs the postgres feature");
}

/// One server: `proj` owned by Alice, `other` owned by Dave.
pub(crate) struct Env {
    _dir: tempfile::TempDir,
    pub state: Arc<AppState>,
    /// The same database the admin store uses, so seeded rows obey the
    /// cascade from `project`.
    pub jobs: JobStore,
    /// The engine behind the scheduler, `None` when search is off.
    pub stub: Option<Arc<Stub>>,
    /// Time 0 of the test, the reference of [`Env::at`].
    pub t0: tokio::time::Instant,
    /// The PostgreSQL schema of a `pg` environment.
    pub pg_schema: Option<String>,
}

impl Env {
    pub async fn new(opts: Options) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut config = ServerConfig::default();
        config.infra.meta.dir = dir.path().join("volumes").display().to_string();
        config.infra.blob.dir = dir.path().join("blobs").display().to_string();
        config.auth.jwt.public_key_path = String::new();
        config.search.settle_delay_seconds = opts.delay.to_string();
        if opts.embedding {
            config.search.embedding.endpoint = "http://embedding.invalid/v1".into();
        }
        let stub = if opts.search {
            let tantivy = dir.path().join("tantivy");
            config.search.enabled = true;
            config.search.mode = "bm25".into();
            config.search.tantivy_dir = tantivy.display().to_string();
            Some(Arc::new(Stub::new(tantivy.to_str().expect("temp dir has a utf-8 path"))))
        } else {
            config.search.enabled = false;
            None
        };
        let mut pg_schema = None;
        if opts.pg {
            let dsn = std::env::var("MCPFS_TEST_PG_DSN").expect("a pg Env needs MCPFS_TEST_PG_DSN");
            let id = uuid::Uuid::new_v4().to_string().replace('-', "");
            let schema = format!("fsa_p112_{}", &id[..12]);
            config.infra.meta.backend = crate::config::backend::POSTGRES.into();
            config.infra.meta.dsn = crate::config::Dsn::new(dsn.clone());
            config.infra.meta.schema = schema.clone();
            pg_schema = Some(schema);
        }
        (opts.config)(&mut config);
        let config = Arc::new(config);

        let admin = if opts.pg {
            pg_admin(pg_schema.as_deref().unwrap()).await
        } else {
            Arc::new(crate::storage::admin::RelationalAdminStore::in_memory().await.unwrap())
        };
        admin.create_project(PROJ, ALICE).await.unwrap();
        admin.create_project(OTHER, DAVE).await.unwrap();
        let jobs = admin.derived_jobs().expect("the relational admin store holds the jobs");
        let stores = Arc::new(crate::storage::StoreManager::new(
            config.clone(),
            crate::storage::test_registry(),
        ));
        // The same composition as `app.rs`: the engine wrapped in the scheduler.
        let search: Option<Arc<dyn SearchBackend>> = stub.clone().map(|s| {
            let engine: Arc<dyn SearchBackend> = s;
            if opts.scheduled {
                Arc::new(crate::search::ScheduledBackend::new(
                    engine,
                    jobs.clone(),
                    stores.clone(),
                    config.search.settle_delay(),
                    config.search.chunking(),
                )) as Arc<dyn SearchBackend>
            } else {
                engine
            }
        });
        if let Some(scheduler) = search.as_ref().and_then(|s| s.scheduler()) {
            admin.set_project_gone_hook(scheduler.project_hook());
            if let Some(stub) = &stub {
                stub.watch(&scheduler);
            }
        }
        admin.set_project_deleted_hook(crate::chunking::ChunkStore::project_deleted_hook(
            stores.clone(),
        ));
        let safety = Arc::new(SafetyManager::new(config.safety.clone()));
        safety.attach_limits(admin.clone());
        let state = Arc::new(AppState {
            config: config.clone(),
            admin,
            stores,
            safety,
            identity: Arc::new(crate::identity::IdentityResolver::new(&config.auth)),
            editors: Arc::new(crate::tools::editor::EditorRegistry::new()),
            doc_service: None,
            search,
        });
        state.stores.provision_volume(PROJ).await.unwrap();
        state.stores.provision_volume(OTHER).await.unwrap();
        if opts.search {
            state.admin.set_index_mode(PROJ, IndexMode::Bm25).await.unwrap();
        }
        let t0 = tokio::time::Instant::now();
        Self { _dir: dir, state, jobs, stub, t0, pg_schema }
    }

    /// Simulates a server restart: stops the first scheduler (its dispatcher and
    /// its running rebuilds die with the process), then builds a second over the
    /// same rows, engine and volumes and recovers, as `app.rs` does at boot.
    pub async fn restart(&mut self) {
        self.restart_with(|_| {}).await;
    }

    /// A restart after the operator changed the configuration with `change`:
    /// the stores are opened again over the same files with the new values.
    pub async fn restart_with(&mut self, change: fn(&mut ServerConfig)) {
        let old = self.state.search.as_ref().and_then(|b| b.scheduler()).expect("a scheduler");
        old.stop();
        let mut config = (*self.state.config).clone();
        change(&mut config);
        let config = Arc::new(config);
        let stores = Arc::new(crate::storage::StoreManager::new(
            config.clone(),
            crate::storage::test_registry(),
        ));
        let engine: Arc<dyn SearchBackend> = self.stub.clone().expect("search is on");
        let backend = crate::search::ScheduledBackend::new(
            engine,
            self.jobs.clone(),
            stores.clone(),
            config.search.settle_delay(),
            config.search.chunking(),
        );
        let scheduler = backend.scheduler().expect("a scheduler");
        self.stub().watch(&scheduler);
        scheduler.recover().await.expect("recovery succeeds");
        // The hook of the stopped scheduler is replaced.
        self.state.admin.set_project_gone_hook(backend.scheduler().unwrap().project_hook());
        let s = &self.state;
        s.admin.set_project_deleted_hook(crate::chunking::ChunkStore::project_deleted_hook(
            stores.clone(),
        ));
        self.state = Arc::new(AppState {
            config,
            admin: s.admin.clone(),
            stores,
            safety: s.safety.clone(),
            identity: s.identity.clone(),
            editors: s.editors.clone(),
            doc_service: None,
            search: Some(Arc::new(backend) as Arc<dyn SearchBackend>),
        });
    }

    /// Drops the PostgreSQL schema of a `pg` environment.
    #[cfg(feature = "postgres")]
    pub async fn drop_schema(&self) {
        use crate::storage::rel::{PoolSettings, PostgresRelationalDb, Query, RelationalDb as _};
        let Some(schema) = &self.pg_schema else { return };
        let dsn = std::env::var("MCPFS_TEST_PG_DSN").unwrap();
        let db =
            PostgresRelationalDb::connect(&dsn, "public", PoolSettings::default()).await.unwrap();
        let sql = format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE");
        db.execute(&Query::new(sql)).await.unwrap();
    }

    pub fn stub(&self) -> &Stub {
        self.stub.as_deref().expect("search is on")
    }

    /// Waits until `secs` seconds after time 0 of the test.
    pub async fn at(&self, secs: u64) {
        tokio::time::sleep_until(self.t0 + Duration::from_secs(secs)).await;
    }

    /// Seconds since time 0 of the test, as a float.
    pub fn since(&self, t: tokio::time::Instant) -> f64 {
        t.duration_since(self.t0).as_secs_f64()
    }

    /// A file already indexed with `text`, as if an earlier rebuild had ended.
    pub async fn seed(&self, path: &str, text: &str) {
        let client = self.state.stores.client(PROJ).await.unwrap();
        client.write_text_atomic(path, text).await.unwrap();
        // Alice has seen it, so her save passes the read before write guard.
        self.state.safety.record_read(ALICE, PROJ, path);
        // Its default set and entries, as an earlier rebuild left them.
        crate::search::indexer::index_default_set(
            &self.stub().inner,
            &*client.meta,
            PROJ,
            path,
            text.to_string(),
            self.state.config.search.chunking(),
            false,
            &|| true,
        )
        .await
        .unwrap();
    }

    /// `fs.write` by Alice with overwrite; asserts it answers at once and returns
    /// the answer.
    pub async fn save_in(&self, project: &str, path: &str, text: &str) -> Value {
        let started = tokio::time::Instant::now();
        let a =
            args(json!({"mount_id": project, "path": path, "content": text, "overwrite": true}));
        let v = tool_json(self.server(ALICE).fs_write(a).await).expect("the save succeeds");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the save waited {:?}",
            started.elapsed()
        );
        v
    }

    pub async fn save(&self, path: &str, text: &str) -> Value {
        self.save_in(PROJ, path, text).await
    }

    /// The paths and chunks `search.query` (bm25) answers for `word` in `proj`.
    pub async fn found(&self, word: &str) -> Vec<(String, String)> {
        let a = args(json!({"mount_id": PROJ, "query": word, "mode": "bm25", "top_k": 50}));
        let v = tool_json(self.server(ALICE).search_query(a).await).expect("search answers");
        v["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r["path"].as_str().unwrap().into(), r["chunk"].as_str().unwrap().into()))
            .collect()
    }

    /// True when a search for `word` finds `path`.
    pub async fn finds(&self, word: &str, path: &str) -> bool {
        self.found(word).await.iter().any(|(p, _)| p == path)
    }

    /// `search.status` chunk count of `proj`.
    pub async fn chunks(&self) -> u64 {
        let a = args(json!({"mount_id": PROJ}));
        let v = tool_json(self.server(ALICE).search_status(a).await).expect("status answers");
        v["bm25_docs"].as_u64().unwrap_or_else(|| panic!("bm25_docs in {v}"))
    }

    /// The jobs of `path` in `proj` as Alice sees them.
    pub async fn jobs(&self, path: &str) -> Vec<Value> {
        jobs_of(&self.list(ALICE, PROJ, path, "").await.unwrap()).clone()
    }

    /// The start instants, in seconds since time 0, of the rebuilds of `path`.
    pub fn starts_of(&self, path: &str) -> Vec<f64> {
        self.stub()
            .starts()
            .iter()
            .filter(|(p, _)| p == path)
            .map(|(_, t)| self.since(*t))
            .collect()
    }

    pub fn ends_of(&self, path: &str) -> Vec<f64> {
        self.stub().ends().iter().filter(|(p, _)| p == path).map(|(_, t)| self.since(*t)).collect()
    }

    pub fn server(&self, person: &str) -> McpServer {
        McpServer::new(self.state.clone(), person.to_string())
    }

    /// `fs.list_derived_jobs` as `person`: the JSON answer, or the error text.
    pub async fn list(
        &self,
        person: &str,
        mount_id: &str,
        path: &str,
        marker: &str,
    ) -> std::result::Result<Value, String> {
        let a = args(json!({"mount_id": mount_id, "path": path, "marker": marker}));
        tool_json(self.server(person).fs_list_derived_jobs(a).await)
    }

    /// Inserts one job row as the scheduler would have left it. `ended_at` is
    /// set for a finished state; `reason` only matters for `Failed`.
    pub async fn seed_job(
        &self,
        project: &str,
        path: &str,
        state: JobState,
        ended_at: Option<&str>,
        reason: Option<&str>,
    ) -> Job {
        let mut job = Job::new_waiting(project, path, JobKind::SearchIndex, JobCause::Change);
        job.state = state;
        job.ended_at = ended_at.map(str::to_string);
        job.reason = reason.map(str::to_string);
        self.jobs.insert(&job).await.unwrap();
        job
    }
}

/// RFC 3339 UTC seconds, `n` seconds after a fixed instant, so seeded end times
/// are distinct and ordered.
pub(crate) fn at(n: i64) -> String {
    let base = chrono::DateTime::parse_from_rfc3339("2026-11-20T10:00:00Z").unwrap();
    (base + chrono::Duration::seconds(n))
        .with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub(crate) fn args<T: DeserializeOwned>(v: Value) -> Parameters<T> {
    Parameters(serde_json::from_value(v).expect("the arguments must deserialize"))
}

fn text_of(r: &CallToolResult) -> String {
    r.content
        .iter()
        .find_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .expect("a text content block")
}

pub(crate) fn tool_json(
    r: std::result::Result<CallToolResult, rmcp::ErrorData>,
) -> std::result::Result<Value, String> {
    let r = r.expect("a tool result, not a protocol error");
    if r.is_error == Some(true) {
        return Err(text_of(&r));
    }
    Ok(serde_json::from_str(&text_of(&r)).expect("the tool result is JSON"))
}

pub(crate) fn jobs_of(v: &Value) -> &Vec<Value> {
    v["jobs"].as_array().unwrap_or_else(|| panic!("jobs is an array: {v}"))
}

// ── US-0266: job records and fs.list_derived_jobs ───────────────────────────

/// SPEC-0100/E2E-016: no job yet, for the project and for one file.
#[tokio::test(start_paused = true)]
async fn e2e_0100_016_no_job_yet() {
    let env = Env::new(Options::default()).await;
    for path in ["", "/notes.md"] {
        let v = env.list(ALICE, PROJ, path, "").await.unwrap();
        assert_eq!(v, json!({"jobs": []}), "path {path:?}");
    }
}

/// SPEC-0100/E2E-020: 10 000 finished jobs are paged by 100, newest end first,
/// and the first answer arrives in under 2 s.
#[tokio::test(start_paused = true)]
async fn e2e_0100_020_a_long_job_history_is_paged() {
    let env = Env::new(Options::default()).await;
    for i in 0..10_000 {
        let path = format!("/f{i:05}.md");
        env.seed_job(PROJ, &path, JobState::Done, Some(&at(i)), None).await;
    }

    // Wall time, not the paused tokio clock.
    let started = std::time::Instant::now();
    let first = env.list(ALICE, PROJ, "", "").await.unwrap();
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(2), "first page took {took:?}");

    let jobs = jobs_of(&first);
    assert_eq!(jobs.len(), 100);
    let ends: Vec<&str> = jobs.iter().map(|j| j["ended_at"].as_str().unwrap()).collect();
    let expected: Vec<String> = (9_900..10_000).rev().map(at).collect();
    assert_eq!(ends, expected, "newest end first");
    assert_eq!(jobs[0]["path"], "/f09999.md");
    assert_eq!(jobs[0]["state"], "done");
    assert_eq!(jobs[0]["kind"], "search_index");
    assert_eq!(jobs[0]["cause"], "change");
    assert!(jobs[0].get("seconds_left").is_none() && jobs[0].get("reason").is_none());
    let marker = first["marker"].as_str().expect("a marker when more jobs follow");

    let second = env.list(ALICE, PROJ, "", marker).await.unwrap();
    let ends: Vec<&str> =
        jobs_of(&second).iter().map(|j| j["ended_at"].as_str().unwrap()).collect();
    let expected: Vec<String> = (9_800..9_900).rev().map(at).collect();
    assert_eq!(ends, expected, "the next 100");
}

/// SPEC-0100/E2E-027: a non member cannot list jobs; his own project still lists
/// only its own jobs.
#[tokio::test(start_paused = true)]
async fn e2e_0100_027_a_non_member_cannot_list_jobs() {
    // Waiting and running rows seeded straight into the table: no scheduler.
    let env = Env::new(Options { scheduled: false, ..Options::default() }).await;
    env.seed_job(PROJ, "/notes.md", JobState::Done, Some(&at(1)), None).await;
    env.seed_job(PROJ, "/plan.md", JobState::Waiting, None, None).await;
    let mine = env.seed_job(OTHER, "/a.md", JobState::Failed, Some(&at(2)), Some("boom")).await;
    let running = env.seed_job(OTHER, "/b.md", JobState::Running, None, None).await;

    let e = env.list(DAVE, PROJ, "", "").await.unwrap_err();
    assert!(e.ends_with("ERR_FORBIDDEN: 'dave@example.com' is not a member of 'proj'"), "got: {e}");

    let v = env.list(DAVE, OTHER, "", "").await.unwrap();
    let jobs = jobs_of(&v);
    assert_eq!(jobs.len(), 2, "only the jobs of other: {v}");
    assert_eq!(jobs[0]["job_id"], running.job_id.as_str(), "running before finished");
    assert_eq!(jobs[0]["state"], "running");
    assert!(jobs[0].get("ended_at").is_none());
    assert_eq!(jobs[1]["job_id"], mine.job_id.as_str());
    assert_eq!(jobs[1]["state"], "failed");
    assert_eq!(jobs[1]["reason"], "boom");
    assert_eq!(jobs[1]["ended_at"], at(2));
    assert!(v.get("marker").is_none());
}

/// SPEC-0100/E2E-070: with search disabled the list is empty, not an error.
#[tokio::test(start_paused = true)]
async fn e2e_0100_070_listing_jobs_when_search_is_not_enabled() {
    let env = Env::new(Options { search: false, ..Options::default() }).await;
    env.seed_job(PROJ, "/notes.md", JobState::Done, Some(&at(1)), None).await;
    let v = env.list(ALICE, PROJ, "", "").await.unwrap();
    assert_eq!(v, json!({"jobs": []}));
}

/// FR-NEW-010 story checks: a path names a file or the files under a folder,
/// a bad marker is refused with the contract text, an absent project is
/// `ERR_PROJECT_NOT_FOUND`.
#[tokio::test(start_paused = true)]
async fn e2e_0100_us0266_path_scope_marker_and_absent_project() {
    // Waiting and running rows seeded straight into the table: no scheduler.
    let env = Env::new(Options { scheduled: false, ..Options::default() }).await;
    env.seed_job(PROJ, "/notes.md", JobState::Done, Some(&at(1)), None).await;
    env.seed_job(PROJ, "/docs/a.md", JobState::Done, Some(&at(2)), None).await;
    env.seed_job(PROJ, "/docs/sub/b.md", JobState::Waiting, None, None).await;
    env.seed_job(PROJ, "/docs_x.md", JobState::Done, Some(&at(3)), None).await;

    let paths = |v: &Value| -> Vec<String> {
        jobs_of(v).iter().map(|j| j["path"].as_str().unwrap().to_string()).collect()
    };
    let file = env.list(ALICE, PROJ, "/notes.md", "").await.unwrap();
    assert_eq!(paths(&file), ["/notes.md"]);
    let folder = env.list(ALICE, PROJ, "/docs", "").await.unwrap();
    assert_eq!(paths(&folder), ["/docs/sub/b.md", "/docs/a.md"], "waiting first");
    assert!(jobs_of(&folder)[0]["seconds_left"].is_u64(), "waiting carries seconds_left");

    let e = env.list(ALICE, PROJ, "", "nope").await.unwrap_err();
    assert!(e.ends_with("ERR_INVALID_ARGUMENT: marker is not valid"), "got: {e}");

    let e = env.list(ALICE, "ghost", "", "").await.unwrap_err();
    assert!(e.contains("ERR_PROJECT_NOT_FOUND"), "got: {e}");
}

const SETTLE_ERR: &str =
    "ERR_INVALID_ARGUMENT: settle delay must be a whole number of seconds, 0 or more";

fn settle_yaml(enabled: bool, value: &str) -> String {
    format!("search:\n  enabled: {enabled}\n  settle_delay_seconds: {value}\n")
}

fn settle_start(enabled: bool, value: &str) -> Result<u64, String> {
    // `from_yaml` installs the process wide git host map; hold the lock the config tests use.
    let _guard = crate::git::remote::tests::lock_for_test();
    ServerConfig::from_yaml(&settle_yaml(enabled, value))
        .map(|c| c.search.settle_delay())
        .map_err(|e| e.to_string())
}

/// SPEC-0100/E2E-025: negative, fractional and text delays stop the start; absent is 30.
#[test]
fn e2e_0100_025_invalid_settle_delay_stops_start() {
    for bad in ["-5", "1.5", "abc"] {
        let e = settle_start(true, bad).unwrap_err();
        assert!(e.contains(SETTLE_ERR), "{bad}: got {e}");
    }
    let _guard = crate::git::remote::tests::lock_for_test();
    let absent = ServerConfig::from_yaml("search:\n  enabled: true\n").unwrap();
    assert_eq!(absent.search.settle_delay(), 30);
    let empty = ServerConfig::from_yaml("search:\n  settle_delay_seconds: ''\n").unwrap();
    assert_eq!(empty.search.settle_delay(), 30);
}

/// SPEC-0100/E2E-071: the check applies with search disabled.
#[test]
fn e2e_0100_071_invalid_settle_delay_with_search_disabled() {
    let e = settle_start(false, "-5").unwrap_err();
    assert!(e.contains(SETTLE_ERR), "got {e}");
    assert_eq!(settle_start(false, "10"), Ok(10));
}

/// SPEC-0100/E2E-074: 2.5 and abc are refused with the spec text, 0 is valid.
#[test]
fn e2e_0100_074_settle_delay_not_a_whole_number() {
    for bad in ["2.5", "abc"] {
        let e = settle_start(true, bad).unwrap_err();
        assert!(e.contains(SETTLE_ERR), "{bad}: got {e}");
    }
    assert_eq!(settle_start(true, "0"), Ok(0));
    assert_eq!(settle_start(true, "'45'"), Ok(45), "a string scalar is accepted");
}

// ── US-0268: a saved file is rebuilt once after it settles ──────────────────

fn with_delay(delay: u64) -> Options {
    Options { delay, ..Options::default() }
}

fn states(jobs: &[Value]) -> Vec<&str> {
    jobs.iter().map(|j| j["state"].as_str().unwrap()).collect()
}

/// `n` repetitions of `word `, under a new `# Part` heading every 200 words,
/// and the number of chunks its default set holds: one per part (SPEC-0101,
/// the entries are the chunks of the default set).
fn words(word: &str, n: usize) -> (String, usize) {
    let parts: Vec<String> = (0..n)
        .collect::<Vec<_>>()
        .chunks(200)
        .enumerate()
        .map(|(k, part)| format!("# Part {}\n\n{}\n", k + 1, format!("{word} ").repeat(part.len())))
        .collect();
    let text = parts.join("\n");
    let chunks = crate::search::test_chunks("/words.md", &text).len();
    (text, chunks)
}

/// The chunks of `path` a search for `word` returns.
async fn chunks_of(env: &Env, word: &str, path: &str) -> usize {
    env.found(word).await.iter().filter(|(p, _)| p == path).count()
}

/// SPEC-0100/E2E-001: three saves 20 s apart make one rebuild, 30 s after the last.
#[tokio::test(start_paused = true)]
async fn e2e_0100_001_three_saves_one_rebuild() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/notes.md", "alpha").await;
    env.save("/notes.md", "beta").await;
    let first = env.jobs("/notes.md").await;
    assert_eq!(states(&first), ["waiting"], "{first:?}");
    let (id, created) = (first[0]["job_id"].clone(), first[0]["created_at"].clone());
    env.at(20).await;
    env.save("/notes.md", "gamma").await;
    env.at(40).await;
    env.save("/notes.md", "delta").await;

    env.at(67).await;
    assert!(!env.finds("delta", "/notes.md").await, "no rebuild before 70 s");
    env.at(80).await;
    assert!(env.finds("delta", "/notes.md").await);
    assert!(!env.finds("alpha", "/notes.md").await);
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_eq!(jobs[0]["kind"], "search_index");
    assert_eq!(jobs[0]["state"], "done");
    assert_eq!(jobs[0]["cause"], "change");
    assert_eq!(jobs[0]["job_id"], id);
    assert_eq!(jobs[0]["created_at"], created);
    assert_eq!(env.starts_of("/notes.md").len(), 1, "one rebuild for three saves");
}

/// SPEC-0100/E2E-002: a delay of 0 rebuilds at once, with no waiting job.
#[tokio::test(start_paused = true)]
async fn e2e_0100_002_delay_zero_rebuilds_at_once() {
    let env = Env::new(with_delay(0)).await;
    env.seed("/notes.md", "alpha").await;
    env.save("/notes.md", "beta").await;
    assert!(!states(&env.jobs("/notes.md").await).contains(&"waiting"));
    env.at(10).await;
    assert!(env.finds("beta", "/notes.md").await);
    let jobs = env.jobs("/notes.md").await;
    assert!(!states(&jobs).contains(&"waiting"), "{jobs:?}");
    assert_eq!(states(&jobs), ["done"]);
}

/// SPEC-0100/E2E-005: the previous entries are served while a rebuild waits.
#[tokio::test(start_paused = true)]
async fn e2e_0100_005_previous_entries_served_while_waiting() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/notes.md", "alpha").await;
    env.save("/notes.md", "beta").await;
    env.at(5).await;
    assert!(env.finds("alpha", "/notes.md").await);
    assert!(!env.finds("beta", "/notes.md").await);
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["waiting"]);
    let left = jobs[0]["seconds_left"].as_u64().unwrap();
    assert!((21..=28).contains(&left), "seconds_left {left}");
    env.at(40).await;
    assert!(!env.finds("alpha", "/notes.md").await);
    assert!(env.finds("beta", "/notes.md").await);
}

/// SPEC-0100/E2E-007: an index failure keeps the old entries and the write.
#[tokio::test(start_paused = true)]
async fn e2e_0100_007_index_failure_keeps_old_entries() {
    let env = Env::new(with_delay(2)).await;
    env.seed("/notes.md", "alpha").await;
    env.stub().set_down(true);
    let v = env.save("/notes.md", "beta").await;
    assert_eq!(v["bytes_written"], 4);
    env.at(12).await;
    assert!(env.finds("alpha", "/notes.md").await);
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["failed"]);
    assert_eq!(jobs[0]["reason"], "embedding service unreachable");
    env.at(72).await;
    assert_eq!(env.starts_of("/notes.md").len(), 1, "no retry on its own");
    assert_eq!(env.jobs("/notes.md").await, jobs);
}

/// SPEC-0100/E2E-008: a change during a running rebuild supersedes it.
#[tokio::test(start_paused = true)]
async fn e2e_0100_008_change_during_a_running_rebuild() {
    let env = Env::new(with_delay(3)).await;
    env.seed("/notes.md", "alpha").await;
    env.stub().set_delay(10);
    env.save("/notes.md", "beta").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["running"]);
    let first = env.jobs("/notes.md").await[0]["job_id"].clone();
    env.save("/notes.md", "gamma").await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["waiting", "running"], "a new job waits: {jobs:?}");
    for t in 6..=30 {
        env.at(t).await;
        assert!(!env.finds("beta", "/notes.md").await, "beta found at {t} s");
        if t == 14 {
            let jobs = env.jobs("/notes.md").await;
            let sup = jobs.iter().find(|j| j["job_id"] == first).expect("first job listed");
            assert_eq!(sup["state"], "superseded", "{jobs:?}");
        }
    }
    assert!(env.finds("gamma", "/notes.md").await);
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);
}

/// SPEC-0100/E2E-009: the boundary of the delay, two cases.
#[tokio::test(start_paused = true)]
async fn e2e_0100_009_the_boundary_of_the_delay() {
    // Case A: saves at 0, 29 and 58 make one rebuild due at 88.
    let env = Env::new(with_delay(30)).await;
    for t in [0, 29, 58] {
        env.at(t).await;
        env.save("/notes.md", &format!("text{t}")).await;
    }
    env.at(86).await;
    assert!(env.starts_of("/notes.md").is_empty(), "nothing starts before 86 s");
    env.at(97).await;
    let starts = env.starts_of("/notes.md");
    assert_eq!(starts.len(), 1);
    assert!(env.ends_of("/notes.md").iter().all(|e| *e <= 97.0));
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);

    // Case B: saves at 0 and 45 make two rebuilds.
    let env = Env::new(with_delay(30)).await;
    env.save("/notes.md", "one").await;
    env.at(45).await;
    env.save("/notes.md", "two").await;
    env.at(80).await;
    let starts = env.starts_of("/notes.md");
    let ends = env.ends_of("/notes.md");
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert!((30.0..=32.0).contains(&starts[0]), "{starts:?}");
    assert!(ends[0] <= 37.0, "{ends:?}");
    assert!((75.0..=77.0).contains(&starts[1]), "{starts:?}");
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);
}

/// SPEC-0100/E2E-018: two files have independent jobs.
#[tokio::test(start_paused = true)]
async fn e2e_0100_018_two_files_independent_jobs() {
    let env = Env::new(with_delay(30)).await;
    env.save("/a.md", "alpha").await;
    env.at(20).await;
    env.save("/b.md", "beta").await;
    env.at(60).await;
    let (a, b) = (env.starts_of("/a.md"), env.starts_of("/b.md"));
    assert!(a.len() == 1 && (30.0..=32.0).contains(&a[0]), "{a:?}");
    assert!(b.len() == 1 && (50.0..=52.0).contains(&b[0]), "{b:?}");
    let jobs = env.jobs("").await;
    assert_eq!(states(&jobs), ["done", "done"]);
    let mut paths: Vec<&str> = jobs.iter().map(|j| j["path"].as_str().unwrap()).collect();
    paths.sort();
    assert_eq!(paths, ["/a.md", "/b.md"]);
}

/// SPEC-0100/E2E-019: a name with an accent and a space is kept exactly.
#[tokio::test(start_paused = true)]
async fn e2e_0100_019_names_with_accents_and_spaces() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/notes été.md", "alpha").await;
    env.save("/notes été.md", "beta").await;
    env.at(31).await;
    let jobs = env.jobs("/notes été.md").await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["path"], "/notes été.md");
    let all = env.jobs("").await;
    assert!(all.iter().any(|j| j["path"] == "/notes été.md"), "{all:?}");
}

/// SPEC-0100/E2E-021: a project that does not index schedules nothing.
#[tokio::test(start_paused = true)]
async fn e2e_0100_021_a_project_that_does_not_index_schedules_nothing() {
    let env = Env::new(with_delay(30)).await;
    env.state.admin.create_project("plain", ALICE).await.unwrap();
    env.state.stores.provision_volume("plain").await.unwrap();
    let v = env.save_in("plain", "/x.md", "text").await;
    assert_eq!(v["bytes_written"], 4);
    env.at(40).await;
    let v = env.list(ALICE, "plain", "", "").await.unwrap();
    assert_eq!(v, json!({"jobs": []}));
    assert!(env.stub().starts().is_empty());
}

/// SPEC-0100/E2E-023: a write, edit, patch, move and delete never wait.
#[tokio::test(start_paused = true)]
async fn e2e_0100_023_a_write_never_waits() {
    let env = Env::new(with_delay(0)).await;
    env.stub().set_delay(30);
    let quick = |started: tokio::time::Instant, what: &str| {
        let took = started.elapsed();
        assert!(took < Duration::from_secs(1), "{what} waited {took:?}");
    };
    let server = env.server(ALICE);
    env.save("/notes.md", "one\ntwo\n").await;
    let t = tokio::time::Instant::now();
    let a = args(
        json!({"mount_id": PROJ, "path": "/notes.md", "old_string": "one", "new_string": "uno"}),
    );
    tool_json(server.fs_edit(a).await).expect("edit");
    quick(t, "edit");
    let t = tokio::time::Instant::now();
    let patch = "*** Begin Patch\n*** Update File: /notes.md\n@@\n-two\n+dos\n*** End Patch\n";
    let a = args(json!({"mount_id": PROJ, "patch_text": patch}));
    tool_json(server.fs_apply_patch(a).await).expect("patch");
    quick(t, "patch");
    let t = tokio::time::Instant::now();
    let a = args(json!({"mount_id": PROJ, "source": "/notes.md", "destination": "/moved.md"}));
    tool_json(server.fs_move(a).await).expect("move");
    quick(t, "move");
    let t = tokio::time::Instant::now();
    let a = args(json!({"mount_id": PROJ, "path": "/moved.md"}));
    tool_json(server.fs_delete(a).await).expect("delete");
    quick(t, "delete");
    assert!(env.ends_of("/notes.md").is_empty(), "the rebuild ends after the calls answered");

    env.at(70).await;
    let starts = env.starts_of("/notes.md");
    assert!(!starts.is_empty() && starts[0] < 1.0, "starts right after the first save: {starts:?}");
    assert!(env.ends_of("/notes.md").first().is_some_and(|e| *e >= 30.0));
}

/// SPEC-0100/E2E-029: a burst of saves with a delay of 0.
#[tokio::test(start_paused = true)]
async fn e2e_0100_029_a_burst_of_saves_with_delay_zero() {
    let env = Env::new(with_delay(0)).await;
    env.stub().set_delay(3);
    for (t, text) in [(0, "v1"), (1, "v2"), (2, "v3")] {
        env.at(t).await;
        env.save("/notes.md", text).await;
        assert!(!states(&env.jobs("/notes.md").await).contains(&"waiting"));
    }
    for t in 3..=20 {
        env.at(t).await;
        let jobs = env.jobs("/notes.md").await;
        assert!(!states(&jobs).contains(&"waiting"), "at {t}: {jobs:?}");
        assert!(states(&jobs).iter().filter(|s| **s == "running").count() <= 1);
    }
    assert_eq!(env.stub().max_running(), 1);
    assert!(env.finds("v3", "/notes.md").await);
    assert!(!env.finds("v1", "/notes.md").await);
    assert!(!env.finds("v2", "/notes.md").await);
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);
}

/// SPEC-0100/E2E-036: after a failure, the next save rebuilds again.
#[tokio::test(start_paused = true)]
async fn e2e_0100_036_recovery_after_a_failure() {
    let env = Env::new(with_delay(2)).await;
    env.seed("/notes.md", "alpha").await;
    env.stub().set_down(true);
    env.save("/notes.md", "beta").await;
    env.at(10).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["failed"]);
    env.stub().set_down(false);
    env.save("/notes.md", "gamma").await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["waiting", "failed"]);
    env.at(20).await;
    assert!(env.finds("gamma", "/notes.md").await);
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"], "the failed job is replaced");
}

/// SPEC-0100/E2E-040: new content is not searchable before its rebuild ends.
#[tokio::test(start_paused = true)]
async fn e2e_0100_040_new_content_not_searchable_before_rebuild() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/notes.md", "alpha").await;
    env.save("/notes.md", "beta").await;
    env.at(15).await;
    assert!(!env.finds("beta", "/notes.md").await);
    assert!(env.finds("alpha", "/notes.md").await);
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["waiting"]);
    let left = jobs[0]["seconds_left"].as_u64().unwrap();
    assert!((14..=16).contains(&left), "seconds_left {left}");
}

/// Old and new chunks of `/big.md` are never served together while it rebuilds.
async fn replaced_in_one_step(delay: u64, embed_secs: u64) {
    let env = Env::new(with_delay(delay)).await;
    env.stub().set_delay(embed_secs);
    let (old, n_old) = words("old", 750);
    let (new, n_new) = words("new", 1200);
    assert_eq!((n_old, n_new), (4, 6), "the fixture texts make 4 and 6 chunks");
    env.seed("/big.md", &old).await;
    assert_eq!(chunks_of(&env, "old", "/big.md").await, 4);
    env.save("/big.md", &new).await;
    for t in 1..=delay + embed_secs + 5 {
        env.at(t).await;
        let (o, n) =
            (chunks_of(&env, "old", "/big.md").await, chunks_of(&env, "new", "/big.md").await);
        assert!((o, n) == (4, 0) || (o, n) == (0, 6), "at {t} s: {o} old and {n} new chunks");
    }
    assert_eq!(chunks_of(&env, "old", "/big.md").await, 0);
    assert_eq!(chunks_of(&env, "new", "/big.md").await, 6);
}

/// SPEC-0100/E2E-041: entries are replaced in one step.
#[tokio::test(start_paused = true)]
async fn e2e_0100_041_entries_are_replaced_in_one_step() {
    replaced_in_one_step(2, 10).await;
}

/// SPEC-0100/E2E-077: no mixed entries during a replacement, delay 0.
#[tokio::test(start_paused = true)]
async fn e2e_0100_077_no_mixed_entries_during_a_replacement() {
    replaced_in_one_step(0, 8).await;
}

/// SPEC-0100/E2E-043: two failures in a row keep one finished job.
#[tokio::test(start_paused = true)]
async fn e2e_0100_043_two_failures_keep_one_finished_job() {
    let env = Env::new(with_delay(2)).await;
    env.stub().set_down(true);
    env.save("/notes.md", "beta").await;
    env.at(10).await;
    let first = env.jobs("/notes.md").await;
    assert_eq!(states(&first), ["failed"]);
    env.save("/notes.md", "gamma").await;
    env.at(20).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["failed"], "{jobs:?}");
    assert_ne!(jobs[0]["job_id"], first[0]["job_id"]);
    assert!(jobs[0]["ended_at"].as_str() > first[0]["ended_at"].as_str(), "{jobs:?} {first:?}");
}

/// SPEC-0100/E2E-044: a discarded result is never visible.
#[tokio::test(start_paused = true)]
async fn e2e_0100_044_a_discarded_result_is_never_visible() {
    let env = Env::new(with_delay(3)).await;
    env.seed("/notes.md", "alpha").await;
    env.stub().then_delays(&[10]);
    env.save("/notes.md", "beta").await;
    env.at(5).await;
    assert_eq!(env.starts_of("/notes.md").len(), 1, "the first rebuild runs");
    env.save("/notes.md", "gamma").await;
    let second_end = loop {
        let now = tokio::time::Instant::now();
        assert!(!env.finds("beta", "/notes.md").await, "beta found at {}", env.since(now));
        if let Some(end) = env.ends_of("/notes.md").get(1) {
            break *end;
        }
        assert!(env.finds("alpha", "/notes.md").await, "alpha lost at {}", env.since(now));
        tokio::time::sleep(Duration::from_secs(1)).await;
    };
    assert!(second_end <= 3.0 + 25.0, "second rebuild ended at {second_end}");
    assert!(env.finds("gamma", "/notes.md").await);
    assert!(!env.finds("beta", "/notes.md").await);
}

/// SPEC-0100/E2E-045: two saves during one running rebuild leave one waiting job.
#[tokio::test(start_paused = true)]
async fn e2e_0100_045_two_saves_during_one_running_rebuild() {
    let env = Env::new(with_delay(30)).await;
    env.stub().set_delay(10);
    env.save("/notes.md", "beta").await;
    env.at(32).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["running"]);
    env.save("/notes.md", "gamma").await;
    env.at(34).await;
    env.save("/notes.md", "delta").await;
    env.at(41).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["waiting", "superseded"], "{jobs:?}");
    let waiting = jobs[0]["job_id"].clone();
    env.at(80).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["done"]);
    assert_eq!(jobs[0]["job_id"], waiting);
    assert_eq!(env.starts_of("/notes.md").len(), 2);
    assert!(env.finds("delta", "/notes.md").await);
}

/// SPEC-0100/E2E-051: an index failure never fails the write.
#[tokio::test(start_paused = true)]
async fn e2e_0100_051_an_index_failure_never_fails_the_write() {
    let env = Env::new(with_delay(0)).await;
    env.seed("/notes.md", "alpha").await;
    let before = env.chunks().await;
    env.stub().set_down(true);
    let v = env.save("/notes.md", "beta").await;
    assert_eq!(v["bytes_written"], 4);
    let a = args(json!({"mount_id": PROJ, "path": "/notes.md"}));
    let read = tool_json(env.server(ALICE).fs_read(a).await).expect("read");
    assert!(read["content"].as_str().unwrap().contains("beta"), "{read}");
    env.at(10).await;
    assert_eq!(env.chunks().await, before);
    assert_eq!(states(&env.jobs("/notes.md").await), ["failed"]);
}

/// SPEC-0100/E2E-052: an empty settle delay setting means the default 30 s.
#[tokio::test(start_paused = true)]
async fn e2e_0100_052_an_empty_settle_delay_means_the_default() {
    let delay = settle_start(true, "''").expect("the server starts");
    assert_eq!(delay, 30);
    let env = Env::new(with_delay(delay)).await;
    env.save("/notes.md", "beta").await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["waiting"]);
    let left = jobs[0]["seconds_left"].as_u64().unwrap();
    assert!((29..=30).contains(&left), "seconds_left {left}");
}

/// SPEC-0100/E2E-053: a configured delay is used.
#[tokio::test(start_paused = true)]
async fn e2e_0100_053_a_configured_delay_is_used() {
    let env = Env::new(with_delay(5)).await;
    env.seed("/notes.md", "alpha").await;
    env.save("/notes.md", "beta").await;
    env.at(1).await;
    assert!(!env.finds("beta", "/notes.md").await);
    env.at(14).await;
    assert!(env.finds("beta", "/notes.md").await);
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);
}

/// SPEC-0100/E2E-073: a rebuild that fails while Alice reads.
#[tokio::test(start_paused = true)]
async fn e2e_0100_073_a_rebuild_that_fails_while_alice_reads() {
    let env = Env::new(with_delay(2)).await;
    env.seed("/notes.md", "alpha").await;
    env.stub().set_down(true);
    env.stub().set_delay(4);
    env.save("/notes.md", "beta").await;
    for t in [1, 4, 8] {
        env.at(t).await;
        assert!(env.finds("alpha", "/notes.md").await, "alpha at {t} s");
        assert!(!env.finds("beta", "/notes.md").await, "beta at {t} s");
        if t == 4 {
            assert_eq!(states(&env.jobs("/notes.md").await), ["running"]);
        }
    }
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["failed"]);
    assert_eq!(jobs[0]["reason"], "embedding service unreachable");
}

/// SPEC-0100/E2E-MOD-001 (was SPEC-0008/E2E-023): searchable after the settle delay.
#[tokio::test(start_paused = true)]
async fn e2e_0100_mod001_content_searchable_after_the_settle_delay() {
    let env = Env::new(with_delay(3)).await;
    env.stub().set_delay(2);
    let (text, n) = words("chunk", 300);
    assert_eq!(n, 2, "the fixture text makes 2 chunks");
    env.save("/doc.md", &text).await;
    env.at(1).await;
    assert_eq!(env.chunks().await, 0);
    env.at(14).await;
    assert_eq!(env.chunks().await, 2);
    assert_eq!(states(&env.jobs("/doc.md").await), ["done"]);
}

/// SPEC-0100/E2E-MOD-002 (was SPEC-0008/E2E-023): a delay of 0 keeps the old timing.
#[tokio::test(start_paused = true)]
async fn e2e_0100_mod002_a_delay_of_zero_keeps_the_old_timing() {
    let env = Env::new(with_delay(0)).await;
    env.stub().set_delay(4);
    let (text, n) = words("chunk", 300);
    assert_eq!(n, 2, "the fixture text makes 2 chunks");
    env.save("/doc.md", &text).await;
    assert!(!states(&env.jobs("/doc.md").await).contains(&"waiting"));
    env.at(1).await;
    assert_eq!(env.chunks().await, 0);
    assert!(!states(&env.jobs("/doc.md").await).contains(&"waiting"));
    env.at(14).await;
    assert_eq!(env.chunks().await, 2);
}

/// Copies `src` to `dst` as Alice, asserting the answer comes at once.
async fn copy_as_alice(env: &Env, project: &str, src: &str, dst: &str) {
    let t = tokio::time::Instant::now();
    let a =
        args(json!({"mount_id": project, "source": src, "destination": dst, "recursive": true}));
    tool_json(env.server(ALICE).fs_copy(a).await).expect("the copy succeeds");
    assert!(t.elapsed() < Duration::from_secs(1), "the copy waited {:?}", t.elapsed());
}

/// The paths of the jobs of the project, sorted, with their states.
async fn job_paths(env: &Env) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = env
        .jobs("")
        .await
        .iter()
        .map(|j| (j["path"].as_str().unwrap().into(), j["state"].as_str().unwrap().into()))
        .collect();
    v.sort();
    v
}

/// SPEC-0100/E2E-049: a copy and a restore schedule the destination only.
#[tokio::test(start_paused = true)]
async fn e2e_0100_049_a_copy_and_a_restore_schedule_the_destination_only() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/a.md", "alpha").await;
    env.seed("/d.md", "delta").await;
    let server = env.server(ALICE);
    let a = args(json!({"mount_id": PROJ, "path": "/d.md"}));
    tool_json(server.fs_delete(a).await).expect("delete");
    let a = args(json!({"mount_id": PROJ}));
    let listed = tool_json(server.fs_trash_list(a).await).expect("trash list");
    let trash_path = listed["entries"][0]["trash_path"].as_str().unwrap().to_string();
    assert!(job_paths(&env).await.is_empty(), "nothing scheduled yet");

    copy_as_alice(&env, PROJ, "/a.md", "/a2.md").await;
    let a = args(json!({"mount_id": PROJ, "trash_path": trash_path}));
    let t = tokio::time::Instant::now();
    tool_json(server.fs_trash_restore(a).await).expect("restore");
    assert!(t.elapsed() < Duration::from_secs(1), "the restore waited");

    let jobs = job_paths(&env).await;
    assert_eq!(
        jobs,
        [
            ("/a2.md".to_string(), "waiting".to_string()),
            ("/d.md".to_string(), "waiting".to_string())
        ]
    );
    assert!(env.stub().starts().is_empty(), "nothing indexed at once");
}

/// SPEC-0100/E2E-063: a copy schedules its destination.
#[tokio::test(start_paused = true)]
async fn e2e_0100_063_a_copy_schedules_its_destination() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/a.md", "alpha").await;
    copy_as_alice(&env, PROJ, "/a.md", "/a2.md").await;
    assert_eq!(states(&env.jobs("/a2.md").await), ["waiting"]);
    assert!(env.jobs("/a.md").await.is_empty());
    assert!(!env.finds("alpha", "/a2.md").await, "not searchable before the delay");
    env.at(45).await;
    assert!(env.finds("alpha", "/a.md").await);
    assert!(env.finds("alpha", "/a2.md").await);
}

/// SPEC-0100/E2E-064: a copy in a project that does not index.
#[tokio::test(start_paused = true)]
async fn e2e_0100_064_a_copy_in_a_project_that_does_not_index() {
    let env = Env::new(with_delay(30)).await;
    env.state.admin.create_project("plain", ALICE).await.unwrap();
    env.state.stores.provision_volume("plain").await.unwrap();
    env.save_in("plain", "/a.md", "alpha").await;
    copy_as_alice(&env, "plain", "/a.md", "/a2.md").await;
    env.at(45).await;
    let v = env.list(ALICE, "plain", "", "").await.unwrap();
    assert_eq!(v, json!({"jobs": []}));
    assert!(env.stub().starts().is_empty());
}

/// SPEC-0100/E2E-068: a copy of a folder schedules every file under it.
#[tokio::test(start_paused = true)]
async fn e2e_0100_068_a_copy_of_a_folder_schedules_every_file() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/docs/a.md", "alpha").await;
    env.seed("/docs/b.md", "bravo").await;
    copy_as_alice(&env, PROJ, "/docs", "/docs2").await;
    assert_eq!(
        job_paths(&env).await,
        [
            ("/docs2/a.md".to_string(), "waiting".to_string()),
            ("/docs2/b.md".to_string(), "waiting".to_string())
        ]
    );
    env.at(45).await;
    assert!(env.finds("alpha", "/docs2/a.md").await);
    assert!(env.finds("bravo", "/docs2/b.md").await);
}

// ── US-0270: waiting rebuilds survive a restart ─────────────────────────────

fn left(job: &Value) -> u64 {
    job["seconds_left"].as_u64().unwrap_or_else(|| panic!("seconds_left in {job}"))
}

/// SPEC-0100/E2E-011: a restart keeps the waiting rebuild, with the full delay.
#[tokio::test(start_paused = true)]
async fn e2e_0100_011_a_restart_keeps_the_waiting_rebuild() {
    let mut env = Env::new(with_delay(30)).await;
    env.seed("/notes.md", "alpha").await;
    env.save("/notes.md", "beta").await;
    env.at(10).await;
    let before = env.jobs("/notes.md").await;
    assert_eq!(states(&before), ["waiting"]);
    assert_eq!(left(&before[0]), 20);

    env.restart().await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["waiting"], "{jobs:?}");
    assert!((28..=30).contains(&left(&jobs[0])), "{jobs:?}");
    assert_eq!(jobs[0]["job_id"], before[0]["job_id"]);

    env.at(50).await;
    assert!(env.finds("beta", "/notes.md").await);
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);
}

/// SPEC-0100/E2E-034: a restart while a rebuild runs ends it as failed and
/// schedules a new one.
#[tokio::test(start_paused = true)]
async fn e2e_0100_034_a_restart_while_a_rebuild_runs() {
    let mut env = Env::new(with_delay(3)).await;
    env.seed("/notes.md", "alpha").await;
    env.stub().then_delays(&[60]);
    env.save("/notes.md", "beta").await;
    env.at(10).await;
    let running = env.jobs("/notes.md").await;
    assert_eq!(states(&running), ["running"]);

    env.restart().await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["waiting", "failed"], "{jobs:?}");
    assert_eq!(jobs[1]["job_id"], running[0]["job_id"]);
    assert_eq!(jobs[1]["reason"], "interrupted by a server restart");
    assert_eq!(jobs[0]["kind"], "search_index");
    assert_eq!(left(&jobs[0]), 3);
    assert_ne!(jobs[0]["job_id"], running[0]["job_id"]);

    env.at(40).await;
    assert!(env.finds("beta", "/notes.md").await);
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);
    assert_eq!(env.starts_of("/notes.md").len(), 2, "the interrupted run and the new one");
}

/// SPEC-0100/E2E-035: a restart with nothing pending changes nothing.
#[tokio::test(start_paused = true)]
async fn e2e_0100_035_a_restart_with_nothing_pending() {
    let mut env = Env::new(with_delay(5)).await;
    env.seed("/notes.md", "alpha").await;
    env.save("/notes.md", "beta").await;
    env.at(20).await;
    let done = env.jobs("/notes.md").await;
    assert_eq!(states(&done), ["done"]);

    env.restart().await;
    env.at(60).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(jobs, done);
    assert_eq!(env.starts_of("/notes.md").len(), 1, "nothing rebuilt again");
}

/// SPEC-0100/E2E-054: two files at a restart, then the same with a delay of 0.
#[tokio::test(start_paused = true)]
async fn e2e_0100_054_two_running_rebuilds_at_a_restart() {
    let mut env = Env::new(with_delay(10)).await;
    env.seed("/a.md", "alpha").await;
    env.seed("/b.md", "alpha").await;
    env.stub().then_delays(&[60]);
    env.save("/a.md", "abeta").await;
    env.at(12).await;
    env.save("/b.md", "bbeta").await;
    env.at(15).await;
    assert_eq!(states(&env.jobs("/a.md").await), ["running"]);
    assert_eq!(left(&env.jobs("/b.md").await[0]), 7);

    env.restart().await;
    let a = env.jobs("/a.md").await;
    assert_eq!(states(&a), ["waiting", "failed"], "{a:?}");
    assert_eq!(a[1]["reason"], "interrupted by a server restart");
    assert_eq!(left(&a[0]), 10);
    let b = env.jobs("/b.md").await;
    assert_eq!(states(&b), ["waiting"]);
    assert_eq!(left(&b[0]), 10);

    env.at(40).await;
    assert!(env.finds("abeta", "/a.md").await);
    assert!(env.finds("bbeta", "/b.md").await);

    // Delay 0: the new rebuild starts right after the start, never listed waiting.
    let mut env = Env::new(with_delay(0)).await;
    env.seed("/a.md", "alpha").await;
    env.stub().then_delays(&[60]);
    env.save("/a.md", "abeta").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/a.md").await), ["running"]);
    env.restart().await;
    env.at(6).await;
    let a = env.jobs("/a.md").await;
    assert!(!states(&a).contains(&"waiting"), "{a:?}");
    assert_eq!(env.starts_of("/a.md").len(), 2);
    assert!((env.starts_of("/a.md")[1] - 5.0).abs() < 1.0);
    env.at(10).await;
    assert!(env.finds("abeta", "/a.md").await);
    assert_eq!(states(&env.jobs("/a.md").await), ["done"]);
}

/// SPEC-0100/FR-NEW-001: the waiting job a restart creates for an interrupted
/// rebuild has cause `change`, also when the interrupted one was manual.
#[tokio::test(start_paused = true)]
async fn e2e_0100_us0270_interrupted_manual_job_is_rescheduled_as_change() {
    let mut env = Env::new(with_delay(3)).await;
    env.seed("/notes.md", "alpha").await;
    env.stub().then_delays(&[60]);
    let v = env.recalc(ALICE, PROJ, "/notes.md", "search_index").await.unwrap();
    assert_eq!(v["state"], "running", "{v}");
    env.at(5).await;
    let running = env.jobs("/notes.md").await;
    assert_eq!(running[0]["cause"], "manual", "{running:?}");

    env.restart().await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["waiting", "failed"], "{jobs:?}");
    assert_eq!(jobs[1]["job_id"], running[0]["job_id"]);
    assert_eq!(jobs[1]["cause"], "manual");
    assert_eq!(jobs[0]["cause"], "change", "{jobs:?}");
}

/// SPEC-0100/E2E-067: two restarts in a row, each with the full delay.
#[tokio::test(start_paused = true)]
async fn e2e_0100_067_two_restarts_in_a_row() {
    let mut env = Env::new(with_delay(30)).await;
    env.seed("/notes.md", "alpha").await;
    env.save("/notes.md", "beta").await;
    env.at(5).await;
    env.restart().await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!((states(&jobs), left(&jobs[0])), (vec!["waiting"], 30));
    env.at(10).await;
    env.restart().await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!((states(&jobs), left(&jobs[0])), (vec!["waiting"], 30));

    env.at(50).await;
    assert!(env.finds("beta", "/notes.md").await);
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);
    assert_eq!(env.starts_of("/notes.md").len(), 1, "one rebuild only");
}

/// SPEC-0100/E2E-072: a restart interrupts a rebuild while the embedding
/// service is down; the new one fails, the old entries stay.
#[tokio::test(start_paused = true)]
async fn e2e_0100_072_a_restart_while_the_embedding_service_is_down() {
    let mut env = Env::new(with_delay(3)).await;
    env.seed("/a.md", "alpha").await;
    env.stub().then_delays(&[60]);
    env.save("/a.md", "beta").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/a.md").await), ["running"]);
    env.stub().set_down(true);

    env.restart().await;
    let jobs = env.jobs("/a.md").await;
    assert_eq!(states(&jobs), ["waiting", "failed"], "{jobs:?}");
    assert_eq!(jobs[1]["reason"], "interrupted by a server restart");

    env.at(20).await;
    let jobs = env.jobs("/a.md").await;
    assert_eq!(states(&jobs), ["failed"], "{jobs:?}");
    assert_eq!(jobs[0]["reason"], "embedding service unreachable");
    assert!(env.finds("alpha", "/a.md").await, "the previous entries stay");
}

// ── US-0271: delete and move follow the jobs ────────────────────────────────

/// `fs.delete` by Alice, recursive so a folder goes too.
async fn delete(env: &Env, path: &str) {
    let a = args(json!({"mount_id": PROJ, "path": path, "recursive": true}));
    tool_json(env.server(ALICE).fs_delete(a).await).expect("the delete succeeds");
}

/// `fs.move` by Alice.
async fn move_to(env: &Env, from: &str, to: &str, overwrite: bool) {
    let a =
        args(json!({"mount_id": PROJ, "source": from, "destination": to, "overwrite": overwrite}));
    tool_json(env.server(ALICE).fs_move(a).await).expect("the move succeeds");
}

/// SPEC-0100/E2E-010: a delete cancels the waiting rebuild.
#[tokio::test(start_paused = true)]
async fn e2e_0100_010_a_delete_cancels_the_waiting_rebuild() {
    let env = Env::new(with_delay(30)).await;
    env.save("/a.md", "alpha").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/a.md").await), ["waiting"]);
    delete(&env, "/a.md").await;
    for t in [6, 35, 40, 60] {
        env.at(t).await;
        assert!(env.jobs("/a.md").await.is_empty(), "a job names /a.md at {t} s");
        assert!(env.jobs("").await.is_empty(), "the project has a job at {t} s");
        assert!(!env.finds("alpha", "/a.md").await, "alpha found at {t} s");
    }
    assert!(env.starts_of("/a.md").is_empty(), "no rebuild ran for /a.md");
}

/// SPEC-0100/E2E-024: a move keeps the time already waited.
#[tokio::test(start_paused = true)]
async fn e2e_0100_024_a_move_keeps_the_wait() {
    let env = Env::new(with_delay(30)).await;
    env.save("/b.md", "beta").await;
    let id = env.jobs("/b.md").await[0]["job_id"].clone();
    env.at(10).await;
    move_to(&env, "/b.md", "/c.md", false).await;
    let jobs = env.jobs("/c.md").await;
    assert_eq!(states(&jobs), ["waiting"], "{jobs:?}");
    assert_eq!(jobs[0]["job_id"], id, "the same job: {jobs:?}");
    let left = jobs[0]["seconds_left"].as_u64().unwrap();
    assert!((18..=22).contains(&left), "{left} s left");
    assert!(env.jobs("/b.md").await.is_empty());
    env.at(40).await;
    assert!(env.finds("beta", "/c.md").await);
    assert!(!env.finds("beta", "/b.md").await);
    assert_eq!(states(&env.jobs("/c.md").await), ["done"]);
    assert!(env.jobs("/b.md").await.is_empty());
}

/// SPEC-0100/E2E-030: a delete during a running rebuild.
#[tokio::test(start_paused = true)]
async fn e2e_0100_030_a_delete_during_a_running_rebuild() {
    let env = Env::new(with_delay(3)).await;
    env.seed("/a.md", "alpha").await;
    env.stub().set_delay(10);
    env.save("/a.md", "beta").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/a.md").await), ["running"]);
    delete(&env, "/a.md").await;
    // Listed as superseded from the moment of the delete until it ends.
    assert_eq!(states(&env.jobs("/a.md").await), ["superseded"]);
    for t in 6..=30 {
        env.at(t).await;
        assert!(!env.finds("alpha", "/a.md").await, "alpha found at {t} s");
        assert!(!env.finds("beta", "/a.md").await, "beta found at {t} s");
    }
    assert_eq!(env.ends_of("/a.md").len(), 1, "the rebuild ended");
    assert!(env.jobs("/a.md").await.is_empty(), "{:?}", env.jobs("/a.md").await);
    assert!(env.jobs("").await.is_empty());
}

/// SPEC-0100/E2E-031: deleting a file with nothing pending.
#[tokio::test(start_paused = true)]
async fn e2e_0100_031_deleting_a_file_with_nothing_pending() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/a.md", "alpha").await;
    env.seed_job(PROJ, "/a.md", JobState::Done, Some(&at(0)), None).await;
    assert_eq!(states(&env.jobs("/a.md").await), ["done"]);
    delete(&env, "/a.md").await;
    env.at(1).await;
    assert!(!env.finds("alpha", "/a.md").await);
    assert!(env.jobs("/a.md").await.is_empty());
}

/// SPEC-0100/E2E-032: moving a file with nothing pending.
#[tokio::test(start_paused = true)]
async fn e2e_0100_032_moving_a_file_with_nothing_pending() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/b.md", "beta").await;
    let done = env.seed_job(PROJ, "/b.md", JobState::Done, Some(&at(0)), None).await;
    move_to(&env, "/b.md", "/c.md", false).await;
    assert!(!env.finds("beta", "/b.md").await);
    env.at(2).await;
    assert!(!env.finds("beta", "/b.md").await);
    env.at(5).await;
    assert!(env.finds("beta", "/c.md").await);
    let jobs = env.jobs("/c.md").await;
    assert_eq!(states(&jobs), ["done"], "no waiting job: {jobs:?}");
    assert_eq!(jobs[0]["job_id"], done.job_id.as_str());
    assert_eq!(jobs[0]["path"], "/c.md");
    assert!(env.jobs("/b.md").await.is_empty());
}

/// SPEC-0100/E2E-033: a move onto an existing file with jobs.
#[tokio::test(start_paused = true)]
async fn e2e_0100_033_a_move_onto_an_existing_file_with_jobs() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/c.md", "gamma").await;
    env.seed_job(PROJ, "/c.md", JobState::Done, Some(&at(0)), None).await;
    env.save("/b.md", "beta").await;
    let id = env.jobs("/b.md").await[0]["job_id"].clone();
    env.at(10).await;
    env.save("/c.md", "delta").await;
    assert_eq!(states(&env.jobs("/c.md").await), ["waiting", "done"]);
    env.at(15).await;
    move_to(&env, "/b.md", "/c.md", true).await;
    let jobs = env.jobs("/c.md").await;
    assert_eq!(states(&jobs), ["waiting"], "{jobs:?}");
    assert_eq!(jobs[0]["job_id"], id, "the job of /b.md: {jobs:?}");
    let left = jobs[0]["seconds_left"].as_u64().unwrap();
    assert!((13..=17).contains(&left), "{left} s left");
    assert!(env.jobs("/b.md").await.is_empty());
}

/// SPEC-0100/E2E-042: a failed job is removed when the file is deleted.
#[tokio::test(start_paused = true)]
async fn e2e_0100_042_a_failed_job_is_removed_when_the_file_is_deleted() {
    let env = Env::new(with_delay(3)).await;
    env.stub().set_down(true);
    env.save("/notes.md", "beta").await;
    env.at(10).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["failed"]);
    delete(&env, "/notes.md").await;
    assert!(env.jobs("/notes.md").await.is_empty());
    let all = env.jobs("").await;
    assert!(all.iter().all(|j| j["state"] != "failed"), "{all:?}");
}

/// SPEC-0100/E2E-075: a move while the file is being rebuilt.
#[tokio::test(start_paused = true)]
async fn e2e_0100_075_a_move_while_the_file_is_being_rebuilt() {
    let env = Env::new(with_delay(3)).await;
    env.seed("/b.md", "alpha").await;
    env.stub().then_delays(&[10, 5]);
    env.save("/b.md", "beta").await;
    env.at(5).await;
    let running = env.jobs("/b.md").await;
    assert_eq!(states(&running), ["running"]);
    let id = running[0]["job_id"].clone();
    move_to(&env, "/b.md", "/c.md", false).await;
    let jobs = env.jobs("/c.md").await;
    assert_eq!(states(&jobs), ["waiting", "superseded"], "{jobs:?}");
    assert_eq!(jobs[1]["job_id"], id);
    assert_eq!(jobs[0]["seconds_left"], 3, "the full delay: {jobs:?}");
    assert!(env.jobs("/b.md").await.is_empty());
    assert!(!env.finds("alpha", "/b.md").await && !env.finds("beta", "/b.md").await);

    env.at(15).await;
    let jobs = env.jobs("/c.md").await;
    let old = jobs.iter().find(|j| j["job_id"] == id).expect("the old job is listed");
    assert_eq!(old["state"], "superseded", "{jobs:?}");
    assert_eq!(old["path"], "/c.md");
    assert_eq!(env.starts_of("/c.md").len(), 1, "the new rebuild waited for the old one");
    env.at(30).await;
    assert!(env.finds("beta", "/c.md").await);
    assert!(!env.finds("beta", "/b.md").await && !env.finds("alpha", "/b.md").await);
    assert_eq!(states(&env.jobs("/c.md").await), ["done"]);
    assert!(env.jobs("/b.md").await.is_empty());
}

// ── US-0272: index mode change and project removal cancel jobs ──────────────

/// `admin.set_index_mode` by Alice, the owner of `proj`.
async fn set_mode(env: &Env, mode: &str) {
    let a = args(json!({"project_id": PROJ, "mode": mode}));
    tool_json(env.server(ALICE).admin_set_index_mode(a).await).expect("the mode is set");
}

/// A soft delete as the purge sweep and the other callers make it, straight on
/// the admin store: the hook must live where every one of them passes.
async fn soft_delete(env: &Env) {
    assert!(env.state.admin.soft_delete_project(PROJ).await.unwrap());
}

/// `admin.undelete_project` by Alice, the owner of `proj`.
async fn restore(env: &Env) {
    let a = args(json!({"project_id": PROJ}));
    tool_json(env.server(ALICE).admin_undelete_project(a).await).expect("the restore succeeds");
}

fn assert_not_found(r: std::result::Result<Value, String>) {
    let e = r.expect_err("a deleted project cannot list its jobs");
    assert!(e.contains("ERR_PROJECT_NOT_FOUND"), "{e}");
}

/// SPEC-0100/E2E-046: the index mode becomes none.
#[tokio::test(start_paused = true)]
async fn e2e_0100_046_the_index_mode_becomes_none() {
    let env = Env::new(with_delay(30)).await;
    env.save("/notes.md", "alpha").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["waiting"]);
    set_mode(&env, "none").await;
    assert!(env.jobs("").await.is_empty(), "{:?}", env.jobs("").await);
    for t in [6, 35, 60] {
        env.at(t).await;
        assert!(env.jobs("").await.is_empty(), "a job exists at {t} s");
    }
    assert!(env.stub().starts().is_empty(), "a rebuild ran");
}

/// SPEC-0100/E2E-047: a project is deleted, then restored.
#[tokio::test(start_paused = true)]
async fn e2e_0100_047_a_project_is_deleted_then_restored() {
    let env = Env::new(with_delay(3)).await;
    env.stub().set_delay(10);
    env.save("/b.md", "beta").await;
    env.at(4).await;
    assert_eq!(states(&env.jobs("/b.md").await), ["running"]);
    env.save("/a.md", "alpha").await;
    env.seed_job(PROJ, "/c.md", JobState::Done, Some(&at(0)), None).await;
    assert_eq!(env.jobs("").await.len(), 3);
    env.at(5).await;
    soft_delete(&env).await;
    assert_not_found(env.list(ALICE, PROJ, "", "").await);
    // The running rebuild ends at 14 s, then the project comes back.
    env.at(40).await;
    assert_not_found(env.list(ALICE, PROJ, "", "").await);
    restore(&env).await;
    assert!(env.jobs("").await.is_empty(), "{:?}", env.jobs("").await);
    env.at(80).await;
    assert!(env.jobs("").await.is_empty());
    assert!(env.starts_of("/a.md").is_empty(), "the waiting job ran");
    assert_eq!(env.starts_of("/b.md").len(), 1, "only the first run of /b.md");
}

/// SPEC-0100/E2E-048: the index mode changes between two active modes.
#[tokio::test(start_paused = true)]
async fn e2e_0100_048_the_index_mode_changes_between_two_active_modes() {
    let env = Env::new(Options { embedding: true, ..with_delay(30) }).await;
    env.state.admin.set_index_mode(PROJ, IndexMode::Rag).await.unwrap();
    env.save("/notes.md", "alpha").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["waiting"]);
    set_mode(&env, "both").await;
    assert!(env.jobs("").await.is_empty(), "{:?}", env.jobs("").await);
    env.at(25).await;
    assert_eq!(env.state.admin.get_index_mode(PROJ).await.unwrap(), IndexMode::Both);
    assert!(env.finds("alpha", "/notes.md").await, "the full index did not run");
    assert!(env.stub().starts().is_empty(), "the job ran");
    assert!(env.jobs("").await.is_empty());
}

/// SPEC-0100/FR-NEW-015: a mode change between two active modes while a
/// rebuild runs; the end of that rebuild keeps what the full index wrote.
#[tokio::test(start_paused = true)]
async fn e2e_0100_us0272_mode_change_between_active_modes_keeps_the_full_index() {
    let env = Env::new(Options { embedding: true, ..with_delay(3) }).await;
    env.stub().set_delay(10);
    env.save("/notes.md", "alpha").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["running"]);
    set_mode(&env, "both").await;
    env.at(6).await;
    assert!(env.finds("alpha", "/notes.md").await, "the full index did not run");
    env.at(20).await;
    assert_eq!(env.ends_of("/notes.md").len(), 1, "the rebuild ended");
    assert!(env.finds("alpha", "/notes.md").await, "the end of the rebuild cleared the file");
    assert!(env.jobs("").await.is_empty(), "{:?}", env.jobs("").await);
}

/// SPEC-0100/E2E-060: a running rebuild when the index mode becomes none.
#[tokio::test(start_paused = true)]
async fn e2e_0100_060_a_running_rebuild_when_the_index_mode_becomes_none() {
    let env = Env::new(with_delay(3)).await;
    env.stub().set_delay(10);
    env.save("/notes.md", "alpha").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["running"]);
    set_mode(&env, "none").await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["superseded"]);
    for t in 6..=20 {
        env.at(t).await;
        assert!(!env.finds("alpha", "/notes.md").await, "alpha found at {t} s");
    }
    assert_eq!(env.ends_of("/notes.md").len(), 1, "the rebuild ended");
    assert!(env.jobs("").await.is_empty(), "{:?}", env.jobs("").await);
    assert_eq!(env.chunks().await, 0);
}

/// SPEC-0100/E2E-061: a project is deleted while a rebuild runs.
#[tokio::test(start_paused = true)]
async fn e2e_0100_061_a_project_is_deleted_while_a_rebuild_runs() {
    let env = Env::new(with_delay(3)).await;
    env.stub().set_delay(10);
    env.save("/notes.md", "alpha").await;
    env.at(5).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["running"]);
    soft_delete(&env).await;
    assert_not_found(env.list(ALICE, PROJ, "", "").await);
    env.at(20).await;
    assert_eq!(env.ends_of("/notes.md").len(), 1, "the rebuild ended");
    assert_not_found(env.list(ALICE, PROJ, "", "").await);
    restore(&env).await;
    assert!(env.jobs("").await.is_empty(), "{:?}", env.jobs("").await);
    assert!(!env.finds("alpha", "/notes.md").await, "the result of the rebuild was kept");
    env.at(60).await;
    assert!(env.jobs("").await.is_empty());
}

/// SPEC-0100/E2E-062: jobs of a restored project.
#[tokio::test(start_paused = true)]
async fn e2e_0100_062_jobs_of_a_restored_project() {
    let env = Env::new(with_delay(30)).await;
    for p in ["/a.md", "/b.md", "/c.md"] {
        env.save(p, "alpha").await;
    }
    assert_eq!(env.jobs("").await.len(), 3);
    env.at(1).await;
    soft_delete(&env).await;
    env.at(5).await;
    restore(&env).await;
    env.at(45).await;
    assert!(env.jobs("").await.is_empty(), "{:?}", env.jobs("").await);
    let a = args(json!({"mount_id": PROJ, "path": "/a.md"}));
    tool_json(env.server(ALICE).fs_read(a).await).expect("the files are back");
    env.save("/a.md", "beta").await;
    assert_eq!(states(&env.jobs("/a.md").await), ["waiting"]);
}

/// SPEC-0100/E2E-065: a restored project lists only new jobs.
#[tokio::test(start_paused = true)]
async fn e2e_0100_065_a_restored_project_lists_only_new_jobs() {
    let env = Env::new(with_delay(30)).await;
    for p in ["/a.md", "/b.md", "/c.md"] {
        env.save(p, "alpha").await;
    }
    soft_delete(&env).await;
    restore(&env).await;
    env.at(2).await;
    env.save("/notes.md", "beta").await;
    let jobs = env.jobs("").await;
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_eq!(jobs[0]["path"], "/notes.md");
}

/// SPEC-0100/E2E-066: no rebuild runs for a deleted project.
#[tokio::test(start_paused = true)]
async fn e2e_0100_066_no_rebuild_runs_for_a_deleted_project() {
    let env = Env::new(with_delay(30)).await;
    for p in ["/a.md", "/b.md", "/c.md"] {
        env.save(p, "alpha").await;
    }
    env.at(5).await;
    soft_delete(&env).await;
    env.at(70).await;
    assert!(env.stub().starts().is_empty(), "{:?}", env.stub().starts());
}

// ── US-0273: fs.recalculate ─────────────────────────────────────────────────

pub(crate) const CAROL: &str = "carol@example.com";

impl Env {
    /// `fs.recalculate` as `person`: the JSON answer, or the error text.
    pub async fn recalc(
        &self,
        person: &str,
        mount_id: &str,
        path: &str,
        kind: &str,
    ) -> std::result::Result<Value, String> {
        let a = args(json!({"mount_id": mount_id, "path": path, "kind": kind}));
        tool_json(self.server(person).fs_recalculate(a).await)
    }

    /// The unchanged waiting rebuild of a refused request, and nothing started.
    async fn assert_nothing_started(&self, before: &[Value]) {
        assert_eq!(self.jobs("").await, before, "the jobs are unchanged");
        if let Some(stub) = &self.stub {
            assert!(stub.starts().is_empty(), "no rebuild started");
        }
    }
}

fn assert_refused(r: std::result::Result<Value, String>, text: &str) {
    let e = r.expect_err("the request is refused");
    assert!(e.ends_with(text), "expected {text:?}, got {e:?}");
}

fn by_id<'a>(jobs: &'a [Value], id: &Value) -> Option<&'a Value> {
    jobs.iter().find(|j| j["job_id"] == *id)
}

/// SPEC-0100/E2E-004: a rebuild by hand, then the list.
#[tokio::test(start_paused = true)]
async fn e2e_0100_004_rebuild_by_hand_then_list() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/notes.md", "alpha").await;
    env.save("/notes.md", "beta").await;
    env.at(5).await;
    let v = env.recalc(ALICE, PROJ, "/notes.md", "search_index").await.unwrap();
    assert_eq!(v["path"], "/notes.md");
    assert_eq!(v["kind"], "search_index");
    assert!(v["job_id"].as_str().is_some_and(|s| !s.is_empty()), "{v}");
    assert!(["running", "done"].contains(&v["state"].as_str().unwrap()), "{v}");
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_eq!(jobs[0]["cause"], "manual");
    assert_eq!(jobs[0]["job_id"], v["job_id"]);
    assert!(!states(&jobs).contains(&"waiting"));
    env.at(12).await;
    assert!(env.finds("beta", "/notes.md").await, "found within 7 s");
}

/// SPEC-0100/E2E-012: a non member cannot rebuild.
#[tokio::test(start_paused = true)]
async fn e2e_0100_012_a_non_member_cannot_rebuild() {
    let env = Env::new(with_delay(30)).await;
    env.save("/notes.md", "alpha").await;
    let before = env.jobs("").await;
    assert_eq!(states(&before), ["waiting"]);
    let r = env.recalc(DAVE, PROJ, "/notes.md", "search_index").await;
    assert_refused(r, "ERR_FORBIDDEN: 'dave@example.com' is not a member of 'proj'");
    env.assert_nothing_started(&before).await;
}

/// SPEC-0100/E2E-013 and E2E-015: an unknown kind and a kind of a later
/// increment are refused alike.
#[tokio::test(start_paused = true)]
async fn e2e_0100_013_015_unknown_kinds() {
    for kind in ["concepts", "conversion"] {
        let env = Env::new(with_delay(30)).await;
        env.seed("/notes.md", "alpha").await;
        let r = env.recalc(ALICE, PROJ, "/notes.md", kind).await;
        assert_refused(r, "ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'");
        env.assert_nothing_started(&[]).await;
    }
}

/// SPEC-0100/E2E-014: an unknown file.
#[tokio::test(start_paused = true)]
async fn e2e_0100_014_unknown_file() {
    let env = Env::new(with_delay(30)).await;
    let r = env.recalc(ALICE, PROJ, "/nope.md", "search_index").await;
    assert_refused(r, "ERR_NOT_FOUND: file not found: /nope.md");
    env.assert_nothing_started(&[]).await;
}

/// SPEC-0100/E2E-017: two rebuilds by hand a second apart.
#[tokio::test(start_paused = true)]
async fn e2e_0100_017_two_rebuilds_by_hand_a_second_apart() {
    let env = Env::new(with_delay(30)).await;
    env.state.admin.add_member(PROJ, CAROL, ALICE).await.unwrap();
    env.seed("/notes.md", "alpha").await;
    env.stub().set_delay(5);
    let a = env.recalc(ALICE, PROJ, "/notes.md", "search_index").await.unwrap();
    env.at(1).await;
    let c = env.recalc(CAROL, PROJ, "/notes.md", "search_index").await.unwrap();
    assert_ne!(a["job_id"], c["job_id"]);
    env.at(6).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(by_id(&jobs, &a["job_id"]).expect("Alice's job")["state"], "superseded");
    let cs = by_id(&jobs, &c["job_id"]).expect("Carol's job")["state"].as_str().unwrap();
    assert!(["waiting", "running"].contains(&cs), "{jobs:?}");
    env.at(15).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_eq!(jobs[0]["job_id"], c["job_id"]);
    assert_eq!(jobs[0]["state"], "done");
    assert_eq!(env.stub().max_running(), 1);
}

/// SPEC-0100/E2E-026: a rebuild by hand replaces the waiting one.
#[tokio::test(start_paused = true)]
async fn e2e_0100_026_a_rebuild_by_hand_replaces_the_waiting_one() {
    let env = Env::new(with_delay(30)).await;
    env.save("/notes.md", "alpha").await;
    let waiting = env.jobs("/notes.md").await;
    assert_eq!(waiting[0]["cause"], "change");
    env.at(5).await;
    env.recalc(ALICE, PROJ, "/notes.md", "search_index").await.unwrap();
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_eq!(jobs[0]["cause"], "manual");
    assert!(by_id(&jobs, &waiting[0]["job_id"]).is_none());
    env.at(60).await;
    assert_eq!(env.starts_of("/notes.md").len(), 1, "no extra job after the delay");
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);
}

/// SPEC-0100/E2E-028: search_index where indexing is not active.
#[tokio::test(start_paused = true)]
async fn e2e_0100_028_search_index_where_indexing_is_not_active() {
    let env = Env::new(with_delay(30)).await;
    env.state.admin.create_project("plain", ALICE).await.unwrap();
    env.state.stores.provision_volume("plain").await.unwrap();
    env.save_in("plain", "/x.md", "text").await;
    let r = env.recalc(ALICE, "plain", "/x.md", "search_index").await;
    assert_refused(r, "ERR_NOT_SUPPORTED: search indexing is not active for project plain");
    env.assert_nothing_started(&[]).await;
}

/// SPEC-0100/E2E-039: a rebuild by hand while one runs.
#[tokio::test(start_paused = true)]
async fn e2e_0100_039_a_rebuild_by_hand_while_one_runs() {
    let env = Env::new(with_delay(3)).await;
    env.seed("/notes.md", "alpha").await;
    env.stub().set_delay(10);
    env.save("/notes.md", "beta").await;
    env.at(5).await;
    let first = env.jobs("/notes.md").await;
    assert_eq!(states(&first), ["running"]);
    let v = env.recalc(ALICE, PROJ, "/notes.md", "search_index").await.unwrap();
    assert_eq!(v["state"], "waiting", "{v}");
    assert_eq!(v["seconds_left"], 0);
    assert_eq!(v["cause"], "manual");
    assert_ne!(v["job_id"], first[0]["job_id"]);
    env.at(14).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(by_id(&jobs, &first[0]["job_id"]).unwrap()["state"], "superseded", "{jobs:?}");
    assert_eq!(by_id(&jobs, &v["job_id"]).unwrap()["state"], "running", "{jobs:?}");
    let starts = env.starts_of("/notes.md");
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert!((13.0..=14.0).contains(&starts[1]), "starts when the first ends: {starts:?}");
    env.at(30).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["job_id"], v["job_id"]);
    assert_eq!(jobs[0]["state"], "done");
    assert!(env.finds("beta", "/notes.md").await);
    assert_eq!(env.stub().max_running(), 1);
}

/// SPEC-0100/E2E-056: a failed manual rebuild is reported.
#[tokio::test(start_paused = true)]
async fn e2e_0100_056_a_failed_manual_rebuild_is_reported() {
    let env = Env::new(with_delay(30)).await;
    env.seed("/notes.md", "alpha").await;
    env.stub().set_down(true);
    let v = env.recalc(ALICE, PROJ, "/notes.md", "search_index").await.unwrap();
    match v["state"].as_str().unwrap() {
        "running" => {}
        "failed" => assert_eq!(v["reason"], "embedding service unreachable"),
        s => panic!("unexpected state {s}: {v}"),
    }
    env.at(5).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(states(&jobs), ["failed"]);
    assert_eq!(jobs[0]["reason"], "embedding service unreachable");
    assert!(env.finds("alpha", "/notes.md").await, "the previous entries stay");
}

/// SPEC-0100/E2E-057: a second rebuild by hand while the first runs.
#[tokio::test(start_paused = true)]
async fn e2e_0100_057_a_second_rebuild_by_hand_while_the_first_runs() {
    let env = Env::new(with_delay(30)).await;
    env.state.admin.add_member(PROJ, CAROL, ALICE).await.unwrap();
    env.seed("/notes.md", "alpha").await;
    env.stub().set_delay(10);
    let a = env.recalc(ALICE, PROJ, "/notes.md", "search_index").await.unwrap();
    assert_eq!(a["state"], "running");
    env.at(2).await;
    let c = env.recalc(CAROL, PROJ, "/notes.md", "search_index").await.unwrap();
    assert_ne!(a["job_id"], c["job_id"]);
    assert_eq!(c["state"], "waiting");
    assert_eq!(c["seconds_left"], 0);
    env.at(11).await;
    let jobs = env.jobs("/notes.md").await;
    assert_eq!(by_id(&jobs, &a["job_id"]).unwrap()["state"], "superseded", "{jobs:?}");
    assert_eq!(by_id(&jobs, &c["job_id"]).unwrap()["state"], "running", "{jobs:?}");
    env.at(25).await;
    assert_eq!(states(&env.jobs("/notes.md").await), ["done"]);
    assert_eq!(env.stub().max_running(), 1);
}

/// SPEC-0100/E2E-059: the project does not exist.
#[tokio::test(start_paused = true)]
async fn e2e_0100_059_the_project_does_not_exist() {
    let env = Env::new(with_delay(30)).await;
    assert_not_found(env.recalc(ALICE, "ghost", "/a.md", "search_index").await);
    assert_not_found(env.list(ALICE, "ghost", "", "").await);
    env.assert_nothing_started(&[]).await;
}

/// SPEC-0100/E2E-069: a rebuild by hand when search is not enabled.
#[tokio::test(start_paused = true)]
async fn e2e_0100_069_a_rebuild_by_hand_when_search_is_not_enabled() {
    let env = Env::new(Options { search: false, ..Options::default() }).await;
    // SPEC-0101/E2E-MOD-003: the refusal comes after "path absent", so the file exists.
    env.save("/notes.md", "alpha").await;
    let r = env.recalc(ALICE, PROJ, "/notes.md", "search_index").await;
    assert_refused(
        r,
        "ERR_NOT_SUPPORTED: search is not enabled; set search.enabled: true in config",
    );
    let r = env.recalc(ALICE, PROJ, "/ghost.md", "search_index").await;
    assert_refused(r, "ERR_NOT_FOUND: file not found: /ghost.md");
    let rows = env.jobs.load_unfinished(None).await.unwrap();
    assert!(rows.is_empty(), "no job started: {rows:?}");
}

/// SPEC-0100/E2E-076: a rebuild by hand on a folder.
#[tokio::test(start_paused = true)]
async fn e2e_0100_076_a_rebuild_by_hand_on_a_folder() {
    let env = Env::new(with_delay(30)).await;
    env.state.stores.client(PROJ).await.unwrap().mkdir("/docs").await.unwrap();
    let r = env.recalc(ALICE, PROJ, "/docs", "search_index").await;
    assert_refused(r, "ERR_INVALID_ARGUMENT: path must be a file");
    let r = env.recalc(ALICE, PROJ, "/", "search_index").await;
    assert_refused(r, "ERR_INVALID_ARGUMENT: path must be a file");
    env.assert_nothing_started(&[]).await;
}

/// FR-NEW-009 story check: the refusals come in the contract order, so a
/// request wrong on every count names the first rule it breaks.
#[tokio::test(start_paused = true)]
async fn e2e_0100_us0273_refusal_order() {
    let env = Env::new(with_delay(30)).await;
    // Not a member wins over a bad kind and a missing file.
    let r = env.recalc(DAVE, PROJ, "/nope.md", "chunks").await;
    assert_refused(r, "ERR_FORBIDDEN: 'dave@example.com' is not a member of 'proj'");
    // A bad kind wins over a missing file.
    let r = env.recalc(ALICE, PROJ, "/nope.md", "concepts").await;
    assert_refused(r, "ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'");
    // A missing file wins over indexing off.
    env.state.admin.create_project("plain", ALICE).await.unwrap();
    env.state.stores.provision_volume("plain").await.unwrap();
    let r = env.recalc(ALICE, "plain", "/nope.md", "search_index").await;
    assert_refused(r, "ERR_NOT_FOUND: file not found: /nope.md");
    // SPEC-0101/FR-MOD-003: a bad kind and a missing file win over search off.
    let off = Env::new(Options { search: false, ..Options::default() }).await;
    let r = off.recalc(ALICE, PROJ, "/nope.md", "concepts").await;
    assert_refused(r, "ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'");
    let r = off.recalc(ALICE, PROJ, "/nope.md", "search_index").await;
    assert_refused(r, "ERR_NOT_FOUND: file not found: /nope.md");
}
