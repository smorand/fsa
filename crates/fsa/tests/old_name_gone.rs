//! DT-010 (SPEC-0099): no spelling of the old product name is left outside the retained
//! identifiers, the deprecation fallbacks and the history folders.
//!
//! The files searched are the tracked ones (`git ls-files`), so ignored or untracked local
//! files such as `config/local.yaml` are never scanned. The search is case sensitive.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// The spellings that must be gone.
const BANNED: [&str; 8] =
    ["mcp-fs", "mcp_fs", "MCP-FS", "MCP FS", "MCPFS_", "MCP_FS_", "McpFs", "mcpfs"];

/// Retained identifiers specific enough to be removed everywhere before searching: the
/// persisted or external ones of Section 5 of the design, and the developer variable
/// families of DEC-005.
const RETAINED_EVERYWHERE: [&str; 11] = [
    "refs/mcpfs/",
    "refs/mcp-fs/rebase-head",
    "mcp-fs-stash: ",
    "mcpfs_migrate:",
    "mcpfs_token",
    "MCPFS_TEST_",
    "MCPFS_REWRITE_",
    "MCPFS_CHILD_CASE",
    "MCPFS_PRINT_RECORD",
    "McpFs/Git",
    "mcp fs.",
];

/// `(path, literal)` pairs: the literal may appear in that file. Deprecation notes and
/// fallback code and tests, and the retained spellings that cannot be matched globally.
const ALLOWED: &[(&str, &str)] = &[
    // this test names the spellings it looks for
    ("crates/fsa/tests/old_name_gone.rs", "mcp-fs"),
    ("crates/fsa/tests/old_name_gone.rs", "mcp_fs"),
    ("crates/fsa/tests/old_name_gone.rs", "MCP-FS"),
    ("crates/fsa/tests/old_name_gone.rs", "MCP FS"),
    ("crates/fsa/tests/old_name_gone.rs", "MCPFS_"),
    ("crates/fsa/tests/old_name_gone.rs", "MCP_FS_"),
    ("crates/fsa/tests/old_name_gone.rs", "McpFs"),
    ("crates/fsa/tests/old_name_gone.rs", "mcpfs"),
    // fallback code and its tests (US-0259 to US-0261)
    ("crates/core/src/cli.rs", "mcp-fs"),
    ("crates/core/src/cli.rs", "MCP_FS_"),
    ("crates/core/src/config.rs", "MCP_FS_"),
    ("crates/core/src/config.rs", "MCPFS_"),
    ("crates/core/src/git/oauth/store.rs", "MCPFS_"),
    // the tests of the names that may not hold a secret (SPEC-0116): they name the old default names
    ("crates/core/src/settings/server_settings8_e2e.rs", "MCPFS_"),
    ("crates/core/src/settings/server_settings8_e2e.rs", "MCP_FS_"),
    ("crates/fsa/tests/env_compat_e2e.rs", "mcp-fs"),
    ("crates/fsa/tests/env_compat_e2e.rs", "MCP_FS_"),
    // retained: the bucket prefix `mcpfs-`, the DSN and the schema `mcpfs`
    ("crates/core/src/config.rs", "mcpfs"),
    ("crates/core/src/storage/blob/s3.rs", "mcpfs"),
    ("crates/core/src/storage/mod.rs", "mcpfs"),
    ("crates/core/src/storage/rel/postgres.rs", "mcpfs"),
    ("crates/core/src/storage/conformance.rs", "mcpfs"),
    ("config/minio.yaml.template", "mcpfs"),
    ("docker-compose.test.yml", "mcpfs"),
    ("crates/fsa/tests/retained_identifiers.rs", "mcpfs"),
    // documentation of the deprecation and of the retained identifiers
    ("README.md", "MCPFS_"),
    ("README.md", "MCP_FS_"),
    ("README.md", "mcp-fs"),
    ("README.md", "mcpfs"),
    (".agent_docs/config.md", "MCPFS_"),
    (".agent_docs/config.md", "MCP_FS_"),
    (".agent_docs/config.md", "mcp-fs"),
    (".agent_docs/config.md", "mcpfs"),
    (".agent_docs/lineage.md", "MCPFS_"),
    (".agent_docs/lineage.md", "MCP_FS_"),
    (".agent_docs/lineage.md", "mcp-fs"),
    (".agent_docs/lineage.md", "mcpfs"),
    (".agent_docs/git.md", "mcpfs"),
    (".agent_docs/architecture.md", "mcpfs"),
    (".agent_docs/backends.md", "mcpfs"),
    (".agent_docs/testing.md", "mcpfs"),
];

fn tracked_files(root: &Path) -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files"])
        .current_dir(root)
        .output()
        .expect("git must be available: this test fails rather than skips");
    assert!(out.status.success(), "git ls-files failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect()
}

#[test]
fn no_old_name_is_left_outside_the_allow_list() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let allowed: BTreeSet<(&str, &str)> = ALLOWED.iter().copied().collect();
    let mut found: BTreeSet<(String, &str)> = BTreeSet::new();
    for path in tracked_files(&root) {
        if path.starts_with("sdlc/") || path.starts_with("plan/") || path == "Cargo.lock" {
            continue;
        }
        let Ok(mut text) = std::fs::read_to_string(root.join(&path)) else { continue };
        for kept in RETAINED_EVERYWHERE {
            text = text.replace(kept, "");
        }
        for banned in BANNED {
            if text.contains(banned) && !allowed.contains(&(path.as_str(), banned)) {
                found.insert((path.clone(), banned));
            }
        }
    }
    assert!(found.is_empty(), "old name left, file and spelling: {found:#?}");
}
