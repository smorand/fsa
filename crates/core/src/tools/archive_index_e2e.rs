//! SPEC-0112 (BL-0177): `fs.extract_archive` schedules the indexing of the
//! files it writes, on the `search::derived_e2e::Env` of the recalculation
//! tests (search on, the engine behind the SPEC-0100 scheduler, a paused
//! clock). Archives are put with raw client writes, which index nothing, so a
//! test reads what the extraction alone did.

use crate::errors::Result;
use crate::search::derived_e2e::{ALICE, Env, Options, PROJ};
use crate::storage::traits::IndexMode;
use crate::tools::archive::extract_archive;
use crate::tools::archive::tests::build_zip;
use serde_json::Value;
use std::io::Write as _;

async fn env() -> Env {
    let env = Env::new(Options::default()).await;
    let c = env.state.stores.client(PROJ).await.unwrap();
    c.makedirs("/up", true).await.unwrap();
    env
}

async fn put(env: &Env, path: &str, bytes: &[u8]) {
    let c = env.state.stores.client(PROJ).await.unwrap();
    c.write_bytes_atomic(path, bytes).await.unwrap();
}

async fn extract(env: &Env, path: &str, dest: Option<&str>, overwrite: bool) -> Result<Value> {
    extract_archive(&env.state, PROJ, ALICE, path, dest, overwrite, None).await
}

async fn waiting(env: &Env, path: &str) -> Vec<Value> {
    env.jobs(path).await.into_iter().filter(|j| j["state"] == "waiting").collect()
}

fn tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut tar_bytes = Vec::new();
    {
        let mut b = tar::Builder::new(&mut tar_bytes);
        for (name, content) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_path(name).unwrap();
            h.set_size(content.len() as u64);
            h.set_cksum();
            b.append(&h, *content).unwrap();
        }
        b.finish().unwrap();
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&tar_bytes).unwrap();
    gz.finish().unwrap()
}

const FIVE: [(&str, &[u8]); 5] = [
    ("a.md", b"aword"),
    ("b.md", b"bword"),
    ("sub/c.md", b"cword"),
    ("sub/d.md", b"dword"),
    ("e.md", b"eword"),
];

/// SPEC-0112/E2E-012: the five documents of an archive become searchable.
#[tokio::test(start_paused = true)]
async fn e2e_0112_012_five_extracted_documents_are_searchable() {
    for (name, bytes) in [("pack.zip", build_zip(&FIVE, None)), ("pack.tar.gz", tar_gz(&FIVE))] {
        let env = env().await;
        put(&env, &format!("/up/{name}"), &bytes).await;
        extract(&env, &format!("/up/{name}"), None, false).await.unwrap();
        let stem = name.split('.').next().unwrap();
        env.at(10).await;
        assert!(
            !env.finds("aword", &format!("/up/{stem}/a.md")).await,
            "{name}: not before the delay"
        );
        env.at(100).await;
        for (entry, text) in FIVE {
            let word = std::str::from_utf8(text).unwrap();
            assert!(env.finds(word, &format!("/up/{stem}/{entry}")).await, "{name}: {word}");
        }
    }
}

/// SPEC-0112/E2E-013: a project whose index mode is none schedules nothing.
#[tokio::test(start_paused = true)]
async fn e2e_0112_013_mode_none_schedules_nothing_for_an_extraction() {
    let env = env().await;
    put(&env, "/up/pack.zip", &build_zip(&FIVE, None)).await;
    put(&env, "/up/one.md.gz", &gz(b"oneword")).await;
    env.state.admin.set_index_mode(PROJ, IndexMode::None).await.unwrap();
    extract(&env, "/up/pack.zip", None, false).await.unwrap();
    extract(&env, "/up/one.md.gz", None, false).await.unwrap();
    for p in ["/up/pack/a.md", "/up/pack/e.md", "/up/one.md"] {
        assert!(waiting(&env, p).await.is_empty(), "{p}: {:?}", env.jobs(p).await);
    }
    env.at(100).await;
    assert!(!env.finds("aword", "/up/pack/a.md").await);
    assert!(!env.finds("oneword", "/up/one.md").await);
}

/// SPEC-0112/E2E-014: an extraction that fails halfway schedules only the
/// files it wrote.
#[tokio::test(start_paused = true)]
async fn e2e_0112_014_a_failure_halfway_schedules_the_files_written() {
    let env = env().await;
    let zip = build_zip(&[("a.md", b"aword"), ("d/b.md", b"bword"), ("z.md", b"zword")], None);
    put(&env, "/up/pack.zip", &zip).await;
    // `/up/out/d` is a file, so the second entry cannot get its parent.
    put(&env, "/up/out/d", b"in the way").await;
    let err = extract(&env, "/up/pack.zip", Some("/up/out"), false).await.unwrap_err();
    assert!(!err.message.is_empty());
    let c = env.state.stores.client(PROJ).await.unwrap();
    assert!(c.exists("/up/out/a.md").await.unwrap(), "the first entry was written");
    assert!(!c.exists("/up/out/z.md").await.unwrap(), "the last entry was not");
    assert_eq!(waiting(&env, "/up/out/a.md").await.len(), 1);
    for p in ["/up/out/d/b.md", "/up/out/z.md", "/up/out/d"] {
        assert!(env.jobs(p).await.is_empty(), "{p}: {:?}", env.jobs(p).await);
    }
    env.at(100).await;
    assert!(env.finds("aword", "/up/out/a.md").await);
}

/// SPEC-0112/E2E-015: a quota refusal schedules nothing.
#[tokio::test(start_paused = true)]
async fn e2e_0112_015_a_quota_refusal_schedules_nothing() {
    let env = env().await;
    let big = "x ".repeat(4000);
    put(&env, "/up/pack.zip", &build_zip(&[("a.md", big.as_bytes())], None)).await;
    put(&env, "/up/one.md.gz", &gz(big.as_bytes())).await;
    env.state.admin.set_quota(PROJ, Some(200)).await.unwrap();
    assert!(extract(&env, "/up/pack.zip", None, false).await.is_err());
    assert!(extract(&env, "/up/one.md.gz", None, false).await.is_err());
    for p in ["/up/pack/a.md", "/up/one.md"] {
        assert!(env.jobs(p).await.is_empty(), "{p}: {:?}", env.jobs(p).await);
    }
}

fn gz(b: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

fn bz(b: &[u8]) -> Vec<u8> {
    let mut e = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

fn xz(b: &[u8]) -> Vec<u8> {
    let mut e = liblzma::write::XzEncoder::new(Vec::new(), 6);
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

/// SPEC-0112/E2E-016: the single file formats follow the index.
#[tokio::test(start_paused = true)]
async fn e2e_0112_016_single_file_formats_are_searchable() {
    let env = env().await;
    for (round, (name, bytes, word)) in [
        ("g.md.gz", gz(b"gzword"), "gzword"),
        ("b.md.bz2", bz(b"bzword"), "bzword"),
        ("x.md.xz", xz(b"xzword"), "xzword"),
    ]
    .into_iter()
    .enumerate()
    {
        put(&env, &format!("/up/{name}"), &bytes).await;
        extract(&env, &format!("/up/{name}"), None, false).await.unwrap();
        let out = format!("/up/{}", name.rsplit_once('.').unwrap().0);
        assert_eq!(waiting(&env, &out).await.len(), 1, "{name}");
        // `at` is absolute time: each round settles 100 s after the last.
        env.at(100 * (round as u64 + 1)).await;
        assert!(env.finds(word, &out).await, "{name}: {word} in {out}");
    }
}

/// SPEC-0112/E2E-017: `overwrite=true` replacing a file counts as a change.
#[tokio::test(start_paused = true)]
async fn e2e_0112_017_an_overwritten_file_is_rebuilt() {
    let env = env().await;
    env.seed("/up/pack/a.md", "oldword").await;
    env.seed("/up/one.md", "oldone").await;
    put(&env, "/up/pack.zip", &build_zip(&[("a.md", b"newword")], None)).await;
    put(&env, "/up/one.md.gz", &gz(b"newone")).await;
    extract(&env, "/up/pack.zip", None, true).await.unwrap();
    extract(&env, "/up/one.md.gz", None, true).await.unwrap();
    env.at(100).await;
    assert!(env.finds("newword", "/up/pack/a.md").await);
    assert!(!env.finds("oldword", "/up/pack/a.md").await);
    assert!(env.finds("newone", "/up/one.md").await);
    assert!(!env.finds("oldone", "/up/one.md").await);
}
