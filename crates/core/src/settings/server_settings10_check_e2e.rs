//! SPEC-0117 functional tests, US-0355: before an `infra` change is saved the new places
//! are probed and must hold the data of the current ones; the server copies nothing.

use super::ServerSettings;
use super::e2e::Fx;
use super::infra_check::{PROBE_TIMEOUT_MS, probes, redact};
use super::infra_testkit::S3Stand;
use super::server_settings_e2e::{ALICE, KEY_A, boot, boot_config};
use crate::config::ServerConfig;
use crate::storage::RelationalRegistry;
use crate::storage::rel::{Query, RelationalDb};
use serde_json::{Value, json};
use std::path::Path;

const BOB: &str = "bob@example.com";
/// The prefix of the bucket names (a retained identifier, read from the defaults).
fn prefix() -> String {
    ServerConfig::default().infra.blob.bucket_prefix
}

const HOLD: &str = "the server never moves data itself";

fn settings(fx: &Fx) -> ServerSettings {
    ServerSettings::of(fx.state.admin.as_ref(), fx.state.config.clone()).unwrap()
}

async fn set(fx: &Fx, patch: Value) -> Result<Value, String> {
    fx.keep(settings(fx).set(ALICE, "infra", patch).await.map_err(|e| e.message))
}

async fn get(fx: &Fx) -> Value {
    fx.keep(settings(fx).get("infra").await.map_err(|e| e.message)).unwrap()
}

async fn nothing_stored(fx: &Fx) {
    assert_eq!(get(fx).await["source"], "default", "infra was stored");
    let all = fx.changes().await;
    assert!(all.iter().all(|c| c["target_kind"] != "setting"), "a refused change was recorded");
}

fn meta_dir(fx: &Fx, name: &str) -> String {
    fx.dir.path().join(name).display().to_string()
}

async fn write(fx: &Fx, project: &str, path: &str, text: &str) {
    fx.state.stores.client(project).await.unwrap().write_text_atomic(path, text).await.unwrap();
}

/// `from` the running configuration with its admin store where the fixture keeps it, and
/// `to` that configuration moved by `edit`, its own admin store: the operator's two files.
fn pair(fx: &Fx, edit: impl FnOnce(&mut ServerConfig)) -> (ServerConfig, ServerConfig) {
    let mut from = fx.state.config.as_ref().clone();
    from.infra.admin.path = fx.dir.path().join("admin.db").display().to_string();
    let mut to = from.clone();
    to.infra.admin.path = fx.dir.path().join("admin-to.db").display().to_string();
    edit(&mut to);
    (from, to)
}

async fn migrate(from: &ServerConfig, to: &ServerConfig) {
    crate::migrate::migrate(from, to).await.expect("the operator's migration");
}

async fn count(db: &dyn RelationalDb, table: &str) -> i64 {
    db.query_opt(&Query::new(format!("SELECT COUNT(*) FROM {table}")))
        .await
        .unwrap()
        .unwrap()
        .i64(0)
        .unwrap()
}

#[tokio::test]
async fn e2e_0117_015_a_pool_change_alone_probes_nothing() {
    let fx = Fx::new().await;
    let before = probes();
    let out = set(&fx, json!({"meta": {"pool": {"max_connections": 20}}})).await.unwrap();
    assert_eq!(out["value"]["meta"]["pool"]["max_connections"], 20);
    assert_eq!(out["value"]["meta"]["pool"]["acquire_timeout_secs"], 30);
    assert_eq!(out["restart_required"], true);
    assert_eq!(probes(), before, "no store was probed");
}

#[tokio::test]
async fn e2e_0117_020_a_meta_store_without_the_data_is_refused_and_names_the_command() {
    let fx = Fx::new().await;
    write(&fx, "proj", "/b.txt", "bee").await;
    write(&fx, "proj", "/c.txt", "sea").await;
    let m = set(&fx, json!({"meta": {"dir": meta_dir(&fx, "v2")}})).await.unwrap_err();
    let lead = "infra.meta points at a store that does not hold the data of the current one \
                (project 'proj': nodes ";
    assert!(m.starts_with(lead), "{m}");
    assert!(m.contains(", 0 there)"), "{m}");
    assert!(m.contains("fsa migrate --from <current config> --to <new config> first"), "{m}");
    assert!(m.ends_with(HOLD), "{m}");
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_021_after_the_migration_the_new_place_is_accepted() {
    let fx = Fx::new().await;
    write(&fx, "proj", "/b.txt", "bee").await;
    let v2 = meta_dir(&fx, "v2");
    set(&fx, json!({"meta": {"dir": v2}})).await.unwrap_err();
    let (from, to) = pair(&fx, |c| c.infra.meta.dir = v2.clone());
    migrate(&from, &to).await;
    let out = set(&fx, json!({"meta": {"dir": v2}})).await.unwrap();
    assert_eq!(out["value"]["meta"]["dir"], v2);
    assert_eq!(out["restart_required"], true);
}

#[tokio::test]
async fn e2e_0117_022_a_project_that_grew_after_the_copy_is_refused_by_name() {
    let fx = Fx::new().await;
    fx.state.admin.create_project("q", BOB).await.unwrap();
    fx.state.stores.provision_volume("q").await.unwrap();
    write(&fx, "q", "/q.txt", "queue").await;
    let v2 = meta_dir(&fx, "v2");
    let (from, to) = pair(&fx, |c| c.infra.meta.dir = v2.clone());
    migrate(&from, &to).await;
    write(&fx, "proj", "/late.txt", "late").await;
    write(&fx, "q", "/late.txt", "later").await;
    let m = set(&fx, json!({"meta": {"dir": v2}})).await.unwrap_err();
    assert!(m.starts_with("infra.meta points at a store that does not hold the data of the current one (project 'proj': nodes "), "{m}");
    migrate(&from, &to).await;
    set(&fx, json!({"meta": {"dir": v2}})).await.unwrap();
}

#[tokio::test]
async fn e2e_0117_023_a_source_with_no_data_moves_without_a_migration() {
    let d = tempfile::tempdir().unwrap();
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let other = tempfile::tempdir().unwrap();
    let s = ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap();
    let dir = other.path().join("volumes").display().to_string();
    let out = s.set(ALICE, "infra", json!({"meta": {"dir": dir}})).await.unwrap();
    assert_eq!(out["restart_required"], true);
    assert_eq!(out["value"]["meta"]["dir"], dir);
}

#[tokio::test]
async fn e2e_0117_024_an_oauth_store_without_its_tokens_is_refused() {
    let fx = Fx::new().await;
    let other = tempfile::tempdir().unwrap();
    let v2 = other.path().join("volumes").display().to_string();
    let running = fx.state.config.as_ref().clone();
    let reg = RelationalRegistry::new();
    let db = crate::storage::open_oauth_db(&running, &reg).await.unwrap();
    db.migrate(&crate::git::oauth::persistence::schema()).await.unwrap();
    db.execute(
        &Query::new("INSERT INTO oauth_tokens (person, host, provider, token_enc, scopes) VALUES (?1,?2,?3,?4,?5)")
            .bind("bob@example.com")
            .bind("gitlab.example.com")
            .bind("gitlab")
            .bind(vec![1u8, 2, 3])
            .bind("api"),
    )
    .await
    .unwrap();
    let (from, to) = pair(&fx, |c| c.infra.meta.dir = v2.clone());
    migrate(&from, &to).await;
    let target = crate::storage::open_oauth_db(&to, &reg).await.unwrap();
    target.execute(&Query::new("DELETE FROM oauth_tokens")).await.unwrap();
    assert_eq!(count(&*target, "oauth_tokens").await, 0);
    let m = set(&fx, json!({"meta": {"dir": v2}})).await.unwrap_err();
    assert!(
        m.starts_with("infra.oauth points at a store that does not hold the data of the current one (oauth_tokens 1 here, 0 there)"),
        "{m}"
    );
    migrate(&from, &to).await;
    set(&fx, json!({"meta": {"dir": v2}})).await.unwrap();
}

#[tokio::test]
async fn e2e_0117_025_a_git_store_without_its_rows_is_refused() {
    let fx = Fx::new().await;
    let other = tempfile::tempdir().unwrap();
    let v2 = other.path().join("volumes").display().to_string();
    let running = fx.state.config.as_ref().clone();
    let reg = RelationalRegistry::new();
    let db = crate::storage::open_git_db(&running, &reg, "proj").await.unwrap();
    db.migrate(&crate::git::db::schema()).await.unwrap();
    db.execute(
        &Query::new(
            "INSERT INTO git_refs (volume_id, name, target, symbolic) VALUES (?1,?2,?3,?4)",
        )
        .bind("proj")
        .bind("refs/heads/main")
        .bind("abc")
        .bind(0i64),
    )
    .await
    .unwrap();
    let (from, to) = pair(&fx, |c| c.infra.meta.dir = v2.clone());
    migrate(&from, &to).await;
    let target = crate::storage::open_git_db(&to, &reg, "proj").await.unwrap();
    target.execute(&Query::new("DELETE FROM git_refs")).await.unwrap();
    let m = set(&fx, json!({"meta": {"dir": v2}})).await.unwrap_err();
    assert!(
        m.starts_with("infra.git points at a store that does not hold the data of the current one (project 'proj': git_refs 1 here, 0 there)"),
        "{m}"
    );
    migrate(&from, &to).await;
    set(&fx, json!({"meta": {"dir": v2}})).await.unwrap();
}

#[tokio::test]
async fn e2e_0117_026_a_place_that_cannot_be_created_is_unreachable() {
    let fx = Fx::new().await;
    let file = fx.dir.path().join("afile");
    std::fs::write(&file, "x").unwrap();
    let m =
        set(&fx, json!({"meta": {"dir": file.join("x").display().to_string()}})).await.unwrap_err();
    assert!(m.starts_with("infra.meta cannot be reached:"), "{m}");
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_028_a_change_back_to_the_running_place_needs_no_check() {
    let fx = Fx::new().await;
    let v2 = meta_dir(&fx, "v2");
    let (from, to) = pair(&fx, |c| c.infra.meta.dir = v2.clone());
    migrate(&from, &to).await;
    set(&fx, json!({"meta": {"dir": v2}})).await.unwrap();
    write(&fx, "proj", "/after.txt", "after").await;
    let running = fx.state.config.infra.meta.dir.clone();
    let before = probes();
    let out = set(&fx, json!({"meta": {"dir": running}})).await.unwrap();
    assert_eq!(out["restart_required"], false);
    assert_eq!(probes(), before, "the running place is not probed");
}

#[tokio::test]
async fn e2e_0117_030_a_refusal_leaves_only_an_empty_schema_behind() {
    let fx = Fx::new().await;
    write(&fx, "proj", "/b.txt", "bee").await;
    let v2 = meta_dir(&fx, "v2");
    set(&fx, json!({"meta": {"dir": v2}})).await.unwrap_err();
    let names: Vec<String> = std::fs::read_dir(&v2)
        .map(|d| d.map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    assert!(names.iter().all(|n| n.starts_with("proj.db")), "{names:?}");
    let reg = RelationalRegistry::new();
    let mut c = fx.state.config.as_ref().clone();
    c.infra.meta.dir = v2;
    let db = crate::storage::open_meta_db(&c, &reg, "proj").await.unwrap();
    assert_eq!(count(&*db, "nodes").await, 0);
    assert_eq!(count(&*db, "blob_refs").await, 0);
}

#[tokio::test]
async fn e2e_0117_040_a_switch_to_s3_without_the_blobs_is_refused() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(404).await;
    let m = set(&fx, s3(&stand)).await.unwrap_err();
    assert_eq!(
        m,
        format!(
            "infra.blob points at a store that does not hold 1 of 1 blobs of the current one: copy them first (fsa migrate --from <current config> --to <new config> lists the missing ones), {HOLD}"
        )
    );
    nothing_stored(&fx).await;
}

fn s3(stand: &S3Stand) -> Value {
    json!({"blob": {"backend": "s3", "endpoint": stand.url, "access_key": "AKIAEXAMPLE0001", "secret_key": "s3cr3t-value-abcdef"}})
}

#[tokio::test]
async fn e2e_0117_041_a_switch_to_s3_with_the_blobs_there_is_accepted() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(200).await;
    let out = set(&fx, s3(&stand)).await.unwrap();
    assert_eq!(out["restart_required"], true);
    assert_eq!(out["value"]["blob"]["backend"], "s3");
}

#[tokio::test]
async fn e2e_0117_042_a_switch_to_s3_with_no_blob_is_accepted_after_one_probe() {
    let d = tempfile::tempdir().unwrap();
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let stand = S3Stand::start(404).await;
    let s = ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap();
    s.set(ALICE, "infra", s3(&stand)).await.unwrap();
    assert_eq!(stand.hits(), vec![format!("/{}probe/fsa-probe", prefix())]);
}

#[tokio::test]
async fn e2e_0117_043_keys_the_endpoint_refuses_are_refused_without_echo() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(403).await;
    let m = set(&fx, s3(&stand)).await.unwrap_err();
    assert!(m.starts_with("infra.blob cannot be used:"), "{m}");
    assert!(!m.contains("AKIAEXAMPLE0001") && !m.contains("s3cr3t-value-abcdef"), "{m}");
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_044_an_endpoint_nobody_listens_on_is_refused() {
    let fx = Fx::new().await;
    let mut patch = s3(&S3Stand::start(200).await);
    patch["blob"]["endpoint"] = json!("http://127.0.0.1:1");
    let m = set(&fx, patch).await.unwrap_err();
    assert!(m.starts_with("infra.blob cannot be used:"), "{m}");
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_045_a_blob_directory_that_cannot_be_created_is_refused() {
    let fx = Fx::new().await;
    let file = fx.dir.path().join("afile");
    std::fs::write(&file, "x").unwrap();
    let m =
        set(&fx, json!({"blob": {"dir": file.join("x").display().to_string()}})).await.unwrap_err();
    assert!(m.starts_with("infra.blob cannot be used:"), "{m}");
    nothing_stored(&fx).await;
}

#[tokio::test]
async fn e2e_0117_046_a_key_change_on_the_running_endpoint_is_probed_and_moves_nothing() {
    let d = tempfile::tempdir().unwrap();
    let stand = S3Stand::start(404).await;
    {
        let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
        let s = ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap();
        s.set(ALICE, "infra", s3(&stand)).await.unwrap();
    }
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(state.config.infra.blob.backend, "s3");
    let s = ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap();
    stand.set_status(403);
    let m = s
        .set(ALICE, "infra", json!({"blob": {"secret_key": "rotated-secret-9999"}}))
        .await
        .unwrap_err()
        .message;
    assert!(m.starts_with("infra.blob cannot be used:"), "{m}");
    stand.set_status(404);
    let before = stand.hits().len();
    s.set(ALICE, "infra", json!({"blob": {"secret_key": "rotated-secret-9999"}})).await.unwrap();
    let new: Vec<String> = stand.hits()[before..].to_vec();
    assert_eq!(new, vec![format!("/{}probe/fsa-probe", prefix())], "only the probe, no address");
}

#[tokio::test]
async fn e2e_0117_047_the_count_is_per_project_bucket() {
    let fx = Fx::new().await;
    fx.state.admin.create_project("q2", BOB).await.unwrap();
    fx.state.stores.provision_volume("q2").await.unwrap();
    write(&fx, "q2", "/other.txt", "other content").await;
    let stand = S3Stand::start(404).await;
    stand.holds_bucket(&format!("{}proj", prefix()));
    let m = set(&fx, s3(&stand)).await.unwrap_err();
    assert!(m.starts_with("infra.blob points at a store that does not hold 1 of 2 blobs"), "{m}");
}

#[tokio::test]
async fn e2e_0117_057_a_probe_that_never_answers_times_out() {
    let fx = Fx::new().await;
    let stand = S3Stand::start(404).await;
    stand.hang();
    PROBE_TIMEOUT_MS.with(|t| t.set(300));
    let t0 = std::time::Instant::now();
    let m = set(&fx, s3(&stand)).await.unwrap_err();
    PROBE_TIMEOUT_MS.with(|t| t.set(15000));
    assert_eq!(m, "infra.blob cannot be used: timed out");
    assert!(t0.elapsed() < std::time::Duration::from_secs(5));
}

#[tokio::test]
async fn e2e_0117_058_a_driver_error_loses_its_dsn_and_keys() {
    let dsn = "postgres://u:hunter2pw@h/db";
    let text = format!("connect {dsn} failed for key KEY123 password hunter2pw");
    let out = redact(&text, &[dsn.into(), "KEY123".into()]);
    assert_eq!(out, "connect <redacted> failed for key <redacted> password <redacted>");
    // A percent encoded password and a query password are hidden in both forms.
    let enc = "postgres://u:p%40ss%2Fx@h/db?password=q1";
    let out = redact("failed p@ss/x and p%40ss%2Fx and q1", &[enc.into()]);
    assert_eq!(out, "failed <redacted> and <redacted> and <redacted>");
    let _ = Path::new("/");
}
