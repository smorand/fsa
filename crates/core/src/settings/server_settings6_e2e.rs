//! SPEC-0116 functional tests, US-0350: the server verifies a token with every key
//! of the stored list and the key file, with the same skew, header order and error
//! words as before.

use super::server_settings_e2e::{ALICE, KEY_A, boot, boot_config, set_on};
use super::server_settings4_e2e::BOB;
use super::server_settings5_e2e::kp;
use crate::state::AppState;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

pub(super) struct Capture(pub Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Everything the process logs on this thread until the guard drops.
pub(super) fn capture_logs() -> (Arc<Mutex<Vec<u8>>>, tracing::subscriber::DefaultGuard) {
    let logs = Arc::new(Mutex::new(Vec::new()));
    let writer = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || Capture(writer.clone()))
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .finish();
    (logs, tracing::subscriber::set_default(subscriber))
}

pub(super) fn logged(logs: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&logs.lock().unwrap()).to_string()
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// A token signed with `private_pem` for `email`; `exp` and `nbf` are seconds from now.
fn token_with(private_pem: &str, email: &str, exp: i64, nbf: Option<i64>, claim: &str) -> String {
    let mut claims = json!({"iss": "web-a2a", claim: email, "exp": now() + exp});
    if let Some(n) = nbf {
        claims["nbf"] = json!(now() + n);
    }
    let key = EncodingKey::from_rsa_pem(private_pem.as_bytes()).unwrap();
    encode(&Header::new(Algorithm::RS256), &claims, &key).unwrap()
}

fn token(private_pem: &str, email: &str) -> String {
    token_with(private_pem, email, 3600, None, "email")
}

/// The token the key file of the platform directory signs (the key file of `boot_config`).
fn file_token(root: &Path, email: &str, ttl: i64) -> String {
    crate::keys::mint_token_from_file(
        root.join("keys").join(crate::keys::PRIVATE_KEY_FILE),
        email,
        crate::keys::DEFAULT_ISSUER,
        crate::keys::DEFAULT_CLAIM,
        ttl,
    )
    .unwrap()
}

fn file_private(root: &Path) -> String {
    std::fs::read_to_string(root.join("keys").join(crate::keys::PRIVATE_KEY_FILE)).unwrap()
}

/// A request to the REST settings list with the given headers: the status and the body.
async fn list(router: &axum::Router, headers: &[(&str, String)]) -> (u16, String) {
    let mut req = axum::http::Request::builder().method("GET").uri("/api/admin/settings");
    for (k, v) in headers {
        req = req.header(*k, v);
    }
    let res = router.clone().oneshot(req.body(axum::body::Body::empty()).unwrap()).await.unwrap();
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

async fn status_of(router: &axum::Router, bearer: &str) -> u16 {
    list(router, &[("Authorization", format!("Bearer {bearer}"))]).await.0
}

/// A platform in `root` whose stored `auth` holds the keys of `pems` (the key file of
/// `boot_config` stays, as in every fixture), started, stopped, and started again.
async fn restarted_with(root: &Path, patch: Value) -> (axum::Router, Arc<AppState>) {
    let (_r, first) = boot(boot_config(root), Some(KEY_A)).await.unwrap();
    set_on(&first, "auth", patch).await;
    drop(first);
    boot(boot_config(root), Some(KEY_A)).await.unwrap()
}

fn keys_patch(list: &[&str]) -> Value {
    json!({"jwt": {"public_keys": list}})
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_018_two_stored_keys_are_accepted_and_a_third_is_not() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (router, _s) =
        restarted_with(d.path(), keys_patch(&[&k[0].public_spki, &k[1].public_spki])).await;
    assert_eq!(status_of(&router, &token(&k[0].private_pkcs8, ALICE)).await, 200);
    assert_eq!(status_of(&router, &token(&k[1].private_pkcs8, ALICE)).await, 200);
    assert_eq!(status_of(&router, &token(&k[2].private_pkcs8, ALICE)).await, 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_019_a_key_taken_from_the_list_is_refused_after_the_restart() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (router, state) =
        restarted_with(d.path(), keys_patch(&[&k[0].public_spki, &k[1].public_spki])).await;
    set_on(&state, "auth", keys_patch(&[&k[1].public_spki])).await;
    assert_eq!(status_of(&router, &token(&k[0].private_pkcs8, ALICE)).await, 200, "not live");
    drop((router, state));
    let (router, _s) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(status_of(&router, &token(&k[0].private_pkcs8, ALICE)).await, 401);
    assert_eq!(status_of(&router, &token(&k[1].private_pkcs8, ALICE)).await, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_020_the_key_file_and_the_stored_list_are_both_accepted() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (router, _s) = restarted_with(d.path(), keys_patch(&[&k[1].public_spki])).await;
    assert_eq!(status_of(&router, &file_token(d.path(), ALICE, 3600)).await, 200);
    assert_eq!(status_of(&router, &token(&k[1].private_pkcs8, ALICE)).await, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_118_a_third_key_is_refused_next_to_a_file_and_a_list() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (router, _s) = restarted_with(d.path(), keys_patch(&[&k[1].public_spki])).await;
    assert_eq!(status_of(&router, &token(&k[2].private_pkcs8, ALICE)).await, 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_143_a_list_extended_after_a_restart_accepts_the_new_key() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (_r, state) = restarted_with(d.path(), keys_patch(&[&k[1].public_spki])).await;
    set_on(&state, "auth", keys_patch(&[&k[1].public_spki, &k[3].public_spki])).await;
    drop((_r, state));
    let (router, _s) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    for bearer in [
        file_token(d.path(), ALICE, 3600),
        token(&k[1].private_pkcs8, ALICE),
        token(&k[3].private_pkcs8, ALICE),
    ] {
        assert_eq!(status_of(&router, &bearer).await, 200);
    }
    assert_eq!(status_of(&router, &token(&k[2].private_pkcs8, ALICE)).await, 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_021_a_token_expired_for_20_seconds_is_accepted_for_40_it_is_not() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (_r, state) = restarted_with(d.path(), keys_patch(&[&k[0].public_spki])).await;
    let ok = token_with(&k[0].private_pkcs8, ALICE, -20, None, "email");
    assert_eq!(state.identity.verify(&ok).unwrap(), ALICE);
    let late = token_with(&k[0].private_pkcs8, ALICE, -40, None, "email");
    assert_eq!(
        state.identity.verify(&late).unwrap_err().message,
        "invalid token: ExpiredSignature"
    );
    let e = state.identity.verify(&late).unwrap_err();
    assert_eq!(e.code, crate::errors::code::UNAUTHENTICATED);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_022_a_token_valid_from_20_seconds_ahead_is_accepted_from_40_it_is_not() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (_r, state) = restarted_with(d.path(), keys_patch(&[&k[0].public_spki])).await;
    let ok = token_with(&k[0].private_pkcs8, ALICE, 3600, Some(20), "email");
    assert_eq!(state.identity.verify(&ok).unwrap(), ALICE);
    let early = token_with(&k[0].private_pkcs8, ALICE, 3600, Some(40), "email");
    let e = state.identity.verify(&early).unwrap_err();
    assert_eq!(e.message, "invalid token: ImmatureSignature");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_023_the_skew_holds_for_a_key_file_with_no_stored_key() {
    let d = tempfile::tempdir().unwrap();
    let (_r, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let private = file_private(d.path());
    let ok = token_with(&private, ALICE, -20, None, "email");
    assert_eq!(state.identity.verify(&ok).unwrap(), ALICE);
    let late = token_with(&private, ALICE, -40, None, "email");
    assert_eq!(
        state.identity.verify(&late).unwrap_err().message,
        "invalid token: ExpiredSignature"
    );
    let early = token_with(&private, ALICE, 3600, Some(40), "email");
    assert_eq!(
        state.identity.verify(&early).unwrap_err().message,
        "invalid token: ImmatureSignature"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_170_a_token_expired_for_10_seconds_is_accepted() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (_r, state) = restarted_with(d.path(), keys_patch(&[&k[0].public_spki])).await;
    let t = token_with(&k[0].private_pkcs8, ALICE, -10, None, "email");
    assert_eq!(state.identity.verify(&t).unwrap(), ALICE);
}

// ---- the header: the configured one first, then Authorization --------------

fn bearer(t: &str) -> String {
    format!("Bearer {t}")
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_024_the_configured_header_wins_over_authorization() {
    let d = tempfile::tempdir().unwrap();
    let (router, _s) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let admin = bearer(&file_token(d.path(), ALICE, 3600));
    let bob = bearer(&file_token(d.path(), BOB, 3600));
    let h = "X-Forwarded-Authorization";
    assert_eq!(list(&router, &[(h, admin.clone()), ("Authorization", bob.clone())]).await.0, 200);
    assert_eq!(list(&router, &[(h, bob.clone()), ("Authorization", admin.clone())]).await.0, 403);
    assert_eq!(list(&router, &[("Authorization", admin)]).await.0, 200);
    assert_eq!(list(&router, &[]).await.0, 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_026_a_start_from_a_yaml_file_alone_has_the_same_precedence() {
    let d = tempfile::tempdir().unwrap();
    let path = super::server_settings_e2e::yaml_for(d.path(), "");
    let config = super::server_settings_e2e::load_yaml(&path);
    let (router, _s) = boot(config, Some(KEY_A)).await.unwrap();
    let admin = bearer(&file_token(d.path(), ALICE, 3600));
    let bob = bearer(&file_token(d.path(), BOB, 3600));
    let h = "X-Forwarded-Authorization";
    assert_eq!(list(&router, &[(h, admin.clone()), ("Authorization", bob.clone())]).await.0, 200);
    assert_eq!(list(&router, &[(h, bob), ("Authorization", admin.clone())]).await.0, 403);
    assert_eq!(list(&router, &[("Authorization", admin)]).await.0, 200);
    assert_eq!(list(&router, &[]).await.0, 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_025_a_stored_header_name_is_read_first_and_authorization_after_it() {
    let d = tempfile::tempdir().unwrap();
    let (router, _s) = restarted_with(d.path(), json!({"jwt": {"header": "X-Fsa-Token"}})).await;
    let admin = bearer(&file_token(d.path(), ALICE, 3600));
    let bob = bearer(&file_token(d.path(), BOB, 3600));
    assert_eq!(
        list(&router, &[("X-Fsa-Token", admin.clone()), ("Authorization", bob)]).await.0,
        200
    );
    assert_eq!(list(&router, &[("X-Forwarded-Authorization", admin.clone())]).await.0, 401);
    assert_eq!(list(&router, &[("Authorization", admin)]).await.0, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_117_a_token_of_an_unknown_key_in_the_stored_header_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let (router, _s) = restarted_with(d.path(), json!({"jwt": {"header": "X-Fsa-Token"}})).await;
    let k3 = token(&kp()[2].private_pkcs8, ALICE);
    assert_eq!(list(&router, &[("X-Fsa-Token", bearer(&k3))]).await.0, 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_133_a_stored_header_name_is_not_live() {
    let d = tempfile::tempdir().unwrap();
    let (router, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(&state, "auth", json!({"jwt": {"header": "X-Fsa-Token"}})).await;
    let admin = bearer(&file_token(d.path(), ALICE, 3600));
    assert_eq!(list(&router, &[("X-Fsa-Token", admin.clone())]).await.0, 401);
    drop((router, state));
    let (router, _s) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    // `boot_config` writes the key file again: sign with the new one.
    let admin = bearer(&file_token(d.path(), ALICE, 3600));
    assert_eq!(list(&router, &[("X-Fsa-Token", admin)]).await.0, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_063_a_bad_token_in_the_configured_header_is_not_rescued_by_authorization() {
    let d = tempfile::tempdir().unwrap();
    let (router, _s) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let good = bearer(&file_token(d.path(), ALICE, 3600));
    let third = bearer(&token(&kp()[2].private_pkcs8, ALICE));
    let h = "X-Forwarded-Authorization";
    assert_eq!(list(&router, &[(h, third), ("Authorization", good.clone())]).await.0, 401);
    assert_eq!(list(&router, &[(h, "Token abc".into()), ("Authorization", good)]).await.0, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_085_only_bearer_and_bearer_in_lower_case_count_as_a_scheme() {
    let d = tempfile::tempdir().unwrap();
    let (router, _s) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    let admin = file_token(d.path(), ALICE, 3600);
    let bob = bearer(&file_token(d.path(), BOB, 3600));
    let h = "X-Forwarded-Authorization";
    let upper = format!("BEARER {admin}");
    assert_eq!(list(&router, &[(h, upper), ("Authorization", bob.clone())]).await.0, 403);
    let lower = format!("bearer {admin}");
    assert_eq!(list(&router, &[(h, lower), ("Authorization", bob)]).await.0, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_171_an_unusable_first_header_leaves_authorization_in_charge() {
    let d = tempfile::tempdir().unwrap();
    let (router, _s) = restarted_with(d.path(), json!({"jwt": {"header": "X-Fsa-Token"}})).await;
    let admin = bearer(&file_token(d.path(), ALICE, 3600));
    assert_eq!(
        list(&router, &[("X-Fsa-Token", "Token abc".into()), ("Authorization", admin)]).await.0,
        200
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_197_a_lower_case_bearer_with_trailing_spaces_is_read() {
    let d = tempfile::tempdir().unwrap();
    let (router, _s) = restarted_with(d.path(), json!({"jwt": {"header": "X-Fsa-Token"}})).await;
    let admin = format!("bearer {}  ", file_token(d.path(), ALICE, 3600));
    let bob = bearer(&file_token(d.path(), BOB, 3600));
    assert_eq!(list(&router, &[("X-Fsa-Token", admin), ("Authorization", bob)]).await.0, 200);
}

// ---- the error selected among several keys ----------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_147_a_token_of_the_second_key_is_accepted() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (_r, state) =
        restarted_with(d.path(), keys_patch(&[&k[0].public_spki, &k[1].public_spki])).await;
    assert_eq!(state.identity.verify(&token(&k[1].private_pkcs8, ALICE)).unwrap(), ALICE);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_148_an_expired_token_of_the_second_key_names_the_expiry() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (_r, state) =
        restarted_with(d.path(), keys_patch(&[&k[0].public_spki, &k[1].public_spki])).await;
    let t = token_with(&k[1].private_pkcs8, ALICE, -40, None, "email");
    assert_eq!(state.identity.verify(&t).unwrap_err().message, "invalid token: ExpiredSignature");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_149_a_token_of_no_key_names_the_signature() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (_r, state) =
        restarted_with(d.path(), keys_patch(&[&k[0].public_spki, &k[1].public_spki])).await;
    let e = state.identity.verify(&token(&k[2].private_pkcs8, ALICE)).unwrap_err();
    assert_eq!(e.message, "invalid token: InvalidSignature");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_150_an_expired_token_of_the_key_file_names_the_expiry() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (_r, state) = restarted_with(d.path(), keys_patch(&[&k[1].public_spki])).await;
    let t = token_with(&file_private(d.path()), ALICE, -40, None, "email");
    assert_eq!(state.identity.verify(&t).unwrap_err().message, "invalid token: ExpiredSignature");
}

// ---- a key file that cannot be read ------------------------------------------

fn ignored(path: &str) -> String {
    format!("auth.jwt.public_key_path '{path}' cannot be read as an RSA public key: it is ignored")
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_087_a_missing_key_file_next_to_a_list_is_ignored_with_one_warning() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (logs, _g) = capture_logs();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(
        &first,
        "auth",
        json!({"jwt": {"public_keys": [&k[1].public_spki], "public_key_path": "/nonexistent/jwt.pub"}}),
    )
    .await;
    drop((_r, first));
    logs.lock().unwrap().clear();
    let (_router, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(state.identity.verify(&token(&k[1].private_pkcs8, ALICE)).unwrap(), ALICE);
    assert!(state.identity.verify(&file_token(d.path(), ALICE, 3600)).is_err());
    assert_eq!(logged(&logs).matches(&ignored("/nonexistent/jwt.pub")).count(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_088_a_key_file_that_is_not_a_key_is_ignored_with_one_warning() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let bad = d.path().join("not-a-key.pub");
    std::fs::write(&bad, "not a key").unwrap();
    let bad = bad.display().to_string();
    let (logs, _g) = capture_logs();
    let (_r, first) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    set_on(
        &first,
        "auth",
        json!({"jwt": {"public_keys": [&k[1].public_spki], "public_key_path": &bad}}),
    )
    .await;
    drop((_r, first));
    logs.lock().unwrap().clear();
    let (_router, state) = boot(boot_config(d.path()), Some(KEY_A)).await.unwrap();
    assert_eq!(state.identity.verify(&token(&k[1].private_pkcs8, ALICE)).unwrap(), ALICE);
    assert_eq!(logged(&logs).matches(&ignored(&bad)).count(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_089_a_readable_key_file_next_to_a_list_gives_no_warning() {
    let d = tempfile::tempdir().unwrap();
    let k = kp();
    let (logs, _g) = capture_logs();
    let (_r, state) = restarted_with(d.path(), keys_patch(&[&k[1].public_spki])).await;
    assert_eq!(state.identity.verify(&file_token(d.path(), ALICE, 3600)).unwrap(), ALICE);
    assert_eq!(state.identity.verify(&token(&k[1].private_pkcs8, ALICE)).unwrap(), ALICE);
    assert!(!logged(&logs).contains("cannot be read as an RSA public key"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_134_a_missing_key_file_with_no_list_logs_nothing_and_rejects_every_token() {
    let d = tempfile::tempdir().unwrap();
    let (logs, _g) = capture_logs();
    let mut config = boot_config(d.path());
    config.auth.jwt.public_key_path = "/nonexistent/jwt.pub".into();
    let (_router, state) = boot(config, Some(KEY_A)).await.unwrap();
    let e = state.identity.verify(&file_token(d.path(), ALICE, 3600)).unwrap_err();
    assert_eq!(e.message, "no JWT public key configured");
    assert!(!logged(&logs).contains("/nonexistent/jwt.pub"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_135_a_key_file_that_is_not_a_key_with_no_list_rejects_every_token_silently() {
    let d = tempfile::tempdir().unwrap();
    let bad = d.path().join("not-a-key.pub");
    std::fs::write(&bad, "not a key").unwrap();
    let (logs, _g) = capture_logs();
    let mut config = boot_config(d.path());
    config.auth.jwt.public_key_path = bad.display().to_string();
    let (_router, state) = boot(config, Some(KEY_A)).await.unwrap();
    let e = state.identity.verify(&file_token(d.path(), ALICE, 3600)).unwrap_err();
    assert_eq!(e.message, "no JWT public key configured");
    assert!(!logged(&logs).contains("not-a-key.pub"));
}

// ---- the accepted power of a stored key (D-0116-4) --------------------------

#[tokio::test(flavor = "multi_thread")]
async fn e2e_0116_157_an_administrator_who_stores_a_key_can_be_accepted_as_a_member() {
    let d = tempfile::tempdir().unwrap();
    let k9 = &kp()[2];
    let (router, state) = restarted_with(d.path(), keys_patch(&[&k9.public_spki])).await;
    state.admin.create_project("proj", BOB).await.unwrap();
    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/api/fs/roots")
        .header("Authorization", bearer(&token(&k9.private_pkcs8, BOB)))
        .body(axum::body::Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status().as_u16(), 200);
    let text =
        String::from_utf8_lossy(&axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap())
            .to_string();
    assert!(text.contains("proj"), "{text}");
    let changes = super::server_settings3_boot_e2e::changes_of(&state).await;
    let l: Vec<&Value> = changes.iter().filter(|c| c["target_id"] == "auth").collect();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["person"], ALICE);
    assert_eq!(l[0]["fields"], json!(["jwt.public_keys"]));
    assert!(!l[0].to_string().contains("BEGIN PUBLIC KEY"));
}
