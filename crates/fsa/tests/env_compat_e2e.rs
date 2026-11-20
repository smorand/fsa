//! DT-001 (SPEC-0099): the renamed environment variables keep working under their old
//! names for a deprecation window. The compiled binary is run with an empty environment
//! so the once per process warning is read from a real standard error.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_fsa");

/// Run `fsa serve` with only `env` set. `serve` resolves the configuration first and,
/// finding no file, stops with an error that names the path it tried.
fn serve(tmp: &Path, env: &[(&str, &str)]) -> String {
    let out = Command::new(BIN)
        .arg("serve")
        .env_clear()
        .env("HOME", tmp)
        .env("XDG_CONFIG_HOME", tmp.join("xdg"))
        .current_dir(tmp)
        .envs(env.iter().copied())
        .output()
        .expect("run fsa serve");
    assert!(!out.status.success(), "serve must stop: no configuration file exists");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn warning(old: &str, new: &str) -> String {
    format!("warning: environment variable {old} is deprecated, use {new}")
}

fn count(text: &str, needle: &str) -> usize {
    text.lines().filter(|l| l.trim() == needle).count()
}

#[test]
fn old_config_variable_is_used_and_warned_once() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old.yaml");
    let err = serve(tmp.path(), &[("MCP_FS_CONFIG", old.to_str().unwrap())]);
    assert_eq!(count(&err, &warning("MCP_FS_CONFIG", "FSA_CONFIG")), 1, "stderr: {err}");
    assert!(err.contains(old.to_str().unwrap()), "the old value is used: {err}");
}

#[test]
fn new_config_variable_wins_and_the_old_one_is_still_warned() {
    let tmp = tempfile::tempdir().unwrap();
    let (new, old) = (tmp.path().join("new.yaml"), tmp.path().join("old.yaml"));
    let err = serve(
        tmp.path(),
        &[("FSA_CONFIG", new.to_str().unwrap()), ("MCP_FS_CONFIG", old.to_str().unwrap())],
    );
    assert!(err.contains(new.to_str().unwrap()), "the new value is used: {err}");
    assert!(!err.contains(old.to_str().unwrap()), "the old value is not: {err}");
    assert_eq!(count(&err, &warning("MCP_FS_CONFIG", "FSA_CONFIG")), 1, "stderr: {err}");
}

#[test]
fn only_the_new_config_variable_prints_no_warning() {
    let tmp = tempfile::tempdir().unwrap();
    let new = tmp.path().join("new.yaml");
    let err = serve(tmp.path(), &[("FSA_CONFIG", new.to_str().unwrap())]);
    assert!(err.contains(new.to_str().unwrap()), "stderr: {err}");
    assert!(!err.contains("warning:"), "stderr: {err}");
}

#[test]
fn a_blank_new_value_falls_back_and_a_blank_old_value_is_silent() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old.yaml");
    let err = serve(tmp.path(), &[("FSA_CONFIG", "  "), ("MCP_FS_CONFIG", old.to_str().unwrap())]);
    assert!(err.contains(old.to_str().unwrap()), "stderr: {err}");
    assert_eq!(count(&err, &warning("MCP_FS_CONFIG", "FSA_CONFIG")), 1, "stderr: {err}");

    let err = serve(tmp.path(), &[("MCP_FS_CONFIG", "   ")]);
    assert!(!err.contains("warning:"), "stderr: {err}");
}

#[test]
fn old_directory_and_name_variables_are_used_and_warned_once_each() {
    let tmp = tempfile::tempdir().unwrap();
    let err =
        serve(tmp.path(), &[("MCP_FS_CONFIG_DIR", "olddir"), ("MCP_FS_CONFIG_NAME", "oldname")]);
    assert!(err.contains("olddir/oldname.yaml"), "stderr: {err}");
    assert_eq!(count(&err, &warning("MCP_FS_CONFIG_DIR", "FSA_CONFIG_DIR")), 1, "stderr: {err}");
    assert_eq!(count(&err, &warning("MCP_FS_CONFIG_NAME", "FSA_CONFIG_NAME")), 1, "stderr: {err}");

    let err = serve(
        tmp.path(),
        &[
            ("FSA_CONFIG_DIR", "newdir"),
            ("FSA_CONFIG_NAME", "newname"),
            ("MCP_FS_CONFIG_DIR", "olddir"),
        ],
    );
    assert!(err.contains("newdir/newname.yaml"), "stderr: {err}");
    assert!(!err.contains("olddir"), "stderr: {err}");
    assert_eq!(count(&err, &warning("MCP_FS_CONFIG_DIR", "FSA_CONFIG_DIR")), 1, "stderr: {err}");
}

#[test]
fn the_not_found_message_names_the_new_variable() {
    let tmp = tempfile::tempdir().unwrap();
    let err = serve(tmp.path(), &[]);
    assert!(err.contains("set FSA_CONFIG"), "stderr: {err}");
}

/// A child process that is killed and reaped when it goes out of scope, so a failing
/// assertion never leaves a server running.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Start `fsa serve` on an ephemeral port with only `env` set, collect standard error up to
/// and including the startup banner, then stop the server. The banner names the file loaded.
fn serve_until_banner(tmp: &Path, env: &[(&str, &str)]) -> String {
    let mut child = KillOnDrop(
        Command::new(BIN)
            .arg("serve")
            .env_clear()
            .env("HOME", tmp)
            .envs(env.iter().copied())
            .current_dir(tmp)
            .stderr(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .expect("start fsa serve"),
    );
    let stderr = child.0.stderr.take().expect("stderr");
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
    // One deadline for the whole read: a server that keeps printing without ever printing
    // the banner must not keep the test alive.
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut lines = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(line) => lines.push(line),
            Err(_) => break,
        }
    }
    lines.join("\n")
}

/// A configuration that serves on an ephemeral loopback port.
fn write_config(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("config.yaml"), "server:\n  host: 127.0.0.1\n  port: 0\n").unwrap();
}

/// DT-003 (SPEC-0099): the old user directory loads with one warning.
#[test]
fn the_old_user_config_directory_is_loaded_with_a_warning() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tmp.path().join("xdg");
    write_config(&xdg.join("mcp-fs"));
    let err = serve_until_banner(tmp.path(), &[("XDG_CONFIG_HOME", xdg.to_str().unwrap())]);
    let want = format!(
        "warning: configuration directory {} is deprecated, move it to {}",
        xdg.join("mcp-fs").display(),
        xdg.join("fsa").display()
    );
    assert_eq!(count(&err, &want), 1, "stderr: {err}");
    assert!(err.contains(&format!("config={}", xdg.join("mcp-fs/config.yaml").display())), "{err}");
}

#[test]
fn the_new_user_config_directory_wins_and_prints_no_warning() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tmp.path().join("xdg");
    write_config(&xdg.join("mcp-fs"));
    write_config(&xdg.join("fsa"));
    let err = serve_until_banner(tmp.path(), &[("XDG_CONFIG_HOME", xdg.to_str().unwrap())]);
    assert!(err.contains(&format!("config={}", xdg.join("fsa/config.yaml").display())), "{err}");
    assert!(!err.contains("warning:"), "stderr: {err}");
}
