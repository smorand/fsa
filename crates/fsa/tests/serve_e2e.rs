//! SPEC-0116 program level tests (FR-NEW-011, FR-NEW-021, FR-NEW-049): the built
//! `fsa serve` prints its banner once the stored settings are applied, refuses to
//! serve with no key, and ends a refused start with `error: <message>` on the last
//! line of standard error, nothing on standard output, and a failure status.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_fsa");
const NO_KEY: &str = "error: no auth key is configured: store auth.jwt.public_keys or set auth.jwt.public_key_path (a first start from a YAML file imports them)";

fn write_yaml(dir: &Path, auth: &str, server: &str) -> std::path::PathBuf {
    let p = dir.join("config.yaml");
    std::fs::write(
        &p,
        format!(
            "server:\n{server}\n{auth}infra:\n  meta:\n    dir: {d}/volumes\n  blob:\n    dir: {d}/blobs\n  admin:\n    path: {d}/admin.db\n",
            d = dir.display()
        ),
    )
    .unwrap();
    p
}

fn run(dir: &Path, config: &Path) -> (bool, String, String) {
    let out = Command::new(BIN)
        .arg("serve")
        .arg("--config")
        .arg(config)
        .env_clear()
        .env("HOME", dir)
        .current_dir(dir)
        .output()
        .expect("run fsa serve");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn last_line(text: &str) -> &str {
    text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or_default()
}

#[test]
fn e2e_0116_080_serving_with_no_key_prints_the_banner_then_the_refusal() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_yaml(tmp.path(), "", "  host: 127.0.0.1\n  port: 0");
    let (ok, stdout, stderr) = run(tmp.path(), &cfg);
    assert!(!ok, "the start is refused");
    assert!(stdout.is_empty(), "stdout: {stdout}");
    let banner = format!(
        "Serving fsa {} on 127.0.0.1:0 (config={})",
        env!("CARGO_PKG_VERSION"),
        cfg.display()
    );
    let lines: Vec<&str> = stderr.lines().collect();
    let at = lines.iter().position(|l| *l == banner).unwrap_or_else(|| panic!("{stderr}"));
    assert!(lines[at + 1..].contains(&NO_KEY), "{stderr}");
    assert_eq!(last_line(&stderr), NO_KEY);
}

#[test]
fn e2e_0116_199_the_refusal_is_the_last_line_with_no_code_and_nothing_on_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_yaml(tmp.path(), "", "  host: 127.0.0.1\n  port: 0");
    let (ok, stdout, stderr) = run(tmp.path(), &cfg);
    assert!(!ok);
    assert!(stdout.is_empty());
    assert_eq!(last_line(&stderr), NO_KEY);
    assert!(!last_line(&stderr).contains("ERR_"));
}

/// Starts `fsa serve` and stops it once the banner is read (the stored settings are
/// applied before the banner, so the first start has seeded the database).
fn start_until_banner(dir: &Path, config: &Path) {
    let mut child = Command::new(BIN)
        .arg("serve")
        .arg("--config")
        .arg(config)
        .env_clear()
        .env("HOME", dir)
        .current_dir(dir)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("start fsa serve");
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let done = line.starts_with("Serving fsa");
            let _ = tx.send(line);
            if done {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut seen = false;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(l) if l.starts_with("Serving fsa") => {
                seen = true;
                break;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    assert!(seen, "no banner");
}

#[test]
fn e2e_0116_200_a_stored_refusal_ends_the_start_with_the_message_and_a_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let (_priv, public) = fsa_core::keys::write_keypair(tmp.path().join("keys")).unwrap();
    let auth = format!(
        "auth:\n  jwt:\n    public_key_path: {}\n  admins: [admin@example.com]\n",
        public.display()
    );
    let cfg = write_yaml(tmp.path(), &auth, "  host: 127.0.0.1\n  port: 0");
    start_until_banner(tmp.path(), &cfg);
    let edit = Command::new("sqlite3")
        .arg(tmp.path().join("admin.db"))
        .arg("update server_setting set value = json_set(value, '$.mcp_path', 'mcp') where section = 'server'")
        .status()
        .expect("sqlite3");
    assert!(edit.success());
    let (ok, stdout, stderr) = run(tmp.path(), &cfg);
    assert!(!ok);
    assert!(stdout.is_empty());
    assert_eq!(
        last_line(&stderr),
        "error: the stored settings are refused: server.mcp_path must start with '/' (got 'mcp')"
    );
}
