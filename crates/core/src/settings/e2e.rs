//! SPEC-0102 functional tests of the registry, driven through the real
//! `McpServer` admin handlers on a platform whose admin store is a SQLite file,
//! so a restart reopens the same rows. Every answer and refusal is kept, and
//! every log line is captured, so a test can search all of them for a secret.

use crate::config::ServerConfig;
use crate::mcp::server::{
    ClearDefaultModelArgs, CreateModelArgs, CreateProviderArgs, ListModelsArgs, MarkerArgs,
    McpServer, ModelIdArgs, ProviderIdArgs, ReadArgs, SetDefaultModelArgs, UpdateModelArgs,
    UpdateProviderArgs,
};
use crate::safety::SafetyManager;
use crate::search::derived_e2e::{args, tool_json};
use crate::state::AppState;
use crate::storage::rel::{Query, RelationalDb, SqliteRelationalDb};
use crate::storage::traits::AdminBackend;
use base64::Engine;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

const ALICE: &str = "admin@example.com";
const BOB: &str = "bob@example.com";
const PROJ: &str = "proj";
const SECRET: &str = "sk-test-123";
pub(crate) const KEY: [u8; 32] = [42u8; 32];

/// The model name of the configuration's embedding block.
const YAML_MODEL: &str = "yaml-model";

/// The search a test platform runs: `search.mode` and the YAML embedding URL.
#[derive(Clone)]
pub(crate) struct Searching {
    pub mode: &'static str,
    pub yaml_url: String,
    /// The `search.reranking` block's URL, when the configuration has it on.
    pub rerank_url: Option<String>,
}

fn key_b64() -> String {
    base64::engine::general_purpose::STANDARD.encode(KEY)
}

struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The registry fixture: Alice is the platform administrator, Bob an ordinary
/// member who owns `proj`.
pub(crate) struct Fx {
    pub(super) dir: tempfile::TempDir,
    pub(super) state: Arc<AppState>,
    seen: Mutex<Vec<String>>,
    logs: Arc<Mutex<Vec<u8>>>,
    /// `search.embedding.dimensions` of the configuration the platform runs on.
    dimension: u32,
    /// The search the platform runs, if any (US-0286).
    search: Option<Searching>,
    _guard: tracing::subscriber::DefaultGuard,
}

impl Fx {
    /// A platform with the master key configured.
    pub async fn new() -> Self {
        Self::with_key(Some(KEY)).await
    }

    pub async fn with_key(key: Option<[u8; 32]>) -> Self {
        Self::build(key, 1536).await
    }

    /// A platform whose vector store has `dimension` (DEC-012).
    pub async fn with_dimension(dimension: u32) -> Self {
        Self::build(Some(KEY), dimension).await
    }

    /// A platform with the master key, vectors of 1024 and `search` running.
    #[cfg(feature = "rag")]
    pub async fn searching(search: Searching) -> Self {
        Self::build_with(Some(KEY), 1024, Some(search)).await
    }

    async fn build(key: Option<[u8; 32]>, dimension: u32) -> Self {
        Self::build_with(key, dimension, None).await
    }

    async fn build_with(key: Option<[u8; 32]>, dimension: u32, search: Option<Searching>) -> Self {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || Capture(writer.clone()))
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .finish();
        let guard = tracing::subscriber::set_default(subscriber);
        let dir = tempfile::tempdir().unwrap();
        let state = Self::state(dir.path(), key, dimension, search.as_ref()).await;
        Self { dir, state, seen: Mutex::new(Vec::new()), logs, dimension, search, _guard: guard }
    }

    async fn state(
        root: &std::path::Path,
        key: Option<[u8; 32]>,
        dimension: u32,
        searching: Option<&Searching>,
    ) -> Arc<AppState> {
        let mut config = ServerConfig::default();
        config.infra.meta.dir = root.join("volumes").display().to_string();
        config.infra.blob.dir = root.join("blobs").display().to_string();
        // A keypair per platform directory, kept over a restart, so the REST
        // tests can mint a bearer token for either person.
        let keys = root.join("keys");
        let public = if keys.join(crate::keys::PUBLIC_KEY_FILE).exists() {
            keys.join(crate::keys::PUBLIC_KEY_FILE)
        } else {
            crate::keys::write_keypair(&keys).unwrap().1
        };
        config.auth.jwt.public_key_path = public.display().to_string();
        config.auth.admins = vec![ALICE.into()];
        config.search.enabled = false;
        config.search.embedding.dimensions = dimension;
        if let Some(s) = searching {
            config.search.enabled = true;
            config.search.mode = s.mode.into();
            config.search.tantivy_dir = root.join("search").display().to_string();
            config.search.embedding.endpoint = s.yaml_url.clone();
            config.search.embedding.model = YAML_MODEL.into();
            config.search.settle_delay_seconds = "0".into();
            if let Some(url) = &s.rerank_url {
                config.search.reranking.enabled = true;
                config.search.reranking.endpoint = url.clone();
                config.search.reranking.model = "yaml-reranker".into();
            }
        }
        let config = Arc::new(config);
        let db = Arc::new(SqliteRelationalDb::open(root.join("admin.db")).unwrap());
        let admin = Arc::new(crate::storage::admin::RelationalAdminStore::new(db));
        admin.connect().await.unwrap();
        admin.set_master_key(key);
        if admin.get_project(PROJ).await.unwrap().is_none() {
            admin.create_project(PROJ, BOB).await.unwrap();
        }
        let stores = Arc::new(crate::storage::StoreManager::new(
            config.clone(),
            crate::storage::test_registry(),
        ));
        let safety = Arc::new(SafetyManager::new(config.safety.clone()));
        safety.attach_limits(admin.clone());
        let search = match searching {
            Some(_) => Self::search_of(&config, admin.clone(), stores.clone()).await,
            None => None,
        };
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
        // A searching platform starts with an empty project, so the full index
        // that follows a mode change calls no embedding service.
        if searching.is_none() {
            let client = state.stores.client(PROJ).await.unwrap();
            client.write_text_atomic("/a.txt", "hello").await.unwrap();
        }
        state
    }

    /// The engine as `app.rs` builds it: the configured engines, the registry
    /// as the embedding source, the rebuild scheduler around them.
    async fn search_of(
        config: &ServerConfig,
        admin: Arc<crate::storage::admin::RelationalAdminStore>,
        stores: Arc<crate::storage::StoreManager>,
    ) -> Option<Arc<dyn crate::search::SearchBackend>> {
        #[cfg(feature = "rag")]
        crate::storage::sqlite::register_sqlite_vec();
        let jobs = admin.derived_jobs().unwrap();
        let targets = Arc::new(crate::settings::RegistryTargets {
            admin,
            fallback: config.search.embedding.clone(),
        });
        let engine = crate::search::build_backend(
            config,
            &crate::storage::test_registry(),
            Arc::new(reqwest::Client::new()),
            targets,
        )
        .await
        .unwrap()
        .unwrap();
        Some(Arc::new(crate::search::ScheduledBackend::new(
            engine,
            jobs,
            stores,
            config.search.settle_delay(),
            config.search.chunking(),
        )))
    }

    /// The operator restarts the platform over the same files with the master
    /// key and another search configuration.
    #[cfg(feature = "rag")]
    pub async fn restart_searching(&mut self, search: Searching) {
        self.search = Some(search);
        self.restart(Some(KEY)).await;
    }

    /// The operator restarts the platform over the same files, with `key`.
    pub async fn restart(&mut self, key: Option<[u8; 32]>) {
        self.state = Self::state(self.dir.path(), key, self.dimension, self.search.as_ref()).await;
    }

    /// One REST call as `who`: the status and the JSON body (`Null` when empty).
    pub(crate) async fn rest(
        &self,
        who: &str,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (axum::http::StatusCode, Value) {
        use tower::ServiceExt;
        let token = crate::keys::mint_token_from_file(
            self.dir.path().join("keys").join(crate::keys::PRIVATE_KEY_FILE),
            who,
            crate::keys::DEFAULT_ISSUER,
            crate::keys::DEFAULT_CLAIM,
            3600,
        )
        .unwrap();
        let mut req = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header("Authorization", format!("Bearer {token}"));
        let body = match body {
            Some(v) => {
                req = req.header("Content-Type", "application/json");
                axum::body::Body::from(v.to_string())
            }
            None => axum::body::Body::empty(),
        };
        let res =
            crate::api::router(self.state.clone()).oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&bytes).to_string();
        // The refusals and the answers of the REST plane are searched for a secret too.
        self.seen.lock().unwrap().push(text.clone());
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    pub(crate) fn server(&self, person: &str) -> McpServer {
        McpServer::new(self.state.clone(), person.to_string())
    }

    /// The refusal as the registry worded it, without the MCP wrapper's
    /// `An error occurred invoking '<tool>': ` lead.
    pub(crate) fn keep(&self, out: Result<Value, String>) -> Result<Value, String> {
        self.seen.lock().unwrap().push(match &out {
            Ok(v) => v.to_string(),
            Err(e) => e.clone(),
        });
        out.map_err(|e| match e.split_once("': ") {
            Some((lead, rest)) if lead.starts_with("An error occurred invoking '") => rest.into(),
            _ => e,
        })
    }

    pub async fn create_as(&self, who: &str, a: Value) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<CreateProviderArgs> = args(a);
        let out = tool_json(self.server(who).admin_create_provider(p).await);
        self.keep(out)
    }

    pub async fn create(&self, a: Value) -> Result<Value, String> {
        self.create_as(ALICE, a).await
    }

    /// A `none` provider of that name.
    pub async fn local(&self, name: &str) -> Value {
        self.create(json!({"name": name, "endpoint": "http://localhost:9010/v1"})).await.unwrap()
    }

    /// `Cloud Embeddings`, bearer, holding [`SECRET`].
    pub async fn cloud(&self) -> Value {
        self.create(json!({"name": "Cloud Embeddings", "endpoint": "https://api.example.com/v1",
            "auth_scheme": "bearer", "secret": SECRET}))
            .await
            .unwrap()
    }

    pub async fn update_as(&self, who: &str, a: Value) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<UpdateProviderArgs> = args(a);
        let out = tool_json(self.server(who).admin_update_provider(p).await);
        self.keep(out)
    }

    pub async fn update(&self, a: Value) -> Result<Value, String> {
        self.update_as(ALICE, a).await
    }

    pub async fn delete_as(&self, who: &str, id: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<ProviderIdArgs> =
            args(json!({"provider_id": id}));
        let out = tool_json(self.server(who).admin_delete_provider(p).await);
        self.keep(out)
    }

    pub async fn list_as(&self, who: &str, marker: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<MarkerArgs> =
            args(json!({"marker": marker}));
        let out = tool_json(self.server(who).admin_list_providers(p).await);
        self.keep(out)
    }

    pub async fn changes_as(&self, who: &str, marker: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<MarkerArgs> =
            args(json!({"marker": marker}));
        let out = tool_json(self.server(who).admin_list_setting_changes(p).await);
        self.keep(out)
    }

    pub async fn model_as(&self, who: &str, a: Value) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<CreateModelArgs> = args(a);
        let out = tool_json(self.server(who).admin_create_model(p).await);
        self.keep(out)
    }

    pub async fn model(&self, a: Value) -> Result<Value, String> {
        self.model_as(ALICE, a).await
    }

    /// A model of `provider_id`: embedding kinds get dimension 1024.
    pub async fn model_of(&self, provider_id: &str, name: &str, kind: &str) -> Value {
        let mut a = json!({"provider_id": provider_id, "name": name, "kind": kind});
        if kind.ends_with("_embedding") {
            a["dimension"] = json!(1024);
        }
        self.model(a).await.unwrap()
    }

    pub async fn update_model_as(&self, who: &str, a: Value) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<UpdateModelArgs> = args(a);
        let out = tool_json(self.server(who).admin_update_model(p).await);
        self.keep(out)
    }

    pub async fn update_model(&self, a: Value) -> Result<Value, String> {
        self.update_model_as(ALICE, a).await
    }

    pub async fn delete_model_as(&self, who: &str, id: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<ModelIdArgs> =
            args(json!({"model_id": id}));
        let out = tool_json(self.server(who).admin_delete_model(p).await);
        self.keep(out)
    }

    pub async fn models_as(
        &self,
        who: &str,
        provider_id: &str,
        kind: &str,
        marker: &str,
    ) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<ListModelsArgs> =
            args(json!({"provider_id": provider_id, "kind": kind, "marker": marker}));
        let out = tool_json(self.server(who).admin_list_models(p).await);
        self.keep(out)
    }

    pub async fn set_default_as(&self, who: &str, kind: &str, id: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<SetDefaultModelArgs> =
            args(json!({"kind": kind, "model_id": id}));
        let out = tool_json(self.server(who).admin_set_default_model(p).await);
        self.keep(out)
    }

    pub async fn set_default(&self, kind: &str, id: &str) -> Result<Value, String> {
        self.set_default_as(ALICE, kind, id).await
    }

    pub async fn clear_default_as(&self, who: &str, kind: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<ClearDefaultModelArgs> =
            args(json!({"kind": kind}));
        let out = tool_json(self.server(who).admin_clear_default_model(p).await);
        self.keep(out)
    }

    pub async fn clear_default(&self, kind: &str) -> Result<Value, String> {
        self.clear_default_as(ALICE, kind).await
    }

    pub async fn defaults_as(&self, who: &str) -> Result<Value, String> {
        let out = tool_json(self.server(who).admin_get_default_models().await);
        self.keep(out)
    }

    pub async fn defaults(&self) -> Value {
        self.defaults_as(ALICE).await.unwrap()
    }

    /// Every model, all pages walked.
    pub async fn models(&self) -> Vec<Value> {
        let mut out = Vec::new();
        let mut marker = String::new();
        loop {
            let page = self.models_as(ALICE, "", "", &marker).await.unwrap();
            out.extend(page["models"].as_array().unwrap().iter().cloned());
            match page["marker"].as_str() {
                Some(m) => marker = m.to_string(),
                None => return out,
            }
        }
    }

    /// Every provider, all pages walked.
    pub async fn providers(&self) -> Vec<Value> {
        let mut out = Vec::new();
        let mut marker = String::new();
        loop {
            let page = self.list_as(ALICE, &marker).await.unwrap();
            out.extend(page["providers"].as_array().unwrap().iter().cloned());
            match page["marker"].as_str() {
                Some(m) => marker = m.to_string(),
                None => return out,
            }
        }
    }

    /// Every change, newest first, all pages walked.
    pub async fn changes(&self) -> Vec<Value> {
        let mut out = Vec::new();
        let mut marker = String::new();
        loop {
            let page = self.changes_as(ALICE, &marker).await.unwrap();
            out.extend(page["changes"].as_array().unwrap().iter().cloned());
            match page["marker"].as_str() {
                Some(m) => marker = m.to_string(),
                None => return out,
            }
        }
    }

    fn names(list: &[Value]) -> Vec<String> {
        list.iter().map(|p| p["name"].as_str().unwrap().to_string()).collect()
    }

    /// The raw `secret_enc` of every provider row, read beside the store.
    async fn secret_blobs(&self) -> Vec<Option<Vec<u8>>> {
        let db = SqliteRelationalDb::open(self.dir.path().join("admin.db")).unwrap();
        let rows = db.query(&Query::new("SELECT secret_enc FROM provider")).await.unwrap();
        rows.iter()
            .map(|r| match r.value(0).unwrap() {
                crate::storage::rel::SqlValue::Null => None,
                _ => Some(r.blob(0).unwrap()),
            })
            .collect()
    }

    /// The secret of every converter row as decrypted with the fixture key,
    /// `None` for a row without one.
    pub(crate) async fn converter_secrets(&self) -> Vec<Option<String>> {
        let db = SqliteRelationalDb::open(self.dir.path().join("admin.db")).unwrap();
        let rows = db.query(&Query::new("SELECT secret_enc FROM converter")).await.unwrap();
        rows.iter()
            .map(|r| match r.value(0).unwrap() {
                crate::storage::rel::SqlValue::Null => None,
                _ => Some(crate::git::oauth::cipher::decrypt(&KEY, &r.blob(0).unwrap()).unwrap()),
            })
            .collect()
    }

    /// `needle` is in no answer, refusal, listing, record or log line.
    pub async fn assert_nowhere(&self, needle: &str) {
        let lists = json!([self.providers().await, self.changes().await]).to_string();
        assert!(!lists.contains(needle), "a listing or record holds the secret");
        for text in self.seen.lock().unwrap().iter() {
            assert!(!text.contains(needle), "an answer holds the secret: {text}");
        }
        let logs = String::from_utf8_lossy(&self.logs.lock().unwrap()).to_string();
        assert!(!logs.contains(needle), "a log line holds the secret");
    }
}

fn is_rfc3339_seconds(t: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(t).is_ok() && t.len() == 20 && t.ends_with('Z')
}

#[tokio::test]
async fn e2e_0102_001_registering_a_local_provider() {
    let fx = Fx::new().await;
    let p = fx.create(json!({"name": "Local GPU", "endpoint": "http://localhost:9010/v1"})).await;
    let p = p.unwrap();
    assert!(p["provider_id"].as_str().unwrap().starts_with("prov-"));
    assert_eq!(p["provider_id"].as_str().unwrap().len(), 17);
    assert_eq!(p["name"], "Local GPU");
    assert_eq!(p["endpoint"], "http://localhost:9010/v1");
    assert_eq!(p["auth_scheme"], "none");
    assert_eq!(p["has_secret"], false);
    assert_eq!(p["created_at"], p["updated_at"]);
    assert!(is_rfc3339_seconds(p["created_at"].as_str().unwrap()), "{}", p["created_at"]);
    let page = fx.list_as(ALICE, "").await.unwrap();
    assert_eq!(page["providers"], json!([p]));
    assert!(page.get("marker").is_none(), "no marker when none remain");
}

#[tokio::test]
async fn e2e_0102_002_registering_a_cloud_provider_with_a_secret() {
    let fx = Fx::new().await;
    let p = fx.cloud().await;
    assert_eq!(p["has_secret"], true);
    assert_eq!(p["auth_scheme"], "bearer");
    assert!(!p.to_string().contains(SECRET));
    let list = fx.providers().await;
    assert_eq!(list[0]["has_secret"], true);
    let blobs = fx.secret_blobs().await;
    let blob = blobs[0].as_ref().expect("the secret is stored");
    assert!(!blob.windows(SECRET.len()).any(|w| w == SECRET.as_bytes()), "stored encrypted");
    assert_eq!(crate::git::oauth::cipher::decrypt(&KEY, blob).unwrap(), SECRET);
    fx.assert_nowhere(SECRET).await;
}

#[tokio::test]
async fn e2e_0102_004_a_provider_name_already_taken() {
    let fx = Fx::new().await;
    fx.local("Local GPU").await;
    let e = fx.create(json!({"name": "local gpu", "endpoint": "http://h/v1"})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: a provider named 'local gpu' already exists");
    assert_eq!(fx.providers().await.len(), 1);
}

#[tokio::test]
async fn e2e_0102_005_the_name_is_trimmed() {
    let fx = Fx::new().await;
    let p = fx.local("  Café Embeddings  ").await;
    assert_eq!(p["name"], "Café Embeddings");
    let e = fx.create(json!({"name": "Café Embeddings", "endpoint": "http://h/v1"})).await;
    assert_eq!(
        e.unwrap_err(),
        "ERR_INVALID_ARGUMENT: a provider named 'Café Embeddings' already exists"
    );
}

#[tokio::test]
async fn e2e_0102_010_replacing_a_secret() {
    let fx = Fx::new().await;
    let before = fx.cloud().await;
    // Times have one second resolution: wait so a later `updated_at` shows.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let id = before["provider_id"].as_str().unwrap();
    let after = fx.update(json!({"provider_id": id, "secret": "sk-new-456"})).await.unwrap();
    assert_eq!(after["has_secret"], true);
    assert_eq!(after["provider_id"], before["provider_id"]);
    assert_eq!(after["created_at"], before["created_at"]);
    assert!(after["updated_at"].as_str().unwrap() > before["updated_at"].as_str().unwrap());
    // The outbound call is US-0286 (E2E-038); here the stored secret is the new one.
    let blobs = fx.secret_blobs().await;
    let blob = blobs[0].as_ref().unwrap();
    assert_eq!(crate::git::oauth::cipher::decrypt(&KEY, blob).unwrap(), "sk-new-456");
    fx.assert_nowhere("sk-new-456").await;
    fx.assert_nowhere(SECRET).await;
}

#[tokio::test]
async fn e2e_0102_011_removing_a_secret() {
    let fx = Fx::new().await;
    let id = fx.cloud().await["provider_id"].as_str().unwrap().to_string();
    let e = fx.update(json!({"provider_id": id, "clear_secret": true})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: auth_scheme 'bearer' needs a secret");
    assert_eq!(fx.providers().await[0]["has_secret"], true, "the refusal changed nothing");
    let p = fx
        .update(json!({"provider_id": id, "clear_secret": true, "auth_scheme": "none"}))
        .await
        .unwrap();
    assert_eq!(p["has_secret"], false);
    assert_eq!(p["auth_scheme"], "none");
    assert_eq!(fx.secret_blobs().await, vec![None], "the secret is gone from the row");
}

#[tokio::test]
async fn e2e_0102_012_no_master_key() {
    let fx = Fx::with_key(None).await;
    let e = fx
        .create(json!({"name": "Cloud Embeddings", "endpoint": "https://api.example.com/v1",
            "auth_scheme": "bearer", "secret": SECRET}))
        .await
        .unwrap_err();
    assert_eq!(e, "ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY");
    fx.local("Local GPU").await;
    assert_eq!(Fx::names(&fx.providers().await), ["Local GPU"]);
    fx.assert_nowhere(SECRET).await;
}

#[tokio::test]
async fn e2e_0102_012_a_restart_without_the_key_keeps_the_providers_listed() {
    let mut fx = Fx::new().await;
    let id = fx.cloud().await["provider_id"].as_str().unwrap().to_string();
    fx.restart(None).await;
    assert_eq!(fx.providers().await[0]["has_secret"], true);
    let e = fx.update(json!({"provider_id": id, "secret": "sk-new-456"})).await.unwrap_err();
    assert_eq!(e, "ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY");
    // A change that writes no secret is still accepted without the key.
    let p = fx.update(json!({"provider_id": id, "name": "Cloud 2"})).await.unwrap();
    assert_eq!(p["has_secret"], true);
}

/// A server configuration for a real start in `root`.
fn boot_config(root: &std::path::Path) -> ServerConfig {
    let (_key, public) = crate::keys::write_keypair(root.join("keys")).unwrap();
    let mut c = ServerConfig::default();
    c.auth.jwt.public_key_path = public.display().to_string();
    c.infra.meta.dir = root.join("volumes").display().to_string();
    c.infra.blob.dir = root.join("blobs").display().to_string();
    c.infra.admin.path = root.join("admin.db").display().to_string();
    c
}

#[tokio::test]
async fn e2e_0102_013_a_malformed_master_key_stops_the_start() {
    let d = tempfile::tempdir().unwrap();
    let bad = |n: &str| (n == "FSA_TOKEN_KEY").then(|| "not-a-key".to_string());
    let e = crate::app::build_with_env(boot_config(d.path()), &bad).await.err().unwrap();
    assert!(e.to_string().starts_with("FSA_TOKEN_KEY must be valid base64."), "{e}");
    let short = |n: &str| {
        (n == "FSA_TOKEN_KEY").then(|| base64::engine::general_purpose::STANDARD.encode([1u8; 16]))
    };
    let e = crate::app::build_with_env(boot_config(d.path()), &short).await.err().unwrap();
    assert!(e.to_string().starts_with("FSA_TOKEN_KEY must decode to 32 bytes"), "{e}");

    // A valid key starts the platform, with and without the git token store,
    // and the key reaches the registry.
    let good = |n: &str| (n == "FSA_TOKEN_KEY").then(key_b64);
    for git in [false, true] {
        let mut c = boot_config(d.path());
        c.git.enabled = git;
        let (_router, state) = crate::app::build_with_env(c, &good).await.unwrap();
        assert_eq!(state.admin.settings().unwrap().key(), Some(KEY));
    }
}

const CHILD: &str = "FSA_E2E_0102_013_CHILD";

/// The real exit path: `cli::run_with` on `serve`, in a child process whose
/// environment holds the malformed key, exits non zero and prints the cipher
/// message on its error output.
#[tokio::test]
async fn e2e_0102_013_the_cli_exits_non_zero_with_the_cipher_message() {
    if let Ok(root) = std::env::var(CHILD) {
        let root = std::path::PathBuf::from(root);
        let c = boot_config(&root);
        let yaml = format!(
            "auth:\n  jwt:\n    public_key_path: '{}'\ninfra:\n  meta:\n    dir: '{}'\n  blob:\n    dir: '{}'\n  admin:\n    path: '{}'\nserver:\n  port: 0\n",
            c.auth.jwt.public_key_path, c.infra.meta.dir, c.infra.blob.dir, c.infra.admin.path
        );
        let path = root.join("config.yaml");
        std::fs::write(&path, yaml).unwrap();
        let cli = crate::cli::Cli {
            command: Some(crate::cli::Command::Serve {
                config: Some(path),
                git: false,
                web: false,
                context7: false,
                sqlite: false,
                db: false,
                doc: false,
                database_url: None,
            }),
        };
        let code = crate::cli::run_with(cli).await;
        std::process::exit(if code == std::process::ExitCode::FAILURE { 7 } else { 0 });
    }
    let d = tempfile::tempdir().unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut child = std::process::Command::new(exe)
        .args([
            "--exact",
            "settings::e2e::e2e_0102_013_the_cli_exits_non_zero_with_the_cipher_message",
            "--nocapture",
        ])
        .env(CHILD, d.path())
        .env("FSA_TOKEN_KEY", "not-a-key")
        .env_remove(concat!("MCP", "FS_TOKEN_KEY"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while child.try_wait().unwrap().is_none() {
        assert!(start.elapsed().as_secs() < 120, "the start must stop, not serve");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(7), "a non zero exit through cli::run: {stderr}");
    assert!(
        stderr.lines().any(|l| l.starts_with("error: FSA_TOKEN_KEY must be valid base64.")),
        "{stderr}"
    );
}

#[tokio::test]
async fn e2e_0102_014_a_secret_is_never_revealed() {
    let fx = Fx::new().await;
    let id = fx.cloud().await["provider_id"].as_str().unwrap().to_string();
    let long = format!("sk-{}", "z".repeat(4997));
    assert_eq!(long.chars().count(), 5000);
    let e = fx.update(json!({"provider_id": id, "secret": long})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: secret must be at most 4096 characters");
    // The REST interface and the screen arrive with US-0288 and US-0289.
    fx.assert_nowhere(SECRET).await;
    fx.assert_nowhere(&long[..64]).await;
    let blobs = fx.secret_blobs().await;
    assert_eq!(
        crate::git::oauth::cipher::decrypt(&KEY, blobs[0].as_ref().unwrap()).unwrap(),
        SECRET
    );
}

#[tokio::test]
async fn e2e_0102_015_a_secret_together_with_clear_secret() {
    let fx = Fx::new().await;
    let before = fx.cloud().await;
    let id = before["provider_id"].as_str().unwrap();
    let e = fx
        .update(json!({"provider_id": id, "secret": "sk-x", "clear_secret": true}))
        .await
        .unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: secret and clear_secret cannot be given together");
    assert_eq!(fx.providers().await, vec![before]);
    fx.assert_nowhere("sk-x").await;
}

#[tokio::test]
async fn e2e_0102_017_an_empty_secret_on_a_bearer_provider() {
    let fx = Fx::new().await;
    let e = fx
        .create(
            json!({"name": "C", "endpoint": "http://h/v1", "auth_scheme": "bearer", "secret": ""}),
        )
        .await
        .unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: auth_scheme 'bearer' needs a secret");
    assert!(fx.providers().await.is_empty());
    assert!(fx.changes().await.is_empty());
}

#[tokio::test]
async fn e2e_0102_020_editing_a_provider_applies_to_its_models() {
    let fx = Fx::new().await;
    let before = fx.local("Local GPU").await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let id = before["provider_id"].as_str().unwrap();
    let p = fx
        .update(json!({"provider_id": id, "endpoint": "http://gpu2:9010/v1", "name": "GPU 2"}))
        .await
        .unwrap();
    assert_eq!(p["name"], "GPU 2");
    assert_eq!(p["endpoint"], "http://gpu2:9010/v1");
    assert!(p["updated_at"].as_str().unwrap() > before["updated_at"].as_str().unwrap());
    // Models arrive with US-0284; the provider keeps its id, which a model names.
    assert_eq!(fx.providers().await, vec![p]);
    let rec = &fx.changes().await[0];
    assert_eq!(rec["action"], "update_provider");
    assert_eq!(rec["fields"], json!(["name", "endpoint"]));
}

#[tokio::test]
async fn e2e_0102_021_editing_an_unknown_provider() {
    let fx = Fx::new().await;
    let e = fx.update(json!({"provider_id": "prov-unknown", "name": "X"})).await.unwrap_err();
    assert_eq!(e, "ERR_NOT_FOUND: provider not found: prov-unknown");
    assert!(fx.providers().await.is_empty());
    assert!(fx.changes().await.is_empty());
}

#[tokio::test]
async fn e2e_0102_023_deleting_an_empty_provider() {
    let fx = Fx::new().await;
    let id = fx.cloud().await["provider_id"].as_str().unwrap().to_string();
    assert_eq!(fx.delete_as(ALICE, &id).await.unwrap(), json!({"deleted": true}));
    assert!(fx.providers().await.is_empty());
    assert!(fx.secret_blobs().await.is_empty(), "the row and its secret are gone");
    let rec = &fx.changes().await[0];
    assert_eq!(rec["action"], "delete_provider");
    assert_eq!(rec["fields"], json!([]));
    assert_eq!(rec["target_name"], "Cloud Embeddings");
}

#[tokio::test]
async fn e2e_0102_024_deleting_an_unknown_provider() {
    let fx = Fx::new().await;
    fx.local("Local GPU").await;
    let e = fx.delete_as(ALICE, "prov-unknown").await.unwrap_err();
    assert_eq!(e, "ERR_NOT_FOUND: provider not found: prov-unknown");
    assert_eq!(fx.providers().await.len(), 1);
    assert_eq!(fx.changes().await.len(), 1);
}

#[tokio::test]
async fn e2e_0102_050_paging_the_providers() {
    let fx = Fx::new().await;
    for i in (1..=150).rev() {
        fx.local(&format!("P{i:03}")).await;
    }
    let first = fx.list_as(ALICE, "").await.unwrap();
    let names = Fx::names(first["providers"].as_array().unwrap());
    let want: Vec<String> = (1..=100).map(|i| format!("P{i:03}")).collect();
    assert_eq!(names, want);
    let marker = first["marker"].as_str().expect("a marker when more follow");
    let second = fx.list_as(ALICE, marker).await.unwrap();
    let names = Fx::names(second["providers"].as_array().unwrap());
    let want: Vec<String> = (101..=150).map(|i| format!("P{i:03}")).collect();
    assert_eq!(names, want);
    assert!(second.get("marker").is_none());
    let e = fx.list_as(ALICE, "not a marker").await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: marker is not valid");
}

#[tokio::test]
async fn e2e_0102_051_the_order_is_by_name_without_regard_to_case() {
    let fx = Fx::new().await;
    for n in ["beta", "Alpha", "charlie"] {
        fx.local(n).await;
    }
    assert_eq!(Fx::names(&fx.providers().await), ["Alpha", "beta", "charlie"]);
    let e = fx.create(json!({"name": "ALPHA", "endpoint": "http://h/v1"})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: a provider named 'ALPHA' already exists");
}

#[tokio::test]
async fn e2e_0102_056_a_secret_change_is_recorded_as_a_name() {
    let fx = Fx::new().await;
    let id = fx.cloud().await["provider_id"].as_str().unwrap().to_string();
    fx.update(json!({"provider_id": id, "secret": "sk-new-456"})).await.unwrap();
    fx.update(json!({"provider_id": id, "clear_secret": true, "auth_scheme": "none"}))
        .await
        .unwrap();
    let changes = fx.changes().await;
    let updates: Vec<&Value> =
        changes.iter().filter(|c| c["action"] == "update_provider").collect();
    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0]["fields"], json!(["auth_scheme", "secret"]));
    assert_eq!(updates[1]["fields"], json!(["secret"]));
    for c in &changes {
        assert_eq!(c["person"], ALICE);
        assert_eq!(c["target_kind"], "provider");
        assert_eq!(c["target_id"], id.as_str());
        assert!(c["change_id"].as_str().unwrap().starts_with("chg-"));
    }
    fx.assert_nowhere("sk-new-456").await;
    fx.assert_nowhere(SECRET).await;
}

#[tokio::test]
async fn e2e_0102_057_a_refused_operation_leaves_no_record() {
    let fx = Fx::new().await;
    let e = fx.create(json!({"name": "F", "endpoint": "ftp://x"})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: endpoint must be an http or https URL");
    assert!(fx.changes().await.is_empty());
    // A duplicate needs a first provider, whose creation is the one record.
    fx.local("Local GPU").await;
    let before = fx.changes().await;
    assert!(fx.create(json!({"name": "LOCAL GPU", "endpoint": "http://h/v1"})).await.is_err());
    assert!(fx.update(json!({"provider_id": "prov-unknown", "name": "x"})).await.is_err());
    assert!(fx.delete_as(BOB, before[0]["target_id"].as_str().unwrap()).await.is_err());
    assert_eq!(fx.changes().await, before);
}

#[tokio::test]
async fn e2e_0102_058_records_survive_a_restart() {
    let mut fx = Fx::new().await;
    let id = fx.local("Local GPU").await["provider_id"].as_str().unwrap().to_string();
    fx.update(json!({"provider_id": id, "name": "GPU"})).await.unwrap();
    fx.delete_as(ALICE, &id).await.unwrap();
    let before = fx.changes().await;
    assert_eq!(before.len(), 3);
    fx.restart(Some(KEY)).await;
    assert_eq!(fx.changes().await, before);
}

#[tokio::test]
async fn e2e_0102_059_paging_the_changes() {
    let fx = Fx::new().await;
    for i in 1..=120 {
        fx.local(&format!("P{i:03}")).await;
    }
    let first = fx.changes_as(ALICE, "").await.unwrap();
    let firsts = first["changes"].as_array().unwrap().clone();
    assert_eq!(firsts.len(), 100);
    assert_eq!(firsts[0]["target_name"], "P120");
    assert_eq!(firsts[99]["target_name"], "P021");
    let second = fx.changes_as(ALICE, first["marker"].as_str().unwrap()).await.unwrap();
    let seconds = second["changes"].as_array().unwrap();
    assert_eq!(seconds.len(), 20);
    assert_eq!(seconds[19]["target_name"], "P001");
    assert!(second.get("marker").is_none());
    // Many share their second: the ids still order them, newest first.
    let ids: Vec<&str> =
        firsts.iter().chain(seconds.iter()).map(|c| c["change_id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] > w[1]), "change ids strictly descending");
    assert_eq!(ids[0], "chg-000000000120");
}

#[tokio::test]
async fn e2e_0102_062_administrator_identity_is_compared_without_regard_to_case() {
    let fx = Fx::new().await;
    fx.local("Local GPU").await;
    assert_eq!(
        fx.list_as("Admin@Example.COM", "").await.unwrap()["providers"].as_array().unwrap().len(),
        1
    );
    let p =
        fx.create_as("Admin@Example.COM", json!({"name": "B", "endpoint": "http://h/v1"})).await;
    assert!(p.is_ok());
    assert_eq!(fx.changes().await[0]["person"], ALICE, "the person is recorded in lower case");
    let refused = "ERR_FORBIDDEN: 'carol@example.com' is not a platform admin";
    assert_eq!(fx.list_as("carol@example.com", "").await.unwrap_err(), refused);
    // Every operation, reading included, is refused to an ordinary member.
    let bob = format!("ERR_FORBIDDEN: '{BOB}' is not a platform admin");
    let id = fx.providers().await[0]["provider_id"].as_str().unwrap().to_string();
    let c = fx.create_as(BOB, json!({"name": "Z", "endpoint": "http://h/v1"})).await;
    assert_eq!(c.unwrap_err(), bob);
    assert_eq!(fx.update_as(BOB, json!({"provider_id": id, "name": "Z"})).await.unwrap_err(), bob);
    assert_eq!(fx.delete_as(BOB, &id).await.unwrap_err(), bob);
    assert_eq!(fx.list_as(BOB, "").await.unwrap_err(), bob);
    assert_eq!(fx.changes_as(BOB, "").await.unwrap_err(), bob);
    assert_eq!(fx.providers().await.len(), 2);
    assert_eq!(fx.changes().await.len(), 2);
}

#[tokio::test]
async fn e2e_0102_063_a_platform_administrator_is_not_a_project_member() {
    let fx = Fx::new().await;
    assert!(fx.list_as(ALICE, "").await.is_ok());
    let read = |who: &str| {
        let p: rmcp::handler::server::wrapper::Parameters<ReadArgs> =
            args(json!({"mount_id": PROJ, "path": "/a.txt"}));
        let server = fx.server(who);
        async move { tool_json(server.fs_read(p).await) }
    };
    let e = read(ALICE).await.unwrap_err();
    assert!(e.ends_with("ERR_FORBIDDEN: 'admin@example.com' is not a member of 'proj'"), "{e}");
    assert!(read(BOB).await.unwrap().to_string().contains("hello"));
}

#[tokio::test]
async fn e2e_0102_090_invalid_provider_fields() {
    let fx = Fx::new().await;
    let cases = [
        (json!({"name": "", "endpoint": "http://h/v1"}), "name must be 1 to 64 characters"),
        (
            json!({"name": "n".repeat(65), "endpoint": "http://h/v1"}),
            "name must be 1 to 64 characters",
        ),
        (
            json!({"name": "a", "endpoint": "localhost:9010"}),
            "endpoint must be an http or https URL",
        ),
        (
            json!({"name": "a", "endpoint": "http://h/v1", "auth_scheme": "basic"}),
            "auth_scheme must be 'none' or 'bearer'",
        ),
        (
            json!({"name": "a", "endpoint": "http://h/v1", "auth_scheme": "none", "secret": "x"}),
            "auth_scheme 'none' takes no secret",
        ),
        (
            json!({"name": "a", "endpoint": "http://h/v1", "auth_scheme": "bearer"}),
            "auth_scheme 'bearer' needs a secret",
        ),
    ];
    for (input, text) in cases {
        let e = fx.create(input).await.unwrap_err();
        assert_eq!(e, format!("ERR_INVALID_ARGUMENT: {text}"));
    }
    assert!(fx.providers().await.is_empty());
    assert!(fx.changes().await.is_empty());
    // An explicit empty scheme is refused; only an absent one defaults to none.
    let e = fx
        .create(json!({"name": "a", "endpoint": "http://h/v1", "auth_scheme": ""}))
        .await
        .unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: auth_scheme must be 'none' or 'bearer'");
    let (s, b) = fx
        .rest(
            ALICE,
            "POST",
            "/api/admin/providers",
            Some(json!({"name": "a", "endpoint": "http://h/v1", "auth_scheme": ""})),
        )
        .await;
    refusal(s, &b, "ERR_INVALID_ARGUMENT: auth_scheme must be 'none' or 'bearer'");
    // A blank name is empty once trimmed.
    let e = fx.create(json!({"name": "   ", "endpoint": "http://h/v1"})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: name must be 1 to 64 characters");
}

#[tokio::test]
async fn e2e_0102_091_limits_of_a_provider_name() {
    let fx = Fx::new().await;
    fx.local("a").await;
    fx.local(&"é".repeat(64)).await;
    let e = fx.create(json!({"name": "é".repeat(65), "endpoint": "http://h/v1"})).await;
    assert_eq!(e.unwrap_err(), "ERR_INVALID_ARGUMENT: name must be 1 to 64 characters");
    assert_eq!(fx.providers().await.len(), 2);
}

#[tokio::test]
async fn e2e_0102_095_an_update_is_checked_as_a_whole() {
    let fx = Fx::new().await;
    let cloud = fx.cloud().await;
    fx.local("Local GPU").await;
    let id = cloud["provider_id"].as_str().unwrap();
    let e = fx.update(json!({"provider_id": id, "name": "local gpu"})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: a provider named 'local gpu' already exists");
    let e = fx.update(json!({"provider_id": id, "auth_scheme": "none"})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: auth_scheme 'none' takes no secret");
    assert!(fx.providers().await.contains(&cloud), "the provider is unchanged");
    assert_eq!(fx.changes().await.len(), 2);
}

#[tokio::test]
async fn e2e_0102_110_renaming_a_provider_with_other_case_of_its_own_name() {
    let fx = Fx::new().await;
    let id = fx.local("Local GPU").await["provider_id"].as_str().unwrap().to_string();
    fx.local("Other").await;
    let p = fx.update(json!({"provider_id": id, "name": "LOCAL GPU"})).await.unwrap();
    assert_eq!(p["name"], "LOCAL GPU");
    let e = fx.update(json!({"provider_id": id, "name": "other"})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: a provider named 'other' already exists");
}

#[tokio::test]
async fn e2e_0102_112_an_endpoint_with_credentials() {
    let fx = Fx::new().await;
    let e = fx.create(json!({"name": "U", "endpoint": "http://user:pw@host:9010/v1"})).await;
    assert_eq!(e.unwrap_err(), "ERR_INVALID_ARGUMENT: endpoint must be an http or https URL");
    fx.assert_nowhere("pw").await;
    // The screen: the refused creation shows its refusal, and neither the page
    // nor the listing it then shows holds the password.
    let router = fx.screen();
    let (s, html) = fx
        .submit(
            &router,
            "provider/create",
            &[("name", "U"), ("endpoint", "http://user:pw@host:9010/v1"), ("auth_scheme", "none")],
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::BAD_REQUEST);
    assert!(html.contains("endpoint must be an http or https URL"), "{html}");
    assert!(!html.contains("pw"), "{html}");
    let (html, _) = fx.open(&router, ALICE).await;
    assert!(!html.contains("pw"), "{html}");
    fx.assert_nowhere("pw").await;
}

#[tokio::test]
async fn e2e_0102_115_an_update_that_changes_nothing() {
    let fx = Fx::new().await;
    let before = fx.local("Local GPU").await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let id = before["provider_id"].as_str().unwrap();
    let p = fx
        .update(
            json!({"provider_id": id, "name": "Local GPU", "endpoint": "http://localhost:9010/v1"}),
        )
        .await
        .unwrap();
    assert_eq!(p, before, "unchanged, updated_at included");
    let p = fx.update(json!({"provider_id": id})).await.unwrap();
    assert_eq!(p, before);
    assert!(fx.changes().await.iter().all(|c| c["action"] != "update_provider"));
}

fn id_of(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap().to_string()
}

#[tokio::test]
async fn e2e_0102_003_registering_models() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    let m = fx
        .model(json!({"provider_id": gpu, "name": "qwen3-embedding-0.6b", "kind": "text_embedding",
            "dimension": 1024, "max_input_tokens": 8192}))
        .await
        .unwrap();
    assert!(m["model_id"].as_str().unwrap().starts_with("mdl-"));
    assert_eq!(m["model_id"].as_str().unwrap().len(), 4 + 12);
    assert_eq!(m["kind"], "text_embedding");
    assert_eq!(m["dimension"], 1024);
    assert_eq!(m["max_input_tokens"], 8192);
    assert_eq!(m["is_default"], false);
    assert_eq!(m["provider_id"], json!(gpu));
    assert!(is_rfc3339_seconds(m["created_at"].as_str().unwrap()));
    let r = fx
        .model(json!({"provider_id": gpu, "name": "bge-reranker", "kind": "reranker"}))
        .await
        .unwrap();
    assert!(r["dimension"].is_null() && r["max_input_tokens"].is_null());
    let list = fx.models().await;
    assert_eq!(Fx::names(&list), ["bge-reranker", "qwen3-embedding-0.6b"]);
    assert!(list.iter().all(|m| m["provider_id"] == json!(gpu)));
}

#[tokio::test]
async fn e2e_0102_006_the_same_model_name_under_two_providers() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    let cloud = id_of(&fx.cloud().await, "provider_id");
    fx.model_of(&gpu, "qwen3-embedding-0.6b", "text_embedding").await;
    fx.model_of(&cloud, "qwen3-embedding-0.6b", "text_embedding").await;
    let e = fx
        .model(json!({"provider_id": gpu, "name": "QWEN3-embedding-0.6b ", "kind": "text_embedding", "dimension": 1024}))
        .await
        .unwrap_err();
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: a model named 'QWEN3-embedding-0.6b' already exists for provider 'Local GPU'"
    );
    assert_eq!(fx.models().await.len(), 2);
}

#[tokio::test]
async fn e2e_0102_022_deleting_a_provider_with_models() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    fx.model_of(&gpu, "a", "chat").await;
    fx.model_of(&gpu, "b", "ocr").await;
    let e = fx.delete_as(ALICE, &gpu).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: provider 'Local GPU' still has 2 model(s)");
    assert_eq!(fx.providers().await.len(), 1);
    assert_eq!(fx.models().await.len(), 2);
    for m in fx.models().await {
        fx.delete_model_as(ALICE, m["model_id"].as_str().unwrap()).await.unwrap();
    }
    assert_eq!(fx.delete_as(ALICE, &gpu).await.unwrap(), json!({"deleted": true}));
}

#[tokio::test]
async fn e2e_0102_025_updating_a_model() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    let id = id_of(&fx.model_of(&gpu, "bge-reranker", "reranker").await, "model_id");
    let a = fx.update_model(json!({"model_id": id, "max_input_tokens": 512})).await.unwrap();
    assert_eq!(a["max_input_tokens"], 512);
    let b = fx.update_model(json!({"model_id": id, "max_input_tokens": 0})).await.unwrap();
    assert!(b["max_input_tokens"].is_null());
    assert_eq!((&b["kind"], &b["provider_id"]), (&json!("reranker"), &json!(gpu)));
    let c = fx.update_model(json!({"model_id": id, "name": "Rerank 2"})).await.unwrap();
    assert_eq!(c["name"], "Rerank 2");
    let recs: Vec<(String, Value)> = fx
        .changes()
        .await
        .iter()
        .map(|c| (c["action"].as_str().unwrap().to_string(), c["fields"].clone()))
        .collect();
    assert_eq!(recs[0], ("update_model".into(), json!(["name"])));
    assert_eq!(recs[1], ("update_model".into(), json!(["max_input_tokens"])));
    assert_eq!(recs[2], ("update_model".into(), json!(["max_input_tokens"])));
    assert_eq!(recs[3].0, "create_model");
}

#[tokio::test]
async fn e2e_0102_025_an_update_is_checked_on_the_model_after_it() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    let emb = id_of(&fx.model_of(&gpu, "e", "text_embedding").await, "model_id");
    let chat = id_of(&fx.model_of(&gpu, "c", "chat").await, "model_id");
    fx.model_of(&gpu, "taken", "chat").await;
    let same = fx.update_model(json!({"model_id": emb, "dimension": 1024})).await.unwrap();
    assert_eq!(same["dimension"], 1024);
    let before = fx.changes().await.len();
    for (a, text) in [
        (json!({"model_id": chat, "dimension": 8}), "dimension applies to embedding kinds only"),
        (json!({"model_id": emb, "dimension": 9000}), "dimension must be between 1 and 8192"),
        (
            json!({"model_id": emb, "max_input_tokens": 1000001}),
            "max_input_tokens must be between 1 and 1000000",
        ),
        (
            json!({"model_id": chat, "max_input_tokens": -1}),
            "max_input_tokens must be between 1 and 1000000",
        ),
        (json!({"model_id": chat, "name": ""}), "name must be 1 to 128 characters"),
        (
            json!({"model_id": chat, "name": "TAKEN"}),
            "a model named 'TAKEN' already exists for provider 'Local GPU'",
        ),
    ] {
        assert_eq!(fx.update_model(a).await.unwrap_err(), format!("ERR_INVALID_ARGUMENT: {text}"));
    }
    assert_eq!(fx.changes().await.len(), before, "a refused update leaves no record");
    let own = fx.update_model(json!({"model_id": chat, "name": "C"})).await.unwrap();
    assert_eq!(own["name"], "C");
}

#[tokio::test]
async fn e2e_0102_026_an_unknown_model() {
    let fx = Fx::new().await;
    let e = fx.update_model(json!({"model_id": "mdl-unknown", "name": "X"})).await.unwrap_err();
    assert_eq!(e, "ERR_NOT_FOUND: model not found: mdl-unknown");
    let e = fx.delete_model_as(ALICE, "mdl-unknown").await.unwrap_err();
    assert_eq!(e, "ERR_NOT_FOUND: model not found: mdl-unknown");
    assert!(fx.models().await.is_empty());
    assert!(fx.changes().await.is_empty());
}

#[tokio::test]
async fn e2e_0102_027_deleting_a_model() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    let id = id_of(&fx.model_of(&gpu, "bge-reranker", "reranker").await, "model_id");
    assert_eq!(fx.delete_model_as(ALICE, &id).await.unwrap(), json!({"deleted": true}));
    assert!(fx.models().await.is_empty());
    let e = fx.delete_model_as(ALICE, &id).await.unwrap_err();
    assert_eq!(e, format!("ERR_NOT_FOUND: model not found: {id}"));
    let rec = &fx.changes().await[0];
    assert_eq!(
        (rec["action"].as_str(), rec["target_kind"].as_str()),
        (Some("delete_model"), Some("model"))
    );
    assert_eq!(
        (rec["target_id"].as_str().unwrap(), rec["target_name"].as_str().unwrap()),
        (id.as_str(), "bge-reranker")
    );
    assert_eq!(rec["fields"], json!([]));
}

#[tokio::test]
async fn e2e_0102_052_an_unusable_marker() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    fx.model_of(&gpu, "m", "chat").await;
    let want = "ERR_INVALID_ARGUMENT: marker is not valid";
    assert_eq!(fx.list_as(ALICE, "zzz").await.unwrap_err(), want);
    assert_eq!(fx.models_as(ALICE, "", "", "zzz").await.unwrap_err(), want);
    assert_eq!(fx.changes_as(ALICE, "zzz").await.unwrap_err(), want);
    assert_eq!(fx.models().await.len(), 1);
    assert_eq!(fx.changes().await.len(), 2);
}

#[tokio::test]
async fn e2e_0102_053_listing_the_models_of_one_provider_and_one_kind() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    let cloud = id_of(&fx.cloud().await, "provider_id");
    fx.model_of(&gpu, "qwen", "text_embedding").await;
    fx.model_of(&gpu, "bge-reranker", "reranker").await;
    fx.model_of(&cloud, "Alpha-rerank", "reranker").await;
    fx.model_of(&cloud, "gpt", "chat").await;
    let mine = fx.models_as(ALICE, &gpu, "", "").await.unwrap();
    assert_eq!(Fx::names(mine["models"].as_array().unwrap()), ["bge-reranker", "qwen"]);
    let rr = fx.models_as(ALICE, "", "reranker", "").await.unwrap();
    assert_eq!(Fx::names(rr["models"].as_array().unwrap()), ["Alpha-rerank", "bge-reranker"]);
    let both = fx.models_as(ALICE, &gpu, "reranker", "").await.unwrap();
    assert_eq!(Fx::names(both["models"].as_array().unwrap()), ["bge-reranker"]);
    assert_eq!(fx.models().await.len(), 4);
    let all = Fx::names(&fx.models().await);
    assert_eq!(
        all,
        ["Alpha-rerank", "gpt", "bge-reranker", "qwen"],
        "provider name then model name"
    );
    let e = fx.models_as(ALICE, "prov-unknown", "", "").await.unwrap_err();
    assert_eq!(e, "ERR_NOT_FOUND: provider not found: prov-unknown");
    let e = fx.models_as(ALICE, "", "vision", "").await.unwrap_err();
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: kind must be one of text_embedding, image_embedding, reranker, ocr, chat"
    );
}

#[tokio::test]
async fn e2e_0102_054_paging_the_models() {
    let fx = Fx::new().await;
    let a = id_of(&fx.local("A").await, "provider_id");
    let b = id_of(&fx.local("B").await, "provider_id");
    for i in (1..=60).rev() {
        fx.model_of(&b, &format!("M{i:03}"), "chat").await;
        fx.model_of(&a, &format!("M{i:03}"), "chat").await;
    }
    let first = fx.models_as(ALICE, "", "", "").await.unwrap();
    assert_eq!(first["models"].as_array().unwrap().len(), 100);
    let marker = first["marker"].as_str().unwrap().to_string();
    let second = fx.models_as(ALICE, "", "", &marker).await.unwrap();
    assert_eq!(second["models"].as_array().unwrap().len(), 20);
    assert!(second.get("marker").is_none());
    let all = fx.models().await;
    let keys: Vec<(String, String)> = all
        .iter()
        .map(|m| {
            (
                m["provider_id"].as_str().unwrap().to_string(),
                m["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let mut want = keys.clone();
    want.sort_by_key(|(p, n)| (if *p == a { 0 } else { 1 }, n.clone()));
    assert_eq!(keys, want);
    assert_eq!(keys.len(), 120);
}

#[tokio::test]
async fn e2e_0102_092_invalid_model_fields() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    let base = |kind: &str| json!({"provider_id": gpu, "name": "m", "kind": kind});
    let with = |mut v: Value, k: &str, n: i64| {
        v[k] = json!(n);
        v
    };
    let cases = [
        (base("text_embedding"), "dimension is required for kind 'text_embedding'"),
        (with(base("chat"), "dimension", 1024), "dimension applies to embedding kinds only"),
        (with(base("text_embedding"), "dimension", 0), "dimension must be between 1 and 8192"),
        (with(base("text_embedding"), "dimension", 8193), "dimension must be between 1 and 8192"),
        (
            with(base("reranker"), "max_input_tokens", 0),
            "max_input_tokens must be between 1 and 1000000",
        ),
        (
            with(base("reranker"), "max_input_tokens", 1000001),
            "max_input_tokens must be between 1 and 1000000",
        ),
        (
            base("vision"),
            "kind must be one of text_embedding, image_embedding, reranker, ocr, chat",
        ),
        (with(base("chat"), "dimension", 0), "dimension applies to embedding kinds only"),
        (
            json!({"provider_id": gpu, "name": "", "kind": "chat"}),
            "name must be 1 to 128 characters",
        ),
    ];
    for (input, text) in cases {
        assert_eq!(fx.model(input).await.unwrap_err(), format!("ERR_INVALID_ARGUMENT: {text}"));
    }
    let e = fx
        .model(json!({"provider_id": "prov-unknown", "name": "m", "kind": "vision"}))
        .await
        .unwrap_err();
    assert_eq!(e, "ERR_NOT_FOUND: provider not found: prov-unknown");
    assert!(fx.models().await.is_empty());
    assert_eq!(fx.changes().await.len(), 1, "only the provider was recorded");
    fx.model_of(&gpu, "m", "chat").await;
    let e = fx.model(base("chat")).await.unwrap_err();
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: a model named 'm' already exists for provider 'Local GPU'"
    );
}

#[tokio::test]
async fn e2e_0102_093_limits_of_a_model() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    fx.model(json!({"provider_id": gpu, "name": "d1", "kind": "text_embedding", "dimension": 1}))
        .await
        .unwrap();
    fx.model(
        json!({"provider_id": gpu, "name": "d8192", "kind": "image_embedding", "dimension": 8192}),
    )
    .await
    .unwrap();
    fx.model(json!({"provider_id": gpu, "name": "n".repeat(128), "kind": "chat"})).await.unwrap();
    for (name, n) in [("t1", 1), ("t2", 1_000_000)] {
        let m = fx
            .model(json!({"provider_id": gpu, "name": name, "kind": "chat", "max_input_tokens": n}))
            .await
            .unwrap();
        assert_eq!(m["max_input_tokens"], n);
    }
    let e = fx
        .model(json!({"provider_id": gpu, "name": "n".repeat(129), "kind": "chat"}))
        .await
        .unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: name must be 1 to 128 characters");
    assert_eq!(fx.models().await.len(), 5);
}

#[tokio::test]
async fn e2e_0102_094_an_image_embedding_model_needs_a_dimension_too() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    let e = fx
        .model(json!({"provider_id": gpu, "name": "clip", "kind": "image_embedding"}))
        .await
        .unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: dimension is required for kind 'image_embedding'");
    let m = fx.model(json!({"provider_id": gpu, "name": "clip", "kind": "image_embedding", "dimension": 1024})).await.unwrap();
    assert_eq!(m["dimension"], 1024);
}

#[tokio::test]
async fn e2e_0102_096_model_changes_are_recorded_and_only_for_admins() {
    let fx = Fx::new().await;
    let gpu = id_of(&fx.local("Local GPU").await, "provider_id");
    let id = id_of(&fx.model_of(&gpu, "m", "text_embedding").await, "model_id");
    let rec = &fx.changes().await[0];
    assert_eq!(rec["action"], "create_model");
    assert_eq!(rec["target_kind"], "model");
    assert_eq!(rec["target_id"], json!(id));
    assert_eq!(rec["person"], ALICE);
    assert!(is_rfc3339_seconds(rec["at"].as_str().unwrap()));
    let before = fx.models().await;
    let body = json!({"provider_id": gpu, "name": "x", "kind": "chat"});
    assert!(fx.model_as(BOB, body).await.is_err());
    assert!(fx.update_model_as(BOB, json!({"model_id": id, "name": "y"})).await.is_err());
    assert!(fx.delete_model_as(BOB, &id).await.is_err());
    assert!(fx.models_as(BOB, "", "", "").await.is_err());
    assert_eq!(fx.models().await, before);
}

#[tokio::test]
async fn e2e_0102_111_valid_inputs_at_once() {
    let fx = Fx::new().await;
    let p = fx.create(json!({"name": "n".repeat(64), "endpoint": "http://h/v1"})).await.unwrap();
    let id = id_of(&p, "provider_id");
    let m = fx
        .model(json!({"provider_id": id, "name": "m".repeat(128), "kind": "text_embedding",
            "dimension": 8192, "max_input_tokens": 1000000}))
        .await
        .unwrap();
    assert_eq!(
        (m["dimension"].as_i64(), m["max_input_tokens"].as_i64()),
        (Some(8192), Some(1_000_000))
    );
    assert_eq!(fx.providers().await.len(), 1);
    assert_eq!(fx.models().await, vec![m]);
}

/// A provider `Local GPU` with the model `name` of `kind`, dimension 1024 when
/// it embeds; returns the model.
async fn local_model(fx: &Fx, name: &str, kind: &str) -> Value {
    let providers = fx.providers().await;
    let pid = match providers.first() {
        Some(p) => p["provider_id"].as_str().unwrap().to_string(),
        None => fx.local("Local GPU").await["provider_id"].as_str().unwrap().to_string(),
    };
    fx.model_of(&pid, name, kind).await
}

fn mid_of(v: &Value) -> String {
    v["model_id"].as_str().unwrap().to_string()
}

const QWEN: &str = "qwen3-embedding-0.6b";

#[tokio::test]
async fn e2e_0102_028_a_server_default_cannot_be_deleted() {
    let fx = Fx::with_dimension(1024).await;
    let m = local_model(&fx, QWEN, "text_embedding").await;
    fx.set_default("text_embedding", &mid_of(&m)).await.unwrap();
    let e = fx.delete_model_as(ALICE, &mid_of(&m)).await.unwrap_err();
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: model 'qwen3-embedding-0.6b' is a server default; clear the default first"
    );
    assert_eq!(fx.models().await.len(), 1);
    fx.clear_default("text_embedding").await.unwrap();
    assert_eq!(fx.delete_model_as(ALICE, &mid_of(&m)).await.unwrap(), json!({"deleted": true}));
    assert!(fx.models().await.is_empty());
}

#[tokio::test]
async fn e2e_0102_029_the_dimension_of_a_default_is_frozen() {
    let fx = Fx::with_dimension(1024).await;
    let m = local_model(&fx, QWEN, "text_embedding").await;
    let id = mid_of(&m);
    fx.set_default("text_embedding", &id).await.unwrap();
    let e = fx.update_model(json!({"model_id": id, "dimension": 768})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: the dimension of a server default model cannot change");
    let renamed = fx.update_model(json!({"model_id": id, "name": "qwen3-renamed"})).await.unwrap();
    assert_eq!(renamed["name"], "qwen3-renamed");
    assert_eq!(renamed["dimension"], 1024);
    assert_eq!(renamed["is_default"], true);
    // Another model's dimension is still free to change.
    let other = local_model(&fx, "other", "text_embedding").await;
    let o = fx.update_model(json!({"model_id": mid_of(&other), "dimension": 768})).await.unwrap();
    assert_eq!(o["dimension"], 768);
}

#[tokio::test]
async fn e2e_0102_030_choosing_the_default_text_embedding_model() {
    let fx = Fx::with_dimension(1024).await;
    let m = local_model(&fx, QWEN, "text_embedding").await;
    let other = local_model(&fx, "plain", "chat").await;
    let set = fx.set_default("text_embedding", &mid_of(&m)).await.unwrap();
    assert_eq!(set["is_default"], true);
    assert_eq!(set["model_id"], m["model_id"]);
    let d = fx.defaults().await;
    assert_eq!(d["text_embedding"]["model_id"], m["model_id"]);
    assert_eq!(d["text_embedding"]["is_default"], true);
    assert_eq!(d["reranker"], Value::Null);
    let models = fx.models().await;
    let flags: Vec<(String, bool)> = models
        .iter()
        .map(|x| (x["model_id"].as_str().unwrap().to_string(), x["is_default"].as_bool().unwrap()))
        .collect();
    assert!(flags.contains(&(mid_of(&m), true)));
    assert!(flags.contains(&(mid_of(&other), false)));
    assert_eq!(flags.iter().filter(|(_, d)| *d).count(), 1);
}

#[tokio::test]
async fn e2e_0102_031_replacing_a_default() {
    let fx = Fx::with_dimension(1024).await;
    let a = local_model(&fx, "first", "text_embedding").await;
    let b = local_model(&fx, "second", "text_embedding").await;
    fx.set_default("text_embedding", &mid_of(&a)).await.unwrap();
    fx.set_default("text_embedding", &mid_of(&b)).await.unwrap();
    assert_eq!(fx.defaults().await["text_embedding"]["model_id"], b["model_id"]);
    let models = fx.models().await;
    let first = models.iter().find(|m| m["model_id"] == a["model_id"]).unwrap();
    assert_eq!(first["is_default"], false);
}

#[tokio::test]
async fn e2e_0102_032_a_default_of_the_wrong_kind() {
    let fx = Fx::new().await;
    let m = local_model(&fx, "bge-reranker", "reranker").await;
    let e = fx.set_default("text_embedding", &mid_of(&m)).await.unwrap_err();
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: model 'bge-reranker' is of kind 'reranker', not 'text_embedding'"
    );
    assert_eq!(fx.defaults().await, json!({"text_embedding": null, "reranker": null}));
}

#[tokio::test]
async fn e2e_0102_033_a_kind_that_has_no_default() {
    let fx = Fx::new().await;
    let m = local_model(&fx, "Llama", "chat").await;
    let before = fx.changes().await.len();
    let text = "ERR_INVALID_ARGUMENT: kind must be 'text_embedding' or 'reranker'";
    assert_eq!(fx.set_default("chat", &mid_of(&m)).await.unwrap_err(), text);
    assert_eq!(fx.clear_default("ocr").await.unwrap_err(), text);
    assert_eq!(fx.defaults().await, json!({"text_embedding": null, "reranker": null}));
    assert_eq!(fx.changes().await.len(), before);
    assert_eq!(fx.models().await[0]["is_default"], false);
}

#[tokio::test]
async fn e2e_0102_034_a_dimension_that_does_not_match_the_vector_store() {
    let fx = Fx::with_dimension(1536).await;
    let m = local_model(&fx, QWEN, "text_embedding").await;
    let e = fx.set_default("text_embedding", &mid_of(&m)).await.unwrap_err();
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: the default text embedding model must have dimension 1536"
    );
    assert_eq!(fx.defaults().await["text_embedding"], Value::Null);
    // The reranker has no dimension to match.
    let r = local_model(&fx, "bge-reranker", "reranker").await;
    assert_eq!(fx.set_default("reranker", &mid_of(&r)).await.unwrap()["is_default"], true);
}

#[tokio::test]
async fn e2e_0102_035_an_unknown_model_as_default() {
    let fx = Fx::new().await;
    let e = fx.set_default("reranker", "mdl-unknown").await.unwrap_err();
    assert_eq!(e, "ERR_NOT_FOUND: model not found: mdl-unknown");
    assert_eq!(fx.defaults().await, json!({"text_embedding": null, "reranker": null}));
    assert!(fx.changes().await.is_empty());
}

#[tokio::test]
async fn e2e_0102_036_clearing_a_default() {
    let fx = Fx::new().await;
    let m = local_model(&fx, "bge-reranker", "reranker").await;
    fx.set_default("reranker", &mid_of(&m)).await.unwrap();
    assert_eq!(fx.clear_default("reranker").await.unwrap(), json!({"cleared": true}));
    assert_eq!(fx.clear_default("reranker").await.unwrap(), json!({"cleared": true}));
    assert_eq!(fx.defaults().await["reranker"], Value::Null);
    let clears: Vec<Value> =
        fx.changes().await.into_iter().filter(|c| c["action"] == "clear_default").collect();
    assert_eq!(clears.len(), 1);
    assert_eq!(clears[0]["target_kind"], "default");
    assert_eq!(clears[0]["target_id"], m["model_id"]);
    assert_eq!(clears[0]["target_name"], "reranker");
    assert_eq!(fx.models().await[0]["is_default"], false);
}

#[tokio::test]
async fn e2e_0102_054_an_empty_registry() {
    let fx = Fx::new().await;
    assert_eq!(fx.list_as(ALICE, "").await.unwrap(), json!({"providers": []}));
    assert_eq!(fx.models_as(ALICE, "", "", "").await.unwrap(), json!({"models": []}));
    assert_eq!(fx.defaults().await, json!({"text_embedding": null, "reranker": null}));
    assert_eq!(fx.changes_as(ALICE, "").await.unwrap(), json!({"changes": []}));
}

#[tokio::test]
async fn e2e_0102_055_the_record_of_changes() {
    let fx = Fx::with_dimension(1024).await;
    let p = fx.local("Local GPU").await;
    let pid = p["provider_id"].as_str().unwrap().to_string();
    fx.update(json!({"provider_id": pid, "endpoint": "http://localhost:9011/v1"})).await.unwrap();
    let m = fx.model_of(&pid, QWEN, "text_embedding").await;
    fx.set_default("text_embedding", &mid_of(&m)).await.unwrap();
    let c = fx.changes().await;
    assert_eq!(c.len(), 4);
    let actions: Vec<&str> = c.iter().map(|x| x["action"].as_str().unwrap()).collect();
    assert_eq!(actions, ["set_default", "create_model", "update_provider", "create_provider"]);
    assert_eq!(c[0]["target_kind"], "default");
    assert_eq!(c[0]["target_id"], m["model_id"]);
    assert_eq!(c[0]["target_name"], "text_embedding");
    assert_eq!(c[0]["fields"], json!([]));
    assert_eq!(c[1]["target_kind"], "model");
    assert_eq!(c[1]["target_id"], m["model_id"]);
    assert_eq!(c[1]["target_name"], QWEN);
    assert_eq!(c[2]["target_kind"], "provider");
    assert_eq!(c[2]["fields"], json!(["endpoint"]));
    for x in &c {
        assert_eq!(x["person"], ALICE);
        assert!(is_rfc3339_seconds(x["at"].as_str().unwrap()));
        assert!(x["change_id"].as_str().unwrap().starts_with("chg-"));
    }
    let ids: Vec<&str> = c.iter().map(|x| x["change_id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] > w[1]), "ids increase with the order made");
    fx.assert_nowhere(SECRET).await;
}

#[tokio::test]
async fn e2e_0102_060_an_ordinary_member_is_refused_everywhere() {
    let fx = Fx::with_dimension(1024).await;
    let m = local_model(&fx, QWEN, "text_embedding").await;
    let pid = m["provider_id"].as_str().unwrap().to_string();
    let mid = mid_of(&m);
    let bob = format!("ERR_FORBIDDEN: '{BOB}' is not a platform admin");
    let before = (fx.providers().await, fx.models().await, fx.changes().await);
    let refused = [
        fx.list_as(BOB, "").await,
        fx.create_as(BOB, json!({"name": "Z", "endpoint": "http://h/v1"})).await,
        fx.update_as(BOB, json!({"provider_id": pid, "name": "Z"})).await,
        fx.delete_as(BOB, &pid).await,
        fx.model_as(BOB, json!({"provider_id": pid, "name": "n", "kind": "chat"})).await,
        fx.update_model_as(BOB, json!({"model_id": mid, "name": "Z"})).await,
        fx.delete_model_as(BOB, &mid).await,
        fx.models_as(BOB, "", "", "").await,
        fx.set_default_as(BOB, "text_embedding", &mid).await,
        fx.clear_default_as(BOB, "text_embedding").await,
        fx.defaults_as(BOB).await,
        fx.changes_as(BOB, "").await,
    ];
    for r in refused {
        assert_eq!(r.unwrap_err(), bob);
    }
    assert_eq!((fx.providers().await, fx.models().await, fx.changes().await), before);
    assert_eq!(fx.defaults().await["text_embedding"], Value::Null);
}

#[tokio::test]
async fn e2e_0102_061_a_project_owner_is_not_a_platform_administrator() {
    let fx = Fx::new().await;
    let e = fx.defaults_as(BOB).await.unwrap_err();
    assert_eq!(e, format!("ERR_FORBIDDEN: '{BOB}' is not a platform admin"));
}

#[tokio::test]
async fn e2e_0102_104_a_provider_whose_model_is_a_default_cannot_be_deleted() {
    let fx = Fx::with_dimension(1024).await;
    let m = local_model(&fx, QWEN, "text_embedding").await;
    fx.set_default("text_embedding", &mid_of(&m)).await.unwrap();
    let e = fx.delete_as(ALICE, m["provider_id"].as_str().unwrap()).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: provider 'Local GPU' still has 1 model(s)");
    assert_eq!(fx.defaults().await["text_embedding"]["model_id"], m["model_id"]);
}

#[tokio::test]
async fn e2e_0102_105_clearing_the_default_of_a_kind_that_has_none() {
    let fx = Fx::new().await;
    assert_eq!(fx.clear_default("text_embedding").await.unwrap(), json!({"cleared": true}));
    assert!(fx.changes().await.iter().all(|c| c["action"] != "clear_default"));
    assert!(fx.changes().await.is_empty());
}

#[tokio::test]
async fn e2e_0102_116_giving_the_same_dimension_to_a_default() {
    let fx = Fx::with_dimension(1024).await;
    let m = local_model(&fx, QWEN, "text_embedding").await;
    let id = mid_of(&m);
    let set = fx.set_default("text_embedding", &id).await.unwrap();
    let before = fx.changes().await.len();
    let same = fx.update_model(json!({"model_id": id, "dimension": 1024})).await.unwrap();
    assert_eq!(same, set);
    assert_eq!(fx.changes().await.len(), before);
}

#[tokio::test]
async fn e2e_0102_117_setting_the_model_that_is_already_the_default_writes_no_record() {
    let fx = Fx::with_dimension(1024).await;
    let m = local_model(&fx, QWEN, "text_embedding").await;
    fx.set_default("text_embedding", &mid_of(&m)).await.unwrap();
    let before = fx.changes().await.len();
    let again = fx.set_default("text_embedding", &mid_of(&m)).await.unwrap();
    assert_eq!(again["is_default"], true);
    assert_eq!(fx.changes().await.len(), before);
}

// ── US-0286: search embeds with the default text embedding model ────────────

#[cfg(feature = "rag")]
/// One call an [`EmbedStub`] received.
#[derive(Clone, Debug)]
pub(crate) struct Call {
    pub method: String,
    pub path: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Value,
}

#[cfg(feature = "rag")]
/// A stub embedding service on a random port: records the method, path,
/// headers and body of every call and answers one vector of `dimension`, or
/// 401 when a bearer is required and the call does not carry it.
pub(crate) struct EmbedStub {
    pub base: String,
    calls: Arc<Mutex<Vec<Call>>>,
    bearer: Arc<Mutex<Option<String>>>,
}

#[cfg(feature = "rag")]
impl EmbedStub {
    pub async fn start(dimension: usize) -> Self {
        use axum::body::Bytes;
        use axum::extract::State;
        use axum::http::{HeaderMap, Method, StatusCode, Uri};
        type Shared = (Arc<Mutex<Vec<Call>>>, Arc<Mutex<Option<String>>>, usize);
        async fn answer(
            State((calls, bearer, dimension)): State<Shared>,
            method: Method,
            uri: Uri,
            headers: HeaderMap,
            body: Bytes,
        ) -> (StatusCode, axum::Json<Value>) {
            let headers = headers
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                .collect::<std::collections::HashMap<_, _>>();
            let auth = headers.get("authorization").cloned();
            calls.lock().unwrap().push(Call {
                method: method.to_string(),
                path: uri.path().to_string(),
                headers,
                body: serde_json::from_slice(&body).unwrap_or(Value::Null),
            });
            if let Some(want) = bearer.lock().unwrap().clone()
                && auth.as_deref() != Some(&format!("Bearer {want}"))
            {
                return (StatusCode::UNAUTHORIZED, axum::Json(json!({"error": "unauthorized"})));
            }
            let v: Vec<f32> = (0..dimension).map(|i| (i + 1) as f32 / dimension as f32).collect();
            (StatusCode::OK, axum::Json(json!({"data": [{"embedding": v, "index": 0}]})))
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        let bearer = Arc::new(Mutex::new(None));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = axum::Router::new().fallback(answer).with_state((
            calls.clone(),
            bearer.clone(),
            dimension,
        ));
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self { base: format!("http://127.0.0.1:{port}"), calls, bearer }
    }

    /// Calls without `Authorization: Bearer <secret>` are refused from now on.
    pub fn require(&self, secret: &str) {
        *self.bearer.lock().unwrap() = Some(secret.to_string());
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
}

#[cfg(feature = "rag")]
/// A base URL nobody listens on.
pub(crate) async fn refusing_base() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

#[cfg(feature = "rag")]
impl Fx {
    /// `admin.set_index_mode` of `proj` by Alice.
    pub async fn index_mode(&self, mode: &str) {
        let p: rmcp::handler::server::wrapper::Parameters<crate::mcp::server::SetIndexModeArgs> =
            args(json!({"project_id": PROJ, "mode": mode}));
        tool_json(self.server(ALICE).admin_set_index_mode(p).await).expect("the mode is set");
    }

    /// `fs.write` by Bob, the owner, then the finished rebuild job of `path`.
    pub async fn save(&self, path: &str, text: &str) -> Value {
        let id = |j: Option<&Value>| j.map(|j| j["job_id"].clone());
        let before = id(self.finished(path).await.first());
        let a = args(json!({"mount_id": PROJ, "path": path, "content": text, "overwrite": true}));
        tool_json(self.server(BOB).fs_write(a).await).expect("the write succeeds");
        for _ in 0..200 {
            let done = self.finished(path).await;
            if !done.is_empty() && id(done.first()) != before {
                return done[0].clone();
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("the rebuild of {path} did not finish");
    }

    /// The finished search index jobs of `path` (the table keeps the last one).
    async fn finished(&self, path: &str) -> Vec<Value> {
        let a = args(json!({"mount_id": PROJ, "path": path, "marker": ""}));
        let v = tool_json(self.server(BOB).fs_list_derived_jobs(a).await).unwrap();
        crate::search::derived_e2e::jobs_of(&v)
            .iter()
            .filter(|j| j["kind"] == "search_index")
            .filter(|j| matches!(j["state"].as_str(), Some("done" | "failed")))
            .cloned()
            .collect()
    }

    /// `search.query` by Bob in `mode`: the paths found, or the refusal.
    pub async fn search(&self, mode: &str, word: &str) -> Result<Vec<String>, String> {
        let a = args(json!({"mount_id": PROJ, "query": word, "mode": mode, "top_k": 10}));
        self.keep(tool_json(self.server(BOB).search_query(a).await)).map(|v| {
            v["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["path"].as_str().unwrap().into())
                .collect()
        })
    }

    /// A provider at `base + "/v1"` with a text embedding model of 1024 set as
    /// the default; returns (provider id, model id).
    pub async fn default_on(
        &self,
        name: &str,
        base: &str,
        secret: Option<&str>,
    ) -> (String, String) {
        let mut a = json!({"name": name, "endpoint": format!("{base}/v1")});
        if let Some(s) = secret {
            a["auth_scheme"] = json!("bearer");
            a["secret"] = json!(s);
        }
        let p = id_of(&self.create(a).await.unwrap(), "provider_id");
        let m =
            id_of(&self.model_of(&p, "qwen3-embedding-0.6b", "text_embedding").await, "model_id");
        self.set_default("text_embedding", &m).await.unwrap();
        (p, m)
    }
}

/// The calls a stub received, all of them `POST /v1/embeddings`.
#[cfg(feature = "rag")]
fn embedding_calls(stub: &EmbedStub) -> Vec<Call> {
    let calls = stub.calls();
    for c in &calls {
        assert_eq!((c.method.as_str(), c.path.as_str()), ("POST", "/v1/embeddings"), "{c:?}");
    }
    calls
}

#[cfg(feature = "rag")]
async fn searching_on(yaml: &EmbedStub, mode: &'static str) -> Fx {
    Fx::searching(Searching {
        mode,
        yaml_url: format!("{}/v1/embeddings", yaml.base),
        rerank_url: None,
    })
    .await
}

/// SPEC-0102/E2E-016: the default's provider secret cannot be read once the
/// master key is gone: the rebuild fails, the search answers the same refusal.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_016_the_master_key_is_removed_later() {
    let (a, c) = (EmbedStub::start(1024).await, EmbedStub::start(1024).await);
    let mut fx = searching_on(&c, "both").await;
    fx.index_mode("both").await;
    fx.default_on("Cloud Embeddings", &a.base, Some(SECRET)).await;
    fx.restart(None).await;
    let refusal = "ERR_INTERNAL_ERROR: the secret of provider 'Cloud Embeddings' cannot be read";
    let job = fx.save("/guide.md", "the guide").await;
    assert_eq!((job["state"].as_str(), job["reason"].as_str()), (Some("failed"), Some(refusal)));
    assert_eq!(fx.search("rag", "guide").await.unwrap_err(), refusal);
    let providers = fx.providers().await;
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["has_secret"], json!(true));
    assert!(
        a.calls().is_empty() && c.calls().is_empty(),
        "no service is called without the secret"
    );
    fx.assert_nowhere(SECRET).await;
}

/// SPEC-0102/E2E-037: the default text embedding model drives indexing and queries.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_037_the_default_text_embedding_model_drives_indexing() {
    let (a, c) = (EmbedStub::start(1024).await, EmbedStub::start(1024).await);
    let fx = searching_on(&c, "both").await;
    fx.index_mode("both").await;
    fx.default_on("Local GPU", &a.base, None).await;
    let job = fx.save("/guide.md", "the guide explains the platform").await;
    assert_eq!(job["state"], json!("done"), "{job}");
    assert_eq!(fx.search("rag", "guide").await.unwrap(), vec!["/guide.md".to_string()]);
    let calls = embedding_calls(&a);
    assert!(calls.len() >= 2, "the document and the query: {calls:?}");
    for call in &calls {
        assert_eq!(call.body["model"], json!("qwen3-embedding-0.6b"));
        assert!(!call.headers.contains_key("authorization"), "{call:?}");
    }
    assert!(calls.iter().any(|c| c.body["input"][0].as_str().unwrap().contains("explains")));
    assert_eq!(calls.last().unwrap().body["input"], json!(["guide"]));
    assert!(c.calls().is_empty(), "the configured embedding settings are not called");
}

/// SPEC-0102/E2E-038: a bearer provider sends its secret, the new one once replaced.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_038_a_bearer_provider_sends_its_secret() {
    let (a, c) = (EmbedStub::start(1024).await, EmbedStub::start(1024).await);
    a.require(SECRET);
    let fx = searching_on(&c, "both").await;
    fx.index_mode("both").await;
    let (p, _) = fx.default_on("Cloud Embeddings", &a.base, Some(SECRET)).await;
    assert_eq!(fx.save("/guide.md", "the guide").await["state"], json!("done"));
    let auth = |c: &Call| c.headers.get("authorization").cloned();
    assert_eq!(auth(embedding_calls(&a).last().unwrap()).as_deref(), Some("Bearer sk-test-123"));
    a.require("sk-new-456");
    fx.update(json!({"provider_id": p, "secret": "sk-new-456"})).await.unwrap();
    assert_eq!(fx.save("/guide.md", "the new guide").await["state"], json!("done"));
    assert_eq!(auth(embedding_calls(&a).last().unwrap()).as_deref(), Some("Bearer sk-new-456"));
    fx.assert_nowhere(SECRET).await;
    fx.assert_nowhere("sk-new-456").await;
}

/// SPEC-0102/E2E-039: an endpoint change applies to the next call, no restart.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_039_a_change_applies_at_once() {
    let (a, b, c) =
        (EmbedStub::start(1024).await, EmbedStub::start(1024).await, EmbedStub::start(1024).await);
    let fx = searching_on(&c, "both").await;
    fx.index_mode("both").await;
    let (p, _) = fx.default_on("Local GPU", &a.base, None).await;
    assert_eq!(fx.save("/guide.md", "the guide").await["state"], json!("done"));
    let on_a = embedding_calls(&a).len();
    assert!(on_a >= 1);
    fx.update(json!({"provider_id": p, "endpoint": format!("{}/v1/", b.base)})).await.unwrap();
    assert_eq!(fx.save("/other.md", "another page").await["state"], json!("done"));
    assert_eq!(embedding_calls(&a).len(), on_a, "stub A receives nothing after the change");
    assert_eq!(embedding_calls(&b).len(), 1, "a trailing slash of the endpoint is trimmed");
    assert!(c.calls().is_empty());
}

/// SPEC-0102/E2E-041: the default's service is down: the rebuild fails, the
/// write succeeded and the previous entries remain searchable.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_041_the_embedding_service_of_the_default_is_down() {
    let (a, c) = (EmbedStub::start(1024).await, EmbedStub::start(1024).await);
    let fx = searching_on(&c, "both").await;
    fx.index_mode("both").await;
    let (p, _) = fx.default_on("Local GPU", &a.base, None).await;
    assert_eq!(fx.save("/guide.md", "the guide").await["state"], json!("done"));
    fx.update(json!({"provider_id": p, "endpoint": format!("{}/v1", refusing_base().await)}))
        .await
        .unwrap();
    let job = fx.save("/guide.md", "rewritten entirely").await;
    assert_eq!(job["state"], json!("failed"));
    assert_eq!(job["reason"], json!("embedding service unreachable"));
    let client = fx.state.stores.client(PROJ).await.unwrap();
    assert_eq!(client.read_text("/guide.md").await.unwrap(), "rewritten entirely");
    assert_eq!(fx.search("bm25", "guide").await.unwrap(), vec!["/guide.md".to_string()]);
}

/// SPEC-0102/E2E-042: vectors of the default's dimension are stored and found.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_042_the_dimension_of_vectors_matches_the_default() {
    let (a, c) = (EmbedStub::start(1024).await, EmbedStub::start(1024).await);
    let fx = searching_on(&c, "both").await;
    fx.index_mode("both").await;
    let (p, _) = fx.default_on("Local GPU", &a.base, None).await;
    assert_eq!(fx.save("/guide.md", "the guide").await["state"], json!("done"));
    assert_eq!(fx.search("rag", "guide").await.unwrap(), vec!["/guide.md".to_string()]);
    let narrow = fx
        .model(json!({"provider_id": p, "name": "narrow", "kind": "text_embedding", "dimension": 1023}))
        .await
        .unwrap();
    assert_eq!(
        fx.set_default("text_embedding", &id_of(&narrow, "model_id")).await.unwrap_err(),
        "ERR_INVALID_ARGUMENT: the default text embedding model must have dimension 1024"
    );
}

/// SPEC-0102/E2E-100: no default: the configuration's service is called.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_100_no_default_the_configuration_is_used() {
    let c = EmbedStub::start(1024).await;
    let fx = searching_on(&c, "both").await;
    fx.index_mode("both").await;
    assert_eq!(fx.save("/guide.md", "the guide").await["state"], json!("done"));
    assert_eq!(fx.search("rag", "guide").await.unwrap(), vec!["/guide.md".to_string()]);
    assert_eq!(fx.search("bm25", "guide").await.unwrap(), vec!["/guide.md".to_string()]);
    let calls = embedding_calls(&c);
    assert_eq!(calls.len(), 2, "the document and the query: {calls:?}");
    assert!(calls.iter().all(|c| c.body["model"] == json!(YAML_MODEL)));
}

/// SPEC-0102/E2E-101: clearing the default returns to the configuration.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_101_clearing_the_default_returns_to_the_configuration() {
    let (a, c) = (EmbedStub::start(1024).await, EmbedStub::start(1024).await);
    let fx = searching_on(&c, "both").await;
    fx.index_mode("both").await;
    fx.default_on("Local GPU", &a.base, None).await;
    assert_eq!(fx.save("/guide.md", "the guide").await["state"], json!("done"));
    let on_a = embedding_calls(&a).len();
    fx.clear_default("text_embedding").await.unwrap();
    assert_eq!(fx.save("/other.md", "another page").await["state"], json!("done"));
    assert_eq!(embedding_calls(&a).len(), on_a);
    assert_eq!(embedding_calls(&c).len(), 1);
    assert_eq!(fx.search("bm25", "guide").await.unwrap(), vec!["/guide.md".to_string()]);
    let paths: Vec<Call> = a.calls().into_iter().chain(c.calls()).collect();
    assert!(paths.iter().all(|c| !c.path.ends_with("/rerank")), "no reranker is called");
}

/// SPEC-0102/E2E-103: the registry starts empty and search works as before.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_103_the_upgrade_creates_the_registry_empty() {
    let c = EmbedStub::start(1024).await;
    let fx = searching_on(&c, "both").await;
    assert!(fx.providers().await.is_empty());
    assert!(fx.models().await.is_empty());
    fx.index_mode("both").await;
    assert_eq!(fx.save("/guide.md", "the guide").await["state"], json!("done"));
    assert_eq!(fx.search("rag", "guide").await.unwrap(), vec!["/guide.md".to_string()]);
    assert!(fx.providers().await.is_empty(), "nothing is imported from the configuration");
}

/// SPEC-0102/E2E-106: a renamed model is used by the next call.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_106_a_changed_model_name_is_used_by_the_next_call() {
    let (a, c) = (EmbedStub::start(1024).await, EmbedStub::start(1024).await);
    let fx = searching_on(&c, "both").await;
    fx.index_mode("both").await;
    let (_, m) = fx.default_on("Local GPU", &a.base, None).await;
    fx.update_model(json!({"model_id": m, "name": "qwen3-embedding-0.6b-v2"})).await.unwrap();
    assert_eq!(fx.save("/guide.md", "the guide").await["state"], json!("done"));
    assert_eq!(embedding_calls(&a).last().unwrap().body["model"], json!("qwen3-embedding-0.6b-v2"));
    let models = fx.models().await;
    assert_eq!(models[0]["dimension"], json!(1024), "the dimension was not touched");
}

/// SPEC-0102/E2E-113: with search mode bm25 the default is never called; with
/// mode both the next index call uses it.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_113_a_default_embedding_model_with_search_mode_bm25() {
    let (a, c) = (EmbedStub::start(1024).await, EmbedStub::start(1024).await);
    let mut fx = searching_on(&c, "bm25").await;
    fx.index_mode("bm25").await;
    fx.default_on("Local GPU", &a.base, None).await;
    assert_eq!(fx.save("/guide.md", "the guide").await["state"], json!("done"));
    assert_eq!(fx.search("bm25", "guide").await.unwrap(), vec!["/guide.md".to_string()]);
    assert!(a.calls().is_empty() && c.calls().is_empty());
    let yaml_url = format!("{}/v1/embeddings", c.base);
    fx.restart_searching(Searching { mode: "both", yaml_url, rerank_url: None }).await;
    fx.index_mode("both").await;
    assert_eq!(fx.save("/next.md", "the next page").await["state"], json!("done"));
    let calls = embedding_calls(&a);
    assert!(!calls.is_empty());
    assert!(calls.iter().all(|c| c.body["model"] == json!("qwen3-embedding-0.6b")));
    assert!(c.calls().is_empty());
}

// ── US-0287: search reranks with the default reranker ───────────────────────

/// A stub reranker: records every call, answers EVERY document it receives with
/// a score that grows with its index (so the last document comes first), never
/// cutting to `top_n` itself so the cut is the implementation's, 401 without the required bearer, or `status` when set.
#[cfg(feature = "rag")]
pub(crate) struct RerankStub {
    pub base: String,
    calls: Arc<Mutex<Vec<Call>>>,
    bearer: Arc<Mutex<Option<String>>>,
    status: Arc<Mutex<u16>>,
}

#[cfg(feature = "rag")]
impl RerankStub {
    pub async fn start() -> Self {
        use axum::body::Bytes;
        use axum::extract::State;
        use axum::http::{HeaderMap, Method, StatusCode, Uri};
        type Shared = (Arc<Mutex<Vec<Call>>>, Arc<Mutex<Option<String>>>, Arc<Mutex<u16>>);
        async fn answer(
            State((calls, bearer, status)): State<Shared>,
            method: Method,
            uri: Uri,
            headers: HeaderMap,
            body: Bytes,
        ) -> (StatusCode, axum::Json<Value>) {
            let headers = headers
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                .collect::<std::collections::HashMap<_, _>>();
            let auth = headers.get("authorization").cloned();
            let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            calls.lock().unwrap().push(Call {
                method: method.to_string(),
                path: uri.path().to_string(),
                headers,
                body: body.clone(),
            });
            let code = *status.lock().unwrap();
            if code != 200 {
                let c = StatusCode::from_u16(code).unwrap();
                return (c, axum::Json(json!({"error": "down"})));
            }
            if let Some(want) = bearer.lock().unwrap().clone()
                && auth.as_deref() != Some(&format!("Bearer {want}"))
            {
                return (StatusCode::UNAUTHORIZED, axum::Json(json!({"error": "unauthorized"})));
            }
            let n = body["documents"].as_array().map_or(0, |d| d.len());
            let results: Vec<Value> = (0..n)
                .rev()
                .map(|i| json!({"index": i, "relevance_score": (i + 1) as f64 / n as f64}))
                .collect();
            (StatusCode::OK, axum::Json(json!({"results": results})))
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        let bearer = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new(200u16));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = axum::Router::new().fallback(answer).with_state((
            calls.clone(),
            bearer.clone(),
            status.clone(),
        ));
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self { base: format!("http://127.0.0.1:{port}"), calls, bearer, status }
    }

    pub fn require(&self, secret: &str) {
        *self.bearer.lock().unwrap() = Some(secret.to_string());
    }

    pub fn fail_with(&self, code: u16) {
        *self.status.lock().unwrap() = code;
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
}

#[cfg(feature = "rag")]
impl Fx {
    /// `search.query` by Bob, `both`, asking `top_k` results: the paths found.
    pub async fn ranked(&self, top_k: u32, rerank: bool) -> Vec<String> {
        let a = args(json!({"mount_id": PROJ, "query": "guide", "mode": "both",
            "top_k": top_k, "rerank": rerank}));
        let v = self.keep(tool_json(self.server(BOB).search_query(a).await)).expect("answers");
        v["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["path"].as_str().unwrap().into())
            .collect()
    }

    /// A provider at `base + "/v1"` with the reranker `bge-reranker` as the default.
    pub async fn reranker_on(&self, name: &str, base: &str, secret: Option<&str>) {
        let mut a = json!({"name": name, "endpoint": format!("{base}/v1")});
        if let Some(s) = secret {
            a["auth_scheme"] = json!("bearer");
            a["secret"] = json!(s);
        }
        let p = id_of(&self.create(a).await.unwrap(), "provider_id");
        let m = id_of(&self.model_of(&p, "bge-reranker", "reranker").await, "model_id");
        self.set_default("reranker", &m).await.unwrap();
    }

    /// Twenty files that all match `guide`, indexed in mode `both`.
    pub async fn twenty_guides(&self) {
        self.index_mode("both").await;
        for i in 0..20 {
            let job = self.save(&format!("/g{i:02}.md"), &format!("the guide number {i}")).await;
            assert_eq!(job["state"], json!("done"), "{job}");
        }
    }
}

#[cfg(feature = "rag")]
async fn searching_with_rerank(c: &EmbedStub, r: &RerankStub) -> Fx {
    let yaml_url = format!("{}/v1/embeddings", c.base);
    Fx::searching(Searching {
        mode: "both",
        yaml_url,
        rerank_url: Some(format!("{}/v1/rerank", r.base)),
    })
    .await
}

#[cfg(feature = "rag")]
fn rerank_calls(stub: &RerankStub) -> Vec<Call> {
    let calls = stub.calls();
    for c in &calls {
        assert_eq!((c.method.as_str(), c.path.as_str()), ("POST", "/v1/rerank"), "{c:?}");
    }
    calls
}

/// SPEC-0102/E2E-040: the default reranker drives reranking.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_040_the_default_reranker_drives_reranking() {
    let (c, r) = (EmbedStub::start(1024).await, RerankStub::start().await);
    let fx = searching_with_rerank(&c, &r).await;
    fx.twenty_guides().await;
    let fused = fx.ranked(20, false).await;
    assert_eq!(fused.len(), 20);
    fx.reranker_on("Local Rerank", &r.base, None).await;
    let got = fx.ranked(20, true).await;
    let calls = rerank_calls(&r);
    assert_eq!(calls.len(), 1, "{calls:?}");
    let b = &calls[0].body;
    assert_eq!(
        (&b["model"], &b["query"], &b["top_n"]),
        (&json!("bge-reranker"), &json!("guide"), &json!(10))
    );
    assert_eq!(b["documents"].as_array().unwrap().len(), 20);
    assert!(!calls[0].headers.contains_key("authorization"));
    let want: Vec<String> = fused.iter().rev().take(10).cloned().collect();
    assert_eq!(got, want);
    fx.clear_default("reranker").await.unwrap();
    assert_eq!(fx.ranked(20, true).await, fused);
    assert_eq!(rerank_calls(&r).len(), 1, "no call without a default");
}

/// SPEC-0102/E2E-102: a registry without defaults changes nothing.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_102_a_registry_without_defaults_changes_nothing() {
    let (c, r) = (EmbedStub::start(1024).await, RerankStub::start().await);
    let fx = searching_with_rerank(&c, &r).await;
    fx.twenty_guides().await;
    let before = fx.ranked(20, true).await;
    let p = id_of(
        &fx.create(json!({"name": "Local Rerank", "endpoint": format!("{}/v1", r.base)}))
            .await
            .unwrap(),
        "provider_id",
    );
    fx.model_of(&p, "bge-reranker", "reranker").await;
    fx.model_of(&p, "qwen3-embedding-0.6b", "text_embedding").await;
    let after = fx.ranked(20, true).await;
    assert_eq!(before.len(), 20);
    assert_eq!(after, before);
    assert!(!c.calls().is_empty(), "the configured embedding stub is called");
    assert!(r.calls().is_empty(), "the configured reranker is never called");
}

/// SPEC-0102/E2E-107: a bearer reranker.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_107_a_bearer_reranker() {
    let (c, r) = (EmbedStub::start(1024).await, RerankStub::start().await);
    r.require("rk-1");
    let fx = searching_with_rerank(&c, &r).await;
    fx.twenty_guides().await;
    let fused = fx.ranked(20, false).await;
    fx.reranker_on("Rerank Cloud", &r.base, Some("rk-1")).await;
    let got = fx.ranked(20, true).await;
    let calls = rerank_calls(&r);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].headers.get("authorization").map(String::as_str), Some("Bearer rk-1"));
    assert_eq!(got, fused.iter().rev().take(10).cloned().collect::<Vec<_>>());
    fx.assert_nowhere("rk-1").await;
}

/// SPEC-0102/E2E-108: a reranker that answers with an error.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_108_a_reranker_that_answers_with_an_error() {
    let (c, r) = (EmbedStub::start(1024).await, RerankStub::start().await);
    let fx = searching_with_rerank(&c, &r).await;
    fx.twenty_guides().await;
    let fused = fx.ranked(20, false).await;
    fx.reranker_on("Local Rerank", &r.base, None).await;
    r.fail_with(503);
    let changes = fx.changes().await.len();
    assert_eq!(fx.ranked(20, true).await, fused);
    assert_eq!(rerank_calls(&r).len(), 1);
    assert_eq!(fx.changes().await.len(), changes, "no setting changed");
    assert_eq!(fx.defaults().await["reranker"]["name"], json!("bge-reranker"));
}

/// SPEC-0102/E2E-114: reranking not asked for.
#[cfg(feature = "rag")]
#[tokio::test]
async fn e2e_0102_114_reranking_not_asked_for() {
    let (c, r) = (EmbedStub::start(1024).await, RerankStub::start().await);
    let fx = searching_with_rerank(&c, &r).await;
    fx.twenty_guides().await;
    fx.reranker_on("Local Rerank", &r.base, None).await;
    let off = fx.ranked(20, false).await;
    assert!(r.calls().is_empty(), "no call when rerank is false");
    assert_eq!(off.len(), 20);
    // The default asks for reranking: the order then differs from the fused one.
    assert_ne!(fx.ranked(20, true).await, off);
    assert_eq!(r.calls().len(), 1);
}

// ── US-0288: the REST interface ──────────────────────────────────────────────

use axum::http::StatusCode;

/// The `{error, detail}` a REST refusal must carry for a tool refusal text.
fn refusal(status: StatusCode, body: &Value, tool: &str) {
    assert_eq!(body["detail"], json!(tool), "same text as the tool");
    let code = tool.split(':').next().unwrap();
    assert_eq!(body["error"], json!(code));
    let want = match code {
        "ERR_FORBIDDEN" => 403,
        "ERR_NOT_FOUND" => 404,
        "ERR_INVALID_ARGUMENT" => 400,
        other => panic!("unexpected code {other}"),
    };
    assert_eq!(status.as_u16(), want, "the status of the code");
}

/// SPEC-0102/E2E-070: the REST interface creates and lists.
#[tokio::test]
async fn e2e_0102_070_the_rest_interface_creates_and_lists() {
    let fx = Fx::new().await;
    let body = json!({"name": "Local GPU", "endpoint": "http://localhost:9010/v1"});
    let (s, p) = fx.rest(ALICE, "POST", "/api/admin/providers", Some(body)).await;
    assert_eq!(s, StatusCode::OK);
    assert!(p["provider_id"].as_str().unwrap().starts_with("prov-"));
    assert_eq!(p["name"], "Local GPU");
    assert_eq!(p["auth_scheme"], "none");
    assert_eq!(p["has_secret"], false);
    assert_eq!(p["created_at"], p["updated_at"]);
    let (s, page) = fx.rest(ALICE, "GET", "/api/admin/providers", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(page["providers"], json!([p]));
    assert!(page.get("marker").is_none());
    // The same answer as the tool, which also lists the provider.
    assert_eq!(fx.list_as(ALICE, "").await.unwrap(), page);
    // The secret travels in the body and is never returned.
    let body = json!({"name": "Cloud Embeddings", "endpoint": "https://api.example.com/v1",
        "auth_scheme": "bearer", "secret": SECRET});
    let (s, c) = fx.rest(ALICE, "POST", "/api/admin/providers", Some(body)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(c["has_secret"], true);
    fx.assert_nowhere(SECRET).await;
}

/// SPEC-0102/E2E-071: the REST interface refuses like the tools.
#[tokio::test]
async fn e2e_0102_071_the_rest_interface_refuses_like_the_tools() {
    let fx = Fx::new().await;
    let p = fx.local("Local GPU").await;
    let pid = id_of(&p, "provider_id");
    let dup = json!({"name": "local gpu", "endpoint": "http://h/v1"});
    let tool = fx.create(dup.clone()).await.unwrap_err();
    let (s, b) = fx.rest(ALICE, "POST", "/api/admin/providers", Some(dup)).await;
    refusal(s, &b, &tool);
    let zero = json!({"provider_id": pid, "name": "m", "kind": "text_embedding", "dimension": 0});
    let tool = fx.model(zero.clone()).await.unwrap_err();
    assert!(tool.contains("dimension must be between 1 and 8192"), "{tool}");
    let (s, b) = fx.rest(ALICE, "POST", "/api/admin/models", Some(zero)).await;
    refusal(s, &b, &tool);
    let ghost = json!({"provider_id": "prov-000000000000", "name": "m", "kind": "chat"});
    let tool = fx.model(ghost.clone()).await.unwrap_err();
    assert!(tool.contains("provider not found"), "{tool}");
    let (s, b) = fx.rest(ALICE, "POST", "/api/admin/models", Some(ghost)).await;
    refusal(s, &b, &tool);
    assert_eq!(fx.providers().await.len(), 1);
    assert!(fx.models().await.is_empty());
    // A missing provider and model through the id routes, as the tools word them.
    let tool = fx.delete_as(ALICE, "prov-000000000000").await.unwrap_err();
    let (s, b) = fx.rest(ALICE, "DELETE", "/api/admin/providers/prov-000000000000", None).await;
    refusal(s, &b, &tool);
    let tool = fx.delete_model_as(ALICE, "mod-000000000000").await.unwrap_err();
    let (s, b) = fx.rest(ALICE, "DELETE", "/api/admin/models/mod-000000000000", None).await;
    refusal(s, &b, &tool);
    // No bearer token: the plane's own 401.
    use tower::ServiceExt;
    let res = crate::api::router(fx.state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/admin/providers")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

/// SPEC-0102/E2E-072: the REST interface refuses a member.
#[tokio::test]
async fn e2e_0102_072_the_rest_interface_refuses_a_member() {
    let fx = Fx::new().await;
    let want = "ERR_FORBIDDEN: 'bob@example.com' is not a platform admin";
    let (s, b) = fx.rest(BOB, "GET", "/api/admin/providers", None).await;
    refusal(s, &b, want);
    let body = json!({"name": "Local GPU", "endpoint": "http://localhost:9010/v1"});
    let (s, b) = fx.rest(BOB, "POST", "/api/admin/providers", Some(body)).await;
    refusal(s, &b, want);
    for (m, u) in [
        ("GET", "/api/admin/models"),
        ("GET", "/api/admin/defaults"),
        ("GET", "/api/admin/setting-changes"),
        ("DELETE", "/api/admin/defaults/reranker"),
    ] {
        let (s, b) = fx.rest(BOB, m, u, None).await;
        refusal(s, &b, want);
    }
    assert!(fx.providers().await.is_empty());
    assert!(fx.changes().await.is_empty());
}

/// SPEC-0102/E2E-073: a change through one interface is visible through the other.
#[tokio::test]
async fn e2e_0102_073_a_change_through_one_interface_is_visible_through_the_other() {
    let fx = Fx::with_dimension(1024).await;
    let p = fx.local("Local GPU").await;
    let pid = id_of(&p, "provider_id");
    let m = fx.model_of(&pid, "bge-m3", "text_embedding").await;
    let mid = id_of(&m, "model_id");
    let (s, edited) = fx
        .rest(ALICE, "PATCH", &format!("/api/admin/models/{mid}"), Some(json!({"name": "bge-m4"})))
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(edited["name"], "bge-m4");
    let page = fx.models_as(ALICE, "", "", "").await.unwrap();
    assert_eq!(page["models"], json!([edited]));
    let changes = fx.changes().await;
    assert!(changes.len() >= 3, "provider, model and edit are recorded: {changes:?}");
    // The listing filters and the defaults routes answer like the tools.
    let uri = format!("/api/admin/models?provider_id={pid}&kind=text_embedding");
    let (s, list) = fx.rest(ALICE, "GET", &uri, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list, fx.models_as(ALICE, &pid, "text_embedding", "").await.unwrap());
    let (s, set) = fx
        .rest(ALICE, "PUT", "/api/admin/defaults/text_embedding", Some(json!({"model_id": mid})))
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(fx.defaults().await["text_embedding"]["model_id"], json!(mid));
    let (s, got) = fx.rest(ALICE, "GET", "/api/admin/defaults", None).await;
    assert_eq!((s, got), (StatusCode::OK, fx.defaults().await));
    let _ = set;
    let (s, cleared) = fx.rest(ALICE, "DELETE", "/api/admin/defaults/text_embedding", None).await;
    assert_eq!((s, cleared), (StatusCode::OK, json!({"cleared": true})));
    let (s, ch) = fx.rest(ALICE, "GET", "/api/admin/setting-changes", None).await;
    assert_eq!((s, ch), (StatusCode::OK, fx.changes_as(ALICE, "").await.unwrap()));
    let (s, up) = fx
        .rest(ALICE, "PATCH", &format!("/api/admin/providers/{pid}"), Some(json!({"name": "L2"})))
        .await;
    assert_eq!((s, up["name"].clone()), (StatusCode::OK, json!("L2")));
    let (s, del) = fx.rest(ALICE, "DELETE", &format!("/api/admin/models/{mid}"), None).await;
    assert_eq!((s, del), (StatusCode::OK, json!({"deleted": true})));
    let (s, del) = fx.rest(ALICE, "DELETE", &format!("/api/admin/providers/{pid}"), None).await;
    assert_eq!((s, del), (StatusCode::OK, json!({"deleted": true})));
}

/// SPEC-0102/E2E-074: the REST description lists the operations.
#[tokio::test]
async fn e2e_0102_074_the_rest_description_lists_the_operations() {
    use tower::ServiceExt;
    let get = |router: axum::Router| async move {
        let res = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/swagger.json")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null))
    };
    let d = tempfile::tempdir().unwrap();
    let on = |n: &str| (n == "FSA_TOKEN_KEY").then(key_b64);
    let (router, _state) = crate::app::build_with_env(boot_config(d.path()), &on).await.unwrap();
    let (s, doc) = get(router).await;
    assert_eq!(s, StatusCode::OK);
    let paths = &doc["paths"];
    for (path, methods) in [
        ("/api/admin/providers", vec!["post", "get"]),
        ("/api/admin/providers/{provider_id}", vec!["patch", "delete"]),
        ("/api/admin/models", vec!["post", "get"]),
        ("/api/admin/models/{model_id}", vec!["patch", "delete"]),
        ("/api/admin/defaults", vec!["get"]),
        ("/api/admin/defaults/{kind}", vec!["put", "delete"]),
        ("/api/admin/setting-changes", vec!["get"]),
    ] {
        for m in methods {
            assert!(paths[path][m].is_object(), "{m} {path} is documented");
        }
    }
    let names = |path: &str, m: &str| -> Vec<String> {
        paths[path][m]["parameters"]
            .as_array()
            .map(|a| a.iter().map(|p| p["name"].as_str().unwrap().to_string()).collect())
            .unwrap_or_default()
    };
    assert_eq!(names("/api/admin/models", "get"), ["provider_id", "kind", "marker"]);
    assert_eq!(names("/api/admin/setting-changes", "get"), ["marker"]);
    assert_eq!(names("/api/admin/defaults/{kind}", "put"), ["kind"]);
    let create = &paths["/api/admin/providers"]["post"];
    assert!(
        create["description"].as_str().unwrap().starts_with("MCP tool: admin.create_provider.")
    );
    let schema = |n: &str| doc["components"]["schemas"][n]["properties"].clone();
    assert!(schema("CreateProviderBody")["secret"].is_object());
    assert!(schema("CreateModelBody")["dimension"].is_object());
    assert!(schema("SetDefaultModelBody")["model_id"].is_object());
    // Disabled by configuration: the description and the routes are gone.
    let d = tempfile::tempdir().unwrap();
    let mut config = boot_config(d.path());
    config.api.enabled = false;
    let (router, _state) = crate::app::build_with_env(config, &on).await.unwrap();
    let (s, _) = get(router.clone()).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let res = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/admin/providers")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// US-0289: the models screen (E2E-080 to E2E-087 and E2E-109).
// ---------------------------------------------------------------------------

/// How a browser or a client names itself to the screen.
#[derive(Clone, Copy)]
pub(super) enum Via {
    /// The forwarded header: no ambient credential, no token needed.
    Header(&'static str),
    /// The `mcpfs_token` cookie: a form needs its one time token.
    Cookie(&'static str),
}

fn form_body(pairs: &[(&str, &str)]) -> String {
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    pairs.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&")
}

impl Fx {
    /// One router, so its one time tokens live across the calls of a test.
    fn screen(&self) -> axum::Router {
        crate::models_screen::router(self.state.clone())
    }

    fn jwt(&self, who: &str) -> String {
        crate::keys::mint_token_from_file(
            self.dir.path().join("keys").join(crate::keys::PRIVATE_KEY_FILE),
            who,
            crate::keys::DEFAULT_ISSUER,
            crate::keys::DEFAULT_CLAIM,
            3600,
        )
        .unwrap()
    }

    /// One call to the screen: the status and the body text, kept for the
    /// secret search.
    pub(super) async fn page(
        &self,
        router: &axum::Router,
        via: Via,
        method: &str,
        uri: &str,
        form: Option<&[(&str, &str)]>,
    ) -> (axum::http::StatusCode, String) {
        use tower::ServiceExt;
        let mut req = axum::http::Request::builder().method(method).uri(uri);
        req = match via {
            Via::Header(who) => {
                req.header("X-Forwarded-Authorization", format!("Bearer {}", self.jwt(who)))
            }
            Via::Cookie(who) => req.header("Cookie", format!("mcpfs_token={}", self.jwt(who))),
        };
        let body = match form {
            Some(f) => {
                req = req.header("Content-Type", "application/x-www-form-urlencoded");
                axum::body::Body::from(form_body(f))
            }
            None => axum::body::Body::empty(),
        };
        let res = router.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&bytes).to_string();
        self.seen.lock().unwrap().push(text.clone());
        (status, text)
    }

    /// The page as a browser of `who` sees it, with the token its forms carry.
    async fn open(&self, router: &axum::Router, who: &'static str) -> (String, String) {
        let (s, html) = self.page(router, Via::Cookie(who), "GET", "/app/models", None).await;
        assert_eq!(s, axum::http::StatusCode::OK, "{html}");
        let token = token_of(&html);
        (html, token)
    }

    /// A form post as a browser: a fresh token from the page, then the post.
    async fn submit(
        &self,
        router: &axum::Router,
        action: &str,
        fields: &[(&str, &str)],
    ) -> (axum::http::StatusCode, String) {
        let (_, token) = self.open(router, ALICE).await;
        let mut all: Vec<(&str, &str)> = fields.to_vec();
        all.push(("csrf_token", &token));
        self.page(router, Via::Cookie(ALICE), "POST", &format!("/app/models/{action}"), Some(&all))
            .await
    }
}

/// The first `csrf_token` value of a page.
pub(super) fn token_of(html: &str) -> String {
    let at = html.find("name=\"csrf_token\" value=\"").expect("a token in the page");
    let rest = &html[at + "name=\"csrf_token\" value=\"".len()..];
    rest[..rest.find('"').unwrap()].to_string()
}

const NO_TOKEN: &str = "missing or invalid csrf_token";

#[tokio::test]
async fn e2e_0102_080_the_models_screen_shows_the_registry() {
    let fx = Fx::with_dimension(1024).await;
    let cloud = fx.cloud().await;
    let local = fx.local("Local GPU").await;
    fx.model_of(local["provider_id"].as_str().unwrap(), "qwen3-embedding-0.6b", "text_embedding")
        .await;
    fx.model_of(cloud["provider_id"].as_str().unwrap(), "rerank-v1", "reranker").await;
    let m = fx.models().await;
    let embed = m.iter().find(|m| m["kind"] == "text_embedding").unwrap();
    fx.set_default("text_embedding", embed["model_id"].as_str().unwrap()).await.unwrap();
    let router = fx.screen();
    let (html, _) = fx.open(&router, ALICE).await;
    for want in [
        "Cloud Embeddings",
        "https://api.example.com/v1",
        "bearer",
        "Local GPU",
        "http://localhost:9010/v1",
        "Secret stored",
        "No secret",
        "qwen3-embedding-0.6b",
        "rerank-v1",
    ] {
        assert!(html.contains(want), "page lacks {want}");
    }
    assert_eq!(html.matches("Secret stored").count(), 1);
    assert!(html.contains("Defaults") && html.contains("Latest changes"));
    assert!(!html.contains(SECRET));
    fx.assert_nowhere(SECRET).await;
}

#[tokio::test]
async fn e2e_0102_081_creating_a_provider_and_a_model_on_the_screen() {
    let fx = Fx::new().await;
    let router = fx.screen();
    let (s, _) = fx
        .submit(
            &router,
            "provider/create",
            &[
                ("name", "Local GPU"),
                ("endpoint", "http://localhost:9010/v1"),
                ("auth_scheme", "none"),
            ],
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    let (html, _) = fx.open(&router, ALICE).await;
    assert!(html.contains("Local GPU"));
    let id = fx.providers().await[0]["provider_id"].as_str().unwrap().to_string();
    let (s, _) = fx
        .submit(
            &router,
            "model/create",
            &[
                ("provider_id", &id),
                ("name", "qwen3-embedding-0.6b"),
                ("kind", "text_embedding"),
                ("dimension", "1024"),
                ("max_input_tokens", ""),
            ],
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    let (html, _) = fx.open(&router, ALICE).await;
    assert!(html.contains("qwen3-embedding-0.6b") && html.contains("1024"));
    assert_eq!(Fx::names(&fx.providers().await), ["Local GPU"]);
    assert_eq!(Fx::names(&fx.models().await), ["qwen3-embedding-0.6b"]);
}

#[tokio::test]
async fn e2e_0102_082_editing_keeps_the_secret_when_the_field_is_empty() {
    let fx = Fx::new().await;
    let id = fx.cloud().await["provider_id"].as_str().unwrap().to_string();
    let router = fx.screen();
    let (html, _) = fx.open(&router, ALICE).await;
    // Every secret input is a password field and carries no value.
    let secret_inputs: Vec<&str> =
        html.split("<input").filter(|i| i.contains("name=\"secret\"")).collect();
    assert!(!secret_inputs.is_empty());
    for i in secret_inputs {
        let tag = &i[..i.find('>').unwrap()];
        assert!(tag.contains("type=\"password\""), "{tag}");
        assert!(!tag.contains("value="), "{tag}");
    }
    let (s, _) = fx
        .submit(
            &router,
            "provider/update",
            &[
                ("provider_id", &id),
                ("name", "Cloud Embeddings"),
                ("endpoint", "https://api2.example.com/v1"),
                ("auth_scheme", "bearer"),
                ("secret", ""),
            ],
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    let (html, _) = fx.open(&router, ALICE).await;
    assert!(html.contains("https://api2.example.com/v1") && html.contains("Secret stored"));
    assert_eq!(fx.providers().await[0]["has_secret"], true);
    fx.assert_nowhere(SECRET).await;
}

#[tokio::test]
async fn e2e_0102_083_a_refused_action_is_explained_on_the_screen() {
    let fx = Fx::new().await;
    fx.local("Local GPU").await;
    let router = fx.screen();
    let (s, html) = fx
        .submit(
            &router,
            "provider/create",
            &[("name", "local gpu"), ("endpoint", "http://localhost:9011/v1")],
        )
        .await;
    assert_ne!(s, axum::http::StatusCode::SEE_OTHER);
    assert!(html.contains("a provider named 'local gpu' already exists"), "{html}");
    assert_eq!(Fx::names(&fx.providers().await), ["Local GPU"]);
}

#[tokio::test]
async fn e2e_0102_084_deleting_and_defaults_on_the_screen() {
    let fx = Fx::with_dimension(1024).await;
    let pid = fx.local("Local GPU").await["provider_id"].as_str().unwrap().to_string();
    let mid = fx.model_of(&pid, "qwen3", "text_embedding").await["model_id"]
        .as_str()
        .unwrap()
        .to_string();
    let router = fx.screen();
    let (s, _) =
        fx.submit(&router, "default/set", &[("kind", "text_embedding"), ("model_id", &mid)]).await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    let (_, html) = fx.submit(&router, "model/delete", &[("model_id", &mid)]).await;
    assert!(html.contains("is a server default"), "{html}");
    assert_eq!(fx.models().await.len(), 1);
    let (s, _) = fx.submit(&router, "default/clear", &[("kind", "text_embedding")]).await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    let (s, _) = fx.submit(&router, "model/delete", &[("model_id", &mid)]).await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    let (html, _) = fx.open(&router, ALICE).await;
    // The change record still names it; the model list does not.
    assert!(!html.contains("data-model="));
    assert!(fx.models().await.is_empty());
}

#[tokio::test]
async fn e2e_0102_085_a_form_without_a_valid_token() {
    let fx = Fx::new().await;
    let router = fx.screen();
    let fields = [("name", "Local GPU"), ("endpoint", "http://localhost:9010/v1")];
    let refused = |s: axum::http::StatusCode, body: &str| {
        assert_eq!(s, axum::http::StatusCode::FORBIDDEN);
        let v: Value = serde_json::from_str(body).unwrap();
        assert_eq!(v, json!({"error": "ERR_FORBIDDEN", "detail": NO_TOKEN}));
    };
    let post = |t: Option<&str>, who: &'static str| {
        let mut f = fields.to_vec();
        if let Some(t) = t {
            f.push(("csrf_token", t));
        }
        let (r, fx) = (router.clone(), &fx);
        let f: Vec<(String, String)> =
            f.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        async move {
            let f: Vec<(&str, &str)> = f.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            fx.page(&r, Via::Cookie(who), "POST", "/app/models/provider/create", Some(&f)).await
        }
    };
    let (s, b) = post(None, ALICE).await;
    refused(s, &b);
    // A token spent once is refused the second time.
    let (_, token) = fx.open(&router, ALICE).await;
    let (s, _) = post(Some(&token), ALICE).await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    let (s, b) = post(Some(&token), ALICE).await;
    refused(s, &b);
    // Someone who is not an admin is told so, whatever the token (FR-NEW-020):
    // the admin refusal wins over the token one.
    let (_, alices) = fx.open(&router, ALICE).await;
    for t in [Some(alices.as_str()), None] {
        let (s, b) = post(t, BOB).await;
        assert_eq!(s, axum::http::StatusCode::FORBIDDEN);
        assert!(b.contains(&format!("ERR_FORBIDDEN: '{BOB}' is not a platform admin")), "{b}");
        assert!(!b.contains(NO_TOKEN), "{b}");
    }
    assert_eq!(Fx::names(&fx.providers().await), ["Local GPU"]);
}

#[tokio::test]
async fn e2e_0102_086_a_header_identity_needs_no_token() {
    let fx = Fx::new().await;
    let router = fx.screen();
    let (s, _) = fx
        .page(
            &router,
            Via::Header(ALICE),
            "POST",
            "/app/models/provider/create",
            Some(&[("name", "Local GPU"), ("endpoint", "http://localhost:9010/v1")]),
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    assert_eq!(Fx::names(&fx.providers().await), ["Local GPU"]);
    let (s, _) = fx
        .page(
            &router,
            Via::Cookie(ALICE),
            "POST",
            "/app/models/provider/create",
            Some(&[("name", "Second"), ("endpoint", "http://localhost:9010/v1")]),
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(fx.providers().await.len(), 1);
}

#[tokio::test]
async fn e2e_0102_087_the_screen_refuses_an_ordinary_member() {
    let fx = Fx::new().await;
    fx.cloud().await;
    let router = fx.screen();
    let want = format!("ERR_FORBIDDEN: '{BOB}' is not a platform admin");
    let (s, body) = fx.page(&router, Via::Cookie(BOB), "GET", "/app/models", None).await;
    assert_eq!(s, axum::http::StatusCode::FORBIDDEN);
    assert!(body.contains(&want), "{body}");
    assert!(!body.contains("Cloud Embeddings") && !body.contains("csrf_token"));
    let posts: [(&str, Vec<(&str, &str)>); 3] = [
        ("provider/create", vec![("name", "X"), ("endpoint", "http://x.test/v1")]),
        ("default/clear", vec![("kind", "reranker")]),
        ("model/delete", vec![("model_id", "model-000000000000")]),
    ];
    for (action, fields) in posts {
        let (s, body) = fx
            .page(
                &router,
                Via::Header(BOB),
                "POST",
                &format!("/app/models/{action}"),
                Some(&fields),
            )
            .await;
        assert_eq!(s, axum::http::StatusCode::FORBIDDEN);
        assert!(body.contains(&want), "{body}");
    }
    assert_eq!(Fx::names(&fx.providers().await), ["Cloud Embeddings"]);
}

#[tokio::test]
async fn e2e_0102_109_a_form_token_is_single_use() {
    let fx = Fx::new().await;
    let router = fx.screen();
    let (_, token) = fx.open(&router, ALICE).await;
    let send = |name: &'static str| {
        let (r, fx, t) = (router.clone(), &fx, token.clone());
        async move {
            fx.page(
                &r,
                Via::Cookie(ALICE),
                "POST",
                "/app/models/provider/create",
                Some(&[
                    ("name", name),
                    ("endpoint", "http://localhost:9010/v1"),
                    ("csrf_token", &t),
                ]),
            )
            .await
        }
    };
    let (s, _) = send("First").await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    let (s, b) = send("Second").await;
    assert_eq!(s, axum::http::StatusCode::FORBIDDEN);
    assert!(b.contains(NO_TOKEN));
    let (_, again) = fx.open(&router, ALICE).await;
    assert_ne!(again, token);
    assert_eq!(Fx::names(&fx.providers().await), ["First"]);
}

#[tokio::test]
async fn e2e_0102_072b_a_non_admin_is_refused_before_the_body_is_read() {
    let fx = Fx::new().await;
    let want = "ERR_FORBIDDEN: 'bob@example.com' is not a platform admin";
    for (m, u) in [
        ("POST", "/api/admin/providers"),
        ("PATCH", "/api/admin/providers/prov-x"),
        ("POST", "/api/admin/models"),
        ("PATCH", "/api/admin/models/model-x"),
        ("PUT", "/api/admin/defaults/reranker"),
    ] {
        let (s, b) = fx.rest(BOB, m, u, None).await;
        refusal(s, &b, want);
    }
}

#[tokio::test]
async fn e2e_0102_088_provider_and_model_actions_on_the_screen() {
    let fx = Fx::new().await;
    let pid = fx.cloud().await["provider_id"].as_str().unwrap().to_string();
    let mid = fx.model_of(&pid, "m", "chat").await["model_id"].as_str().unwrap().to_string();
    let router = fx.screen();
    // model/update
    let (s, _) = fx
        .submit(&router, "model/update", &[("model_id", &mid), ("name", "m2"), ("dimension", "")])
        .await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    assert_eq!(Fx::names(&fx.models().await), ["m2"]);
    // The clear secret box: the secret goes, the page says so.
    let (s, _) = fx
        .submit(
            &router,
            "provider/update",
            &[("provider_id", &pid), ("auth_scheme", "none"), ("clear_secret", "true")],
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    assert_eq!(fx.providers().await[0]["has_secret"], false);
    let (html, _) = fx.open(&router, ALICE).await;
    assert!(html.contains("No secret") && !html.contains("Secret stored"), "{html}");
    // provider/delete is refused while a model remains, then done.
    let (_, html) = fx.submit(&router, "provider/delete", &[("provider_id", &pid)]).await;
    assert!(html.contains("class=\"error\""), "{html}");
    assert_eq!(fx.providers().await.len(), 1);
    fx.submit(&router, "model/delete", &[("model_id", &mid)]).await;
    let (s, _) = fx.submit(&router, "provider/delete", &[("provider_id", &pid)]).await;
    assert_eq!(s, axum::http::StatusCode::SEE_OTHER);
    assert!(fx.providers().await.is_empty());
    fx.assert_nowhere(SECRET).await;
}

#[tokio::test]
async fn e2e_0102_089_a_header_identity_posts_every_action() {
    let fx = Fx::new().await;
    let pid = fx.local("Local GPU").await["provider_id"].as_str().unwrap().to_string();
    let router = fx.screen();
    let post = |action: &'static str, f: Vec<(&'static str, String)>| {
        let (r, fx) = (router.clone(), &fx);
        async move {
            let f: Vec<(&str, &str)> = f.iter().map(|(k, v)| (*k, v.as_str())).collect();
            fx.page(&r, Via::Header(ALICE), "POST", &format!("/app/models/{action}"), Some(&f))
                .await
                .0
        }
    };
    let ok = axum::http::StatusCode::SEE_OTHER;
    assert_eq!(
        post("provider/update", vec![("provider_id", pid.clone()), ("name", "GPU2".into())]).await,
        ok
    );
    assert_eq!(
        post(
            "model/create",
            vec![("provider_id", pid.clone()), ("name", "c".into()), ("kind", "chat".into()),],
        )
        .await,
        ok
    );
    let mid = fx.models().await[0]["model_id"].as_str().unwrap().to_string();
    assert_eq!(
        post("model/update", vec![("model_id", mid.clone()), ("name", "c2".into())]).await,
        ok
    );
    assert_eq!(post("default/clear", vec![("kind", "reranker".into())]).await, ok);
    assert_eq!(post("model/delete", vec![("model_id", mid)]).await, ok);
    assert_eq!(post("provider/delete", vec![("provider_id", pid)]).await, ok);
    assert!(fx.providers().await.is_empty());
}

#[tokio::test]
async fn e2e_0102_090b_the_screen_escapes_what_it_shows() {
    let fx = Fx::new().await;
    fx.create(json!({"name": "<b>\"x\"</b>", "endpoint": "http://h.test/v1?a=1&b=2"}))
        .await
        .unwrap();
    let router = fx.screen();
    let (html, _) = fx.open(&router, ALICE).await;
    assert!(!html.contains("<b>"), "{html}");
    assert!(html.contains("&lt;b&gt;"), "{html}");
    assert!(!html.contains("a=1&b=2"), "{html}");
    assert!(html.contains("a=1&amp;b=2"), "{html}");
}

#[tokio::test]
async fn e2e_0102_091b_a_default_keeps_the_field_check_first() {
    let fx = Fx::new().await;
    let pid = fx.local("Local GPU").await["provider_id"].as_str().unwrap().to_string();
    let mid = fx.model_of(&pid, "rr", "reranker").await["model_id"].as_str().unwrap().to_string();
    fx.set_default("reranker", &mid).await.unwrap();
    let e = fx.update_model(json!({"model_id": mid, "dimension": 5})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: dimension applies to embedding kinds only");
}
