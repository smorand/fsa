//! SPEC-0097: the web access lists are checked when a command reads the configuration
//! (DEC-005, DEC-016). Driven through `ServerConfig::from_yaml` and `cli::dispatch`, never a
//! child process; the refusal text is the one the operator reads after `error: `.

use clap::Parser;

use crate::cli::{Cli, dispatch};
use crate::config::ServerConfig;

fn refusal(entry: &str, list: &str) -> String {
    format!(
        "ERR_INVALID_ARGUMENT: invalid entry '{entry}' in the web {list} list: expected a host \
         name, a *.host name, an IP address or an address range"
    )
}

fn yaml_list(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| format!("{i:?}")).collect();
    format!("[{}]", quoted.join(", "))
}

fn web_yaml(enabled: bool, black: &[&str], white: &[&str]) -> String {
    format!(
        "web:\n  enabled: {enabled}\n  black_list_url: {}\n  white_list_url: {}\n",
        yaml_list(black),
        yaml_list(white)
    )
}

fn assert_refused(enabled: bool, black: &[&str], white: &[&str], expected: &str) {
    let err = ServerConfig::from_yaml(&web_yaml(enabled, black, white))
        .expect_err("the configuration must be refused");
    assert_eq!(err.to_string(), expected);
}

/// A configuration file whose state lives under `root`, so a command that gets past the lists
/// would find real (empty) stores instead of the operator's.
fn write_config(root: &std::path::Path, name: &str, web: &str, port: u16) -> std::path::PathBuf {
    let yaml = format!(
        "server:\n  port: {port}\ninfra:\n  meta:\n    dir: {:?}\n  blob:\n    dir: {:?}\n  admin:\n    path: {:?}\n{web}",
        root.join("volumes").display().to_string(),
        root.join("blobs").display().to_string(),
        root.join("admin.db").display().to_string(),
    );
    let path = root.join(name);
    std::fs::write(&path, yaml).unwrap();
    path
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

// ── SPEC-0097/E2E-041 ───────────────────────────────────────────────────────

#[test]
fn e2e_0097_041_valid_lists_start_the_server() {
    // The host map is process wide: a start that succeeds publishes it.
    let _hosts = crate::git::remote::tests::lock_for_test();
    let black = ["10.0.0.0/8", "*.corp.example", "fd00::/8", "blocked.example"];
    let c = ServerConfig::from_yaml(&web_yaml(true, &black, &["docs.example"])).unwrap();
    assert_eq!(c.web.black_list_url, black);
    assert_eq!(c.web.white_list_url, ["docs.example"]);
}

// ── SPEC-0097/E2E-042 ───────────────────────────────────────────────────────

#[test]
fn e2e_0097_042_an_address_with_a_scheme_and_path() {
    assert_refused(
        true,
        &["https://blocked.example/x"],
        &[],
        &refusal("https://blocked.example/x", "black"),
    );
}

// ── SPEC-0097/E2E-043 ───────────────────────────────────────────────────────

#[test]
fn e2e_0097_043_impossible_range_length() {
    assert_refused(true, &[], &["10.0.0.0/33"], &refusal("10.0.0.0/33", "white"));
}

// ── SPEC-0097/E2E-044 ───────────────────────────────────────────────────────

#[test]
fn e2e_0097_044_wildcard_alone() {
    assert_refused(true, &["*"], &[], &refusal("*", "black"));
}

// ── SPEC-0097/E2E-045 ───────────────────────────────────────────────────────

#[test]
fn e2e_0097_045_an_entry_with_a_port() {
    assert_refused(true, &["example.com:8080"], &[], &refusal("example.com:8080", "black"));
}

// ── SPEC-0097/E2E-047 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_047_a_refused_start_leaves_nothing_running() {
    let d = tempfile::tempdir().unwrap();
    let port = free_port();
    let path = write_config(d.path(), "config.yaml", &web_yaml(true, &["*"], &[]), port);

    let err = dispatch(Cli::parse_from(["fsa", "serve", "--config", path.to_str().unwrap()]))
        .await
        .expect_err("serve must refuse");

    assert_eq!(err.to_string(), refusal("*", "black"));
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "something listens");
}

// ── SPEC-0097/E2E-056 ───────────────────────────────────────────────────────

#[test]
fn e2e_0097_056_malformed_name_entry() {
    assert_refused(true, &["a..b"], &[], &refusal("a..b", "black"));
}

// ── SPEC-0097/E2E-063 ───────────────────────────────────────────────────────

#[test]
fn e2e_0097_063_the_web_switch_off_does_not_skip_the_check() {
    assert_refused(false, &["*"], &[], &refusal("*", "black"));
}

// ── SPEC-0097/E2E-066 ───────────────────────────────────────────────────────

#[test]
fn e2e_0097_066_valid_lists_with_the_web_tools_off() {
    // The host map is process wide: a start that succeeds publishes it.
    let _hosts = crate::git::remote::tests::lock_for_test();
    let c = ServerConfig::from_yaml(&web_yaml(false, &["10.0.0.0/8"], &[])).unwrap();
    assert_eq!(c.web.black_list_url, ["10.0.0.0/8"]);
    assert!(c.web.white_list_url.is_empty());
}

// ── SPEC-0097/E2E-067 ───────────────────────────────────────────────────────

#[test]
fn e2e_0097_067_bad_white_list_entry_with_the_web_tools_off() {
    assert_refused(false, &[], &["10.0.0.0/33"], &refusal("10.0.0.0/33", "white"));
}

// ── SPEC-0097/FR-NEW-012: purge and migrate read the configuration too ───────

#[tokio::test]
async fn e2e_0097_012_purge_refuses_before_any_work() {
    let d = tempfile::tempdir().unwrap();
    let path = write_config(d.path(), "config.yaml", &web_yaml(false, &[], &["a..b"]), 5002);

    let err = dispatch(Cli::parse_from(["fsa", "purge", "--config", path.to_str().unwrap()]))
        .await
        .expect_err("purge must refuse");

    assert_eq!(err.to_string(), refusal("a..b", "white"));
    assert!(!d.path().join("admin.db").exists(), "work started before the check");
}

#[tokio::test]
async fn e2e_0097_012_migrate_names_the_source_before_the_destination() {
    let d = tempfile::tempdir().unwrap();
    let from = write_config(d.path(), "from.yaml", &web_yaml(false, &["src..bad"], &[]), 5002);
    let to = write_config(d.path(), "to.yaml", &web_yaml(false, &["dst..bad"], &[]), 5002);

    let err = dispatch(Cli::parse_from([
        "fsa",
        "migrate",
        "--from",
        from.to_str().unwrap(),
        "--to",
        to.to_str().unwrap(),
    ]))
    .await
    .expect_err("migrate must refuse");

    assert_eq!(err.to_string(), refusal("src..bad", "black"));
}

#[test]
fn e2e_0097_012_black_list_is_named_before_the_white_list() {
    assert_refused(false, &["x..y"], &["a..b"], &refusal("x..y", "black"));
}

// ── SPEC-0097/E2E-059 to E2E-086: the rest of the entry grammar (US-0247) ────

#[test]
fn e2e_0097_059_numeric_entry_that_is_not_a_four_part_address() {
    for entry in ["2130706433", "0x7f000001", "10.1"] {
        assert_refused(true, &[], &[entry], &refusal(entry, "white"));
    }
}

#[test]
fn e2e_0097_060_more_malformed_entries() {
    let long_label = format!("{}.example", "a".repeat(64));
    for entry in ["010.0.0.1", " blocked.example", long_label.as_str(), "xn--a"] {
        assert_refused(true, &[entry], &[], &refusal(entry, "black"));
    }
}

#[test]
fn e2e_0097_072_malformed_range_lengths_and_bracketed_or_zoned_ipv6() {
    for entry in ["10.0.0.0/08", "10.0.0.0/+8", "[fd00::1]", "fe80::1%eth0"] {
        assert_refused(true, &[entry], &[], &refusal(entry, "black"));
    }
}

#[test]
fn e2e_0097_085_an_empty_punycode_label() {
    for entry in ["xn--", "XN--"] {
        assert_refused(true, &[entry], &[], &refusal(entry, "black"));
    }
}

#[test]
fn e2e_0097_057_an_accent_cannot_hide_a_percent_escape() {
    let entry = "b\u{fc}%63her.example";
    assert_refused(true, &[entry], &[], &refusal(entry, "black"));
}

#[test]
fn e2e_0097_086_more_than_one_trailing_dot() {
    for entry in ["blocked.example..", "."] {
        assert_refused(true, &[entry], &[], &refusal(entry, "black"));
    }
}

#[test]
fn e2e_0097_086_exactly_one_trailing_dot_is_accepted() {
    // The host map is process wide: a start that succeeds publishes it.
    let _hosts = crate::git::remote::tests::lock_for_test();
    let c = ServerConfig::from_yaml(&web_yaml(true, &["blocked.example.", "*.corp.example."], &[]))
        .unwrap();
    assert_eq!(c.web.black_list_url.len(), 2);
}

// ── SPEC-0097/E2E-089 to E2E-091: purge and migrate (US-0247) ────────────────

#[tokio::test]
async fn e2e_0097_089_the_purge_command_refuses_a_bad_entry() {
    let d = tempfile::tempdir().unwrap();
    let path = write_config(d.path(), "config.yaml", &web_yaml(true, &["*"], &[]), 5002);

    let err = dispatch(Cli::parse_from(["fsa", "purge", "--config", path.to_str().unwrap()]))
        .await
        .expect_err("purge must refuse");

    assert_eq!(format!("error: {err}"), format!("error: {}", refusal("*", "black")));
    assert!(!d.path().join("admin.db").exists(), "a purge step ran");
}

#[tokio::test]
async fn e2e_0097_090_the_migrate_command_refuses_a_bad_entry() {
    let d = tempfile::tempdir().unwrap();
    let from = write_config(d.path(), "from.yaml", &web_yaml(true, &[], &["10.0.0.0/33"]), 5002);
    let to = write_config(d.path(), "to.yaml", &web_yaml(true, &[], &[]), 5002);

    let err = dispatch(Cli::parse_from([
        "fsa",
        "migrate",
        "--from",
        from.to_str().unwrap(),
        "--to",
        to.to_str().unwrap(),
    ]))
    .await
    .expect_err("migrate must refuse");

    assert_eq!(err.to_string(), refusal("10.0.0.0/33", "white"));
    assert!(!d.path().join("admin.db").exists(), "something was copied");
}

#[tokio::test]
async fn e2e_0097_091_the_purge_command_accepts_valid_lists() {
    let d = tempfile::tempdir().unwrap();
    let path = write_config(d.path(), "config.yaml", &web_yaml(true, &["10.0.0.0/8"], &[]), 5002);

    dispatch(Cli::parse_from(["fsa", "purge", "--config", path.to_str().unwrap()]))
        .await
        .expect("purge with valid lists must run");
}
