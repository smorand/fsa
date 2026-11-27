//! SPEC-0116 functional tests, US-0349: the rules of a change to the `auth`
//! section (keys, guards, types, order) and the access to both new sections.

use super::e2e::{Fx, Via, token_of};
use super::key_fixtures::{CERT, EC_P256, RSA_1024, RSA_8192, RSA_8448};
use super::server_settings4_e2e::{ALICE, BOB, get, lines, nothing_stored, set};
use crate::mcp::server::{SectionArgs, SetSettingArgs};
use crate::search::derived_e2e::{args, tool_json};
use serde_json::{Value, json};
use std::sync::OnceLock;

/// One RSA key pair in the forms the tests need.
pub(super) struct Kp {
    pub private_pkcs8: String,
    pub private_pkcs1: String,
    pub public_spki: String,
    pub public_pkcs1: String,
}

/// Four fresh 2048 bit key pairs, made once per test process (K1 to K4).
pub(super) fn kp() -> &'static [Kp; 4] {
    use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
    static KEYS: OnceLock<[Kp; 4]> = OnceLock::new();
    KEYS.get_or_init(|| {
        std::array::from_fn(|_| {
            let private = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 2048).unwrap();
            let public = private.to_public_key();
            Kp {
                private_pkcs8: private.to_pkcs8_pem(LineEnding::LF).unwrap().to_string(),
                private_pkcs1: private.to_pkcs1_pem(LineEnding::LF).unwrap().to_string(),
                public_spki: public.to_public_key_pem(LineEnding::LF).unwrap(),
                public_pkcs1: public.to_pkcs1_pem(LineEnding::LF).unwrap(),
            }
        })
    })
}

fn form(i: usize) -> String {
    format!(
        "auth.jwt.public_keys[{i}] is not a public key in PEM form (an RSA public key is expected)"
    )
}
fn private(i: usize) -> String {
    format!("auth.jwt.public_keys[{i}] holds a private key: only a public key is accepted")
}
fn short(i: usize) -> String {
    format!("auth.jwt.public_keys[{i}] holds an RSA key shorter than 2048 bits")
}
fn long(i: usize) -> String {
    format!("auth.jwt.public_keys[{i}] holds an RSA key longer than 8192 bits")
}
const MORE: &str = "auth.jwt.public_keys holds more than 8 keys";
const NOKEY: &str = "auth.jwt needs at least one public key: give public_keys or public_key_path";
const NOADMIN: &str = "auth.admins needs at least one platform administrator";
const NOCLAIM: &str = "auth.jwt.username_claim must not be empty";
const EMPTY_ADMIN: &str = "invalid setting 'auth': admins must not hold an empty name";
const EMPTY_HEADER: &str = "invalid setting 'auth': jwt.header must not be empty";
const EMPTY_ALGS: &str = "invalid setting 'auth': jwt.algorithms must not be empty";

fn wrong(section: &str, key: &str) -> String {
    format!("invalid setting '{section}': the value of '{key}' has the wrong type")
}

fn keys(list: &[&str]) -> Value {
    json!({"jwt": {"public_keys": list}})
}

/// An operator editing the stored value of `section` by hand: no check.
pub(super) async fn put_raw(fx: &Fx, section: &str, value: Value) {
    let store = fx.state.admin.settings().unwrap();
    let exists = store.all_server_settings().await.unwrap().iter().any(|r| r.section == section);
    let mut tx = store.begin().await.unwrap();
    let row = super::server_settings_store::ServerSettingRow {
        section: section.into(),
        value: value.to_string(),
        secret_enc: None,
        updated_at: "2026-11-27T00:00:00Z".into(),
        updated_by: "operator".into(),
    };
    super::store::SettingsStore::put_server_setting(&mut tx, &row, exists).await.unwrap();
    tx.commit().await.unwrap();
}

/// The stored `auth` of the fixture with `edit` applied, as an operator wrote it.
async fn put_auth(fx: &Fx, edit: impl FnOnce(&mut Value)) {
    let mut v = get(fx, "auth").await["value"].clone();
    edit(&mut v);
    put_raw(fx, "auth", v).await;
}

async fn refused(fx: &Fx, section: &str, patch: Value) -> String {
    set(fx, section, patch).await.unwrap_err()
}

#[tokio::test]
async fn e2e_0116_011_a_key_that_is_not_a_public_key_is_refused() {
    let fx = Fx::new().await;
    for e in ["not a key", "", CERT] {
        assert_eq!(refused(&fx, "auth", keys(&[e])).await, form(0));
    }
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_012_a_private_key_is_refused_and_never_echoed() {
    let fx = Fx::new().await;
    let k = &kp()[0];
    for p in [&k.private_pkcs1, &k.private_pkcs8] {
        assert_eq!(refused(&fx, "auth", keys(&[p])).await, private(0));
    }
    assert_eq!(refused(&fx, "auth", keys(&[&k.public_spki, &k.private_pkcs8])).await, private(1));
    let both = format!("{}{}", k.public_spki, k.private_pkcs8);
    assert_eq!(refused(&fx, "auth", keys(&[&both])).await, private(0));
    nothing_stored(&fx, "auth").await;
    for line in k.private_pkcs8.lines().filter(|l| l.len() > 20 && !l.starts_with("-----")) {
        fx.assert_nowhere(line).await;
    }
}

#[tokio::test]
async fn e2e_0116_013_nine_keys_are_refused_and_eight_are_accepted() {
    let fx = Fx::new().await;
    let k = kp()[0].public_spki.as_str();
    assert_eq!(refused(&fx, "auth", keys(&[k; 9])).await, MORE);
    nothing_stored(&fx, "auth").await;
    set(&fx, "auth", keys(&[k; 8])).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_014_one_bad_entry_stores_none_of_the_others() {
    let fx = Fx::new().await;
    let m = refused(&fx, "auth", keys(&[&kp()[0].public_spki, "not a key"])).await;
    assert_eq!(m, form(1));
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_015_the_last_key_cannot_be_taken_away() {
    let fx = Fx::new().await;
    // The fixture holds only the key file path.
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"public_key_path": ""}})).await, NOKEY);
    nothing_stored(&fx, "auth").await;
    let k1 = &kp()[0].public_spki;
    set(&fx, "auth", json!({"jwt": {"public_key_path": "", "public_keys": [k1]}})).await.unwrap();
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"public_keys": []}})).await, NOKEY);
    // A section that holds no key now may change another key.
    put_auth(&fx, |v| {
        v["jwt"]["public_key_path"] = json!("");
        v["jwt"]["public_keys"] = json!([]);
    })
    .await;
    set(&fx, "auth", json!({"admins": ["b@example.com"]})).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_016_the_last_administrator_cannot_be_taken_away() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", json!({"admins": []})).await, NOADMIN);
    nothing_stored(&fx, "auth").await;
    set(&fx, "auth", json!({"admins": ["a@example.com"]})).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_017_the_identity_claim_cannot_be_emptied() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"username_claim": ""}})).await, NOCLAIM);
    nothing_stored(&fx, "auth").await;
    set(&fx, "auth", json!({"jwt": {"username_claim": "sub"}})).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_035_a_read_gives_the_public_keys_as_given_and_no_secret() {
    let fx = Fx::new().await;
    let k1 = &kp()[0].public_spki;
    let out = set(&fx, "auth", keys(&[k1])).await.unwrap();
    assert_eq!(out["value"]["jwt"]["public_keys"], json!([k1]));
    assert_eq!(out["secrets"], json!({}));
    assert_eq!(get(&fx, "auth").await, out);
    assert_eq!(get(&fx, "server").await["secrets"], json!({}));
}

#[tokio::test]
async fn e2e_0116_036_no_private_key_text_is_anywhere() {
    let fx = Fx::new().await;
    let k = &kp()[0];
    set(&fx, "auth", keys(&[&k.public_spki])).await.unwrap();
    set(&fx, "server", json!({"port": 47840})).await.unwrap();
    let _ = refused(&fx, "auth", keys(&[&k.private_pkcs8])).await;
    for s in ["auth", "server"] {
        assert!(!get(&fx, s).await.to_string().contains("PRIVATE KEY"));
    }
    assert!(!fx.changes().await.iter().any(|c| c.to_string().contains("PRIVATE KEY")));
    fx.assert_nowhere("PRIVATE KEY").await;
}

#[tokio::test]
async fn e2e_0116_059_a_repeated_key_and_blank_lines_are_accepted_as_given() {
    let fx = Fx::new().await;
    let (k1, k2) = (&kp()[0].public_spki, &kp()[1].public_spki);
    set(&fx, "auth", keys(&[k1, k1])).await.unwrap();
    let padded = format!("{k1}\n\n");
    let out = set(&fx, "auth", keys(&[&padded])).await.unwrap();
    assert_eq!(out["value"]["jwt"]["public_keys"], json!([padded]));
    let out = set(&fx, "auth", keys(&[k1, k2])).await.unwrap();
    assert_eq!(out["value"]["jwt"]["public_keys"], json!([k1, k2]));
}

#[tokio::test]
async fn e2e_0116_060_both_public_forms_of_an_rsa_key_are_accepted() {
    let fx = Fx::new().await;
    let out = set(&fx, "auth", keys(&[&kp()[0].public_pkcs1, &kp()[1].public_spki])).await.unwrap();
    assert_eq!(out["value"]["jwt"]["public_keys"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn e2e_0116_061_an_elliptic_curve_key_is_refused() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", keys(&[EC_P256])).await, form(0));
    let labelled = EC_P256.replace("PUBLIC KEY", "EC PUBLIC KEY");
    assert_eq!(refused(&fx, "auth", keys(&[&labelled])).await, form(0));
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_062_several_broken_rules_report_the_first_of_the_order() {
    let fx = Fx::new().await;
    let k1 = &kp()[0].public_spki;
    let bad = json!({"jwt": {"public_keys": ["not a key"], "username_claim": ""}, "admins": []});
    assert_eq!(refused(&fx, "auth", bad).await, form(0));
    set(&fx, "auth", keys(&[k1])).await.unwrap();
    let none = json!({"jwt": {"public_keys": [], "public_key_path": "", "username_claim": ""}, "admins": []});
    assert_eq!(refused(&fx, "auth", none).await, NOKEY);
    let noadmin = json!({"jwt": {"public_keys": [k1], "username_claim": ""}, "admins": []});
    assert_eq!(refused(&fx, "auth", noadmin).await, NOADMIN);
    let mut nine: Vec<&str> = vec![k1; 9];
    nine[3] = "not a key";
    assert_eq!(refused(&fx, "auth", keys(&nine)).await, form(3));
}

#[tokio::test]
async fn e2e_0116_064_every_private_label_is_refused_and_two_blocks_are_not_a_key() {
    let fx = Fx::new().await;
    for label in ["EC PRIVATE KEY", "ENCRYPTED PRIVATE KEY", "OPENSSH PRIVATE KEY"] {
        let e = format!("-----BEGIN {label}-----\nMIIEvQIBADANBgkq\n-----END {label}-----\n");
        assert_eq!(refused(&fx, "auth", keys(&[&e])).await, private(0), "{label}");
    }
    let two = format!("{}{}", kp()[0].public_spki, kp()[1].public_spki);
    assert_eq!(refused(&fx, "auth", keys(&[&two])).await, form(0));
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_074_wrong_types_and_empty_entries_are_refused() {
    let fx = Fx::new().await;
    let cases: Vec<(Value, String)> = vec![
        (json!({"jwt": {"public_keys": "abc"}}), wrong("auth", "jwt.public_keys")),
        (json!({"jwt": {"public_keys": [5]}}), wrong("auth", "jwt.public_keys")),
        (json!({"admins": [5]}), wrong("auth", "admins")),
        (json!({"admins": "a@example.com"}), wrong("auth", "admins")),
        (json!({"jwt": {"issuer": 3}}), wrong("auth", "jwt.issuer")),
        (json!({"admins": [""]}), EMPTY_ADMIN.into()),
        (json!({"jwt": {"header": ""}}), EMPTY_HEADER.into()),
        (json!({"jwt": {"algorithms": []}}), EMPTY_ALGS.into()),
    ];
    for (patch, msg) in cases {
        assert_eq!(refused(&fx, "auth", patch).await, msg);
    }
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_075_an_algorithm_the_build_cannot_verify_is_accepted_at_a_change() {
    let fx = Fx::new().await;
    set(&fx, "auth", json!({"jwt": {"algorithms": ["ES999"]}})).await.unwrap();
    set(&fx, "auth", json!({"jwt": {"algorithms": ["RS256", "PS384"]}})).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_076_repeated_administrators_and_null_issuer_audience_are_stored_as_given() {
    let fx = Fx::new().await;
    let out = set(
        &fx,
        "auth",
        json!({"admins": ["a@example.com", "a@example.com"], "jwt": {"issuer": null, "audience": null}}),
    )
    .await
    .unwrap();
    assert_eq!(out["value"]["admins"], json!(["a@example.com", "a@example.com"]));
    assert_eq!(out["value"]["jwt"]["issuer"], Value::Null);
    assert_eq!(get(&fx, "auth").await["value"], out["value"]);
}

#[tokio::test]
async fn e2e_0116_077_crlf_and_blank_lines_are_accepted_a_leading_space_is_not() {
    let fx = Fx::new().await;
    let k1 = &kp()[0].public_spki;
    let crlf = format!("{}   \r\n", k1.replace('\n', "\r\n"));
    let out = set(&fx, "auth", keys(&[&crlf])).await.unwrap();
    assert_eq!(out["value"]["jwt"]["public_keys"], json!([crlf]));
    let spaced = format!(" {k1}");
    assert_eq!(refused(&fx, "auth", keys(&[&spaced])).await, form(0));
}

#[tokio::test]
async fn e2e_0116_083_the_order_of_the_refusals_of_a_change() {
    let fx = Fx::new().await;
    let host = "invalid server.host 'bad host': expected an IPv4 address or a host name";
    let m = refused(&fx, "server", json!({"host": "bad host", "mcp_path": "mcp", "port": 0})).await;
    assert_eq!(m, host);
    let m = refused(&fx, "server", json!({"mcp_path": "mcp", "port": 0})).await;
    assert_eq!(m, "server.mcp_path must start with '/' (got 'mcp')");
    let m = refused(&fx, "server", json!({"host": "bad host", "port": "80"})).await;
    assert_eq!(m, wrong("server", "port"));
    let m =
        refused(&fx, "auth", json!({"admins": [""], "jwt": {"public_keys": ["not a key"]}})).await;
    assert_eq!(m, EMPTY_ADMIN);
    let m = refused(&fx, "auth", json!({"admins": [], "jwt": {"header": ""}})).await;
    assert_eq!(m, EMPTY_HEADER);
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_094_a_server_change_with_a_wrong_type_reports_the_type_first() {
    let fx = Fx::new().await;
    let m = refused(
        &fx,
        "server",
        json!({"host": "bad host", "mcp_path": "mcp", "public_base_url": "", "port": "80"}),
    )
    .await;
    assert_eq!(m, wrong("server", "port"));
    nothing_stored(&fx, "server").await;
}

#[tokio::test]
async fn e2e_0116_095_an_auth_change_with_a_wrong_type_reports_the_type_first() {
    let fx = Fx::new().await;
    let m = refused(&fx, "auth", json!({"jwt": {"issuer": 3}, "admins": [""]})).await;
    assert_eq!(m, wrong("auth", "jwt.issuer"));
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_096_the_empty_checks_come_in_the_order_admins_header_algorithms() {
    let fx = Fx::new().await;
    let all = json!({"admins": [""], "jwt": {"header": "", "algorithms": []}});
    assert_eq!(refused(&fx, "auth", all).await, EMPTY_ADMIN);
    let rest = json!({"jwt": {"header": "", "algorithms": []}});
    assert_eq!(refused(&fx, "auth", rest).await, EMPTY_HEADER);
}

#[tokio::test]
async fn e2e_0116_097_a_key_under_2048_bits_is_refused() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", keys(&[RSA_1024])).await, short(0));
    nothing_stored(&fx, "auth").await;
    set(&fx, "auth", keys(&[&kp()[0].public_spki])).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_098_the_position_of_the_short_key_is_named() {
    let fx = Fx::new().await;
    let m = refused(&fx, "auth", keys(&[&kp()[0].public_spki, RSA_1024])).await;
    assert_eq!(m, short(1));
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_099_a_form_error_comes_before_a_size_error() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", keys(&[RSA_1024, "not a key"])).await, form(1));
}

#[tokio::test]
async fn e2e_0116_100_a_blank_entry_counts_as_empty() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", json!({"admins": ["  "]})).await, EMPTY_ADMIN);
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"header": " "}})).await, EMPTY_HEADER);
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"algorithms": [" "]}})).await, EMPTY_ALGS);
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_101_a_blank_path_and_a_blank_claim_count_as_none() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"public_key_path": "   "}})).await, NOKEY);
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"username_claim": "\t"}})).await, NOCLAIM);
}

#[tokio::test]
async fn e2e_0116_102_an_administrator_name_is_stored_untrimmed() {
    let fx = Fx::new().await;
    let out = set(&fx, "auth", json!({"admins": [" a@example.com"]})).await.unwrap();
    assert_eq!(out["value"]["admins"], json!([" a@example.com"]));
}

#[tokio::test]
async fn e2e_0116_113_a_stored_key_that_is_too_short_does_not_block_another_key() {
    let fx = Fx::new().await;
    put_auth(&fx, |v| v["jwt"]["public_keys"] = json!([RSA_1024])).await;
    let out = set(&fx, "auth", json!({"admins": ["a@example.com"]})).await.unwrap();
    assert_eq!(out["value"]["jwt"]["public_keys"], json!([RSA_1024]));
    assert_eq!(refused(&fx, "auth", keys(&[RSA_1024])).await, short(0));
}

#[tokio::test]
async fn e2e_0116_114_a_stored_bad_key_does_not_block_another_key() {
    let fx = Fx::new().await;
    put_auth(&fx, |v| v["jwt"]["public_keys"] = json!(["not a key"])).await;
    set(&fx, "auth", json!({"jwt": {"username_claim": "sub"}})).await.unwrap();
    assert_eq!(refused(&fx, "auth", keys(&["not a key"])).await, form(0));
}

#[tokio::test]
async fn e2e_0116_115_the_number_of_keys_is_judged_on_the_list_as_it_will_be() {
    let fx = Fx::new().await;
    let k = kp()[0].public_spki.as_str();
    put_auth(&fx, |v| v["jwt"]["public_keys"] = json!(vec![k; 8])).await;
    set(&fx, "auth", json!({"admins": ["a@example.com"]})).await.unwrap();
    assert_eq!(refused(&fx, "auth", keys(&[k; 9])).await, MORE);
}

#[tokio::test]
async fn e2e_0116_116_a_read_of_auth_holds_the_key_list_and_the_path() {
    let fx = Fx::new().await;
    let a = get(&fx, "auth").await;
    assert_eq!(a["value"]["jwt"]["public_keys"], json!([]));
    assert!(a["value"]["jwt"]["public_key_path"].is_string());
    let k1 = &kp()[0].public_spki;
    set(&fx, "auth", keys(&[k1])).await.unwrap();
    let out =
        set(&fx, "auth", json!({"jwt": {"public_keys": [], "public_key_path": "/k/jwt.pub"}}))
            .await
            .unwrap();
    assert_eq!(out["value"]["jwt"]["public_keys"], json!([]));
    assert_eq!(out["value"]["jwt"]["public_key_path"], "/k/jwt.pub");
}

#[tokio::test]
async fn e2e_0116_140_an_empty_issuer_or_audience_is_refused() {
    let fx = Fx::new().await;
    let issuer = "invalid setting 'auth': jwt.issuer must not be empty (give null for no issuer)";
    let audience =
        "invalid setting 'auth': jwt.audience must not be empty (give null for no audience)";
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"issuer": ""}})).await, issuer);
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"issuer": "  "}})).await, issuer);
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"audience": ""}})).await, audience);
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_141_an_issuer_is_stored_and_a_null_audience_is_accepted() {
    let fx = Fx::new().await;
    let out = set(&fx, "auth", json!({"jwt": {"issuer": "corp", "audience": null}})).await.unwrap();
    assert_eq!(out["value"]["jwt"]["issuer"], "corp");
}

#[tokio::test]
async fn e2e_0116_142_a_stored_issuer_survives_a_change_of_the_administrators() {
    let fx = Fx::new().await;
    set(&fx, "auth", json!({"jwt": {"issuer": "web-a2a"}})).await.unwrap();
    let out = set(&fx, "auth", json!({"admins": ["a@example.com"]})).await.unwrap();
    assert_eq!(out["value"]["jwt"]["issuer"], "web-a2a");
}

#[tokio::test]
async fn e2e_0116_144_a_stored_port_zero_does_not_block_another_key() {
    let fx = Fx::new().await;
    let mut v = get(&fx, "server").await["value"].clone();
    v["port"] = json!(0);
    v["host"] = json!("127.0.0.1");
    put_raw(&fx, "server", v).await;
    let out = set(&fx, "server", json!({"host": "10.0.0.5"})).await.unwrap();
    assert_eq!(out["value"]["host"], "10.0.0.5");
    assert_eq!(out["value"]["port"], 0);
    assert_eq!(out["restart_required"], true);
}

#[tokio::test]
async fn e2e_0116_145_the_port_is_judged_where_it_is_given() {
    let fx = Fx::new().await;
    let mut v = get(&fx, "server").await["value"].clone();
    v["port"] = json!(0);
    put_raw(&fx, "server", v).await;
    let range = "invalid setting 'server': server.port must be a whole number from 1 to 65535";
    assert_eq!(refused(&fx, "server", json!({"port": 70000})).await, range);
    assert_eq!(refused(&fx, "server", json!({"port": 0})).await, range);
    let host = "invalid server.host 'bad host': expected an IPv4 address or a host name";
    assert_eq!(refused(&fx, "server", json!({"host": "bad host"})).await, host);
    set(&fx, "server", json!({"port": 47851})).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_146_an_empty_stored_entry_does_not_block_another_key() {
    let fx = Fx::new().await;
    put_auth(&fx, |v| v["admins"] = json!([""])).await;
    let out = set(&fx, "auth", json!({"jwt": {"issuer": "corp"}})).await.unwrap();
    assert_eq!(out["value"]["admins"], json!([""]));
    assert_eq!(refused(&fx, "auth", json!({"admins": [" "]})).await, EMPTY_ADMIN);
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"header": ""}})).await, EMPTY_HEADER);
    put_auth(&fx, |v| v["jwt"]["header"] = json!("")).await;
    set(&fx, "auth", json!({"admins": ["a@example.com"]})).await.unwrap();
}

#[tokio::test]
async fn e2e_0116_151_a_lone_private_begin_line_is_a_private_key() {
    let fx = Fx::new().await;
    let m = refused(&fx, "auth", keys(&["-----BEGIN RSA PRIVATE KEY-----"])).await;
    assert_eq!(m, private(0));
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_152_a_cut_private_key_is_refused_and_not_echoed() {
    let fx = Fx::new().await;
    let cut = "-----BEGIN PRIVATE KEY-----\nMIIEsecretbody";
    let m = refused(&fx, "auth", keys(&[&kp()[0].public_spki, cut])).await;
    assert_eq!(m, private(1));
    assert!(!m.contains("secretbody"));
    nothing_stored(&fx, "auth").await;
    fx.assert_nowhere("secretbody").await;
}

#[tokio::test]
async fn e2e_0116_153_a_begin_line_without_closing_dashes_is_not_a_private_label() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", keys(&["-----BEGIN PRIVATE KEY"])).await, form(0));
    assert_eq!(refused(&fx, "auth", keys(&["-----BEGIN PUBLIC KEY-----"])).await, form(0));
}

#[tokio::test]
async fn e2e_0116_158_a_private_key_with_crlf_is_private_a_leading_space_is_not() {
    let fx = Fx::new().await;
    let crlf = kp()[0].private_pkcs8.replace('\n', "\r\n");
    assert_eq!(refused(&fx, "auth", keys(&[&crlf])).await, private(0));
    let spaced = format!(" {}", kp()[0].private_pkcs8);
    assert_eq!(refused(&fx, "auth", keys(&[&spaced])).await, form(0));
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_160_a_private_key_given_as_the_list_is_not_echoed_by_the_type_refusal() {
    let fx = Fx::new().await;
    let p = &kp()[0].private_pkcs8;
    let body: Vec<&str> = p.lines().filter(|l| l.len() > 20 && !l.starts_with("-----")).collect();
    let m = refused(&fx, "auth", json!({"jwt": {"public_keys": p}})).await;
    assert_eq!(m, wrong("auth", "jwt.public_keys"));
    for l in &body {
        assert!(!m.contains(l));
        fx.assert_nowhere(l).await;
    }
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_161_a_private_key_given_as_another_value_is_not_echoed() {
    let fx = Fx::new().await;
    let p = &kp()[0].private_pkcs8;
    let m = refused(&fx, "auth", json!({"admins": p})).await;
    assert_eq!(m, wrong("auth", "admins"));
    let m2 = refused(&fx, "server", json!({"port": p})).await;
    assert_eq!(m2, wrong("server", "port"));
    for l in p.lines().filter(|l| l.len() > 20 && !l.starts_with("-----")) {
        assert!(!m.contains(l) && !m2.contains(l));
    }
}

#[tokio::test]
async fn e2e_0116_162_the_type_refusal_names_the_key() {
    let fx = Fx::new().await;
    assert_eq!(
        refused(&fx, "auth", json!({"jwt": {"public_keys": "abc"}})).await,
        wrong("auth", "jwt.public_keys")
    );
    assert_eq!(refused(&fx, "server", json!({"port": 3.5})).await, wrong("server", "port"));
    assert_eq!(
        refused(&fx, "auth", json!({"jwt": {"header": 5}})).await,
        wrong("auth", "jwt.header")
    );
}

#[tokio::test]
async fn e2e_0116_166_a_key_over_8192_bits_is_refused_and_one_of_8192_is_accepted() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", keys(&[RSA_8448])).await, long(0));
    nothing_stored(&fx, "auth").await;
    let out = set(&fx, "auth", keys(&[RSA_8192])).await.unwrap();
    assert_eq!(out["value"]["jwt"]["public_keys"], json!([RSA_8192]));
}

#[tokio::test]
async fn e2e_0116_167_the_position_of_the_long_key_is_named() {
    let fx = Fx::new().await;
    let k1 = &kp()[0].public_spki;
    assert_eq!(refused(&fx, "auth", keys(&[k1, RSA_8448])).await, long(1));
    assert_eq!(refused(&fx, "auth", keys(&[RSA_1024, RSA_8448])).await, short(0));
}

#[tokio::test]
async fn e2e_0116_168_a_key_of_exactly_8192_bits_is_stored_as_given() {
    let fx = Fx::new().await;
    set(&fx, "auth", keys(&[RSA_8192])).await.unwrap();
    assert_eq!(get(&fx, "auth").await["value"]["jwt"]["public_keys"], json!([RSA_8192]));
}

#[tokio::test]
async fn e2e_0116_169_a_section_or_the_jwt_object_of_the_wrong_type_is_refused() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", json!({"jwt": 5})).await, wrong("auth", "jwt"));
    assert_eq!(refused(&fx, "auth", json!({"jwt": []})).await, wrong("auth", "jwt"));
    assert_eq!(
        refused(&fx, "server", json!([])).await,
        "invalid setting 'server': the value has the wrong type"
    );
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_180_the_stages_take_the_short_key_before_the_long_one() {
    let fx = Fx::new().await;
    assert_eq!(refused(&fx, "auth", keys(&[RSA_8448, RSA_1024])).await, short(1));
}

#[tokio::test]
async fn e2e_0116_181_a_private_key_is_judged_before_a_bad_form_whatever_the_position() {
    let fx = Fx::new().await;
    let m = refused(&fx, "auth", keys(&["not a key", &kp()[0].private_pkcs8])).await;
    assert_eq!(m, private(1));
}

#[tokio::test]
async fn e2e_0116_182_a_private_key_at_the_first_place_wins_over_a_later_bad_form() {
    let fx = Fx::new().await;
    let m = refused(&fx, "auth", keys(&[&kp()[0].private_pkcs8, "not a key"])).await;
    assert_eq!(m, private(0));
    nothing_stored(&fx, "auth").await;
}

// ---- who may read and change the two sections ------------------------------

async fn tool_set(fx: &Fx, who: &str, section: &str, value: Value) -> Result<Value, String> {
    let p: rmcp::handler::server::wrapper::Parameters<SetSettingArgs> =
        args(json!({"section": section, "value": value}));
    fx.keep(tool_json(fx.server(who).admin_set_setting(p).await))
}

async fn tool_get(fx: &Fx, who: &str, section: &str) -> Result<Value, String> {
    let p: rmcp::handler::server::wrapper::Parameters<SectionArgs> =
        args(json!({"section": section}));
    fx.keep(tool_json(fx.server(who).admin_get_setting(p).await))
}

const FORBIDDEN: &str = "ERR_FORBIDDEN: 'bob@example.com' is not a platform admin";

#[tokio::test]
async fn e2e_0116_044_a_member_is_refused_on_tool_route_and_screen() {
    let fx = Fx::new().await;
    for s in ["server", "auth"] {
        assert_eq!(tool_get(&fx, BOB, s).await.unwrap_err(), FORBIDDEN, "{s}");
        assert_eq!(tool_set(&fx, BOB, s, json!({})).await.unwrap_err(), FORBIDDEN, "{s}");
        let (st, _) = fx.rest(BOB, "GET", &format!("/api/admin/settings/{s}"), None).await;
        assert_eq!(st, 403, "{s}");
        let (st, _) = fx
            .rest(BOB, "PUT", &format!("/api/admin/settings/{s}"), Some(json!({"value": {}})))
            .await;
        assert_eq!(st, 403, "{s}");
    }
    let (st, _) = fx.rest(BOB, "GET", "/api/admin/settings", None).await;
    assert_eq!(st, 403);
    let router = crate::settings_screen::router(fx.state.clone());
    let (st, _) = fx.page(&router, Via::Cookie(BOB), "GET", "/app/settings", None).await;
    assert_eq!(st, 403);
    nothing_stored(&fx, "server").await;
}

#[tokio::test]
async fn e2e_0116_119_a_member_listing_the_settings_is_refused_and_nothing_is_recorded() {
    let fx = Fx::new().await;
    let out = fx.keep(tool_json(fx.server(BOB).admin_list_settings().await));
    assert_eq!(out.unwrap_err(), FORBIDDEN);
    assert!(lines(&fx.changes().await).is_empty());
}

#[tokio::test]
async fn e2e_0116_046_the_administrator_reads_sets_and_lists_by_tool_and_route() {
    let fx = Fx::new().await;
    let out = tool_set(&fx, ALICE, "server", json!({"host": "127.0.0.1", "port": 47831})).await;
    let out = out.unwrap();
    assert_eq!(out["value"]["port"], 47831);
    assert_eq!(tool_get(&fx, ALICE, "server").await.unwrap(), out);
    let (st, body) = fx.rest(ALICE, "GET", "/api/admin/settings/server", None).await;
    assert_eq!(st, 200, "{body}");
    assert_eq!(body, out);
    let k1 = &kp()[0].public_spki;
    let (st, body) = fx
        .rest(ALICE, "PUT", "/api/admin/settings/auth", Some(json!({"value": keys(&[k1])})))
        .await;
    assert_eq!(st, 200, "{body}");
    assert_eq!(body["value"]["jwt"]["public_keys"], json!([k1]));
    let (st, list) = fx.rest(ALICE, "GET", "/api/admin/settings", None).await;
    assert_eq!(st, 200);
    assert_eq!(list["settings"].as_array().unwrap().len(), 13);
}

async fn screen_post(fx: &Fx, section: &str, value: &str) -> (axum::http::StatusCode, String) {
    let router = crate::settings_screen::router(fx.state.clone());
    let (_, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/settings", None).await;
    let token = token_of(&html);
    fx.page(
        &router,
        Via::Cookie(ALICE),
        "POST",
        "/app/settings/set",
        Some(&[("section", section), ("value", value), ("csrf_token", &token)]),
    )
    .await
}

#[tokio::test]
async fn e2e_0116_045_the_screen_lists_thirteen_sections_and_shows_a_refused_key() {
    let fx = Fx::new().await;
    let router = crate::settings_screen::router(fx.state.clone());
    let (st, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/settings", None).await;
    assert_eq!(st, 200, "{html}");
    for s in super::server_settings::SECTIONS {
        assert!(html.contains(&format!("data-section=\"{s}\"")), "{s}");
    }
    assert_eq!(super::server_settings::SECTIONS.len(), 13);
    let (st, _) = screen_post(&fx, "server", "{\"port\": 47835}").await;
    assert_eq!(st, 303);
    let (_, html) = fx.page(&router, Via::Cookie(ALICE), "GET", "/app/settings", None).await;
    assert!(html.contains("47835"), "{html}");
    assert!(html.contains("Source: database (restart required)"), "{html}");
    let (st, page) =
        screen_post(&fx, "auth", "{\"jwt\": {\"public_keys\": [\"not a key\"]}}").await;
    assert_eq!(st, 400, "{page}");
    assert!(page.contains("is not a public key in PEM form"), "{page}");
    assert_eq!(get(&fx, "auth").await["source"], "default");
}

#[tokio::test]
async fn e2e_0116_037_the_record_names_the_keys_of_an_auth_change_never_a_value() {
    let fx = Fx::new().await;
    let (k1, k2) = (&kp()[0].public_spki, &kp()[1].public_spki);
    set(
        &fx,
        "auth",
        json!({"jwt": {"public_keys": [k1, k2]}, "admins": ["a@example.com", "b@example.com"]}),
    )
    .await
    .unwrap();
    let all = fx.changes().await;
    let l = lines(&all)[0];
    assert_eq!(l["fields"], json!(["admins", "jwt.public_keys"]));
    let text = l.to_string();
    for needle in [k1.as_str(), k2.as_str(), "a@example.com", "b@example.com"] {
        assert!(!text.contains(needle));
    }
}

// ---- found by the review of the diff ---------------------------------------------------

#[tokio::test]
async fn e2e_0116_201_a_path_of_white_space_counts_as_no_path_at_a_change_and_at_the_start() {
    let fx = Fx::new().await;
    // The fixture holds only the key file path: a path of a newline is no key.
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"public_key_path": "\n"}})).await, NOKEY);
    assert_eq!(refused(&fx, "auth", json!({"admins": ["\n"]})).await, EMPTY_ADMIN);
    assert_eq!(refused(&fx, "auth", json!({"jwt": {"header": "\u{a0}"}})).await, EMPTY_HEADER);
    nothing_stored(&fx, "auth").await;
}

#[tokio::test]
async fn e2e_0116_202_a_port_above_the_signed_range_gets_the_port_refusal() {
    let fx = Fx::new().await;
    let range = "invalid setting 'server': server.port must be a whole number from 1 to 65535";
    assert_eq!(refused(&fx, "server", json!({"port": u64::MAX})).await, range);
    nothing_stored(&fx, "server").await;
}
