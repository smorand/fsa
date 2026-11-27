//! SPEC-0117 functional tests, US-0356: the start reads the stored `infra` section, the
//! YAML seeds it once, and it survives `fsa migrate`.

use super::ServerSettings;
use super::infra_testkit::S3Stand;
use super::server_settings_e2e::{ALICE, KEY_A, boot, boot_config, load_yaml};
use super::server_settings_store::ServerSettingRow;
use super::store::SettingsStore;
use crate::config::ServerConfig;
use crate::state::AppState;
use crate::storage::rel::{Query, RelationalDb, SqlValue, SqliteRelationalDb};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, Mutex};

const KEY_B: [u8; 32] = [9u8; 32];
const ACCESS: &str = "AKIAYAML00000001";
const SECRET: &str = "yaml-secret-0001";

fn settings(state: &Arc<AppState>) -> ServerSettings {
    ServerSettings::of(state.admin.as_ref(), state.config.clone()).unwrap()
}

async fn set(state: &Arc<AppState>, patch: Value) -> Result<Value, String> {
    settings(state).set(ALICE, "infra", patch).await.map_err(|e| e.message)
}

/// A YAML file as `yaml_for` writes it, with the given meta directory and blob block.
fn yaml(root: &Path, meta_dir: &Path, blob: &str) -> std::path::PathBuf {
    let p = root.join("config.yaml");
    let (_k, public) = crate::keys::write_keypair(root.join("keys")).unwrap();
    std::fs::write(
        &p,
        format!(
            "auth:\n  jwt:\n    public_key_path: {}\n  admins: [{ALICE}]\ninfra:\n  meta:\n    dir: {}\n  blob:\n{blob}\n  admin:\n    path: {}\n",
            public.display(),
            meta_dir.display(),
            root.join("admin.db").display(),
        ),
    )
    .unwrap();
    p
}

fn local_blob(root: &Path) -> String {
    format!("    dir: {}", root.join("blobs").display())
}

async fn raw_rows(root: &Path) -> Vec<(String, String, Option<Vec<u8>>, String)> {
    let db = SqliteRelationalDb::open(root.join("admin.db")).unwrap();
    let rows = db
        .query(&Query::new("SELECT section, value, secret_enc, updated_by FROM server_setting"))
        .await
        .unwrap();
    rows.iter()
        .map(|r| {
            let enc = match r.value(2).unwrap() {
                SqlValue::Null => None,
                _ => Some(r.blob(2).unwrap()),
            };
            (r.text(0).unwrap(), r.text(1).unwrap(), enc, r.text(3).unwrap())
        })
        .collect()
}

async fn put_raw(state: &Arc<AppState>, section: &str, value: &Value) {
    let store = state.admin.settings().unwrap();
    let exists = store.all_server_settings().await.unwrap().iter().any(|r| r.section == section);
    let mut tx = store.begin().await.unwrap();
    let row = ServerSettingRow {
        section: section.into(),
        value: value.to_string(),
        secret_enc: None,
        updated_at: "2026-11-27T00:00:00Z".into(),
        updated_by: "operator".into(),
    };
    SettingsStore::put_server_setting(&mut tx, &row, exists).await.unwrap();
    tx.commit().await.unwrap();
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
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

#[tokio::test]
async fn e2e_0117_031_a_start_with_only_the_url_and_the_key_runs_on_the_stored_places() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let (_r, first) = boot(boot_config(root), Some(KEY_A)).await.unwrap();
    first.admin.create_project("p", ALICE).await.unwrap();
    first.stores.provision_volume("p").await.unwrap();
    first.stores.client("p").await.unwrap().write_text_atomic("/x.txt", "stored").await.unwrap();
    let (v2, b2) = (root.join("v2"), root.join("b2"));
    let mut from = boot_config(root);
    from.infra.admin.path = root.join("admin.db").display().to_string();
    let mut to = from.clone();
    to.infra.admin.path = root.join("admin-to.db").display().to_string();
    to.infra.meta.dir = v2.display().to_string();
    crate::migrate::migrate(&from, &to).await.unwrap();
    copy_dir(&root.join("blobs"), &b2);
    set(&first, json!({"meta": {"dir": v2}, "blob": {"dir": b2}})).await.unwrap();
    drop(first);
    let (_r, second) = boot(boot_config(root), Some(KEY_A)).await.unwrap();
    assert_eq!(second.config.infra.meta.dir, v2.display().to_string());
    assert_eq!(second.config.infra.blob.dir, b2.display().to_string());
    let text = second.stores.client("p").await.unwrap().read_text("/x.txt").await.unwrap();
    assert_eq!(text, "stored");
}

#[tokio::test]
async fn e2e_0117_029_the_section_and_its_secrets_survive_the_migration_of_the_admin_store() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let stand = S3Stand::start(404).await;
    let (_r, first) = boot(boot_config(root), Some(KEY_A)).await.unwrap();
    set(
        &first,
        json!({"blob": {"backend": "s3", "endpoint": stand.url, "access_key": ACCESS, "secret_key": SECRET}}),
    )
    .await
    .unwrap();
    let before = settings(&first).get("infra").await.unwrap();
    drop(first);
    let mut from = boot_config(root);
    from.infra.admin.path = root.join("admin.db").display().to_string();
    let mut to = from.clone();
    to.infra.admin.path = root.join("admin-to.db").display().to_string();
    crate::migrate::migrate(&from, &to).await.unwrap();
    let mut c = boot_config(root);
    c.set_database_url(&format!("sqlite:{}", root.join("admin-to.db").display())).unwrap();
    let (_r, moved) = boot(c, Some(KEY_A)).await.unwrap();
    assert_eq!(moved.config.infra.blob.backend, "s3");
    assert_eq!(moved.config.infra.blob.secret_key, SECRET);
    let after = settings(&moved).get("infra").await.unwrap();
    assert_eq!(after["value"], before["value"]);
    assert_eq!(after["secrets"], before["secrets"]);
}

#[tokio::test]
async fn e2e_0117_048_the_yaml_seeds_infra_once_and_a_later_edit_is_ignored() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let path = yaml(root, &root.join("volumes"), &local_blob(root));
    let (_r, first) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    let row = raw_rows(root).await.into_iter().find(|r| r.0 == "infra").expect("an infra row");
    assert_eq!(row.3, "system:yaml");
    let all = super::server_settings3_boot_e2e::changes_of(&first).await;
    let seeds: Vec<&Value> =
        all.iter().filter(|c| c["target_kind"] == "setting" && c["target_id"] == "infra").collect();
    assert_eq!(seeds.len(), 1);
    assert_eq!(seeds[0]["action"], "seed_setting");
    drop(first);
    let path = yaml(root, &root.join("other"), &local_blob(root));
    let (_r, second) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(second.config.infra.meta.dir, root.join("volumes").display().to_string());
}

#[tokio::test]
async fn e2e_0117_049_the_yaml_keys_are_encrypted_at_the_seed() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let blob = format!(
        "    backend: minio\n    endpoint: http://minio.test:9000\n    access_key: {ACCESS}\n    secret_key: {SECRET}\n    dir: {}",
        root.join("blobs").display()
    );
    let path = yaml(root, &root.join("volumes"), &blob);
    let (_r, state) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    let row = raw_rows(root).await.into_iter().find(|r| r.0 == "infra").unwrap();
    assert!(!row.1.contains(ACCESS) && !row.1.contains(SECRET), "{}", row.1);
    let enc = String::from_utf8_lossy(row.2.as_ref().expect("encrypted secrets")).to_string();
    assert!(!enc.contains(ACCESS) && !enc.contains(SECRET));
    let read = settings(&state).get("infra").await.unwrap();
    assert_eq!(read["secrets"]["blob.access_key"], json!({"has_value": true, "last4": "0001"}));
    assert_eq!(read["secrets"]["blob.secret_key"], json!({"has_value": true, "last4": "0001"}));
    assert_eq!(state.config.infra.blob.access_key, ACCESS);
}

#[tokio::test]
async fn e2e_0117_050_a_stored_secret_needs_the_key_it_was_stored_with() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let stand = S3Stand::start(404).await;
    let (_r, first) = boot(boot_config(root), Some(KEY_A)).await.unwrap();
    set(
        &first,
        json!({"blob": {"backend": "s3", "endpoint": stand.url, "access_key": ACCESS, "secret_key": SECRET}}),
    )
    .await
    .unwrap();
    drop(first);
    let want = "the stored setting 'infra' holds a secret that cannot be read: set FSA_TOKEN_KEY to the key it was stored with";
    for key in [Some(KEY_B), None] {
        let e = boot(boot_config(root), key).await.err().expect("the start is refused");
        assert!(e.contains(want), "{e}");
    }
}

#[tokio::test]
async fn e2e_0117_051_a_stored_section_the_start_refuses_stops_the_start() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let (_r, first) = boot(boot_config(root), Some(KEY_A)).await.unwrap();
    let base = super::server_settings::section_json(&ServerConfig::default(), "infra");
    let mut no_dsn = base.clone();
    no_dsn["meta"]["backend"] = json!("postgres");
    put_raw(&first, "infra", &no_dsn).await;
    drop(first);
    let e = boot(boot_config(root), Some(KEY_A)).await.err().unwrap();
    assert!(
        e.contains("the stored settings are refused: infra.meta.dsn is required when infra.meta.backend is 'postgres'"),
        "{e}"
    );
    let state = {
        let d2 = tempfile::tempdir().unwrap();
        let (_r, s) = boot(boot_config(d2.path()), Some(KEY_A)).await.unwrap();
        (d2, s)
    };
    let mut with_admin = base.clone();
    with_admin["admin"] = json!({"path": "x"});
    put_raw(&state.1, "infra", &with_admin).await;
    let e = boot(boot_config(state.0.path()), Some(KEY_A)).await.err().unwrap();
    assert!(e.contains("the stored setting 'infra' is invalid:"), "{e}");
}

#[tokio::test]
async fn e2e_0117_052_a_yaml_secret_without_a_master_key_stays_in_the_file() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let logs = Arc::new(Mutex::new(Vec::new()));
    let w = logs.clone();
    let sub =
        tracing_subscriber::fmt().with_writer(move || Capture(w.clone())).with_ansi(false).finish();
    let _guard = tracing::subscriber::set_default(sub);
    let blob = format!(
        "    backend: minio\n    endpoint: http://minio.test:9000\n    access_key: {ACCESS}\n    secret_key: {SECRET}\n    dir: {}",
        root.join("blobs").display()
    );
    let path = yaml(root, &root.join("volumes"), &blob);
    let (_r, state) = boot(load_yaml(&path), None).await.unwrap();
    assert_eq!(state.config.infra.blob.secret_key, SECRET);
    assert!(raw_rows(root).await.iter().all(|r| r.0 != "infra"), "infra was imported");
    let text = String::from_utf8_lossy(&logs.lock().unwrap()).to_string();
    assert_eq!(text.matches("holds a secret and no master key is set").count(), 1, "{text}");
    assert!(!text.contains(SECRET) && !text.contains(ACCESS));
}

#[tokio::test]
async fn e2e_0117_053_the_admin_store_is_the_one_the_start_gave() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let (_r, first) = boot(boot_config(root), Some(KEY_A)).await.unwrap();
    set(&first, json!({"blob": {"region": "eu-west-1"}})).await.unwrap();
    let admin = first.config.infra.admin.path.clone();
    drop(first);
    let (_r, second) = boot(boot_config(root), Some(KEY_A)).await.unwrap();
    assert_eq!(second.config.infra.blob.region, "eu-west-1");
    assert_eq!(second.config.infra.admin.path, admin);
    // A YAML start: the YAML block is the one in force.
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let path = yaml(root, &root.join("volumes"), &local_blob(root));
    let (_r, third) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    set(&third, json!({"blob": {"region": "eu-west-1"}})).await.unwrap();
    let want = root.join("admin.db").display().to_string();
    drop(third);
    let (_r, fourth) = boot(load_yaml(&path), Some(KEY_A)).await.unwrap();
    assert_eq!(fourth.config.infra.admin.path, want);
    assert_eq!(fourth.config.infra.blob.region, "eu-west-1");
}

#[tokio::test]
async fn e2e_0117_054_a_yaml_alone_starts_as_before() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let path = yaml(root, &root.join("volumes"), &local_blob(root));
    let (_r, state) = boot(load_yaml(&path), None).await.unwrap();
    assert_eq!(state.config.infra.meta.dir, root.join("volumes").display().to_string());
    assert_eq!(state.config.infra.blob.dir, root.join("blobs").display().to_string());
    state.admin.create_project("p", ALICE).await.unwrap();
    state.stores.provision_volume("p").await.unwrap();
    let c = state.stores.client("p").await.unwrap();
    c.write_text_atomic("/y.txt", "yaml").await.unwrap();
    assert_eq!(c.read_text("/y.txt").await.unwrap(), "yaml");
    assert!(root.join("volumes").join("p.db").exists());
}
