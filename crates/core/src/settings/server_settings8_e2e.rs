//! SPEC-0116 functional tests, US-0352: the names of the variables that hold a secret
//! (FR-NEW-018 at a change, FR-NEW-019 at use, FR-NEW-024, 034, 036, 044 at the start).

use super::e2e::Fx;
use super::server_settings_e2e::{KEY_A, boot, boot_config, load_yaml, yaml_for};
use super::server_settings4_e2e::{get, lines, nothing_stored, set};
use super::server_settings5_e2e::put_raw;
use super::server_settings6_e2e::{capture_logs, logged};
use crate::config::{EmbeddingConfig, GitConfig, OcrConfig};
use crate::docs::ocr::{MultimodalOcrProvider, OcrProvider};
use crate::git::oauth::device_flow::{DeviceFlowClient, HttpDeviceFlowClient};
use crate::search::embedding::{EmbedTargets, StaticTargets};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const MASTER: [&str; 12] = [
    "FSA_TOKEN_KEY",
    "MCPFS_TOKEN_KEY",
    "FSA_DATABASE_URL",
    "FSA_MINIO_ACCESS_KEY",
    "FSA_MINIO_SECRET_KEY",
    "FSA_META_DSN",
    "MCPFS_META_DSN",
    "MCPFS_ADMIN_DSN",
    "MCPFS_GIT_DSN",
    "MCPFS_OAUTH_DSN",
    "FSA_DOC_SERVICE_TOKEN",
    "MCPFS_DOC_SERVICE_TOKEN",
];

/// The five keys: (section, the patch that sets it, the dotted name, the sink).
fn patch_of(dotted: &str, v: Value) -> (&'static str, Value) {
    match dotted {
        "git.github_client_secret_env" => ("git", json!({"github_client_secret_env": v})),
        "git.gitlab_client_secret_env" => ("git", json!({"gitlab_client_secret_env": v})),
        "extract.ocr.api_key_env" => ("extract", json!({"ocr": {"api_key_env": v}})),
        "search.embedding.api_key_env" => ("search", json!({"embedding": {"api_key_env": v}})),
        "search.reranking.api_key_env" => ("search", json!({"reranking": {"api_key_env": v}})),
        other => panic!("{other}"),
    }
}

const FIVE: [&str; 5] = [
    "git.github_client_secret_env",
    "git.gitlab_client_secret_env",
    "extract.ocr.api_key_env",
    "search.embedding.api_key_env",
    "search.reranking.api_key_env",
];

fn sink(dotted: &str) -> &'static str {
    match dotted {
        "git.github_client_secret_env" => "GITHUB",
        "git.gitlab_client_secret_env" => "GITLAB",
        "extract.ocr.api_key_env" => "OCR",
        "search.embedding.api_key_env" => "EMBEDDING",
        _ => "RERANKING",
    }
}

fn reserved(dotted: &str, name: &str) -> String {
    format!("{dotted} '{name}' names a variable reserved by the server, it cannot hold a secret")
}

fn form(dotted: &str, name: &str) -> String {
    format!(
        "{dotted} '{name}' is not an accepted variable name: use FSA_SECRET_{}_ followed by capital letters, digits and underscores",
        sink(dotted)
    )
}

async fn try_name(fx: &Fx, dotted: &str, v: Value) -> Result<Value, String> {
    let (section, patch) = patch_of(dotted, v);
    set(fx, section, patch).await
}

async fn refused_name(fx: &Fx, dotted: &str, v: Value) -> String {
    try_name(fx, dotted, v).await.unwrap_err()
}

// ---- at a change ----------------------------------------------------------------

#[tokio::test]
async fn e2e_0116_050_a_master_name_is_refused_for_each_of_the_five_keys() {
    let fx = Fx::new().await;
    for dotted in FIVE {
        for name in MASTER {
            assert_eq!(refused_name(&fx, dotted, json!(name)).await, reserved(dotted, name));
        }
    }
    for section in ["git", "extract", "search"] {
        assert_eq!(get(&fx, section).await["source"], "default");
    }
    assert!(lines(&fx.changes().await).is_empty());
}

#[tokio::test]
async fn e2e_0116_051_a_name_outside_the_namespace_or_the_form_is_refused() {
    let fx = Fx::new().await;
    for n in [
        "my secret",
        "lower",
        "1ABC",
        "A-B",
        "ÉTÉ",
        "MY_GITLAB_SECRET",
        "FSA_SECRET_GITLAB_",
        "FSA_SECRET_GITLAB_lower",
        "FSA_SECRET_OCR_X",
        "",
    ] {
        let d = "git.gitlab_client_secret_env";
        assert_eq!(refused_name(&fx, d, json!(n)).await, form(d, n), "{n}");
    }
    let d = "git.gitlab_client_secret_env";
    assert_eq!(refused_name(&fx, d, Value::Null).await, form(d, ""));
    let g = "git.github_client_secret_env";
    for n in ["a b", ""] {
        assert_eq!(refused_name(&fx, g, json!(n)).await, form(g, n));
    }
    let o = "extract.ocr.api_key_env";
    for n in ["lower", ""] {
        assert_eq!(refused_name(&fx, o, json!(n)).await, form(o, n));
    }
    let e = "search.embedding.api_key_env";
    assert_eq!(refused_name(&fx, e, json!("a b")).await, form(e, "a b"));
    assert_eq!(refused_name(&fx, e, Value::Null).await, form(e, ""));
    let r = "search.reranking.api_key_env";
    assert_eq!(refused_name(&fx, r, json!("x y")).await, form(r, "x y"));
    assert_eq!(refused_name(&fx, r, Value::Null).await, form(r, ""));
    for section in ["git", "extract", "search"] {
        assert_eq!(get(&fx, section).await["source"], "default");
    }
    // What is accepted.
    for (d, n) in [(d, "GITLAB_CLIENT_SECRET"), (d, "FSA_SECRET_GITLAB_MAIN")] {
        try_name(&fx, d, json!(n)).await.unwrap();
    }
    try_name(&fx, g, Value::Null).await.unwrap();
    try_name(&fx, o, Value::Null).await.unwrap();
    try_name(&fx, e, json!("")).await.unwrap();
    try_name(&fx, r, json!("")).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_052_a_stored_name_is_not_judged_by_a_change_that_does_not_give_it() {
    let fx = Fx::new().await;
    let mut v = get(&fx, "git").await["value"].clone();
    v["gitlab_client_secret_env"] = json!("lower");
    put_raw(&fx, "git", v).await;
    let out = set(&fx, "git", json!({"max_pack_size_mb": 64})).await.unwrap();
    assert_eq!(out["value"]["gitlab_client_secret_env"], "lower");
    let d = "git.gitlab_client_secret_env";
    assert_eq!(refused_name(&fx, d, json!("lower")).await, form(d, "lower"));
}

#[tokio::test]
async fn e2e_0116_065_an_empty_or_null_name_where_none_is_not_allowed_is_refused() {
    let fx = Fx::new().await;
    let e = "search.embedding.api_key_env";
    let r = "search.reranking.api_key_env";
    let g = "git.github_client_secret_env";
    let o = "extract.ocr.api_key_env";
    for (d, n) in [
        (e, "FSA_SECRET_EMBEDDING_A"),
        (r, "FSA_SECRET_RERANKING_A"),
        (g, "FSA_SECRET_GITHUB_A"),
        (o, "FSA_SECRET_OCR_A"),
    ] {
        try_name(&fx, d, json!(n)).await.unwrap();
    }
    assert_eq!(refused_name(&fx, e, Value::Null).await, form(e, ""));
    assert_eq!(refused_name(&fx, r, Value::Null).await, form(r, ""));
    assert_eq!(refused_name(&fx, g, json!("")).await, form(g, ""));
    assert_eq!(refused_name(&fx, o, json!("")).await, form(o, ""));
    assert_eq!(get(&fx, "git").await["value"]["github_client_secret_env"], "FSA_SECRET_GITHUB_A");
    assert_eq!(get(&fx, "extract").await["value"]["ocr"]["api_key_env"], "FSA_SECRET_OCR_A");
}

#[tokio::test]
async fn e2e_0116_072_each_key_accepts_its_own_defaults_only() {
    let fx = Fx::new().await;
    let d = "git.gitlab_client_secret_env";
    assert_eq!(refused_name(&fx, d, json!("FSA_OCR_KEY")).await, form(d, "FSA_OCR_KEY"));
    assert_eq!(refused_name(&fx, d, json!("FSA_X")).await, form(d, "FSA_X"));
    assert_eq!(refused_name(&fx, d, json!("fsa_secret_x")).await, form(d, "fsa_secret_x"));
    let o = "extract.ocr.api_key_env";
    for n in ["FSA_OCR_KEY", "MCP_FS_OCR_KEY"] {
        try_name(&fx, o, json!(n)).await.unwrap();
        assert_eq!(
            refused_name(&fx, "search.embedding.api_key_env", json!(n)).await,
            form("search.embedding.api_key_env", n)
        );
    }
    let g = "git.github_client_secret_env";
    for n in ["FSA_GITHUB_CLIENT_SECRET", "MCPFS_GITHUB_CLIENT_SECRET"] {
        try_name(&fx, g, json!(n)).await.unwrap();
        assert_eq!(refused_name(&fx, d, json!(n)).await, form(d, n));
    }
}

#[tokio::test]
async fn e2e_0116_073_an_ambient_variable_is_refused_a_namespace_name_is_accepted() {
    let fx = Fx::new().await;
    let o = "extract.ocr.api_key_env";
    for n in ["DATABASE_URL", "PGPASSWORD", "HOME", "OPENAI_API_KEY", "AWS_SECRET_ACCESS_KEY"] {
        assert_eq!(refused_name(&fx, o, json!(n)).await, form(o, n));
    }
    let g = "git.github_client_secret_env";
    assert_eq!(refused_name(&fx, g, json!("GITHUB_TOKEN")).await, form(g, "GITHUB_TOKEN"));
    nothing_stored(&fx, "extract").await;
    try_name(&fx, o, json!("FSA_SECRET_OCR_OPENAI")).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_078_the_name_checks_run_before_every_other_check_of_the_change() {
    let fx = Fx::new().await;
    let m = set(
        &fx,
        "git",
        json!({"github_client_secret_env": "FSA_X", "gitlab_client_secret_env": "FSA_TOKEN_KEY"}),
    )
    .await
    .unwrap_err();
    assert_eq!(m, form("git.github_client_secret_env", "FSA_X"));
    let m = set(
        &fx,
        "git",
        json!({"gitlab_client_secret_env": "FSA_TOKEN_KEY", "max_pack_size_mb": "big"}),
    )
    .await
    .unwrap_err();
    assert_eq!(m, reserved("git.gitlab_client_secret_env", "FSA_TOKEN_KEY"));
}

#[tokio::test]
async fn e2e_0116_079_a_name_that_is_not_a_text_is_the_type_refusal_of_its_section() {
    let fx = Fx::new().await;
    let m = set(&fx, "git", json!({"github_client_secret_env": 5})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'git':"), "{m}");
    let m = set(&fx, "search", json!({"embedding": {"api_key_env": true}})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'search':"), "{m}");
    let m = set(&fx, "extract", json!({"ocr": {"api_key_env": ["A_KEY"]}})).await.unwrap_err();
    assert!(m.starts_with("invalid setting 'extract':"), "{m}");
    for s in ["git", "search", "extract"] {
        assert_eq!(get(&fx, s).await["source"], "default");
    }
}

#[tokio::test]
async fn e2e_0116_086_a_name_with_non_ascii_letters_or_digits_is_refused() {
    let fx = Fx::new().await;
    let d = "git.gitlab_client_secret_env";
    for n in ["FSA_SECRET_GITLAB_É", "FSA_SECRET_GITLAB_٣"] {
        assert_eq!(refused_name(&fx, d, json!(n)).await, form(d, n));
    }
    try_name(&fx, d, json!("FSA_SECRET_GITLAB_A_1")).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_105_a_name_made_for_another_key_is_refused() {
    let fx = Fx::new().await;
    let o = "extract.ocr.api_key_env";
    let d = "git.gitlab_client_secret_env";
    assert_eq!(
        refused_name(&fx, o, json!("FSA_SECRET_GITLAB_MAIN")).await,
        form(o, "FSA_SECRET_GITLAB_MAIN")
    );
    assert_eq!(refused_name(&fx, d, json!("FSA_SECRET_OCR_X")).await, form(d, "FSA_SECRET_OCR_X"));
    try_name(&fx, o, json!("FSA_SECRET_OCR_X")).await.unwrap();
}

// ---- stand ins -----------------------------------------------------------------------

type Seen = Arc<Mutex<Vec<HashMap<String, String>>>>;

/// A stand in for a git provider: records the forms posted to the device and token
/// endpoints of both GitHub and GitLab.
async fn provider() -> (String, Seen) {
    use axum::{Router, extract::Form, routing::post};
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let (a, b) = (seen.clone(), seen.clone());
    let device = move |Form(f): Form<HashMap<String, String>>| {
        let seen = a.clone();
        async move {
            seen.lock().unwrap().push(f);
            axum::Json(json!({"device_code": "dc", "user_code": "u", "verification_uri": "v",
                "expires_in": 600, "interval": 1}))
        }
    };
    let token = move |Form(f): Form<HashMap<String, String>>| {
        let seen = b.clone();
        async move {
            seen.lock().unwrap().push(f);
            axum::Json(json!({"access_token": "tok", "scope": "repo"}))
        }
    };
    let app = Router::new()
        .route("/login/device/code", post(device.clone()))
        .route("/oauth/authorize_device", post(device))
        .route("/login/oauth/access_token", post(token.clone()))
        .route("/oauth/token", post(token));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), seen)
}

/// One device flow poll of `provider` (`github` or `gitlab`) with the git section `git`:
/// the poll answer. Every form the stand in saw is in `seen`.
async fn poll(
    git: GitConfig,
    base: &str,
    which: &str,
) -> Result<crate::git::oauth::device_flow::TokenPoll, crate::errors::ToolError> {
    let client = HttpDeviceFlowClient::new(git).unwrap().with_github_urls(
        format!("{base}/login/device/code"),
        format!("{base}/login/oauth/access_token"),
    );
    let instance = (which == "gitlab").then_some(base);
    let code = client.request_device_code(which, instance).await.unwrap();
    client.poll_for_token(&code).await
}

fn token_forms(seen: &Seen) -> Vec<HashMap<String, String>> {
    seen.lock().unwrap().iter().filter(|f| f.contains_key("device_code")).cloned().collect()
}

/// A stand in for an OCR endpoint: records the `Authorization` header of each request.
async fn ocr_stand_in() -> (String, Arc<Mutex<Vec<Option<String>>>>) {
    use axum::{Router, http::HeaderMap, routing::post};
    let seen: Arc<Mutex<Vec<Option<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let s = seen.clone();
    let app = Router::new().route(
        "/v1/chat",
        post(move |headers: HeaderMap| {
            let s = s.clone();
            async move {
                let auth =
                    headers.get("authorization").and_then(|v| v.to_str().ok()).map(str::to_string);
                s.lock().unwrap().push(auth);
                axum::Json(json!({"choices": [{"message": {"content": "text"}}]}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}/v1/chat"), seen)
}

fn ocr_config(endpoint: &str, name: Option<&str>) -> OcrConfig {
    OcrConfig {
        provider: "multimodal".into(),
        endpoint: endpoint.into(),
        model: "m".into(),
        api_key_env: name.map(str::to_string),
        prompt: "p".into(),
    }
}

const MARKER: &str = "s3cret-marker";

/// Sets `name` to the marker for the duration of a test; the name is the test's own.
struct EnvVar(&'static str);
impl EnvVar {
    fn set(name: &'static str) -> Self {
        // SAFETY: every test owns its variable names, no other test reads them.
        unsafe { std::env::set_var(name, MARKER) };
        Self(name)
    }
}
impl Drop for EnvVar {
    fn drop(&mut self) {
        // SAFETY: as above.
        unsafe { std::env::remove_var(self.0) };
    }
}

// ---- at use ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_0116_053_a_device_flow_never_reads_a_master_name() {
    let (base, seen) = provider().await;
    let _v = EnvVar::set("FSA_DATABASE_URL");
    let git = GitConfig {
        gitlab_client_id: "gl".into(),
        gitlab_client_secret_env: "FSA_DATABASE_URL".into(),
        ..GitConfig::default()
    };
    let e = poll(git, &base, "gitlab").await.unwrap_err();
    assert_eq!(
        e.message,
        "GitLab client secret variable 'FSA_DATABASE_URL' is not allowed: the server reserves it"
    );
    assert_eq!(e.code, crate::errors::code::INVALID_ARGUMENT);
    assert!(token_forms(&seen).is_empty());
    assert!(!format!("{:?}", seen.lock().unwrap()).contains(MARKER));
    let git = GitConfig {
        github_client_id: "gh".into(),
        github_client_secret_env: Some("MCPFS_TOKEN_KEY".into()),
        ..GitConfig::default()
    };
    let e = poll(git, &base, "github").await.unwrap_err();
    assert_eq!(
        e.message,
        "GitHub client secret variable 'MCPFS_TOKEN_KEY' is not allowed: the server reserves it"
    );
    assert!(token_forms(&seen).is_empty());
}

#[tokio::test]
async fn e2e_0116_057_an_ocr_read_never_reads_a_master_name() {
    let (url, seen) = ocr_stand_in().await;
    let _v = EnvVar::set("FSA_DATABASE_URL");
    let p = MultimodalOcrProvider::new(ocr_config(&url, Some("FSA_DATABASE_URL")));
    let e = p.extract_text(b"x", "image/png").await.unwrap_err();
    assert_eq!(
        e.message,
        "OCR key variable 'FSA_DATABASE_URL' is not allowed: the server reserves it"
    );
    assert_eq!(e.code, crate::errors::code::INVALID_ARGUMENT);
    assert!(seen.lock().unwrap().is_empty());
    let _k = EnvVar::set("E2E116_057_OCR_KEY");
    let p = MultimodalOcrProvider::new(ocr_config(&url, Some("E2E116_057_OCR_KEY")));
    p.extract_text(b"x", "image/png").await.unwrap();
    assert_eq!(seen.lock().unwrap().as_slice(), [Some(format!("Bearer {MARKER}"))]);
}

#[tokio::test]
async fn e2e_0116_058_an_embedding_call_never_reads_a_master_name() {
    let cfg =
        EmbeddingConfig { api_key_env: "MCPFS_TOKEN_KEY".into(), ..EmbeddingConfig::default() };
    let e = StaticTargets(cfg).target().await.unwrap_err();
    assert_eq!(
        e.message,
        "Embedding key variable 'MCPFS_TOKEN_KEY' is not allowed: the server reserves it"
    );
    assert_eq!(e.code, crate::errors::code::INVALID_ARGUMENT);
}

#[tokio::test]
async fn e2e_0116_106_every_master_name_is_refused_by_every_sink() {
    let (base, seen) = provider().await;
    let (url, ocr_seen) = ocr_stand_in().await;
    for name in MASTER {
        // The variable holds the marker for the length of this iteration.
        // The two names the process itself reads from its environment are never set here:
        // the refusal comes before any read, and a stray value would break other tests.
        let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
        let _v = (!name.ends_with("TOKEN_KEY")).then(|| EnvVar::set(leaked));
        let gl = GitConfig {
            gitlab_client_id: "gl".into(),
            gitlab_client_secret_env: name.into(),
            ..GitConfig::default()
        };
        let gh = GitConfig {
            github_client_id: "gh".into(),
            github_client_secret_env: Some(name.into()),
            ..GitConfig::default()
        };
        let msgs = [
            (
                poll(gl, &base, "gitlab").await.unwrap_err().message,
                format!(
                    "GitLab client secret variable '{name}' is not allowed: the server reserves it"
                ),
            ),
            (
                poll(gh, &base, "github").await.unwrap_err().message,
                format!(
                    "GitHub client secret variable '{name}' is not allowed: the server reserves it"
                ),
            ),
            (
                MultimodalOcrProvider::new(ocr_config(&url, Some(name)))
                    .extract_text(b"x", "image/png")
                    .await
                    .unwrap_err()
                    .message,
                format!("OCR key variable '{name}' is not allowed: the server reserves it"),
            ),
            (
                StaticTargets(EmbeddingConfig {
                    api_key_env: name.into(),
                    ..EmbeddingConfig::default()
                })
                .target()
                .await
                .unwrap_err()
                .message,
                format!("Embedding key variable '{name}' is not allowed: the server reserves it"),
            ),
        ];
        for (got, want) in msgs {
            assert_eq!(got, want);
        }
    }
    assert!(token_forms(&seen).is_empty() && ocr_seen.lock().unwrap().is_empty());
}

// ---- at the start ----------------------------------------------------------------------

fn outside(dotted: &str, name: &str) -> String {
    format!(
        "{dotted} names '{name}' outside the secret namespace: it is still read as a secret, prefer a name starting with FSA_SECRET_"
    )
}

fn not_used(dotted: &str, name: &str) -> String {
    format!("{dotted} names '{name}' outside the secret namespace: the stored name is not used")
}

fn master_warning(dotted: &str, name: &str) -> String {
    format!(
        "{dotted} names the reserved variable '{name}': the server will not read it as a secret"
    )
}

/// A YAML file (JSON) for `root` with the extra sections `extra` merged in.
fn yaml_with(root: &std::path::Path, extra: Value) -> std::path::PathBuf {
    let base = yaml_for(root, "");
    let mut doc: Value = yaml_to_json(&std::fs::read_to_string(&base).unwrap());
    for (k, v) in extra.as_object().unwrap() {
        doc[k] = v.clone();
    }
    let p = root.join("config.json.yaml");
    std::fs::write(&p, doc.to_string()).unwrap();
    p
}

fn yaml_to_json(text: &str) -> Value {
    let v: serde_json::Value = serde_norway::from_str(text).unwrap();
    v
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_054_a_yaml_master_name_starts_with_a_warning_and_is_never_read() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let path = yaml_with(
        d.path(),
        json!({"git": {"gitlab_client_secret_env": "FSA_TOKEN_KEY", "gitlab_client_id": "gl"}}),
    );
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        logged(&logs)
            .matches(&master_warning("git.gitlab_client_secret_env", "FSA_TOKEN_KEY"))
            .count(),
        1
    );
    assert!(!logged(&logs).contains(MARKER));
    let _ = state;
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_055_a_lower_case_yaml_name_keeps_working() {
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let _v = EnvVar::set("my_gitlab_secret_e2e055");
    let path = yaml_with(
        d.path(),
        json!({"git": {"gitlab_client_secret_env": "my_gitlab_secret_e2e055", "gitlab_client_id": "gl"}}),
    );
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    let poll = poll(state.config.git.clone(), &base, "gitlab").await.unwrap();
    assert!(poll.success);
    assert_eq!(token_forms(&seen)[0]["client_secret"], MARKER);
}

async fn start_with_git_row(
    d: &std::path::Path,
    edit: impl FnOnce(&mut Value),
    yaml: Option<Value>,
) -> (crate::config::ServerConfig, Arc<crate::state::AppState>) {
    start_with_git_row_logs(d, edit, yaml, None).await
}

/// The same, forgetting what the first start logged when `logs` is given.
async fn start_with_git_row_logs(
    d: &std::path::Path,
    edit: impl FnOnce(&mut Value),
    yaml: Option<Value>,
    logs: Option<&Arc<Mutex<Vec<u8>>>>,
) -> (crate::config::ServerConfig, Arc<crate::state::AppState>) {
    let (_r, first) = match &yaml {
        Some(y) => boot(load_yaml(&yaml_with(d, y.clone())), Some(KEY_A)).await.unwrap(),
        None => boot(boot_config(d), Some(KEY_A)).await.unwrap(),
    };
    let mut v = get_value(&first, "git").await;
    edit(&mut v);
    put_text(&first, "git", &v.to_string()).await;
    drop((_r, first));
    if let Some(l) = logs {
        l.lock().unwrap().clear();
    }
    let (_r, second) = match &yaml {
        Some(y) => boot(load_yaml(&yaml_with(d, y.clone())), Some(KEY_A)).await.unwrap(),
        None => boot(boot_config(d), Some(KEY_A)).await.unwrap(),
    };
    (second.config.as_ref().clone(), second)
}

async fn get_value(state: &Arc<crate::state::AppState>, section: &str) -> Value {
    super::server_settings::section_json(&state.config, section)
}

async fn put_text(state: &Arc<crate::state::AppState>, section: &str, text: &str) {
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
async fn e2e_0116_056_a_stored_master_name_warns_and_the_use_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let (logs, _g) = capture_logs();
    let (_c, state) = start_with_git_row(
        d.path(),
        |v| v["github_client_secret_env"] = json!("MCPFS_TOKEN_KEY"),
        None,
    )
    .await;
    let log = logged(&logs);
    assert!(
        log.matches(&master_warning("git.github_client_secret_env", "MCPFS_TOKEN_KEY")).count()
            >= 1
    );
    assert!(!log.contains("the stored name is not used"));
    let git = GitConfig { github_client_id: "gh".into(), ..state.config.git.clone() };
    let e = poll(git, &base, "github").await.unwrap_err();
    assert_eq!(
        e.message,
        "GitHub client secret variable 'MCPFS_TOKEN_KEY' is not allowed: the server reserves it"
    );
    assert!(token_forms(&seen).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_082_a_stored_minio_name_is_refused_at_use_and_not_logged_as_unused() {
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("FSA_MINIO_SECRET_KEY");
    let (_c, state) = start_with_git_row(
        d.path(),
        |v| v["gitlab_client_secret_env"] = json!("FSA_MINIO_SECRET_KEY"),
        None,
    )
    .await;
    assert!(!logged(&logs).contains("the stored name is not used"));
    let git = GitConfig { gitlab_client_id: "gl".into(), ..state.config.git.clone() };
    let e = poll(git, &base, "gitlab").await.unwrap_err();
    assert_eq!(
        e.message,
        "GitLab client secret variable 'FSA_MINIO_SECRET_KEY' is not allowed: the server reserves it"
    );
    assert_eq!(e.code, crate::errors::code::INVALID_ARGUMENT);
    assert!(token_forms(&seen).is_empty());
    assert!(!format!("{:?}", seen.lock().unwrap()).contains(MARKER));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_136_a_stored_master_name_is_kept_and_a_read_says_no_restart() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let (_c, state) = start_with_git_row(
        d.path(),
        |v| v["gitlab_client_secret_env"] = json!("FSA_MINIO_SECRET_KEY"),
        None,
    )
    .await;
    let log = logged(&logs);
    assert_eq!(
        log.matches(&master_warning("git.gitlab_client_secret_env", "FSA_MINIO_SECRET_KEY"))
            .count(),
        1
    );
    assert!(!log.contains("the stored name is not used"));
    let s = super::ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap();
    let out = s.get("git").await.unwrap();
    assert_eq!(out["value"]["gitlab_client_secret_env"], "FSA_MINIO_SECRET_KEY");
    assert_eq!(out["restart_required"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_137_a_stored_master_name_for_ocr_warns_once_and_is_refused_at_use() {
    let d = tempfile::tempdir().unwrap();
    let (url, seen) = ocr_stand_in().await;
    let (logs, _g) = capture_logs();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut v = get_value(&first, "extract").await;
    v["ocr"]["api_key_env"] = json!("MCPFS_TOKEN_KEY");
    put_text(&first, "extract", &v.to_string()).await;
    drop((_r, first));
    logs.lock().unwrap().clear();
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let log = logged(&logs);
    assert_eq!(
        log.matches(&master_warning("extract.ocr.api_key_env", "MCPFS_TOKEN_KEY")).count(),
        1
    );
    assert!(!log.contains("the stored name is not used"));
    let mut ocr = state.config.extract.ocr.clone();
    ocr.provider = "multimodal".into();
    ocr.endpoint = url;
    let e = MultimodalOcrProvider::new(ocr).extract_text(b"x", "image/png").await.unwrap_err();
    assert_eq!(
        e.message,
        "OCR key variable 'MCPFS_TOKEN_KEY' is not allowed: the server reserves it"
    );
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_128_a_stored_name_outside_the_namespace_is_not_used() {
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("E2E116_STORED_KEY");
    let (_c, state) = start_with_git_row(
        d.path(),
        |v| {
            v["gitlab_client_secret_env"] = json!("E2E116_STORED_KEY");
            v["gitlab_client_id"] = json!("gl");
        },
        None,
    )
    .await;
    assert_eq!(
        logged(&logs)
            .matches(&not_used("git.gitlab_client_secret_env", "E2E116_STORED_KEY"))
            .count(),
        1
    );
    let e = poll(state.config.git.clone(), &base, "gitlab").await.unwrap_err();
    assert_eq!(
        e.message,
        "GitLab client secret not set. Set $GITLAB_CLIENT_SECRET environment variable."
    );
    assert!(token_forms(&seen).is_empty());
    assert!(!logged(&logs).contains(MARKER));
    let s = super::ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap();
    let out = s.get("git").await.unwrap();
    assert_eq!(out["value"]["gitlab_client_secret_env"], "E2E116_STORED_KEY");
    assert_eq!(out["restart_required"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_129_a_stored_ocr_name_outside_the_namespace_sends_no_key() {
    let d = tempfile::tempdir().unwrap();
    let (url, seen) = ocr_stand_in().await;
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("E2E116_STORED_OCR");
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut v = get_value(&first, "extract").await;
    v["ocr"]["api_key_env"] = json!("E2E116_STORED_OCR");
    put_text(&first, "extract", &v.to_string()).await;
    drop((_r, first));
    logs.lock().unwrap().clear();
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(
        logged(&logs).matches(&not_used("extract.ocr.api_key_env", "E2E116_STORED_OCR")).count(),
        1
    );
    let mut ocr = state.config.extract.ocr.clone();
    ocr.provider = "multimodal".into();
    ocr.endpoint = url;
    MultimodalOcrProvider::new(ocr).extract_text(b"x", "image/png").await.unwrap();
    assert_eq!(seen.lock().unwrap().as_slice(), [None], "no Authorization header");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_130_stored_names_in_the_namespace_or_the_defaults_are_used_as_stored() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut g = get_value(&first, "git").await;
    g["gitlab_client_secret_env"] = json!("FSA_SECRET_GITLAB_MAIN");
    put_text(&first, "git", &g.to_string()).await;
    let mut e = get_value(&first, "extract").await;
    e["ocr"]["api_key_env"] = json!("FSA_OCR_KEY");
    put_text(&first, "extract", &e.to_string()).await;
    drop((_r, first));
    logs.lock().unwrap().clear();
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert!(!logged(&logs).contains("the stored name is not used"));
    assert_eq!(state.config.git.gitlab_client_secret_env, "FSA_SECRET_GITLAB_MAIN");
    assert_eq!(state.config.extract.ocr.api_key_env.as_deref(), Some("FSA_OCR_KEY"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_138_a_stored_name_that_is_not_a_master_name_is_judged_by_the_namespace_rule() {
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("E2E116_STORED_KEY_138");
    let (_c, state) = start_with_git_row(
        d.path(),
        |v| {
            v["gitlab_client_secret_env"] = json!("E2E116_STORED_KEY_138");
            v["gitlab_client_id"] = json!("gl");
        },
        None,
    )
    .await;
    let log = logged(&logs);
    assert_eq!(
        log.matches(&not_used("git.gitlab_client_secret_env", "E2E116_STORED_KEY_138")).count(),
        1
    );
    assert!(!log.contains("names the reserved variable"));
    let e = poll(state.config.git.clone(), &base, "gitlab").await.unwrap_err();
    assert_eq!(
        e.message,
        "GitLab client secret not set. Set $GITLAB_CLIENT_SECRET environment variable."
    );
    assert!(token_forms(&seen).is_empty());
    assert!(!log.contains(MARKER));
}

// ---- the warning for a name that is still read -------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_090_a_yaml_gitlab_name_outside_the_namespace_warns_and_is_read() {
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("E2E116_GL_KEY_090");
    let path = yaml_with(
        d.path(),
        json!({"git": {"gitlab_client_secret_env": "E2E116_GL_KEY_090", "gitlab_client_id": "gl"}}),
    );
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        logged(&logs)
            .matches(&outside("git.gitlab_client_secret_env", "E2E116_GL_KEY_090"))
            .count(),
        1
    );
    poll(state.config.git.clone(), &base, "gitlab").await.unwrap();
    assert_eq!(token_forms(&seen)[0]["client_secret"], MARKER);
    assert!(!logged(&logs).contains(MARKER));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_104_a_yaml_github_name_outside_the_namespace_warns_and_is_read() {
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("E2E116_GH_KEY_104");
    let path = yaml_with(
        d.path(),
        json!({"git": {"github_client_secret_env": "E2E116_GH_KEY_104", "github_client_id": "gh"}}),
    );
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        logged(&logs)
            .matches(&outside("git.github_client_secret_env", "E2E116_GH_KEY_104"))
            .count(),
        1
    );
    poll(state.config.git.clone(), &base, "github").await.unwrap();
    assert_eq!(token_forms(&seen)[0]["client_secret"], MARKER);
    assert!(!logged(&logs).contains(MARKER));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_084_a_yaml_ocr_name_outside_the_namespace_warns_and_is_read() {
    let d = tempfile::tempdir().unwrap();
    let (url, seen) = ocr_stand_in().await;
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("E2E116_OCR_KEY");
    let path = yaml_with(d.path(), json!({"extract": {"ocr": {"api_key_env": "E2E116_OCR_KEY"}}}));
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        logged(&logs).matches(&outside("extract.ocr.api_key_env", "E2E116_OCR_KEY")).count(),
        1
    );
    let mut ocr = state.config.extract.ocr.clone();
    ocr.provider = "multimodal".into();
    ocr.endpoint = url;
    MultimodalOcrProvider::new(ocr).extract_text(b"x", "image/png").await.unwrap();
    assert_eq!(seen.lock().unwrap().as_slice(), [Some(format!("Bearer {MARKER}"))]);
    assert!(!logged(&logs).contains(MARKER));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_109_an_ambient_ocr_name_in_a_yaml_file_warns_and_is_read() {
    let d = tempfile::tempdir().unwrap();
    let (url, seen) = ocr_stand_in().await;
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("AWS_SECRET_ACCESS_KEY");
    let path =
        yaml_with(d.path(), json!({"extract": {"ocr": {"api_key_env": "AWS_SECRET_ACCESS_KEY"}}}));
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        logged(&logs).matches(&outside("extract.ocr.api_key_env", "AWS_SECRET_ACCESS_KEY")).count(),
        1
    );
    let mut ocr = state.config.extract.ocr.clone();
    ocr.provider = "multimodal".into();
    ocr.endpoint = url;
    MultimodalOcrProvider::new(ocr).extract_text(b"x", "image/png").await.unwrap();
    assert_eq!(seen.lock().unwrap().as_slice(), [Some(format!("Bearer {MARKER}"))]);
    assert!(!logged(&logs).contains(MARKER));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_091_a_yaml_embedding_name_outside_the_namespace_warns_and_is_read() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("E2E116_EMB_KEY");
    let path =
        yaml_with(d.path(), json!({"search": {"embedding": {"api_key_env": "E2E116_EMB_KEY"}}}));
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        logged(&logs).matches(&outside("search.embedding.api_key_env", "E2E116_EMB_KEY")).count(),
        1
    );
    let target = StaticTargets(state.config.search.embedding.clone()).target().await.unwrap();
    assert_eq!(target.bearer.as_ref().map(|b| b.expose().to_string()), Some(MARKER.to_string()));
    assert!(!logged(&logs).contains(MARKER));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_092_a_yaml_reranking_name_outside_the_namespace_warns() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let path =
        yaml_with(d.path(), json!({"search": {"reranking": {"api_key_env": "E2E116_RR_KEY"}}}));
    let _booted = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(
        logged(&logs).matches(&outside("search.reranking.api_key_env", "E2E116_RR_KEY")).count(),
        1
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_093_namespace_and_default_names_are_not_warned_about() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let path = yaml_with(
        d.path(),
        json!({
        "git": {"gitlab_client_secret_env": "FSA_SECRET_GITLAB_X"},
        "extract": {"ocr": {"api_key_env": "FSA_OCR_KEY"}}}),
    );
    let _booted = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert!(!logged(&logs).contains("outside the secret namespace"));
    let d2 = tempfile::tempdir().unwrap();
    let path =
        yaml_with(d2.path(), json!({"git": {"gitlab_client_secret_env": "GITLAB_CLIENT_SECRET"}}));
    let _booted = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert!(!logged(&logs).contains("outside the secret namespace"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_107_an_empty_name_is_not_warned_about() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let path = yaml_with(
        d.path(),
        json!({"search": {"embedding": {"api_key_env": ""}, "reranking": {"api_key_env": ""}}}),
    );
    let _booted = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert!(!logged(&logs).contains("outside the secret namespace"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_131_a_yaml_name_seeded_by_the_first_start_keeps_working_while_the_file_names_it()
{
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let (logs, _g) = capture_logs();
    let _v = EnvVar::set("E2E116_GL_KEY_131");
    let yaml =
        json!({"git": {"gitlab_client_secret_env": "E2E116_GL_KEY_131", "gitlab_client_id": "gl"}});
    for round in 0..2 {
        logs.lock().unwrap().clear();
        let (_r, state) =
            boot(load_yaml(&yaml_with(d.path(), yaml.clone())), Some(KEY_A)).await.unwrap();
        let log = logged(&logs);
        assert_eq!(
            log.matches(&outside("git.gitlab_client_secret_env", "E2E116_GL_KEY_131")).count(),
            1,
            "round {round}"
        );
        assert!(!log.contains("the stored name is not used"));
        poll(state.config.git.clone(), &base, "gitlab").await.unwrap();
        assert_eq!(token_forms(&seen).last().unwrap()["client_secret"], MARKER);
    }
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let mut git = state.config.git.clone();
    git.gitlab_client_id = "gl".into();
    let e = poll(git, &base, "gitlab").await.unwrap_err();
    assert_eq!(
        e.message,
        "GitLab client secret not set. Set $GITLAB_CLIENT_SECRET environment variable."
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_164_a_stored_name_that_differs_from_the_yaml_one_is_not_used_and_the_yaml_one_is_not_read()
 {
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let (logs, _g) = capture_logs();
    let _a = EnvVar::set("E2E116_STORED_KEY_164");
    let _b = EnvVar::set("E2E116_GL_KEY_164");
    let yaml =
        json!({"git": {"gitlab_client_secret_env": "E2E116_GL_KEY_164", "gitlab_client_id": "gl"}});
    let (_c, state) = start_with_git_row_logs(
        d.path(),
        |v| v["gitlab_client_secret_env"] = json!("E2E116_STORED_KEY_164"),
        Some(yaml),
        Some(&logs),
    )
    .await;
    let log = logged(&logs);
    assert!(log.contains(&not_used("git.gitlab_client_secret_env", "E2E116_STORED_KEY_164")));
    assert!(!log.contains("it is still read as a secret"));
    let e = poll(state.config.git.clone(), &base, "gitlab").await.unwrap_err();
    assert_eq!(
        e.message,
        "GitLab client secret not set. Set $GITLAB_CLIENT_SECRET environment variable."
    );
    assert!(token_forms(&seen).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_165_a_stored_namespace_name_is_used_next_to_a_yaml_file() {
    let d = tempfile::tempdir().unwrap();
    let (base, seen) = provider().await;
    let (logs, _g) = capture_logs();
    let _a = EnvVar::set("FSA_SECRET_GITLAB_MAIN");
    let yaml =
        json!({"git": {"gitlab_client_secret_env": "E2E116_GL_KEY_165", "gitlab_client_id": "gl"}});
    let (_c, state) = start_with_git_row_logs(
        d.path(),
        |v| v["gitlab_client_secret_env"] = json!("FSA_SECRET_GITLAB_MAIN"),
        Some(yaml),
        Some(&logs),
    )
    .await;
    poll(state.config.git.clone(), &base, "gitlab").await.unwrap();
    assert_eq!(token_forms(&seen)[0]["client_secret"], MARKER);
    let log = logged(&logs);
    assert!(
        !log.contains("the stored name is not used")
            && !log.contains("outside the secret namespace: it is still read")
    );
}

// ---- the kept powers, pinned (D-0116-2, D-0116-5) ------------------------------------
//
// An administrator who points a destination at a stand in, and a secret held in a variable of
// the secret namespace, see that secret reach the stand in. Reported to the owner, not changed;
// each test fails on the day a change binds a secret to its destination.

#[tokio::test]
async fn e2e_0116_184_the_gitlab_instance_url_receives_the_client_secret() {
    let (base, seen) = provider().await;
    let _v = EnvVar::set("FSA_SECRET_GITLAB_PIN184");
    let git = GitConfig {
        gitlab_client_id: "gl".into(),
        gitlab_client_secret_env: "FSA_SECRET_GITLAB_PIN184".into(),
        gitlab_instance_url: base.clone(),
        ..GitConfig::default()
    };
    let client = HttpDeviceFlowClient::new(git).unwrap();
    let code = client.request_device_code("gitlab", None).await.unwrap();
    client.poll_for_token(&code).await.unwrap();
    assert_eq!(token_forms(&seen)[0]["client_secret"], MARKER);
}

#[tokio::test]
async fn e2e_0116_185_a_member_chosen_instance_url_receives_the_client_secret() {
    let (base, seen) = provider().await;
    let _v = EnvVar::set("FSA_SECRET_GITLAB_PIN185");
    let git = GitConfig {
        gitlab_client_id: "gl".into(),
        gitlab_client_secret_env: "FSA_SECRET_GITLAB_PIN185".into(),
        ..GitConfig::default()
    };
    poll(git, &base, "gitlab").await.unwrap();
    assert_eq!(token_forms(&seen)[0]["client_secret"], MARKER);
}

#[tokio::test]
async fn e2e_0116_186_the_converter_url_receives_the_stored_token() {
    use axum::{Router, http::HeaderMap, routing::post};
    let seen: Arc<Mutex<Vec<Option<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let s = seen.clone();
    let app = Router::new().route(
        "/convert",
        post(move |headers: HeaderMap| {
            let s = s.clone();
            async move {
                s.lock().unwrap().push(
                    headers.get("authorization").and_then(|v| v.to_str().ok()).map(str::to_string),
                );
                "# ok"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let cfg = crate::config::DocServiceConfig {
        enabled: true,
        mode: "api".into(),
        api: crate::config::DocServiceApiConfig {
            url: format!("http://{addr}/convert"),
            auth_token: crate::config::Secret::new(MARKER),
            ..Default::default()
        },
        ..Default::default()
    };
    let svc = crate::docs::service::from_config(&cfg).unwrap().unwrap();
    svc.to_markdown(b"PK", "deck.pptx").await.unwrap();
    assert_eq!(seen.lock().unwrap().as_slice(), [Some(MARKER.to_string())]);
}

#[tokio::test]
async fn e2e_0116_187_the_ocr_endpoint_receives_the_key() {
    let (url, seen) = ocr_stand_in().await;
    let _v = EnvVar::set("FSA_SECRET_OCR_PIN187");
    let p = MultimodalOcrProvider::new(ocr_config(&url, Some("FSA_SECRET_OCR_PIN187")));
    p.extract_text(b"x", "image/png").await.unwrap();
    assert_eq!(seen.lock().unwrap().as_slice(), [Some(format!("Bearer {MARKER}"))]);
}

#[tokio::test]
async fn e2e_0116_188_the_embedding_endpoint_receives_the_key() {
    let _v = EnvVar::set("FSA_SECRET_EMBEDDING_PIN188");
    let cfg = EmbeddingConfig {
        endpoint: "http://stand-in.test/v1/embeddings".into(),
        api_key_env: "FSA_SECRET_EMBEDDING_PIN188".into(),
        ..EmbeddingConfig::default()
    };
    let target = StaticTargets(cfg).target().await.unwrap();
    assert_eq!(target.url, "http://stand-in.test/v1/embeddings");
    assert_eq!(target.bearer.as_ref().map(|b| b.expose().to_string()), Some(MARKER.to_string()));
}
