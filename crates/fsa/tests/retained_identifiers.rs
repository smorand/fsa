//! DT-012 (SPEC-0099): the identifiers stored in existing deployments keep their spelling,
//! so a bucket, a trash directory or a git ref written by an older version is still found.

use fsa_core::config::ServerConfig;
use std::path::Path;

fn source(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

#[test]
fn the_default_bucket_prefix_is_still_mcpfs() {
    assert_eq!(ServerConfig::default().infra.blob.bucket_prefix, "mcpfs-");
}

#[test]
fn the_default_trash_directory_is_still_mcp_trash() {
    assert_eq!(ServerConfig::default().safety.trash_dir, ".mcp_trash");
}

#[test]
fn the_refs_the_lock_key_and_the_cookie_keep_their_spelling() {
    assert!(source("crates/core/src/git/remote.rs").contains("refs/mcpfs/incoming/"));
    assert!(source("crates/core/src/git/remote.rs").contains("refs/mcpfs/probe/"));
    assert!(source("crates/core/src/storage/rel/postgres.rs").contains("mcpfs_migrate:"));
    assert!(source("crates/core/src/trash_screen.rs").contains("mcpfs_token"));
    assert!(source("crates/core/src/tools/git.rs").contains("refs/mcp-fs/rebase-head"));
    assert!(source("crates/core/src/tools/git.rs").contains("mcp-fs-stash: "));
}
