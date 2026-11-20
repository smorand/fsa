//! DT-011 (SPEC-0099): the workspace packages and the binary carry the product name.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

#[test]
fn packages_are_fsa_fsa_core_and_agent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&root)
        .output()
        .expect("run cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).expect("metadata json");
    let names: BTreeSet<String> = meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .map(|p| p["name"].as_str().expect("package name").to_string())
        .collect();
    let want: BTreeSet<String> =
        ["agent", "fsa", "fsa-core"].iter().map(|s| s.to_string()).collect();
    assert_eq!(names, want, "workspace packages");
}

#[test]
fn the_binary_is_named_fsa_and_prints_its_name_in_version() {
    let bin = env!("CARGO_BIN_EXE_fsa");
    assert!(Path::new(bin).file_name().is_some_and(|n| n == "fsa"), "binary path {bin}");
    let out = Command::new(bin).arg("--version").output().expect("run fsa --version");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("fsa "), "version line: {text}");
}

/// DT-004 (SPEC-0099): the command line names itself `fsa`.
#[test]
fn help_starts_with_the_product_name() {
    let bin = env!("CARGO_BIN_EXE_fsa");
    let out = Command::new(bin).arg("--help").output().expect("run fsa --help");
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next().unwrap_or_default();
    assert!(first.starts_with("fsa:"), "first help line: {first}");
}
