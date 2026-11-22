//! SPEC-0101 end to end tests. Later stories extend this module.

use super::ChunkDefaults;
use crate::config::ServerConfig;

fn start(yaml: &str) -> Result<ChunkDefaults, String> {
    // `from_yaml` installs the process wide git host map; hold the lock the config tests use.
    let _guard = crate::git::remote::tests::lock_for_test();
    ServerConfig::from_yaml(yaml).map(|c| c.search.chunking()).map_err(|e| e.to_string())
}

fn key(enabled: bool, name: &str, value: &str) -> String {
    format!("search:\n  enabled: {enabled}\n  chunking:\n    {name}: {value}\n")
}

fn err(name: &str, min: u32, max: u32) -> String {
    format!(
        "ERR_INVALID_ARGUMENT: search.chunking.{name} must be a whole number between {min} and {max}"
    )
}

/// SPEC-0101/E2E-131: a default out of range stops the start; 6 starts.
#[test]
fn e2e_0101_131_default_out_of_range() {
    let e = start(&key(true, "heading_cut_depth", "9")).unwrap_err();
    assert!(e.contains(&err("heading_cut_depth", 1, 6)), "got {e}");
    assert_eq!(start(&key(true, "heading_cut_depth", "6")).unwrap().heading_cut_depth, 6);
    for (name, bad, min, max) in [
        ("fixed_size_tokens", "31", 32, 8192),
        ("fixed_size_tokens", "8193", 32, 8192),
        ("fixed_overlap_tokens", "8192", 0, 8191),
        ("fixed_overlap_tokens", "-1", 0, 8191),
        ("max_tokens", "63", 64, 8192),
        ("list_max_items", "1", 2, 200),
        ("list_max_items", "201", 2, 200),
        ("heading_cut_depth", "0", 1, 6),
    ] {
        let e = start(&key(true, name, bad)).unwrap_err();
        assert!(e.contains(&err(name, min, max)), "{name}={bad}: got {e}");
    }
}

/// SPEC-0101/E2E-132: abc and 2.5 are refused, even with search disabled.
#[test]
fn e2e_0101_132_default_not_a_whole_number() {
    for enabled in [true, false] {
        for bad in ["abc", "2.5"] {
            let e = start(&key(enabled, "max_tokens", bad)).unwrap_err();
            assert!(e.contains(&err("max_tokens", 64, 8192)), "{enabled} {bad}: got {e}");
        }
    }
    assert_eq!(start(&key(false, "max_tokens", "'512'")).unwrap().max_tokens, 512);
}

/// SPEC-0101/E2E-145: an overlap not below the size stops the start; 99 starts.
#[test]
fn e2e_0101_145_overlap_not_below_size() {
    let both = |o: &str| {
        format!("search:\n  chunking:\n    fixed_size_tokens: 100\n    fixed_overlap_tokens: {o}\n")
    };
    let e = start(&both("100")).unwrap_err();
    assert!(
        e.contains("ERR_INVALID_ARGUMENT: search.chunking.fixed_overlap_tokens must be smaller than search.chunking.fixed_size_tokens"),
        "got {e}"
    );
    assert_eq!(start(&both("99")).unwrap().fixed_overlap_tokens, 99);
    // The size default 2048 is the reference when only the overlap is set.
    let e = start(&key(false, "fixed_overlap_tokens", "2048")).unwrap_err();
    assert!(e.contains("must be smaller than"), "got {e}");
}

/// SPEC-0101/E2E-133: absent and empty keys read as the defaults.
#[test]
fn e2e_0101_133_absent_and_empty_are_defaults() {
    let want = ChunkDefaults {
        fixed_size_tokens: 2048,
        fixed_overlap_tokens: 0,
        heading_cut_depth: 2,
        max_tokens: 2048,
        list_max_items: 40,
    };
    assert_eq!(start("search:\n  enabled: true\n").unwrap(), want);
    assert_eq!(
        start("search:\n  chunking:\n    max_tokens: ''\n    list_max_items:\n").unwrap(),
        want
    );
    let set = start(&key(true, "list_max_items", "7")).unwrap();
    assert_eq!(set.list_max_items, 7);
    assert_eq!(set.max_tokens, 2048);
}

// ── US-0275: chunk set store and the read tools ─────────────────────────────

use super::model::{Chunk, Set};
use crate::search::derived_e2e::{ALICE, DAVE, Env, Options, PROJ, args, tool_json};
use serde_json::{Value, json};

/// The environment of the read tool tests: `proj` indexing in mode `bm25`,
/// settle delay 3 s, Alice a member, Dave not.
async fn env() -> Env {
    Env::new(Options { delay: 3, ..Options::default() }).await
}

/// Writes `path` (so it exists) and returns its current `sha256`.
async fn file(env: &Env, path: &str, text: &str) -> String {
    let client = env.state.stores.client(PROJ).await.unwrap();
    client.write_text_atomic(path, text).await.unwrap();
    client.stat(path).await.unwrap().sha256.unwrap_or_default()
}

fn chunk(number: u32, text: String) -> Chunk {
    Chunk {
        number,
        unit: number.div_ceil(2),
        text,
        heading_path: vec!["Big".into(), format!("Part {}", number.div_ceil(10))],
        page_start: Some(number),
        page_end: Some(number + 1),
        token_count: 7,
        figures: if number.is_multiple_of(5) { vec![format!("image-{number}")] } else { vec![] },
        document_title: "big".into(),
    }
}

fn text_of(number: u32) -> String {
    format!("Chunk number {number} of the big document.")
}

/// Stores a set of `chunks` on `path` through `ChunkStore`, current against
/// `sha`, and returns its id.
async fn seed_set(
    env: &Env,
    path: &str,
    set_id: &str,
    is_default: bool,
    built_at: &str,
    sha: &str,
    chunks: Vec<Chunk>,
) -> String {
    let client = env.state.stores.client(PROJ).await.unwrap();
    let store = client.meta.chunk_store().expect("the relational meta store holds chunk sets");
    let set = Set {
        set_id: set_id.into(),
        path: path.into(),
        request_key: format!(r#"{{"strategy":"structural","set":"{set_id}"}}"#),
        strategy: "structural".into(),
        settings: r#"{"heading_cut_depth":2,"list_max_items":40,"max_tokens":64}"#.into(),
        requested_strategy: "structural".into(),
        requested_settings: "{}".into(),
        is_default,
        fallback_from: None,
        chunk_count: 0,
        token_count: 0,
        source_sha: sha.into(),
        built_at: built_at.into(),
    };
    store.insert_set(&set, &chunks).await.unwrap();
    set_id.to_string()
}

/// `/big.md` with a default set of `n` chunks; returns the set id.
async fn big(env: &Env, n: u32) -> String {
    let sha = file(env, "/big.md", "# Big\n\nbody\n").await;
    let chunks = (1..=n).map(|i| chunk(i, text_of(i))).collect();
    seed_set(env, "/big.md", "set-0000d0f1", true, "2026-11-20T10:00:00Z", &sha, chunks).await
}

async fn list_sets(env: &Env, person: &str, mount: &str, path: &str) -> Result<Value, String> {
    let a = args(json!({"mount_id": mount, "path": path}));
    tool_json(env.server(person).fs_list_chunk_sets(a).await)
}

async fn list(env: &Env, set_id: &str, marker: &str) -> Result<Value, String> {
    let a = args(json!({"mount_id": PROJ, "path": "/big.md", "set_id": set_id, "marker": marker}));
    tool_json(env.server(ALICE).fs_list_chunks(a).await)
}

async fn get(
    env: &Env,
    set_id: &str,
    number: i64,
    before: i64,
    after: i64,
) -> Result<Value, String> {
    let a = args(json!({
        "mount_id": PROJ, "path": "/big.md", "set_id": set_id,
        "number": number, "before": before, "after": after,
    }));
    tool_json(env.server(ALICE).fs_get_chunk(a).await)
}

fn numbers(v: &Value) -> Vec<u64> {
    v["chunks"]
        .as_array()
        .unwrap_or_else(|| panic!("chunks is an array: {v}"))
        .iter()
        .map(|c| c["number"].as_u64().unwrap())
        .collect()
}

fn assert_refused(got: Result<Value, String>, want: &str) {
    let e = got.expect_err("the request must be refused");
    assert!(e.ends_with(want), "want {want}, got: {e}");
}

/// SPEC-0101/E2E-010: 250 chunks are listed 100 at a time with a marker.
#[tokio::test(start_paused = true)]
async fn e2e_0101_010_listing_sets_and_paging_through_chunks() {
    let env = env().await;
    let id = big(&env, 250).await;

    let started = std::time::Instant::now();
    let first = list(&env, &id, "").await.unwrap();
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(2), "first page took {took:?}");
    assert_eq!(numbers(&first), (1..=100).collect::<Vec<u64>>());
    let c = &first["chunks"][4];
    assert_eq!(c["number"], 5);
    assert_eq!(c["unit"], 3);
    assert_eq!(c["heading_path"], json!(["Big", "Part 1"]));
    assert_eq!((c["page_start"].clone(), c["page_end"].clone()), (json!(5), json!(6)));
    assert_eq!(c["token_count"], 7);
    assert_eq!(c["figures"], json!(["image-5"]));
    assert_eq!(c["preview"], text_of(5));
    let m1 = first["marker"].as_str().expect("a marker when chunks remain");

    let second = list(&env, &id, m1).await.unwrap();
    assert_eq!(numbers(&second), (101..=200).collect::<Vec<u64>>());
    let m2 = second["marker"].as_str().expect("a second marker");

    let third = list(&env, &id, m2).await.unwrap();
    assert_eq!(numbers(&third), (201..=250).collect::<Vec<u64>>());
    assert!(third.get("marker").is_none(), "no marker on the last page: {third}");

    let sets = list_sets(&env, ALICE, PROJ, "/big.md").await.unwrap();
    assert_eq!(sets["sets"][0]["chunk_count"], 250);
    assert_eq!(sets["sets"][0]["token_count"], 250 * 7);
}

/// SPEC-0101/E2E-011: chunk 5 with 2 before and 1 after.
#[tokio::test(start_paused = true)]
async fn e2e_0101_011_a_chunk_with_its_neighbours() {
    let env = env().await;
    let id = big(&env, 20).await;
    let v = get(&env, &id, 5, 2, 1).await.unwrap();
    assert_eq!(numbers(&v), [3, 4, 5, 6]);
    for c in v["chunks"].as_array().unwrap() {
        let n = c["number"].as_u64().unwrap() as u32;
        assert_eq!(c["text"], text_of(n));
        assert_eq!(c["document_title"], "big");
        assert_eq!(c["heading_path"], json!(["Big", "Part 1"]));
        assert_eq!(c["unit"], n.div_ceil(2));
        assert_eq!(c["page_start"], n);
        assert_eq!(c["page_end"], n + 1);
        assert_eq!(c["token_count"], 7);
        assert!(c["figures"].is_array());
        assert!(c.get("preview").is_none());
    }
    let alone = get(&env, &id, 5, 0, 0).await.unwrap();
    assert_eq!(numbers(&alone), [5]);
    assert_eq!(alone["chunks"][0]["text"], v["chunks"][2]["text"]);
}

/// SPEC-0101/E2E-012: neighbours beyond the first and last chunk are absent.
#[tokio::test(start_paused = true)]
async fn e2e_0101_012_a_window_beyond_the_ends() {
    let env = env().await;
    let id = big(&env, 20).await;
    assert_eq!(numbers(&get(&env, &id, 1, 3, 0).await.unwrap()), [1]);
    assert_eq!(numbers(&get(&env, &id, 20, 0, 2).await.unwrap()), [20]);
    assert_eq!(numbers(&get(&env, &id, 2, 3, 1).await.unwrap()), [1, 2, 3], "in order");
}

/// SPEC-0101/E2E-013: two windows asked one after the other join without change.
#[tokio::test(start_paused = true)]
async fn e2e_0101_013_widening_a_window_by_asking_again() {
    let env = env().await;
    let id = big(&env, 20).await;
    let a = get(&env, &id, 10, 1, 1).await.unwrap();
    let b = get(&env, &id, 8, 2, 0).await.unwrap();
    assert_eq!(numbers(&a), [9, 10, 11]);
    assert_eq!(numbers(&b), [6, 7, 8]);
    let mut union: Vec<Value> = b["chunks"].as_array().unwrap().clone();
    union.extend(a["chunks"].as_array().unwrap().iter().cloned());
    for c in &union {
        let n = c["number"].as_u64().unwrap() as u32;
        let alone = get(&env, &id, i64::from(n), 0, 0).await.unwrap();
        assert_eq!(&alone["chunks"][0], c, "chunk {n} unchanged");
    }
    let nums: Vec<u64> = union.iter().map(|c| c["number"].as_u64().unwrap()).collect();
    assert_eq!(nums, [6, 7, 8, 9, 10, 11]);
}

/// SPEC-0101/E2E-014: a short chunk previews whole, a long one its first 200
/// characters; no summary carries the full text.
#[tokio::test(start_paused = true)]
async fn e2e_0101_014_the_preview_of_a_short_and_of_a_long_chunk() {
    let env = env().await;
    let sha = file(&env, "/big.md", "# Big\n").await;
    let short = "s".repeat(50);
    let long: String = (0..3000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
    let id = seed_set(
        &env,
        "/big.md",
        "set-0000aaaa",
        true,
        "2026-11-20T10:00:00Z",
        &sha,
        vec![chunk(1, short.clone()), chunk(2, long.clone())],
    )
    .await;
    let v = list(&env, &id, "").await.unwrap();
    assert_eq!(v["chunks"][0]["preview"], short);
    assert_eq!(v["chunks"][1]["preview"], long[..200]);
    for c in v["chunks"].as_array().unwrap() {
        assert!(c.get("text").is_none(), "a summary carries no text: {c}");
    }
}

/// SPEC-0101/E2E-015: an unknown set is refused and nothing changes.
#[tokio::test(start_paused = true)]
async fn e2e_0101_015_an_unknown_set() {
    let env = env().await;
    big(&env, 3).await;
    let before = list_sets(&env, ALICE, PROJ, "/big.md").await.unwrap();
    assert_refused(list(&env, "set-999", "").await, "ERR_NOT_FOUND: chunk set not found: set-999");
    assert_refused(
        get(&env, "set-999", 1, 0, 0).await,
        "ERR_NOT_FOUND: chunk set not found: set-999",
    );
    assert_eq!(list_sets(&env, ALICE, PROJ, "/big.md").await.unwrap(), before);
}

/// SPEC-0101/E2E-016: chunk 99 and chunk 0 do not exist.
#[tokio::test(start_paused = true)]
async fn e2e_0101_016_a_chunk_number_that_does_not_exist() {
    let env = env().await;
    let id = big(&env, 20).await;
    let before = list_sets(&env, ALICE, PROJ, "/big.md").await.unwrap();
    for n in [99, 0] {
        let want = format!("ERR_NOT_FOUND: chunk {n} not found in set {id}");
        assert_refused(get(&env, &id, n, 0, 0).await, &want);
    }
    assert_eq!(list_sets(&env, ALICE, PROJ, "/big.md").await.unwrap(), before);
}

/// SPEC-0101/E2E-017: a window above 20 or below 0 is refused; 20 and 20 pass.
#[tokio::test(start_paused = true)]
async fn e2e_0101_017_a_window_above_20() {
    let env = env().await;
    let id = big(&env, 20).await;
    let want = "ERR_INVALID_ARGUMENT: before and after must be between 0 and 20";
    assert_refused(get(&env, &id, 5, 21, 0).await, want);
    assert_refused(get(&env, &id, 5, 0, -1).await, want);
    let all = get(&env, &id, 5, 20, 20).await.unwrap();
    assert_eq!(numbers(&all), (1..=20).collect::<Vec<u64>>());
}

/// SPEC-0101/E2E-018: an unusable marker and a marker of another set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_018_an_unusable_marker() {
    let env = env().await;
    let id = big(&env, 250).await;
    let want = "ERR_INVALID_ARGUMENT: marker is not valid";
    assert_refused(list(&env, &id, "zzz").await, want);

    let sha = file(&env, "/big.md", "# Big\n\nbody\n").await;
    let chunks = (1..=150).map(|i| chunk(i, text_of(i))).collect();
    let other =
        seed_set(&env, "/big.md", "set-0000beef", false, "2026-11-20T10:01:00Z", &sha, chunks)
            .await;
    let foreign = list(&env, &other, "").await.unwrap()["marker"].as_str().unwrap().to_string();
    assert_refused(list(&env, &id, &foreign).await, want);
}

/// SPEC-0101/E2E-019: a file just created, before its rebuild, has no set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_019_a_file_with_no_set() {
    let env = env().await;
    env.save("/new.md", "# New\n\nfresh text\n").await;
    let v = list_sets(&env, ALICE, PROJ, "/new.md").await.unwrap();
    assert_eq!(v, json!({"sets": []}));
    let jobs = env.jobs("/new.md").await;
    assert!(jobs.iter().all(|j| j["state"] != "failed"), "no failed job: {jobs:?}");
}

/// SPEC-0101/E2E-020: the default set first, then oldest first, ties by set id.
#[tokio::test(start_paused = true)]
async fn e2e_0101_020_order_of_the_sets_of_a_file() {
    let env = env().await;
    let sha = file(&env, "/big.md", "# Big\n").await;
    let one = || vec![chunk(1, text_of(1))];
    seed_set(&env, "/big.md", "set-000000c5", false, "2026-11-20T10:05:00Z", &sha, one()).await;
    seed_set(&env, "/big.md", "set-000000b2", false, "2026-11-20T10:02:00Z", &sha, one()).await;
    seed_set(&env, "/big.md", "set-000000ff", true, "2026-11-20T10:00:00Z", &sha, one()).await;
    seed_set(&env, "/big.md", "set-000000a2", false, "2026-11-20T10:02:00Z", &sha, one()).await;
    let v = list_sets(&env, ALICE, PROJ, "/big.md").await.unwrap();
    let ids: Vec<&str> =
        v["sets"].as_array().unwrap().iter().map(|s| s["set_id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["set-000000ff", "set-000000a2", "set-000000b2", "set-000000c5"]);
    let d = &v["sets"][0];
    assert_eq!(d["default"], true);
    assert_eq!(d["strategy"], "structural");
    assert_eq!(d["settings"]["max_tokens"], 64);
    assert_eq!(d["state"], "current");
    assert_eq!(d["built_at"], "2026-11-20T10:00:00Z");
    assert!(d.get("fallback_from").is_none());
    assert_eq!(v["sets"][1]["default"], false);
}

/// SPEC-0101/E2E-022: a project that does not exist.
#[tokio::test(start_paused = true)]
async fn e2e_0101_022_a_project_that_does_not_exist() {
    let env = env().await;
    let e = list_sets(&env, ALICE, "ghost", "/a.md").await.unwrap_err();
    assert!(e.contains("ERR_PROJECT_NOT_FOUND"), "got: {e}");
    assert!(env.state.admin.get_project("ghost").await.unwrap().is_none(), "nothing created");
}

/// SPEC-0101/E2E-023: an unknown path, then a folder, in that order.
#[tokio::test(start_paused = true)]
async fn e2e_0101_023_an_unknown_path_and_a_folder() {
    let env = env().await;
    let client = env.state.stores.client(PROJ).await.unwrap();
    client.makedirs("/docs", true).await.unwrap();
    assert_refused(
        list_sets(&env, ALICE, PROJ, "/nope.md").await,
        "ERR_NOT_FOUND: file not found: /nope.md",
    );
    for folder in ["/docs", "/"] {
        assert_refused(
            list_sets(&env, ALICE, PROJ, folder).await,
            "ERR_INVALID_ARGUMENT: path must be a file",
        );
    }
    // The path check comes before the set check of the other tools.
    let a = args(json!({"mount_id": PROJ, "path": "/nope.md", "set_id": "set-1"}));
    assert_refused(
        tool_json(env.server(ALICE).fs_list_chunks(a).await),
        "ERR_NOT_FOUND: file not found: /nope.md",
    );
    let a = args(json!({"mount_id": PROJ, "path": "/docs", "set_id": "set-1", "number": 1}));
    assert_refused(
        tool_json(env.server(ALICE).fs_get_chunk(a).await),
        "ERR_INVALID_ARGUMENT: path must be a file",
    );
}

/// FR-NEW-028 and FR-NEW-014 story checks: the three tools refuse a non member
/// before anything else; a set of another file is not reachable through this
/// one; a changed file reads its set as out of date.
#[tokio::test(start_paused = true)]
async fn e2e_0101_us0275_members_only_other_file_and_state() {
    let env = env().await;
    let id = big(&env, 3).await;
    let forbidden = "ERR_FORBIDDEN: 'dave@example.com' is not a member of 'proj'";
    assert_refused(list_sets(&env, DAVE, PROJ, "/nope.md").await, forbidden);
    let a = args(json!({"mount_id": PROJ, "path": "/big.md", "set_id": id}));
    assert_refused(tool_json(env.server(DAVE).fs_list_chunks(a).await), forbidden);
    let a = args(json!({"mount_id": PROJ, "path": "/big.md", "set_id": id, "number": 1}));
    assert_refused(tool_json(env.server(DAVE).fs_get_chunk(a).await), forbidden);

    file(&env, "/other.md", "# Other\n").await;
    let a = args(json!({"mount_id": PROJ, "path": "/other.md", "set_id": id}));
    assert_refused(
        tool_json(env.server(ALICE).fs_list_chunks(a).await),
        &format!("ERR_NOT_FOUND: chunk set not found: {id}"),
    );

    file(&env, "/big.md", "# Big\n\nchanged\n").await;
    let v = list_sets(&env, ALICE, PROJ, "/big.md").await.unwrap();
    assert_eq!(v["sets"][0]["state"], "out_of_date");
}

// ── US-0276: creating and deleting sets, and the fixed strategy ─────────────

use super::tokens;
use crate::search::derived_e2e::CAROL;

/// Prose of exactly `n` `cl100k_base` tokens. Its words are one token each,
/// so a window boundary never falls inside a word.
pub(crate) fn prose(n: usize) -> String {
    let words = ["the", " cat", " sat", " on", " a", " mat", " and", " ran", "."];
    let s: String = (0..2 * n + 16).map(|i| words[i % words.len()]).collect();
    let toks = tokens::encode(&s);
    let out = tokens::decode_lossy(&toks[..n]);
    assert_eq!(tokens::count(&out), n, "the helper text is token exact");
    out
}

async fn create(
    env: &Env,
    person: &str,
    path: &str,
    strategy: &str,
    settings: Value,
) -> Result<Value, String> {
    let a =
        args(json!({"mount_id": PROJ, "path": path, "strategy": strategy, "settings": settings}));
    tool_json(env.server(person).fs_create_chunk_set(a).await)
}

async fn delete(env: &Env, person: &str, path: &str, set_id: &str) -> Result<Value, String> {
    let a = args(json!({"mount_id": PROJ, "path": path, "set_id": set_id}));
    tool_json(env.server(person).fs_delete_chunk_set(a).await)
}

async fn sets_of(env: &Env, path: &str) -> Value {
    list_sets(env, ALICE, PROJ, path).await.unwrap()
}

fn set_count(v: &Value) -> usize {
    v["sets"].as_array().map(Vec::len).unwrap_or(0)
}

/// Every chunk of `set_id` on `path`, in full, through the read tools.
async fn all_chunks(env: &Env, path: &str, set_id: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut marker = String::new();
    loop {
        let a = args(json!({"mount_id": PROJ, "path": path, "set_id": set_id, "marker": marker}));
        let page = tool_json(env.server(ALICE).fs_list_chunks(a).await).unwrap();
        for c in page["chunks"].as_array().unwrap() {
            let a = args(
                json!({"mount_id": PROJ, "path": path, "set_id": set_id, "number": c["number"]}),
            );
            let full = tool_json(env.server(ALICE).fs_get_chunk(a).await).unwrap();
            out.push(full["chunks"][0].clone());
        }
        match page["marker"].as_str() {
            Some(m) => marker = m.to_string(),
            None => return out,
        }
    }
}

/// `/essay.md` holding `n` tokens of prose and a default set; returns the
/// file text and the default set id.
async fn essay(env: &Env, n: usize) -> (String, String) {
    let text = prose(n);
    let sha = file(env, "/essay.md", &text).await;
    let id = seed_set(
        env,
        "/essay.md",
        "set-0000def0",
        true,
        "2026-11-20T10:00:00Z",
        &sha,
        vec![chunk(1, text_of(1))],
    )
    .await;
    (text, id)
}

/// Checks a fixed set against the slices of the ruler: chunk `i` is the
/// decoding of tokens `i * step` to `i * step + size`.
fn assert_fixed(chunks: &[Value], text: &str, size: usize, step: usize) {
    let toks = tokens::encode(text);
    for (i, c) in chunks.iter().enumerate() {
        let start = i * step;
        let end = (start + size).min(toks.len());
        assert_eq!(c["text"], tokens::decode_lossy(&toks[start..end]), "chunk {}", i + 1);
        assert_eq!(c["token_count"], end - start);
        assert!(tokens::count(c["text"].as_str().unwrap()) <= size, "chunk {} too long", i + 1);
        assert_eq!(c["number"], i + 1);
        assert_eq!(c["unit"], i + 1);
        assert_eq!(c["heading_path"], json!([]));
        assert_eq!((c["page_start"].clone(), c["page_end"].clone()), (Value::Null, Value::Null));
        assert_eq!(c["figures"], json!([]));
        assert_eq!(c["document_title"], "essay");
    }
    assert_eq!(
        chunks
            .last()
            .map(|c| (chunks.len() - 1) * step + c["token_count"].as_u64().unwrap() as usize),
        Some(toks.len()),
        "the last chunk reaches the end"
    );
}

/// SPEC-0101/E2E-021: a person who is not a member is refused by every tool.
#[tokio::test(start_paused = true)]
async fn e2e_0101_021_a_person_who_is_not_a_member() {
    let env = env().await;
    let id = big(&env, 3).await;
    let before = sets_of(&env, "/big.md").await;
    let forbidden = "ERR_FORBIDDEN: 'dave@example.com' is not a member of 'proj'";
    assert_refused(list_sets(&env, DAVE, PROJ, "/big.md").await, forbidden);
    let a = args(json!({"mount_id": PROJ, "path": "/big.md", "set_id": id}));
    assert_refused(tool_json(env.server(DAVE).fs_list_chunks(a).await), forbidden);
    let a = args(json!({"mount_id": PROJ, "path": "/big.md", "set_id": id, "number": 1}));
    assert_refused(tool_json(env.server(DAVE).fs_get_chunk(a).await), forbidden);
    assert_refused(create(&env, DAVE, "/big.md", "fixed", json!({})).await, forbidden);
    assert_refused(delete(&env, DAVE, "/big.md", &id).await, forbidden);
    assert_eq!(sets_of(&env, "/big.md").await, before, "no set created or removed");
}

/// SPEC-0101/E2E-030: a fixed set of 1000 tokens with 200 of overlap by hand.
#[tokio::test(start_paused = true)]
async fn e2e_0101_030_a_fixed_set_with_overlap_by_hand() {
    let env = env().await;
    let (text, default_id) = essay(&env, 3000).await;
    let set = create(
        &env,
        ALICE,
        "/essay.md",
        "fixed",
        json!({"size_tokens": 1000, "overlap_tokens": 200}),
    )
    .await
    .unwrap();
    assert_eq!(set["default"], false);
    assert_eq!(set["strategy"], "fixed");
    assert_eq!(set["settings"], json!({"overlap_tokens": 200, "size_tokens": 1000}));
    assert_eq!(set["chunk_count"], 4);
    assert_eq!(set["state"], "current");
    assert!(set.get("fallback_from").is_none());
    let id = set["set_id"].as_str().unwrap();
    assert!(id.starts_with("set-") && id.len() == 12, "{id}");
    let chunks = all_chunks(&env, "/essay.md", id).await;
    assert_eq!(chunks.len(), 4);
    assert_fixed(&chunks, &text, 1000, 800);
    let tokens: u64 = chunks.iter().map(|c| c["token_count"].as_u64().unwrap()).sum();
    assert_eq!(set["token_count"], tokens);

    let v = sets_of(&env, "/essay.md").await;
    let ids: Vec<&str> =
        v["sets"].as_array().unwrap().iter().map(|s| s["set_id"].as_str().unwrap()).collect();
    assert_eq!(ids, [default_id.as_str(), id]);
    assert_eq!(v["sets"][1], set);
}

/// SPEC-0101/E2E-031: creating the same set twice answers the same set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_031_creating_the_same_set_twice() {
    let env = env().await;
    essay(&env, 1200).await;
    let a = create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": 500})).await.unwrap();
    let b = create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": 500})).await.unwrap();
    assert_eq!(a["set_id"], b["set_id"]);
    // The defaults spelled out are the same request.
    let c =
        create(&env, ALICE, "/essay.md", "fixed", json!({"overlap_tokens": 0, "size_tokens": 500}))
            .await
            .unwrap();
    assert_eq!(a["set_id"], c["set_id"]);
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 2, "one extra set");
}

/// SPEC-0101/E2E-033: an unknown strategy.
#[tokio::test(start_paused = true)]
async fn e2e_0101_033_an_unknown_strategy() {
    let env = env().await;
    essay(&env, 100).await;
    assert_refused(
        create(&env, ALICE, "/essay.md", "sentence", json!({})).await,
        "ERR_INVALID_ARGUMENT: strategy must be 'fixed' or 'structural'",
    );
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 1);
}

/// SPEC-0101/E2E-034: a setting that does not apply to the strategy.
#[tokio::test(start_paused = true)]
async fn e2e_0101_034_a_setting_that_does_not_apply() {
    let env = env().await;
    essay(&env, 100).await;
    assert_refused(
        create(&env, ALICE, "/essay.md", "structural", json!({"overlap_tokens": 50})).await,
        "ERR_INVALID_ARGUMENT: setting 'overlap_tokens' does not apply to strategy 'structural'",
    );
    assert_refused(
        create(&env, ALICE, "/essay.md", "fixed", json!({"heading_cut_depth": 3})).await,
        "ERR_INVALID_ARGUMENT: setting 'heading_cut_depth' does not apply to strategy 'fixed'",
    );
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 1);
}

/// SPEC-0101/E2E-035: a size out of range; the bounds themselves pass.
#[tokio::test(start_paused = true)]
async fn e2e_0101_035_a_setting_out_of_range() {
    let env = env().await;
    essay(&env, 100).await;
    for bad in [31, 8193] {
        assert_refused(
            create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": bad})).await,
            "ERR_INVALID_ARGUMENT: size_tokens must be between 32 and 8192",
        );
    }
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 1);
    for good in [32, 8192] {
        let s =
            create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": good})).await.unwrap();
        assert_eq!(s["settings"]["size_tokens"], good);
    }
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 3);
}

/// SPEC-0101/E2E-036: an overlap not below the size; 99 passes.
#[tokio::test(start_paused = true)]
async fn e2e_0101_036_an_overlap_not_below_the_size() {
    let env = env().await;
    essay(&env, 300).await;
    assert_refused(
        create(
            &env,
            ALICE,
            "/essay.md",
            "fixed",
            json!({"size_tokens": 100, "overlap_tokens": 100}),
        )
        .await,
        "ERR_INVALID_ARGUMENT: overlap_tokens must be smaller than size_tokens",
    );
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 1);
    let s = create(
        &env,
        ALICE,
        "/essay.md",
        "fixed",
        json!({"size_tokens": 100, "overlap_tokens": 99}),
    )
    .await
    .unwrap();
    // 300 tokens one token apart: windows start at 0 to 200.
    assert_eq!(s["chunk_count"], 201);
}

/// SPEC-0101/E2E-037: a sixth set is refused; a deletion frees a place.
#[tokio::test(start_paused = true)]
async fn e2e_0101_037_a_sixth_set() {
    let env = env().await;
    essay(&env, 400).await;
    let mut ids = Vec::new();
    for size in [100, 200, 300, 400] {
        let s =
            create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": size})).await.unwrap();
        ids.push(s["set_id"].as_str().unwrap().to_string());
    }
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 5);
    assert_refused(
        create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": 500})).await,
        "ERR_INVALID_ARGUMENT: a file holds at most 5 chunk sets",
    );
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 5);
    // An equal request is answered, not counted.
    let same =
        create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": 300})).await.unwrap();
    assert_eq!(same["set_id"], ids[2]);
    assert_eq!(delete(&env, ALICE, "/essay.md", &ids[0]).await.unwrap(), json!({"deleted": true}));
    create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": 500})).await.unwrap();
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 5);
}

/// SPEC-0101/E2E-038: deleting an extra set removes it and its chunks.
#[tokio::test(start_paused = true)]
async fn e2e_0101_038_deleting_an_extra_set() {
    let env = env().await;
    let (_, default_id) = essay(&env, 300).await;
    let default_before = sets_of(&env, "/essay.md").await["sets"][0].clone();
    let default_chunks = all_chunks(&env, "/essay.md", &default_id).await;
    let s = create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": 100})).await.unwrap();
    let id = s["set_id"].as_str().unwrap();
    assert_eq!(delete(&env, ALICE, "/essay.md", id).await.unwrap(), json!({"deleted": true}));
    let v = sets_of(&env, "/essay.md").await;
    assert_eq!(v["sets"], json!([default_before]), "the default set only, unchanged");
    let a = args(json!({"mount_id": PROJ, "path": "/essay.md", "set_id": id}));
    assert_refused(
        tool_json(env.server(ALICE).fs_list_chunks(a).await),
        &format!("ERR_NOT_FOUND: chunk set not found: {id}"),
    );
    assert_eq!(all_chunks(&env, "/essay.md", &default_id).await, default_chunks);
    // The chunk rows went with the set.
    let store = env.state.stores.client(PROJ).await.unwrap().meta.chunk_store().unwrap();
    assert!(store.list_chunks(id, 0, 10).await.unwrap().is_empty());
}

/// SPEC-0101/E2E-039: the default set cannot be deleted.
#[tokio::test(start_paused = true)]
async fn e2e_0101_039_deleting_the_default_set() {
    let env = env().await;
    let (_, default_id) = essay(&env, 100).await;
    let before = sets_of(&env, "/essay.md").await;
    assert_refused(
        delete(&env, ALICE, "/essay.md", &default_id).await,
        "ERR_INVALID_ARGUMENT: the default chunk set cannot be deleted",
    );
    assert_eq!(sets_of(&env, "/essay.md").await, before);
}

/// SPEC-0101/E2E-040: deleting an unknown set, or a set of another file.
#[tokio::test(start_paused = true)]
async fn e2e_0101_040_deleting_an_unknown_set() {
    let env = env().await;
    essay(&env, 100).await;
    let before = sets_of(&env, "/essay.md").await;
    assert_refused(
        delete(&env, ALICE, "/essay.md", "set-999").await,
        "ERR_NOT_FOUND: chunk set not found: set-999",
    );
    file(&env, "/other.md", &prose(50)).await;
    let other =
        create(&env, ALICE, "/other.md", "fixed", json!({"size_tokens": 32})).await.unwrap();
    let other_id = other["set_id"].as_str().unwrap();
    assert_refused(
        delete(&env, ALICE, "/essay.md", other_id).await,
        &format!("ERR_NOT_FOUND: chunk set not found: {other_id}"),
    );
    assert_eq!(sets_of(&env, "/essay.md").await, before);
    assert_eq!(set_count(&sets_of(&env, "/other.md").await), 1);
}

/// SPEC-0101/E2E-041: an image holds no text to cut.
#[tokio::test(start_paused = true)]
async fn e2e_0101_041_no_text_to_cut() {
    let env = env().await;
    let client = env.state.stores.client(PROJ).await.unwrap();
    client.write_bytes_atomic("/logo.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\xff\xfe").await.unwrap();
    assert_refused(
        create(&env, ALICE, "/logo.png", "fixed", json!({})).await,
        "ERR_NOT_SUPPORTED: no text to cut for /logo.png",
    );
    assert_eq!(set_count(&sets_of(&env, "/logo.png").await), 0);
}

/// SPEC-0101/E2E-042: two members create the same set together.
#[tokio::test(start_paused = true)]
async fn e2e_0101_042_two_members_create_the_same_set_together() {
    let env = env().await;
    essay(&env, 1200).await;
    env.state.admin.add_member(PROJ, CAROL, ALICE).await.unwrap();
    let settings = json!({"size_tokens": 500});
    let (a, c) = tokio::join!(
        create(&env, ALICE, "/essay.md", "fixed", settings.clone()),
        create(&env, CAROL, "/essay.md", "fixed", settings.clone()),
    );
    assert_eq!(a.unwrap()["set_id"], c.unwrap()["set_id"]);
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 2, "one extra set");
}

/// SPEC-0101/E2E-044: fixed chunks without and with overlap on 10000 tokens.
#[tokio::test(start_paused = true)]
async fn e2e_0101_044_fixed_chunks_with_and_without_overlap() {
    let env = env().await;
    let (text, _) = essay(&env, 10000).await;
    let a = create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": 2048})).await.unwrap();
    assert_eq!(a["settings"], json!({"overlap_tokens": 0, "size_tokens": 2048}));
    let ca = all_chunks(&env, "/essay.md", a["set_id"].as_str().unwrap()).await;
    assert_eq!(ca.len(), 5);
    assert_eq!(ca[4]["token_count"], 1808);
    assert_fixed(&ca, &text, 2048, 2048);

    let b = create(
        &env,
        ALICE,
        "/essay.md",
        "fixed",
        json!({"size_tokens": 1000, "overlap_tokens": 200}),
    )
    .await
    .unwrap();
    let cb = all_chunks(&env, "/essay.md", b["set_id"].as_str().unwrap()).await;
    assert_eq!(cb.len(), 13);
    assert_fixed(&cb, &text, 1000, 800);
}

/// SPEC-0101/E2E-045: a removed member loses access.
#[tokio::test(start_paused = true)]
async fn e2e_0101_045_a_removed_member_loses_access() {
    let env = env().await;
    let (_, default_id) = essay(&env, 300).await;
    env.state.admin.add_member(PROJ, CAROL, ALICE).await.unwrap();
    let extra =
        create(&env, CAROL, "/essay.md", "fixed", json!({"size_tokens": 100})).await.unwrap();
    env.state.admin.remove_member(PROJ, CAROL).await.unwrap();
    let before = sets_of(&env, "/essay.md").await;
    let forbidden = "ERR_FORBIDDEN: 'carol@example.com' is not a member of 'proj'";
    assert_refused(list_sets(&env, CAROL, PROJ, "/essay.md").await, forbidden);
    assert_refused(
        create(&env, CAROL, "/essay.md", "fixed", json!({"size_tokens": 64})).await,
        forbidden,
    );
    assert_refused(
        delete(&env, CAROL, "/essay.md", extra["set_id"].as_str().unwrap()).await,
        forbidden,
    );
    assert_refused(delete(&env, CAROL, "/essay.md", &default_id).await, forbidden);
    assert_eq!(sets_of(&env, "/essay.md").await, before);
}

/// SPEC-0101/E2E-051: a document never converted has no text to cut.
#[tokio::test(start_paused = true)]
async fn e2e_0101_051_a_document_not_converted_yet() {
    let env = env().await;
    let client = env.state.stores.client(PROJ).await.unwrap();
    client.write_bytes_atomic("/raw.pdf", b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n1 0 obj\n").await.unwrap();
    assert_refused(
        create(&env, ALICE, "/raw.pdf", "fixed", json!({})).await,
        "ERR_NOT_SUPPORTED: no text to cut for /raw.pdf",
    );
    assert_eq!(set_count(&sets_of(&env, "/raw.pdf").await), 0);
}

/// SPEC-0101/E2E-139: an unknown setting name and values that are not whole numbers.
#[tokio::test(start_paused = true)]
async fn e2e_0101_139_unknown_setting_and_not_a_whole_number() {
    let env = env().await;
    essay(&env, 100).await;
    assert_refused(
        create(&env, ALICE, "/essay.md", "fixed", json!({"window": 5})).await,
        "ERR_INVALID_ARGUMENT: unknown setting 'window'",
    );
    for bad in [json!(2.5), json!("abc")] {
        assert_refused(
            create(&env, ALICE, "/essay.md", "fixed", json!({"size_tokens": bad})).await,
            "ERR_INVALID_ARGUMENT: size_tokens must be a whole number between 32 and 8192",
        );
    }
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 1);
}

/// FR-NEW-020 story check: a converted document is cut from its document view
/// (the conversion text with its image captions and tables), not its bytes.
#[tokio::test(start_paused = true)]
async fn e2e_0101_us0276_a_converted_document_is_cut_from_its_view() {
    let env = env().await;
    let client = env.state.stores.client(PROJ).await.unwrap();
    client.write_bytes_atomic("/report.pdf", b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n").await.unwrap();
    let markdown = "# Report\n\nThe yearly figures.\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n";
    let s = &env.state;
    let converting = crate::state::AppState {
        config: s.config.clone(),
        admin: s.admin.clone(),
        stores: s.stores.clone(),
        safety: s.safety.clone(),
        identity: s.identity.clone(),
        editors: s.editors.clone(),
        doc_service: Some(std::sync::Arc::new(crate::docs::service::StubDocService::ok(markdown))),
        search: None,
    };
    let server = crate::mcp::server::McpServer::new(std::sync::Arc::new(converting), ALICE.into());
    let a = args(json!({"mount_id": PROJ, "path": "/report.pdf", "overwrite": false}));
    tool_json(server.fs_documentize(a).await).unwrap();
    let view =
        crate::tools::artifacts::document_view(s, PROJ, "/report.pdf", true, Some("markdown"))
            .await
            .unwrap();
    let want = view["text"].as_str().unwrap().to_string();
    assert!(want.contains("yearly figures"), "{want}");

    let set =
        create(&env, ALICE, "/report.pdf", "fixed", json!({"size_tokens": 8192})).await.unwrap();
    assert_eq!(set["chunk_count"], 1);
    let chunks = all_chunks(&env, "/report.pdf", set["set_id"].as_str().unwrap()).await;
    assert_eq!(chunks[0]["text"], tokens::clean(&want));
    assert_eq!(chunks[0]["document_title"], "report");
}

/// Story checks: `structural` is validated before it is built; a text file is cleaned (NUL out, CRLF to LF) before
/// the cut; an empty file makes a set of 0 chunks.
#[tokio::test(start_paused = true)]
async fn e2e_0101_us0276_structural_cleaning_and_empty() {
    let env = env().await;
    essay(&env, 100).await;
    assert_refused(
        create(&env, ALICE, "/essay.md", "structural", json!({"max_tokens": 63})).await,
        "ERR_INVALID_ARGUMENT: max_tokens must be between 64 and 8192",
    );
    // US-0277 builds it: the essay holds no heading, so it is the fallback.
    let s = create(&env, ALICE, "/essay.md", "structural", json!({})).await.unwrap();
    assert_eq!(s["fallback_from"], "structural");
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 2);

    file(&env, "/crlf.md", "one\r\ntwo\0 three\r\n").await;
    let s = create(&env, ALICE, "/crlf.md", "fixed", json!({})).await.unwrap();
    let c = all_chunks(&env, "/crlf.md", s["set_id"].as_str().unwrap()).await;
    assert_eq!(c[0]["text"], "one\ntwo three\n");

    file(&env, "/empty.md", "").await;
    let s = create(&env, ALICE, "/empty.md", "fixed", json!({})).await.unwrap();
    assert_eq!((s["chunk_count"].clone(), s["token_count"].clone()), (json!(0), json!(0)));
}

// ── US-0277: the structural strategy: sections, units, merging ──────────────

use super::service::{Requested, Strategy};

/// Writes `path` holding `text`, creates a `structural` set with `settings`
/// and returns the set and every chunk in full.
async fn structural(env: &Env, path: &str, text: &str, settings: Value) -> (Value, Vec<Value>) {
    file(env, path, text).await;
    let set = create(env, ALICE, path, "structural", settings).await.unwrap();
    let chunks = all_chunks(env, path, set["set_id"].as_str().unwrap()).await;
    (set, chunks)
}

fn hpaths(chunks: &[Value]) -> Vec<String> {
    chunks
        .iter()
        .map(|c| {
            let p: Vec<&str> =
                c["heading_path"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
            p.join(" > ")
        })
        .collect()
}

fn texts(chunks: &[Value]) -> Vec<&str> {
    chunks.iter().map(|c| c["text"].as_str().unwrap()).collect()
}

/// The next rebuild of a stored set: the cut of `text` with the strategy and
/// settings it requested (DEC-017). The recalculation tool is US-0282; this
/// reads what it will read.
async fn rebuild_with_request(
    env: &Env,
    path: &str,
    key_req: &Requested,
    text: &str,
) -> super::service::Cut {
    let d = env.state.config.search.chunking();
    let store = env.state.stores.client(PROJ).await.unwrap().meta.chunk_store().unwrap();
    let row = store.find_set(path, &key_req.request_key(&d)).await.unwrap().expect("the set row");
    let settings: Value = serde_json::from_str(&row.requested_settings).unwrap();
    let req = Requested::parse(&row.requested_strategy, settings.as_object().unwrap(), &d).unwrap();
    super::service::cut(text.to_string(), super::service::title(path), req, d).await.unwrap()
}

/// SPEC-0101/E2E-043: structural settings out of range.
#[tokio::test(start_paused = true)]
async fn e2e_0101_043_structural_settings_out_of_range() {
    let env = env().await;
    essay(&env, 300).await;
    for (settings, want) in [
        (
            json!({"heading_cut_depth": 7}),
            "ERR_INVALID_ARGUMENT: heading_cut_depth must be between 1 and 6",
        ),
        (json!({"max_tokens": 63}), "ERR_INVALID_ARGUMENT: max_tokens must be between 64 and 8192"),
        (
            json!({"list_max_items": 1}),
            "ERR_INVALID_ARGUMENT: list_max_items must be between 2 and 200",
        ),
    ] {
        assert_refused(create(&env, ALICE, "/essay.md", "structural", settings).await, want);
    }
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 1, "no set created");
    create(&env, ALICE, "/essay.md", "structural", json!({"max_tokens": 64})).await.unwrap();
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 2);
}

/// SPEC-0101/E2E-050: a converted document is chunked from its document view.
#[tokio::test(start_paused = true)]
async fn e2e_0101_050_a_converted_document_is_chunked_from_its_metadata_text() {
    use crate::docs::service::{Conversion, Manifest, PictureMeta, StubDocService};
    let env = env().await;
    let client = env.state.stores.client(PROJ).await.unwrap();
    client.write_bytes_atomic("/report.pdf", b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n").await.unwrap();
    let markdown = "# Report\n\nBody of page one.\n\n![Figure](figures/f1.png)\n\nFigure 1: sales\n\n\
        ---\n*[Page 2]*\n\nBody of page two.\n\n![Figure](figures/f2.png)\n\nFigure 2: costs\n\nTotal 42\n\n\
        ---\n*[Page 3]*\n\n| item | value |\n| --- | --- |\n| r1 | 1 |\n| r2 | 2 |\n| r3 | 3 |\n| r4 | 4 |\n\n\
        Body of page three.\n";
    let png = |tag: &str| [b"\x89PNG\r\n\x1a\n".as_slice(), tag.as_bytes()].concat();
    let pic = |caption: &str, page| PictureMeta {
        caption: caption.into(),
        caption_failed: false,
        page: Some(page),
    };
    let conversion = Conversion {
        markdown: markdown.into(),
        files: [
            ("figures/f1.png".to_string(), png("pixels-one")),
            ("figures/f2.png".to_string(), png("pixels-two")),
        ]
        .into_iter()
        .collect(),
        manifest: Some(Manifest {
            pictures: [
                ("figures/f1.png".to_string(), pic("Figure 1: sales", 1)),
                ("figures/f2.png".to_string(), pic("Figure 2: costs", 2)),
            ]
            .into_iter()
            .collect(),
        }),
    };
    let s = &env.state;
    let converting = crate::state::AppState {
        config: s.config.clone(),
        admin: s.admin.clone(),
        stores: s.stores.clone(),
        safety: s.safety.clone(),
        identity: s.identity.clone(),
        editors: s.editors.clone(),
        doc_service: Some(std::sync::Arc::new(StubDocService::bundle(conversion))),
        search: None,
    };
    let server = crate::mcp::server::McpServer::new(std::sync::Arc::new(converting), ALICE.into());
    let a = args(json!({"mount_id": PROJ, "path": "/report.pdf", "overwrite": false}));
    tool_json(server.fs_documentize(a).await).unwrap();
    let view =
        crate::tools::artifacts::document_view(s, PROJ, "/report.pdf", true, Some("markdown"))
            .await
            .unwrap();
    let view = tokens::clean(view["text"].as_str().unwrap());

    let set = create(&env, ALICE, "/report.pdf", "structural", json!({})).await.unwrap();
    assert_eq!(set["strategy"], "structural");
    let chunks = all_chunks(&env, "/report.pdf", set["set_id"].as_str().unwrap()).await;
    assert!(!chunks.is_empty());
    let all = texts(&chunks).join("\n\n");
    for want in [
        "Body of page one.",
        "Body of page two.",
        "Body of page three.",
        "Figure 1: sales",
        "Figure 2: costs",
        "Total 42",
        "r1",
        "r2",
        "r3",
        "r4",
    ] {
        assert!(all.contains(want), "{want} missing from the chunks: {all}");
    }
    for c in &chunks {
        assert_eq!(c["document_title"], "report");
        let t = c["text"].as_str().unwrap();
        assert!(!t.contains("pixels") && !t.contains("PNG"), "no image bytes: {t}");
        for line in t.lines().filter(|l| !l.is_empty()) {
            assert!(view.lines().any(|v| v == line), "{line:?} is not a line of the view:\n{view}");
        }
    }
    assert_eq!(chunks.last().unwrap()["page_end"], 3);
}

/// SPEC-0101/E2E-062: a structural set by hand on a text without headings.
#[tokio::test(start_paused = true)]
async fn e2e_0101_062_a_structural_set_by_hand_without_headings() {
    let env = env().await;
    let text = prose(3000);
    file(&env, "/notes.txt", &text).await;
    let set =
        create(&env, ALICE, "/notes.txt", "structural", json!({"max_tokens": 1000})).await.unwrap();
    assert_eq!(set["strategy"], "fixed");
    assert_eq!(set["fallback_from"], "structural");
    assert_eq!(set["settings"], json!({"overlap_tokens": 0, "size_tokens": 2048}));
    assert_eq!(set["chunk_count"], 2);
    let again =
        create(&env, ALICE, "/notes.txt", "structural", json!({"max_tokens": 1000})).await.unwrap();
    assert_eq!(again["set_id"], set["set_id"]);
    let fixed =
        create(&env, ALICE, "/notes.txt", "fixed", json!({"size_tokens": 2048})).await.unwrap();
    assert_ne!(fixed["set_id"], set["set_id"]);
    assert_eq!(set_count(&sets_of(&env, "/notes.txt").await), 2);

    let d = env.state.config.search.chunking();
    let m = json!({"max_tokens": 1000});
    let req = Requested::parse("structural", m.as_object().unwrap(), &d).unwrap();
    let headed = format!("# Notes\n\n{text}");
    file(&env, "/notes.txt", &headed).await;
    let next = rebuild_with_request(&env, "/notes.txt", &req, &headed).await;
    assert_eq!(next.used, Strategy::Structural);
    assert_eq!(next.fallback_from, None);
    assert_eq!(next.settings["max_tokens"], 1000);
}

/// SPEC-0101/E2E-063: a heading inside a code block is not a heading.
#[tokio::test(start_paused = true)]
async fn e2e_0101_063_a_heading_inside_a_code_block() {
    let env = env().await;
    let code = "```\n# not a heading\nlet x = 1;\n```\n\nsome words\n";
    let (set, chunks) = structural(&env, "/snip.md", code, json!({})).await;
    assert_eq!(set["strategy"], "fixed");
    assert_eq!(set["fallback_from"], "structural");
    assert_eq!(texts(&chunks), [code]);

    let d = env.state.config.search.chunking();
    let req = Requested::parse("structural", &Default::default(), &d).unwrap();
    let headed = format!("# Real\n\n{code}");
    file(&env, "/snip.md", &headed).await;
    let next = rebuild_with_request(&env, "/snip.md", &req, &headed).await;
    assert_eq!(next.used, Strategy::Structural);
    assert_eq!(next.chunks.len(), 1);
    assert_eq!(next.chunks[0].heading_path, ["Real"]);
}

/// SPEC-0101/E2E-070: sections follow the headings.
#[tokio::test(start_paused = true)]
async fn e2e_0101_070_sections_follow_the_headings() {
    let env = env().await;
    let (intro, a, b, c) = (prose(100), prose(1500), prose(500), prose(500));
    let text = format!("{intro}\n\n# A\n\n{a}\n\n## B\n\n{b}\n\n### C\n\n{c}\n");
    let (set, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_eq!(set["strategy"], "structural");
    assert_eq!(hpaths(&chunks), ["", "A", "A > B"]);
    assert_eq!(
        texts(&chunks),
        [intro, format!("# A\n\n{a}"), format!("## B\n\n{b}\n\n### C\n\n{c}")]
    );
    let tok = |i: usize| chunks[i]["token_count"].as_u64().unwrap();
    assert!(tok(1) + tok(2) > 2048, "A and B too big to merge");
}

/// SPEC-0101/E2E-071: a document starting at level 3.
#[tokio::test(start_paused = true)]
async fn e2e_0101_071_a_document_starting_at_level_3() {
    let env = env().await;
    let text = format!("### X\n\n{}\n\n### Y\n\n{}", prose(1500), prose(1500));
    let (_, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_eq!(hpaths(&chunks), ["X", "Y"]);
    assert!(texts(&chunks)[1].starts_with("### Y\n\n"));
}

/// SPEC-0101/E2E-072: two short sections under one heading merge.
#[tokio::test(start_paused = true)]
async fn e2e_0101_072_two_short_sections_under_one_heading_merge() {
    let env = env().await;
    let (a, b, c) = (prose(100), prose(100), prose(100));
    let text = format!("# A\n\n{a}\n\n## B\n\n{b}\n\n## C\n\n{c}");
    let (set, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_eq!(set["chunk_count"], 1);
    assert_eq!(hpaths(&chunks), ["A"]);
    assert_eq!(texts(&chunks), [text.as_str()]);
    assert_eq!(chunks[0]["unit"], 1);
}

/// SPEC-0101/E2E-073: two short sections under different headings do not merge.
#[tokio::test(start_paused = true)]
async fn e2e_0101_073_two_short_sections_under_different_headings() {
    let env = env().await;
    let text = format!("# A\n\n{}\n\n# B\n\n{}", prose(100), prose(100));
    let (_, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_eq!(hpaths(&chunks), ["A", "B"]);
    let n: Vec<u64> = chunks.iter().map(|c| c["number"].as_u64().unwrap()).collect();
    assert_eq!(n, [1, 2]);
}

/// SPEC-0101/E2E-074: a page marker records the page and cuts nothing.
#[tokio::test(start_paused = true)]
async fn e2e_0101_074_a_page_marker() {
    let env = env().await;
    let text = "# T\n\nparagraph one\n---\n*[Page 3]*\nparagraph two\n";
    let (_, chunks) = structural(&env, "/doc.md", text, json!({})).await;
    assert_eq!(chunks.len(), 1);
    assert_eq!(
        (chunks[0]["page_start"].clone(), chunks[0]["page_end"].clone()),
        (json!(3), json!(3))
    );
    assert_eq!(texts(&chunks), ["# T\n\nparagraph one\n\nparagraph two"]);
    let (_, plain) = structural(&env, "/plain.md", "# T\n\nparagraph one\n", json!({})).await;
    assert_eq!(
        (plain[0]["page_start"].clone(), plain[0]["page_end"].clone()),
        (Value::Null, Value::Null)
    );
}

/// SPEC-0101/E2E-075: a thematic break is ignored.
#[tokio::test(start_paused = true)]
async fn e2e_0101_075_a_thematic_break() {
    let env = env().await;
    let (_, chunks) =
        structural(&env, "/doc.md", "# T\n\nparagraph one\n---\nparagraph two\n", json!({})).await;
    assert_eq!(texts(&chunks), ["# T\n\nparagraph one\n\nparagraph two"]);
    assert_eq!(chunks[0]["page_start"], Value::Null);
    assert_eq!(chunks[0]["page_end"], Value::Null);
}

/// SPEC-0101/E2E-076: the metadata table is left out.
#[tokio::test(start_paused = true)]
async fn e2e_0101_076_the_metadata_table() {
    let env = env().await;
    let text = "| | |\n|---|---|\n| Author | Smith |\n\n# A\n\ntext of a\n";
    let (_, chunks) = structural(&env, "/doc.md", text, json!({})).await;
    assert!(texts(&chunks).iter().all(|t| !t.contains("Author") && !t.contains("Smith")));
    assert_eq!(texts(&chunks), ["# A\n\ntext of a"]);
}

/// SPEC-0101/E2E-077: a list introduction travels with its list.
#[tokio::test(start_paused = true)]
async fn e2e_0101_077_a_list_introduction() {
    let env = env().await;
    let text = "# T\n\nThe failure has known reasons.\nCauses:\n- heat\n- cold\n- wind\n";
    let (_, chunks) = structural(&env, "/doc.md", text, json!({})).await;
    let all = texts(&chunks).join("\n\n");
    assert_eq!(all.matches("Causes:").count(), 1);
    let holder = texts(&chunks).into_iter().find(|t| t.contains("- heat")).unwrap();
    assert!(holder.contains("Causes:\n- heat\n- cold\n- wind"), "{holder}");
}

/// SPEC-0101/E2E-084: carriage returns and NUL characters.
#[tokio::test(start_paused = true)]
async fn e2e_0101_084_carriage_returns_and_nul() {
    let env = env().await;
    let clean = "# A\n\none\n\n## B\n\n- x\n- y\n\n# C\n\nthree\n";
    let dirty = "# A\r\n\r\no\0ne\r\n\r\n## B\r\n\r\n- x\r\n- y\0\r\n\r\n# C\r\n\r\nthree\r\n";
    let (_, a) = structural(&env, "/a/doc.md", clean, json!({})).await;
    let (_, b) = structural(&env, "/b/doc.md", dirty, json!({})).await;
    assert_eq!(a, b);
    assert!(texts(&b).iter().all(|t| !t.contains('\0') && !t.contains('\r')));
}

/// SPEC-0101/E2E-087: a figure follows the unit before it.
#[tokio::test(start_paused = true)]
async fn e2e_0101_087_a_figure_follows_the_unit_before_it() {
    let env = env().await;
    let text = "# A\n\nparagraph one\n\n![chart](chart.png)\n\n# B\n\nparagraph two\n";
    let (_, chunks) = structural(&env, "/doc.md", text, json!({})).await;
    assert_eq!(chunks[0]["figures"], json!(["chart.png"]));
    assert_eq!(chunks[1]["figures"], json!([]));
    assert!(texts(&chunks).iter().all(|t| !t.contains("chart")));
}

/// SPEC-0101/E2E-088: a figure before every unit.
#[tokio::test(start_paused = true)]
async fn e2e_0101_088_a_figure_before_every_unit() {
    let env = env().await;
    let text = "# T\n\n[Figure]\n![logo](logo.png)\n\nparagraph one\n";
    let (_, chunks) = structural(&env, "/doc.md", text, json!({})).await;
    assert_eq!(chunks[0]["figures"], json!(["logo.png"]));
    let n: usize = chunks.iter().map(|c| c["figures"].as_array().unwrap().len()).sum();
    assert_eq!(n, 1);
}

/// SPEC-0101/E2E-089: what a chunk holds.
#[tokio::test(start_paused = true)]
async fn e2e_0101_089_what_a_chunk_holds() {
    let env = env().await;
    let text = "# A\n\ntext of a\n\n---\n*[Page 2]*\n\n# B\n\ntext of b\n";
    let (set, chunks) = structural(&env, "/guide.md", text, json!({})).await;
    let a =
        args(json!({"mount_id": PROJ, "path": "/guide.md", "set_id": set["set_id"], "number": 2}));
    let c = tool_json(env.server(ALICE).fs_get_chunk(a).await).unwrap()["chunks"][0].clone();
    assert_eq!(c["document_title"], "guide");
    assert_eq!(c["heading_path"], json!(["B"]));
    assert_eq!((c["number"].clone(), c["unit"].clone()), (json!(2), json!(2)));
    assert_eq!((c["page_start"].clone(), c["page_end"].clone()), (json!(2), json!(2)));
    assert_eq!(c["text"], "# B\n\ntext of b");
    assert_eq!(c["token_count"], tokens::count("# B\n\ntext of b"));
    assert_eq!(c["figures"], json!([]));
    let units: Vec<u64> = chunks.iter().map(|c| c["unit"].as_u64().unwrap()).collect();
    assert_eq!(units, [1, 2]);
}

/// SPEC-0101/E2E-090: heading_cut_depth 1.
#[tokio::test(start_paused = true)]
async fn e2e_0101_090_heading_cut_depth_1() {
    let env = env().await;
    let text =
        format!("# A\n\n{}\n\n## B\n\n{}\n\n# C\n\n{}", prose(1500), prose(1500), prose(1500));
    let (_, one) = structural(&env, "/one.md", &text, json!({"heading_cut_depth": 1})).await;
    assert!(!hpaths(&one).contains(&"A > B".to_string()), "{:?}", hpaths(&one));
    for (t, p) in texts(&one).iter().zip(hpaths(&one)) {
        if t.contains("# A") || t.contains("## B") {
            assert_eq!(p, "A");
        }
    }
    assert_eq!(hpaths(&one).iter().filter(|p| *p == "C").count(), 1);
    assert_bounded(&one, 2048);
    let (_, two) = structural(&env, "/two.md", &text, json!({"heading_cut_depth": 2})).await;
    assert!(hpaths(&two).contains(&"A > B".to_string()));
}

/// SPEC-0101/E2E-093: an empty file.
#[tokio::test(start_paused = true)]
async fn e2e_0101_093_an_empty_file() {
    let env = env().await;
    let (set, chunks) = structural(&env, "/empty.md", "", json!({})).await;
    assert_eq!(set["strategy"], "fixed");
    assert_eq!(set["fallback_from"], "structural");
    assert_eq!((set["chunk_count"].clone(), set["token_count"].clone()), (json!(0), json!(0)));
    assert!(chunks.is_empty());
    let a = args(json!({"mount_id": PROJ, "path": "/empty.md", "set_id": set["set_id"]}));
    let page = tool_json(env.server(ALICE).fs_list_chunks(a).await).unwrap();
    assert_eq!(page["chunks"], json!([]));
    assert!(page.get("marker").is_none());
}

/// SPEC-0101/E2E-095: an inline image is not a figure.
#[tokio::test(start_paused = true)]
async fn e2e_0101_095_an_inline_image_is_not_a_figure() {
    let env = env().await;
    let (_, chunks) =
        structural(&env, "/doc.md", "# T\n\nsee ![chart](chart.png) for details\n", json!({}))
            .await;
    assert_eq!(texts(&chunks), ["# T\n\nsee ![chart](chart.png) for details"]);
    assert_eq!(chunks[0]["figures"], json!([]));
}

// ── US-0278: the structural strategy: oversize pieces and list batches ──────

/// Every chunk holds at most `max` tokens, by the ruler and by its record.
fn assert_bounded(chunks: &[Value], max: usize) {
    for c in chunks {
        let t = c["text"].as_str().unwrap();
        assert!(tokens::count(t) <= max, "chunk {} has {} tokens", c["number"], tokens::count(t));
        assert_eq!(c["token_count"], tokens::count(t), "chunk {}", c["number"]);
    }
}

/// The pieces, in order, cover `text` exactly: each is found after the
/// previous one with only whitespace between, so nothing is skipped and
/// nothing is repeated.
fn assert_covers(text: &str, pieces: &[&str]) {
    let mut pos = 0;
    for p in pieces {
        let at = pos + text[pos..].find(p).unwrap_or_else(|| panic!("{p:.80?} not found in order"));
        assert!(text[pos..at].trim().is_empty(), "text skipped before {p:.80?}");
        pos = at + p.len();
    }
    assert!(text[pos..].trim().is_empty(), "text left after the last piece");
}

/// A paragraph of about `n` tokens that no other paragraph repeats.
fn para(tag: &str, n: usize) -> String {
    format!("{tag} {}", prose(n))
}

fn units(chunks: &[Value]) -> Vec<u64> {
    chunks.iter().map(|c| c["unit"].as_u64().unwrap()).collect()
}

/// SPEC-0101/E2E-078: a list of 95 items.
#[tokio::test(start_paused = true)]
async fn e2e_0101_078_a_list_of_95_items() {
    let env = env().await;
    let items: Vec<String> = (1..=95).map(|i| format!("- i{i} {}", prose(6))).collect();
    let text = format!("# L\n\n{}\n", items.join("\n"));
    let (_, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_eq!(chunks.len(), 4, "{:?}", texts(&chunks));
    assert_eq!(texts(&chunks)[0], "# L");
    assert_eq!(texts(&chunks)[1], items[..40].join("\n"));
    assert_eq!(texts(&chunks)[2], items[40..80].join("\n"));
    assert_eq!(texts(&chunks)[3], items[80..].join("\n"));
    assert_eq!(units(&chunks), [1, 2, 3, 4], "no batch merged");
    let all = texts(&chunks).join("\n");
    for i in &items {
        assert_eq!(all.matches(i.as_str()).count(), 1, "{i:.20}");
    }
}

/// The table of E2E-079 and E2E-094: `rows` rows of about `n` tokens each.
fn table(rows: usize, n: usize) -> (String, Vec<String>) {
    let rows: Vec<String> = (1..=rows).map(|i| format!("| r{i} | {} |", prose(n))).collect();
    (format!("| key | value |\n|---|---|\n{}", rows.join("\n")), rows)
}

/// Each table chunk starts with the two header lines and holds whole rows;
/// every row appears once over the chunks.
fn assert_row_groups(chunks: &[Value], rows: &[String]) -> Vec<u64> {
    let mut seen = Vec::new();
    for t in texts(chunks).into_iter().filter(|t| t.contains("| r")) {
        assert!(t.starts_with("| key | value |\n|---|---|\n"), "{t:.80}");
        for line in t.lines().skip(2) {
            assert!(rows.contains(&line.to_string()), "a cut row: {line:.80}");
            seen.push(line.to_string());
        }
    }
    assert_eq!(seen, rows, "every row once, in order");
    chunks
        .iter()
        .filter(|c| c["text"].as_str().unwrap().contains("| r"))
        .map(|c| c["unit"].as_u64().unwrap())
        .collect()
}

/// SPEC-0101/E2E-079: a table of 5000 tokens.
#[tokio::test(start_paused = true)]
async fn e2e_0101_079_a_table_of_5000_tokens() {
    let env = env().await;
    let (t, rows) = table(200, 20);
    assert!((4800..5400).contains(&tokens::count(&t)), "{}", tokens::count(&t));
    let (_, chunks) = structural(&env, "/doc.md", &format!("# T\n\n{t}\n"), json!({})).await;
    assert_bounded(&chunks, 2048);
    let u = assert_row_groups(&chunks, &rows);
    assert!(u.len() >= 3, "{u:?}");
    assert!(u.iter().all(|x| *x == u[0]), "pieces share the unit: {u:?}");
}

/// SPEC-0101/E2E-080: a code block never closed.
#[tokio::test(start_paused = true)]
async fn e2e_0101_080_a_code_block_never_closed() {
    let env = env().await;
    let lines: Vec<String> = (1..=300).map(|i| format!("let v{i} = {};", prose(6))).collect();
    let text = format!("# C\n\n```\n{}", lines.join("\n"));
    let (_, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_bounded(&chunks, 2048);
    let code: Vec<&str> = texts(&chunks).into_iter().filter(|t| t.starts_with("```")).collect();
    assert!(code.len() >= 2, "{}", code.len());
    let mut body = Vec::new();
    for t in &code {
        assert!(t.starts_with("```\n") && t.ends_with("\n```"), "{t:.80}");
        let inner: Vec<&str> = t.lines().collect();
        body.extend(inner[1..inner.len() - 1].iter().map(|l| l.to_string()));
    }
    assert_eq!(body, lines);
}

/// SPEC-0101/E2E-081: prose above the limit.
#[tokio::test(start_paused = true)]
async fn e2e_0101_081_prose_above_the_limit() {
    let env = env().await;
    let paras: Vec<String> = (1..=25).map(|i| para(&format!("p{i}"), 200)).collect();
    let text = format!("# P\n\n{}", paras.join("\n\n"));
    let (_, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert!(chunks.len() >= 3);
    assert_bounded(&chunks, 2048);
    assert_covers(&text, &texts(&chunks));
    for t in texts(&chunks) {
        for line in t.split("\n\n") {
            assert!(line == "# P" || paras.contains(&line.to_string()), "cut inside: {line:.80}");
        }
    }
    assert_eq!(texts(&chunks).join("\n\n"), text, "the pieces joined give back the text");
    assert!(units(&chunks).iter().all(|u| *u == 1));
}

/// SPEC-0101/E2E-082: one paragraph with no line break.
#[tokio::test(start_paused = true)]
async fn e2e_0101_082_one_paragraph_with_no_line_break() {
    let env = env().await;
    let text = format!("# W\n\n{}", prose(3000));
    let (_, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_eq!(
        chunks.len(),
        2,
        "{:?}",
        chunks.iter().map(|c| &c["token_count"]).collect::<Vec<_>>()
    );
    assert_bounded(&chunks, 2048);
    assert_covers(&text, &texts(&chunks));
    let first = texts(&chunks)[0];
    let next = &text[first.len()..];
    assert!(next.starts_with(' ') || first.ends_with(' '), "cut between words");
}

/// SPEC-0101/E2E-083: one string with no space. Its heading line is a
/// paragraph of the same unit; the string alone does not fit next to it,
/// so the heading is its own piece and the string is cut by token count.
#[tokio::test(start_paused = true)]
async fn e2e_0101_083_one_string_with_no_space() {
    let env = env().await;
    // Digits are read three at a time by the ruler: 3000 tokens.
    let s: String = (0..3000).map(|i| format!("{:03}", 100 + i % 900)).collect();
    assert_eq!(tokens::count(&s), 3000);
    let text = format!("# S\n\n{s}");
    let (_, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_bounded(&chunks, 2048);
    assert_covers(&text, &texts(&chunks));
    let pieces: Vec<&str> = texts(&chunks).into_iter().filter(|t| *t != "# S").collect();
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces.concat(), s);
    assert_eq!(tokens::count(pieces[0]), 2048);
}

/// The text of E2E-085 and E2E-086: about 30 000 tokens of headings of
/// level 1 to 4, prose, lists, tables and a paragraph with no line break.
fn long_doc() -> String {
    let mut parts = Vec::new();
    for s in 1..=6 {
        parts.push(format!("# Part {s}"));
        parts.push(para(&format!("a{s}"), 400));
        parts.push(format!("## Chapter {s}"));
        parts.push(para(&format!("b{s}"), 600));
        let items: Vec<String> = (1..=50).map(|i| format!("- l{s}x{i} {}", prose(6))).collect();
        parts.push(items.join("\n"));
        parts.push(format!("### Section {s}"));
        let rows: Vec<String> = (1..=30).map(|i| format!("| t{s}x{i} | {} |", prose(10))).collect();
        parts.push(format!("| k | v |\n|---|---|\n{}", rows.join("\n")));
        parts.push(format!("#### Detail {s}"));
        for p in 1..=4 {
            parts.push(para(&format!("c{s}x{p}"), 300));
        }
        parts.push(para(&format!("d{s}"), 2200));
    }
    parts.join("\n\n")
}

/// SPEC-0101/E2E-085: the same text twice.
#[tokio::test(start_paused = true)]
async fn e2e_0101_085_the_same_text_twice() {
    let env = env().await;
    let text = long_doc();
    assert!((28_000..36_000).contains(&tokens::count(&text)), "{}", tokens::count(&text));
    let (s1, a) = structural(&env, "/a/doc.md", &text, json!({})).await;
    let (s2, b) = structural(&env, "/b/doc.md", &text, json!({})).await;
    assert!(a.len() > 15);
    assert_eq!(a, b, "the same chunks, texts, numbers, units and counts");
    assert_eq!(s1["token_count"], s2["token_count"]);
    let d = env.state.config.search.chunking();
    let req = Requested::parse("structural", &Default::default(), &d).unwrap();
    let again = super::cut(&text, "doc", &req, &d);
    assert_eq!(again.chunks, super::cut(&text, "doc", &req, &d).chunks);
}

/// SPEC-0101/E2E-086: no chunk above the limit and at most 2 titles.
#[tokio::test(start_paused = true)]
async fn e2e_0101_086_no_chunk_above_the_limit_and_at_most_2_titles() {
    let env = env().await;
    let text = long_doc();
    let (set, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_eq!(set["strategy"], "structural");
    assert_bounded(&chunks, 2048);
    assert!(chunks.iter().all(|c| c["heading_path"].as_array().unwrap().len() <= 2));
    assert_covers(&text, &texts(&chunks));
}

/// SPEC-0101/E2E-091: max_tokens 64.
#[tokio::test(start_paused = true)]
async fn e2e_0101_091_max_tokens_64() {
    let env = env().await;
    let paras: Vec<String> = (1..=6).map(|i| para(&format!("p{i}"), 48)).collect();
    let text = format!("# A\n\n{}", paras.join("\n\n"));
    let (_, chunks) = structural(&env, "/doc.md", &text, json!({"max_tokens": 64})).await;
    assert_bounded(&chunks, 64);
    assert_eq!(chunks.len(), 6, "{:?}", texts(&chunks));
    for (t, p) in texts(&chunks).iter().zip(&paras) {
        assert!(t.ends_with(p.as_str()), "one whole paragraph: {t:.80}");
    }
    assert_covers(&text, &texts(&chunks));
}

/// SPEC-0101/E2E-092: list_max_items 2.
#[tokio::test(start_paused = true)]
async fn e2e_0101_092_list_max_items_2() {
    let env = env().await;
    let text = "# T\n\nItems:\n- a\n- b\n- c\n- d\n- e\n";
    let (_, chunks) = structural(&env, "/doc.md", text, json!({"list_max_items": 2})).await;
    assert_eq!(texts(&chunks), ["# T", "Items:\n- a\n- b", "- c\n- d", "- e"]);
    assert_eq!(units(&chunks), [1, 2, 3, 4]);
}

/// SPEC-0101/E2E-094: a table is never cut inside a row.
#[tokio::test(start_paused = true)]
async fn e2e_0101_094_a_table_is_never_cut_inside_a_row() {
    let env = env().await;
    let (t, rows) = table(10, 300);
    let text = format!("# A\n\n{}\n\n{t}\n\n{}", para("before", 50), para("after", 50));
    let (_, chunks) = structural(&env, "/doc.md", &text, json!({})).await;
    assert_bounded(&chunks, 2048);
    let u = assert_row_groups(&chunks, &rows);
    assert!(u.len() >= 2 && u.iter().all(|x| *x == u[0]), "{u:?}");
    let all = texts(&chunks).join("\n\n");
    assert_eq!(all.matches("before the").count(), 1);
    assert_eq!(all.matches("after the").count(), 1);
}

// ── US-0279: search builds its entries from the default set ─────────────────

use crate::search::SearchBackend;

/// `search.index` by Alice with `extra` arguments.
async fn index_by_hand(env: &Env, path: &str, extra: Value) -> Result<Value, String> {
    let mut a = json!({"mount_id": PROJ, "path": path});
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        a[k] = v;
    }
    tool_json(env.server(ALICE).search_index(args(a)).await)
}

/// The results `search.query` (bm25) answers Alice for `word`.
async fn results(env: &Env, word: &str) -> Vec<Value> {
    let a = args(json!({"mount_id": PROJ, "query": word, "mode": "bm25", "top_k": 50}));
    let v = tool_json(env.server(ALICE).search_query(a).await).expect("search answers");
    v["results"].as_array().unwrap().clone()
}

/// The default set of `path` as `fs.list_chunk_sets` shows it, if any.
async fn default_of(env: &Env, path: &str) -> Option<Value> {
    let v = sets_of(env, path).await;
    v["sets"].as_array().unwrap().iter().find(|s| s["default"] == true).cloned()
}

fn status_docs(v: &Value) -> u64 {
    v["bm25_docs"].as_u64().unwrap_or_else(|| panic!("bm25_docs in {v}"))
}

async fn status(env: &Env) -> Value {
    tool_json(env.server(ALICE).search_status(args(json!({"mount_id": PROJ}))).await).unwrap()
}

const GUIDE: &str = "# Setup\n\nInstall the kubelet on every node.\n\n# Usage\n\nRun it daily.\n";

/// SPEC-0101/E2E-120: search results carry the chunk and its heading path.
#[tokio::test(start_paused = true)]
async fn e2e_0101_120_results_carry_the_chunk_and_its_heading_path() {
    let env = env().await;
    file(&env, "/guide.md", GUIDE).await;
    index_by_hand(&env, "/guide.md", json!({})).await.unwrap();
    assert_eq!(default_of(&env, "/guide.md").await.expect("a default set")["state"], "current");

    let r = results(&env, "kubelet").await;
    assert_eq!(r[0]["path"], "/guide.md");
    assert_eq!(r[0]["chunk_number"], 1);
    assert_eq!(r[0]["heading_path"], json!(["Setup"]));
    assert_eq!(r[0]["rank"], 1);
    assert!(r[0]["score"].as_f64().unwrap() > 0.0);
    assert!(r[0]["chunk"].as_str().unwrap().contains("kubelet"));

    // The title is only in the header line, yet it finds the chunks.
    let by_title = results(&env, "guide").await;
    assert!(
        by_title.iter().any(|x| x["path"] == "/guide.md" && x["chunk_number"] == 1),
        "{by_title:?}"
    );
}

/// SPEC-0101/E2E-121: the heading is part of the indexed text, the result
/// shows the chunk without the header line.
#[tokio::test(start_paused = true)]
async fn e2e_0101_121_the_heading_is_part_of_the_indexed_text() {
    let env = env().await;
    file(&env, "/guide.md", "# Setup\n\nInstall the agent on every node.\n").await;
    index_by_hand(&env, "/guide.md", json!({})).await.unwrap();
    let set = default_of(&env, "/guide.md").await.unwrap();
    let chunks = all_chunks(&env, "/guide.md", set["set_id"].as_str().unwrap()).await;

    let r = results(&env, "setup").await;
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0]["chunk"], chunks[0]["text"], "the chunk text, no header line");
    assert!(!r[0]["chunk"].as_str().unwrap().starts_with("guide"));
    // The engine holds the header line first.
    let raw = env.stub().query_bm25(PROJ, "setup", 10).await.unwrap();
    assert_eq!(raw[0].chunk, format!("guide > Setup\n{}", chunks[0]["text"].as_str().unwrap()));
}

/// SPEC-0101/E2E-122: indexing by hand builds and uses the default set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_122_indexing_by_hand_uses_the_default_set() {
    let env = env().await;
    file(&env, "/guide.md", GUIDE).await;
    assert!(default_of(&env, "/guide.md").await.is_none(), "no set yet");
    let v = index_by_hand(&env, "/guide.md", json!({})).await.unwrap();
    let set = default_of(&env, "/guide.md").await.expect("the default set now exists");
    assert_eq!(set["chunk_count"], 2);
    assert_eq!(v["indexed"], set["chunk_count"]);
    assert_eq!(v["skipped"], 0);
    assert!(results(&env, "kubelet").await.iter().any(|x| x["path"] == "/guide.md"));
}

/// SPEC-0101/E2E-123: the removed arguments are refused.
#[tokio::test(start_paused = true)]
async fn e2e_0101_123_the_removed_arguments() {
    let env = env().await;
    file(&env, "/guide.md", GUIDE).await;
    for extra in [json!({"chunk_size": 500}), json!({"chunk_overlap": 10})] {
        assert_refused(
            index_by_hand(&env, "/guide.md", extra).await,
            "ERR_INVALID_ARGUMENT: chunk_size and chunk_overlap are no longer accepted",
        );
    }
    assert_eq!(status_docs(&status(&env).await), 0, "nothing was indexed");
    assert!(default_of(&env, "/guide.md").await.is_none());
    // Hidden from the schema the tool publishes.
    let schema =
        serde_json::to_value(schemars::schema_for!(crate::mcp::server::SearchIndexArgs)).unwrap();
    let props: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
    assert_eq!(props, ["mount_id", "path", "recursive"]);
}

/// SPEC-0101/E2E-124: indexing a folder by hand.
#[tokio::test(start_paused = true)]
async fn e2e_0101_124_indexing_a_folder_by_hand() {
    let env = env().await;
    file(&env, "/docs/a.md", GUIDE).await;
    file(&env, "/docs/b.txt", "plain words about turbines\n").await;
    let client = env.state.stores.client(PROJ).await.unwrap();
    client
        .write_bytes_atomic("/docs/c.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\xff\xfe")
        .await
        .unwrap();

    let v = index_by_hand(&env, "/docs", json!({"recursive": true})).await.unwrap();
    let a = default_of(&env, "/docs/a.md").await.expect("a.md has its set");
    let b = default_of(&env, "/docs/b.txt").await.expect("b.txt has its set");
    assert!(default_of(&env, "/docs/c.png").await.is_none());
    let total = a["chunk_count"].as_u64().unwrap() + b["chunk_count"].as_u64().unwrap();
    assert_eq!(v["indexed"], total);
    assert_eq!(v["skipped"], 1, "c.png is skipped: {v}");
    assert_eq!(status_docs(&status(&env).await), total);
    assert!(results(&env, "turbines").await.iter().any(|x| x["path"] == "/docs/b.txt"));
}

/// SPEC-0101/E2E-125: a chunk without heading path.
#[tokio::test(start_paused = true)]
async fn e2e_0101_125_a_chunk_without_heading_path() {
    let env = env().await;
    file(&env, "/plain.md", "just some words about windmills\n").await;
    index_by_hand(&env, "/plain.md", json!({})).await.unwrap();
    assert_eq!(default_of(&env, "/plain.md").await.unwrap()["fallback_from"], "structural");

    let r = results(&env, "windmills").await;
    assert_eq!(r[0]["heading_path"], json!([]));
    assert_eq!(r[0]["chunk_number"], 1);
    let raw = env.stub().query_bm25(PROJ, "windmills", 10).await.unwrap();
    assert_eq!(raw[0].chunk.split_once('\n').unwrap().0, "plain", "the title alone");
}

/// SPEC-0101/E2E-126: the embedding service is down for an index by hand.
#[tokio::test(start_paused = true)]
async fn e2e_0101_126_embedding_down_for_indexing_by_hand() {
    let env = Env::new(Options { delay: 3, embedding: true, ..Options::default() }).await;
    env.state.admin.set_index_mode(PROJ, crate::storage::traits::IndexMode::Both).await.unwrap();
    file(&env, "/guide.md", GUIDE).await;
    index_by_hand(&env, "/guide.md", json!({})).await.unwrap();
    let sets_before = sets_of(&env, "/guide.md").await;
    let before = results(&env, "kubelet").await;

    env.stub().set_down(true);
    let e = index_by_hand(&env, "/guide.md", json!({})).await.unwrap_err();
    assert!(e.ends_with("embedding service unreachable"), "got {e}");
    assert_eq!(results(&env, "kubelet").await, before, "the entries are unchanged");
    assert_eq!(sets_of(&env, "/guide.md").await, sets_before, "the set is unchanged");
}

/// SPEC-0101/E2E-142: `search.index` on a default set out of date rebuilds it.
#[tokio::test(start_paused = true)]
async fn e2e_0101_142_search_index_on_a_stale_default_set() {
    let env = Env::new(Options { delay: 30, ..Options::default() }).await;
    file(&env, "/guide.md", GUIDE).await;
    index_by_hand(&env, "/guide.md", json!({})).await.unwrap();
    let id = default_of(&env, "/guide.md").await.unwrap()["set_id"].clone();

    env.state.safety.record_read(ALICE, PROJ, "/guide.md");
    env.save("/guide.md", "# Setup\n\nInstall the scheduler.\n").await;
    env.at(1).await;
    assert_eq!(default_of(&env, "/guide.md").await.unwrap()["state"], "out_of_date");

    let v = index_by_hand(&env, "/guide.md", json!({})).await.unwrap();
    let set = default_of(&env, "/guide.md").await.unwrap();
    assert_eq!(set["state"], "current");
    assert_eq!(set["set_id"], id, "the set keeps its id");
    assert_eq!(v["indexed"], set["chunk_count"]);
    assert!(results(&env, "scheduler").await.iter().any(|x| x["path"] == "/guide.md"));
    assert!(results(&env, "kubelet").await.is_empty());
}

/// SPEC-0101/E2E-143: `search.index` on a binary file.
#[tokio::test(start_paused = true)]
async fn e2e_0101_143_search_index_on_a_binary_file() {
    let env = env().await;
    let client = env.state.stores.client(PROJ).await.unwrap();
    client.write_bytes_atomic("/logo.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\xff\xfe").await.unwrap();
    assert_refused(
        index_by_hand(&env, "/logo.png", json!({})).await,
        "ERR_NOT_SUPPORTED: no text to cut for /logo.png",
    );
    assert_eq!(status_docs(&status(&env).await), 0);
    assert!(default_of(&env, "/logo.png").await.is_none());
}

/// SPEC-0101/E2E-144: a result from an entry built before the upgrade.
#[tokio::test(start_paused = true)]
async fn e2e_0101_144_a_result_from_an_entry_built_before_the_upgrade() {
    let env = env().await;
    let text = "# Old\n\nnotes about lighthouses\n";
    file(&env, "/old.md", text).await;
    pre_upgrade_entry(&env, "/old.md", text).await;

    let r = results(&env, "lighthouses").await;
    assert_eq!(r[0]["chunk_number"], 0);
    assert_eq!(r[0]["heading_path"], json!([]));
    assert_eq!(r[0]["chunk"], text, "the text unchanged");

    env.recalc(ALICE, PROJ, "/old.md", "search_index").await.unwrap();
    env.at(5).await;
    let r = results(&env, "lighthouses").await;
    assert_eq!(r[0]["chunk_number"], 1);
    assert_eq!(r[0]["heading_path"], json!(["Old"]));
}

/// An entry as the character chunker stored it: the plain text, chunk 0.
async fn pre_upgrade_entry(env: &Env, path: &str, text: &str) {
    let entry = crate::search::IndexedChunk { number: 0, text: text.to_string() };
    env.stub().index_chunks_checked(PROJ, path, &[entry], &|| true).await.unwrap();
}

/// SPEC-0101/E2E-MOD-001 (was SPEC-0008/E2E-004): status counts chunks.
#[tokio::test(start_paused = true)]
async fn e2e_0101_mod001_status_counts_chunks_not_files() {
    let env = env().await;
    file(&env, "/long.md", &prose(1000)).await;
    file(&env, "/doc1.md", "short note about the sea.").await;
    for p in ["/long.md", "/doc1.md"] {
        index_by_hand(&env, p, json!({})).await.unwrap();
    }
    let s = status(&env).await;
    assert_eq!(s["mode"], "bm25");
    let total =
        |sets: [Value; 2]| sets.iter().map(|s| s["chunk_count"].as_u64().unwrap()).sum::<u64>();
    let counts =
        [default_of(&env, "/long.md").await.unwrap(), default_of(&env, "/doc1.md").await.unwrap()];
    assert_eq!(status_docs(&s), 2);
    assert_eq!(status_docs(&s), total(counts));

    let three =
        format!("# A\n\n{}\n\n# B\n\n{}\n\n# C\n\n{}\n", prose(1500), prose(1500), prose(1500));
    file(&env, "/long.md", &three).await;
    index_by_hand(&env, "/long.md", json!({})).await.unwrap();
    let s = status(&env).await;
    let counts =
        [default_of(&env, "/long.md").await.unwrap(), default_of(&env, "/doc1.md").await.unwrap()];
    assert_eq!(status_docs(&s), 4);
    assert_eq!(status_docs(&s), total(counts));
}

/// SPEC-0101/E2E-MOD-002 (was SPEC-0008/E2E-011): re indexing is idempotent.
#[tokio::test(start_paused = true)]
async fn e2e_0101_mod002_reindexing_is_idempotent() {
    let env = env().await;
    file(&env, "/doc1.md", GUIDE).await;
    let first = index_by_hand(&env, "/doc1.md", json!({})).await.unwrap();
    let set = default_of(&env, "/doc1.md").await.unwrap();
    let id = set["set_id"].as_str().unwrap().to_string();
    let chunks = all_chunks(&env, "/doc1.md", &id).await;
    let s1 = status(&env).await;

    let second = index_by_hand(&env, "/doc1.md", json!({})).await.unwrap();
    assert_eq!(second["indexed"], first["indexed"]);
    assert_eq!(default_of(&env, "/doc1.md").await.unwrap()["set_id"], id.as_str());
    assert_eq!(all_chunks(&env, "/doc1.md", &id).await, chunks);
    assert_eq!(status(&env).await, s1);

    let r = results(&env, "doc1").await;
    let mut seen: Vec<(String, u64)> = r
        .iter()
        .map(|x| (x["path"].as_str().unwrap().to_string(), x["chunk_number"].as_u64().unwrap()))
        .collect();
    assert_eq!(seen.len(), chunks.len(), "each chunk once: {r:?}");
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), chunks.len(), "no duplicate: {r:?}");
}

// ── US-0280: the rebuild builds the chunk sets before the search entries ─────

use crate::storage::traits::IndexMode;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\xff\xfe\0\0";

/// Saves `text` at `path` through `fs.write`, Alice having read it first.
async fn put(env: &Env, path: &str, text: &str) {
    env.state.safety.record_read(ALICE, PROJ, path);
    env.save(path, text).await;
}

/// Saves `bytes` at `path` through `fs.write_bytes`, Alice having read it first.
async fn put_bytes(env: &Env, path: &str, bytes: &[u8]) -> Value {
    use base64::Engine as _;
    env.state.safety.record_read(ALICE, PROJ, path);
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let a = args(json!({"mount_id": PROJ, "path": path, "base64": b64, "overwrite": true}));
    tool_json(env.server(ALICE).fs_write_bytes(a).await).expect("the write answers")
}

/// `/guide.md`: a title, then 3 sections of about 300 tokens under `## `.
fn guide3() -> String {
    format!(
        "# Guide\n\n## One\n\n{}\n\n## Two\n\nzeppelin {}\n\n## Three\n\n{}\n",
        prose(300),
        prose(300),
        prose(300)
    )
}

fn jobs_states(jobs: &[Value]) -> Vec<String> {
    jobs.iter().map(|j| j["state"].as_str().unwrap().to_string()).collect()
}

/// The env of E2E-002 and E2E-003: mode `both` with an embedding endpoint.
async fn env_both() -> Env {
    let env = Env::new(Options { delay: 3, embedding: true, ..Options::default() }).await;
    env.state.admin.set_index_mode(PROJ, IndexMode::Both).await.unwrap();
    env
}

/// SPEC-0101/E2E-001: a saved Markdown file gets its default set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_001_a_saved_markdown_file_gets_its_default_set() {
    let env = env().await;
    put(&env, "/guide.md", &guide3()).await;
    env.at(15).await;
    let v = sets_of(&env, "/guide.md").await;
    let sets = v["sets"].as_array().unwrap();
    assert_eq!(sets.len(), 1, "{v}");
    let s = &sets[0];
    assert_eq!(s["strategy"], "structural");
    assert_eq!(s["default"], true);
    assert_eq!(s["state"], "current");
    assert_eq!(s["chunk_count"], 1);
    assert_eq!(
        s["settings"],
        json!({"heading_cut_depth": 2, "list_max_items": 40, "max_tokens": 2048})
    );
    let chunks = all_chunks(&env, "/guide.md", s["set_id"].as_str().unwrap()).await;
    assert_eq!(s["token_count"], chunks[0]["token_count"]);
    let r = results(&env, "zeppelin").await;
    assert_eq!(r[0]["path"], "/guide.md");
    assert_eq!(r[0]["chunk_number"], 1);
}

/// SPEC-0101/E2E-002: a failed rebuild keeps the previous sets and entries.
#[tokio::test(start_paused = true)]
async fn e2e_0101_002_a_failed_rebuild_keeps_the_previous_sets() {
    let env = env_both().await;
    let old = format!("# Old\n\nlighthouse {}\n\n# Two\n\n{}\n", prose(1500), prose(1500));
    env.seed("/guide.md", &old).await;
    let before = default_of(&env, "/guide.md").await.unwrap();
    assert_eq!(before["chunk_count"], 2);
    let id = before["set_id"].as_str().unwrap().to_string();
    let old_chunks = all_chunks(&env, "/guide.md", &id).await;

    env.stub().set_down(true);
    put(&env, "/guide.md", "# New\n\nsubmarine text\n").await;
    env.at(15).await;
    let jobs = env.jobs("/guide.md").await;
    assert_eq!(jobs_states(&jobs), ["failed"], "{jobs:?}");
    assert_eq!(jobs[0]["reason"], "embedding service unreachable");
    let after = default_of(&env, "/guide.md").await.unwrap();
    assert_eq!(after["set_id"], id.as_str());
    assert_eq!(after["state"], "out_of_date");
    assert_eq!(after["chunk_count"], 2);
    assert_eq!(after["built_at"], before["built_at"]);
    assert_eq!(all_chunks(&env, "/guide.md", &id).await, old_chunks, "the old chunks");
    env.stub().set_down(false);
    assert!(env.finds("lighthouse", "/guide.md").await);
}

/// SPEC-0101/E2E-003: a save during a rebuild discards its sets.
#[tokio::test(start_paused = true)]
async fn e2e_0101_003_a_save_during_a_rebuild_discards_its_sets() {
    let env = env_both().await;
    env.seed("/guide.md", "# G\n\nfirst words\n").await;
    let id = default_of(&env, "/guide.md").await.unwrap()["set_id"].as_str().unwrap().to_string();
    env.stub().then_delays(&[10, 0]);
    put(&env, "/guide.md", "# G\n\ndiscarded words\n").await;
    env.at(5).await;
    assert_eq!(jobs_states(&env.jobs("/guide.md").await), ["running"]);
    put(&env, "/guide.md", "# G\n\nlast words\n").await;
    let mut seen = Vec::new();
    for t in 6..=30 {
        env.at(t).await;
        let s = default_of(&env, "/guide.md").await.unwrap();
        let chunks = all_chunks(&env, "/guide.md", &id).await;
        seen.push((t, s["state"].as_str().unwrap().to_string(), chunks[0]["text"].clone()));
    }
    for (t, state, text) in &seen {
        let text = text.as_str().unwrap();
        assert!(!text.contains("discarded"), "at {t}s: {text}");
        if state == "out_of_date" {
            assert!(text.contains("first"), "at {t}s the previous chunks: {text}");
        }
    }
    let (_, state, text) = seen.last().unwrap();
    assert_eq!(state, "current");
    assert!(text.as_str().unwrap().contains("last words"));
    // The discarded rebuild ran from 3 s to 13 s; the next one starts then.
    assert!(seen.iter().any(|(t, s, _)| *t == 13 && s == "out_of_date"), "{seen:?}");
}

/// SPEC-0101/E2E-004: a project that does not index has no default set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_004_a_project_that_does_not_index_has_no_default_set() {
    let env = env().await;
    env.state.admin.set_index_mode(PROJ, IndexMode::None).await.unwrap();
    put(&env, "/guide.md", &guide3()).await;
    env.at(15).await;
    assert_eq!(sets_of(&env, "/guide.md").await, json!({"sets": []}));
    let set = create(&env, ALICE, "/guide.md", "fixed", json!({})).await.unwrap();
    assert_eq!(set["default"], false);
    assert_eq!(set_count(&sets_of(&env, "/guide.md").await), 1);
}

/// SPEC-0101/E2E-005: a binary file gets no set and no error.
#[tokio::test(start_paused = true)]
async fn e2e_0101_005_a_binary_file_gets_no_set_and_no_error() {
    let env = env().await;
    let png = [PNG, &[0u8; 2048]].concat();
    let v = put_bytes(&env, "/logo.png", &png).await;
    assert!(v.get("error").is_none(), "{v}");
    env.at(15).await;
    assert_eq!(sets_of(&env, "/logo.png").await, json!({"sets": []}));
    let jobs = env.jobs("/logo.png").await;
    assert!(!jobs_states(&jobs).contains(&"failed".to_string()), "{jobs:?}");
}

/// SPEC-0101/E2E-006: a file that stops being text loses its sets.
#[tokio::test(start_paused = true)]
async fn e2e_0101_006_a_file_that_stops_being_text_loses_its_sets() {
    let env = env().await;
    env.seed("/data.md", "# Data\n\nmarmalade notes\n").await;
    create(&env, ALICE, "/data.md", "fixed", json!({})).await.unwrap();
    assert_eq!(set_count(&sets_of(&env, "/data.md").await), 2);
    put_bytes(&env, "/data.md", PNG).await;
    env.at(15).await;
    assert_eq!(sets_of(&env, "/data.md").await, json!({"sets": []}));
    assert_eq!(jobs_states(&env.jobs("/data.md").await), ["done"]);
    assert!(!env.finds("marmalade", "/data.md").await, "its entries went too");
}

/// SPEC-0101/E2E-007: the default set is rebuilt with the current server defaults.
#[tokio::test(start_paused = true)]
async fn e2e_0101_007_the_default_set_follows_the_server_defaults() {
    let mut env = env().await;
    env.seed("/guide.md", "# T\n\nshort\n").await;
    let id = default_of(&env, "/guide.md").await.unwrap()["set_id"].clone();
    assert_eq!(default_of(&env, "/guide.md").await.unwrap()["settings"]["max_tokens"], 2048);
    let extra = create(&env, ALICE, "/guide.md", "structural", json!({})).await;
    // Equal to the default before the restart: answered by the default set.
    assert_eq!(extra.unwrap()["default"], true);
    let hand =
        create(&env, ALICE, "/guide.md", "structural", json!({"max_tokens": 300})).await.unwrap();

    env.restart_with(|c| c.search.chunking.max_tokens = "512".into()).await;
    let paras: Vec<String> = (0..6).map(|i| para(&format!("p{i}"), 499)).collect();
    put(&env, "/guide.md", &format!("# T\n\n{}\n", paras.join("\n\n"))).await;
    env.at(15).await;
    let d = default_of(&env, "/guide.md").await.unwrap();
    assert_eq!(d["set_id"], id, "the default keeps its id");
    assert_eq!(d["state"], "current");
    assert_eq!(d["settings"]["max_tokens"], 512);
    let chunks = all_chunks(&env, "/guide.md", d["set_id"].as_str().unwrap()).await;
    assert!(chunks.len() >= 6, "{}", chunks.len());
    assert!(chunks.iter().all(|c| c["token_count"].as_u64().unwrap() <= 512));
    let v = sets_of(&env, "/guide.md").await;
    let kept = v["sets"].as_array().unwrap().iter().find(|s| s["set_id"] == hand["set_id"]);
    let kept = kept.unwrap_or_else(|| panic!("the extra set is kept: {v}"));
    assert_eq!(kept["settings"]["max_tokens"], 300, "its own settings");
    assert_eq!(kept["state"], "current", "rebuilt too");
}

/// SPEC-0101 story check (FR-NEW-021, DEC-013): every extra set is rebuilt with
/// its own request, keeping its id.
#[tokio::test(start_paused = true)]
async fn e2e_0101_us0280_extra_sets_are_rebuilt_keeping_their_ids() {
    let env = env().await;
    env.seed("/guide.md", "# G\n\nalpha words\n").await;
    let fixed = create(&env, ALICE, "/guide.md", "fixed", json!({"size_tokens": 64})).await;
    let fixed = fixed.unwrap();
    env.at(5).await;
    put(&env, "/guide.md", "# G\n\nbeta words\n").await;
    env.at(15).await;
    let v = sets_of(&env, "/guide.md").await;
    let sets = v["sets"].as_array().unwrap();
    assert_eq!(sets.len(), 2, "{v}");
    let f = sets.iter().find(|s| s["set_id"] == fixed["set_id"]).expect("same id");
    assert_eq!(f["state"], "current");
    assert_eq!(f["settings"], json!({"overlap_tokens": 0, "size_tokens": 64}));
    let chunks = all_chunks(&env, "/guide.md", f["set_id"].as_str().unwrap()).await;
    assert!(chunks[0]["text"].as_str().unwrap().contains("beta"));
}

/// SPEC-0101 story check (DEC-005): a default whose key changed with the
/// server defaults absorbs a hand made set with the new key.
#[tokio::test(start_paused = true)]
async fn e2e_0101_us0280_the_default_absorbs_a_set_with_its_new_key() {
    let mut env = env().await;
    env.seed("/guide.md", "# G\n\nalpha words\n").await;
    let id = default_of(&env, "/guide.md").await.unwrap()["set_id"].clone();
    let hand =
        create(&env, ALICE, "/guide.md", "structural", json!({"max_tokens": 512})).await.unwrap();
    assert_eq!(hand["default"], false);
    env.restart_with(|c| c.search.chunking.max_tokens = "512".into()).await;
    put(&env, "/guide.md", "# G\n\nbeta words\n").await;
    env.at(15).await;
    let v = sets_of(&env, "/guide.md").await;
    let sets = v["sets"].as_array().unwrap();
    assert_eq!(sets.len(), 1, "the hand made set is absorbed: {v}");
    assert_eq!(sets[0]["set_id"], id);
    assert_eq!(sets[0]["default"], true);
    assert_eq!(sets[0]["settings"]["max_tokens"], 512);
    assert_eq!(jobs_states(&env.jobs("/guide.md").await), ["done"]);
}

/// SPEC-0101/E2E-032: a set equal to the default set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_032_a_set_equal_to_the_default_set() {
    let env = env().await;
    put(&env, "/essay.md", &format!("# Essay\n\n{}\n", prose(200))).await;
    env.at(15).await;
    let d = default_of(&env, "/essay.md").await.unwrap();
    let v = create(&env, ALICE, "/essay.md", "structural", json!({})).await.unwrap();
    assert_eq!(v["default"], true);
    assert_eq!(v["set_id"], d["set_id"]);
    assert_eq!(set_count(&sets_of(&env, "/essay.md").await), 1);
}

/// Converts `/report.pdf` through a stub document service; its companion is
/// `/report.md`.
async fn convert(env: &Env, markdown: &str) {
    let s = &env.state;
    let converting = crate::state::AppState {
        config: s.config.clone(),
        admin: s.admin.clone(),
        stores: s.stores.clone(),
        safety: s.safety.clone(),
        identity: s.identity.clone(),
        editors: s.editors.clone(),
        doc_service: Some(std::sync::Arc::new(crate::docs::service::StubDocService::ok(markdown))),
        search: s.search.clone(),
    };
    let server = crate::mcp::server::McpServer::new(std::sync::Arc::new(converting), ALICE.into());
    s.safety.record_read(ALICE, PROJ, "/report.md");
    let a = args(json!({"mount_id": PROJ, "path": "/report.pdf", "overwrite": true}));
    tool_json(server.fs_documentize(a).await).unwrap();
}

/// SPEC-0101/E2E-053: no automatic default set for a converted document; its
/// companion gets one at once (FR-NEW-021).
#[tokio::test(start_paused = true)]
async fn e2e_0101_053_no_automatic_default_set_for_a_converted_document() {
    let env = env().await;
    put_bytes(&env, "/report.pdf", b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n").await;
    convert(&env, "# Report\n\nThe yearly figures.\n").await;
    // The companion is indexed at once, never scheduled.
    tokio::task::yield_now().await;
    env.at(1).await;
    let md = default_of(&env, "/report.md").await.expect("the companion has a default set");
    assert_eq!(md["state"], "current");
    env.at(15).await;
    assert_eq!(sets_of(&env, "/report.pdf").await, json!({"sets": []}));
}

/// SPEC-0101/E2E-060: a text without headings falls back to fixed.
#[tokio::test(start_paused = true)]
async fn e2e_0101_060_a_text_without_headings_falls_back_to_fixed() {
    let env = env().await;
    put(&env, "/notes.txt", &prose(3000)).await;
    env.at(15).await;
    let d = default_of(&env, "/notes.txt").await.unwrap();
    assert_eq!(d["strategy"], "fixed");
    assert_eq!(d["fallback_from"], "structural");
    assert_eq!(d["settings"], json!({"overlap_tokens": 0, "size_tokens": 2048}));
    assert_eq!(d["chunk_count"], 2);
    let chunks = all_chunks(&env, "/notes.txt", d["set_id"].as_str().unwrap()).await;
    assert_eq!(chunks[1]["token_count"], 952);
}

/// SPEC-0101/E2E-061: adding a heading ends the fallback.
#[tokio::test(start_paused = true)]
async fn e2e_0101_061_adding_a_heading_ends_the_fallback() {
    let env = env().await;
    let text = prose(3000);
    put(&env, "/notes.txt", &text).await;
    env.at(15).await;
    assert_eq!(default_of(&env, "/notes.txt").await.unwrap()["strategy"], "fixed");
    put(&env, "/notes.txt", &format!("# Notes\n{text}")).await;
    env.at(30).await;
    let d = default_of(&env, "/notes.txt").await.unwrap();
    assert_eq!(d["strategy"], "structural");
    assert!(d.get("fallback_from").is_none(), "{d}");
    let chunks = all_chunks(&env, "/notes.txt", d["set_id"].as_str().unwrap()).await;
    assert!(chunks[0]["text"].as_str().unwrap().starts_with("# Notes"));
}

/// SPEC-0101/E2E-116: a rebuild by hand of the entries builds the sets first.
#[tokio::test(start_paused = true)]
async fn e2e_0101_116_a_rebuild_by_hand_builds_the_sets_first() {
    let env = env().await;
    file(&env, "/guide.md", GUIDE).await;
    pre_upgrade_entry(&env, "/guide.md", GUIDE).await;
    env.recalc(ALICE, PROJ, "/guide.md", "search_index").await.unwrap();
    env.at(7).await;
    let d = default_of(&env, "/guide.md").await.expect("a default set");
    let r = results(&env, "kubelet").await;
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0]["chunk_number"], 1);
    let chunks = all_chunks(&env, "/guide.md", d["set_id"].as_str().unwrap()).await;
    assert_eq!(r[0]["chunk"], chunks[0]["text"]);
}

/// SPEC-0101/E2E-117: out of date while a rebuild waits.
#[tokio::test(start_paused = true)]
async fn e2e_0101_117_out_of_date_while_a_rebuild_waits() {
    let env = env().await;
    env.seed("/guide.md", "# G\n\nold words\n").await;
    let id = default_of(&env, "/guide.md").await.unwrap()["set_id"].as_str().unwrap().to_string();
    put(&env, "/guide.md", "# G\n\nnew words\n").await;
    env.at(1).await;
    assert_eq!(default_of(&env, "/guide.md").await.unwrap()["state"], "out_of_date");
    assert!(all_chunks(&env, "/guide.md", &id).await[0]["text"].as_str().unwrap().contains("old"));
    env.at(15).await;
    assert_eq!(default_of(&env, "/guide.md").await.unwrap()["state"], "current");
    assert!(all_chunks(&env, "/guide.md", &id).await[0]["text"].as_str().unwrap().contains("new"));
}

/// SPEC-0101/E2E-130: server defaults are used.
#[tokio::test(start_paused = true)]
async fn e2e_0101_130_server_defaults_are_used() {
    let env = Env::new(Options {
        delay: 3,
        config: |c| {
            c.search.chunking.max_tokens = "1024".into();
            c.search.chunking.list_max_items = "20".into();
        },
        ..Options::default()
    })
    .await;
    put(&env, "/guide.md", "# T\n\nsome words\n").await;
    env.at(15).await;
    let d = default_of(&env, "/guide.md").await.unwrap();
    assert_eq!(d["settings"]["max_tokens"], 1024);
    assert_eq!(d["settings"]["list_max_items"], 20);
    file(&env, "/other.md", "# T\n\nsome words\n").await;
    let hand = create(&env, ALICE, "/other.md", "structural", json!({})).await.unwrap();
    assert_eq!(hand["settings"], d["settings"]);
}

/// SPEC-0101/E2E-133: absent or empty defaults build the default set with
/// 2048, 2, 40, and a fallback with 2048 and 0.
#[tokio::test(start_paused = true)]
async fn e2e_0101_133_absent_or_empty_defaults_at_the_rebuild() {
    let empty = Env::new(Options {
        delay: 3,
        config: |c| {
            c.search.chunking.max_tokens = String::new();
            c.search.chunking.heading_cut_depth = String::new();
        },
        ..Options::default()
    })
    .await;
    for env in [env().await, empty] {
        put(&env, "/a.md", "# T\n\nsome words\n").await;
        put(&env, "/b.txt", "no heading here\n").await;
        // Each env has its own time 0, so the wait is relative.
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        let a = default_of(&env, "/a.md").await.unwrap();
        assert_eq!(
            a["settings"],
            json!({"heading_cut_depth": 2, "list_max_items": 40, "max_tokens": 2048})
        );
        let b = default_of(&env, "/b.txt").await.unwrap();
        assert_eq!(b["settings"], json!({"overlap_tokens": 0, "size_tokens": 2048}));
    }
}

/// SPEC-0101/E2E-134: nothing converts at upgrade.
#[tokio::test(start_paused = true)]
async fn e2e_0101_134_nothing_converts_at_upgrade() {
    let mut env = env().await;
    let text = "# Old\n\nnotes about lighthouses\n";
    file(&env, "/old.md", text).await;
    pre_upgrade_entry(&env, "/old.md", text).await;
    env.restart().await;
    env.at(60).await;
    assert_eq!(sets_of(&env, "/old.md").await, json!({"sets": []}));
    assert!(env.finds("lighthouses", "/old.md").await);
    assert!(env.jobs("/old.md").await.is_empty());
}

/// SPEC-0101/E2E-135: the first rebuild converts the file.
#[tokio::test(start_paused = true)]
async fn e2e_0101_135_the_first_rebuild_converts_the_file() {
    let env = env().await;
    let text = "# Old\n\nnotes about lighthouses\n";
    file(&env, "/old.md", text).await;
    pre_upgrade_entry(&env, "/old.md", text).await;
    put(&env, "/old.md", text).await;
    env.at(15).await;
    assert!(default_of(&env, "/old.md").await.is_some());
    let r = results(&env, "lighthouses").await;
    assert!(!r.is_empty());
    assert!(r.iter().all(|x| x["chunk_number"] != 0), "{r:?}");
}

/// SPEC-0101/E2E-136: a rebuild by hand converts one file only.
#[tokio::test(start_paused = true)]
async fn e2e_0101_136_a_rebuild_by_hand_converts_one_file_only() {
    let env = env().await;
    for p in ["/old1.md", "/old2.md"] {
        let text = "# Old\n\nnotes about lighthouses\n";
        file(&env, p, text).await;
        pre_upgrade_entry(&env, p, text).await;
    }
    env.recalc(ALICE, PROJ, "/old1.md", "search_index").await.unwrap();
    env.at(7).await;
    assert!(default_of(&env, "/old1.md").await.is_some());
    assert_eq!(sets_of(&env, "/old2.md").await, json!({"sets": []}));
    assert!(env.finds("lighthouses", "/old1.md").await);
    assert!(env.finds("lighthouses", "/old2.md").await);
}

/// SPEC-0101/E2E-140: a non text file is not rebuilt automatically.
#[tokio::test(start_paused = true)]
async fn e2e_0101_140_a_non_text_file_is_not_rebuilt_automatically() {
    let env = env().await;
    put_bytes(&env, "/report.pdf", b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n").await;
    convert(&env, "# Report\n\nThe yearly figures.\n").await;
    env.at(15).await;
    let hand = create(&env, ALICE, "/report.pdf", "fixed", json!({})).await.unwrap();
    let id = hand["set_id"].as_str().unwrap().to_string();
    let before = all_chunks(&env, "/report.pdf", &id).await;

    put_bytes(&env, "/report.pdf", b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\nversion two\n").await;
    convert(&env, "# Report\n\nThe new figures.\n").await;
    env.at(30).await;
    let v = sets_of(&env, "/report.pdf").await;
    assert_eq!(v["sets"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["sets"][0]["state"], "out_of_date");
    assert_eq!(v["sets"][0]["built_at"], hand["built_at"]);
    assert_eq!(all_chunks(&env, "/report.pdf", &id).await, before, "unchanged");
    let jobs = env.jobs("/report.pdf").await;
    assert_eq!(jobs_states(&jobs).last().map(String::as_str), Some("done"), "{jobs:?}");
}

/// SPEC-0101/E2E-141: a mode change keeps the sets.
#[tokio::test(start_paused = true)]
async fn e2e_0101_141_a_mode_change_keeps_the_sets() {
    let env = env().await;
    env.seed("/guide.md", "# G\n\nold words\n").await;
    let before = default_of(&env, "/guide.md").await.unwrap();
    env.state.admin.set_index_mode(PROJ, IndexMode::None).await.unwrap();
    let indexer = crate::search::indexer::ProjectIndexer::new(
        env.state.search.as_ref().unwrap(),
        env.state.config.search.chunking(),
    );
    let client = env.state.stores.client(PROJ).await.unwrap();
    indexer
        .on_mode_change(PROJ, IndexMode::Bm25, IndexMode::None, client.clone(), ".mcp_trash")
        .await
        .unwrap();
    put(&env, "/guide.md", "# G\n\nnew words\n").await;
    env.at(15).await;
    let d = default_of(&env, "/guide.md").await.unwrap();
    assert_eq!(d["state"], "out_of_date");
    assert_eq!(d["built_at"], before["built_at"]);
    assert_eq!(d["chunk_count"], before["chunk_count"]);
    assert!(env.jobs("/guide.md").await.is_empty(), "no rebuild ran");

    env.state.admin.set_index_mode(PROJ, IndexMode::Bm25).await.unwrap();
    indexer
        .on_mode_change(PROJ, IndexMode::None, IndexMode::Bm25, client, ".mcp_trash")
        .await
        .unwrap();
    env.at(20).await;
    let d = default_of(&env, "/guide.md").await.unwrap();
    assert_eq!(d["state"], "current");
    assert_eq!(d["set_id"], before["set_id"]);
}

// ── US-0281: sets follow moves, copies and deletions ────────────────────────

/// `env()` with hard delete allowed, so `fs.delete` can skip the trash.
async fn env_hard() -> Env {
    Env::new(Options {
        delay: 3,
        config: |c| c.safety.allow_hard_delete = true,
        ..Options::default()
    })
    .await
}

async fn mv(env: &Env, from: &str, to: &str, overwrite: bool) {
    let a =
        args(json!({"mount_id": PROJ, "source": from, "destination": to, "overwrite": overwrite}));
    tool_json(env.server(ALICE).fs_move(a).await).expect("the move succeeds");
}

async fn cp(env: &Env, from: &str, to: &str) {
    let a = args(json!({"mount_id": PROJ, "source": from, "destination": to, "recursive": true}));
    tool_json(env.server(ALICE).fs_copy(a).await).expect("the copy succeeds");
}

async fn rm(env: &Env, path: &str, recursive: bool, trash: bool) {
    let a = args(json!({"mount_id": PROJ, "path": path, "recursive": recursive, "trash": trash}));
    tool_json(env.server(ALICE).fs_delete(a).await).expect("the delete succeeds");
}

/// Every chunk set row of the volume, by path, straight from the table.
async fn set_paths(env: &Env) -> Vec<String> {
    let client = env.state.stores.client(PROJ).await.unwrap();
    client.meta.chunk_store().unwrap().paths_with_sets().await.unwrap()
}

/// `(set_id, chunks)` of every set of `path`, in listing order.
async fn snapshot(env: &Env, path: &str) -> Vec<(String, Vec<Value>)> {
    let v = sets_of(env, path).await;
    let mut out = Vec::new();
    for s in v["sets"].as_array().unwrap() {
        let id = s["set_id"].as_str().unwrap().to_string();
        let chunks = all_chunks(env, path, &id).await;
        out.push((id, chunks));
    }
    out
}

/// `/a.md` with its default set and an extra fixed set.
async fn two_sets(env: &Env, path: &str) {
    env.seed(path, "# A\n\nalpha words\n").await;
    create(env, ALICE, path, "fixed", json!({"size_tokens": 64})).await.unwrap();
    assert_eq!(set_count(&sets_of(env, path).await), 2);
}

/// SPEC-0101/E2E-100: a move carries the sets.
#[tokio::test(start_paused = true)]
async fn e2e_0101_100_a_move_carries_the_sets() {
    let env = env().await;
    two_sets(&env, "/a.md").await;
    let before = snapshot(&env, "/a.md").await;
    mv(&env, "/a.md", "/b.md", false).await;
    assert_eq!(snapshot(&env, "/b.md").await, before);
    let moved = sets_of(&env, "/b.md").await;
    assert!(moved["sets"].as_array().unwrap().iter().all(|s| s["state"] == "current"), "{moved}");
    assert_refused(
        list_sets(&env, ALICE, PROJ, "/a.md").await,
        "ERR_NOT_FOUND: file not found: /a.md",
    );
    assert_eq!(set_paths(&env).await, ["/b.md", "/b.md"]);
}

/// SPEC-0101/E2E-101: a copy gets its own default set only.
#[tokio::test(start_paused = true)]
async fn e2e_0101_101_a_copy_gets_its_own_default_set_only() {
    let env = env().await;
    two_sets(&env, "/a.md").await;
    cp(&env, "/a.md", "/c.md").await;
    assert!(set_paths(&env).await.iter().all(|p| p == "/a.md"), "a copy copies no set");
    env.at(15).await;
    let c = sets_of(&env, "/c.md").await;
    assert_eq!(set_count(&c), 1, "{c}");
    assert_eq!(c["sets"][0]["default"], true);
    assert_eq!(set_count(&sets_of(&env, "/a.md").await), 2);
}

/// SPEC-0101/E2E-102: a delete removes the sets.
#[tokio::test(start_paused = true)]
async fn e2e_0101_102_a_delete_removes_the_sets() {
    let env = env_hard().await;
    two_sets(&env, "/a.md").await;
    rm(&env, "/a.md", false, false).await;
    assert_refused(
        list_sets(&env, ALICE, PROJ, "/a.md").await,
        "ERR_NOT_FOUND: file not found: /a.md",
    );
    assert!(set_paths(&env).await.is_empty());
    put(&env, "/a.md", "# New\n\nfresh\n").await;
    assert_eq!(set_count(&sets_of(&env, "/a.md").await), 0, "no set until its rebuild");
    assert!(set_paths(&env).await.is_empty());
}

/// A delete into the trash removes the sets too.
#[tokio::test(start_paused = true)]
async fn e2e_0101_us0281_a_move_to_the_trash_removes_the_sets() {
    let env = env().await;
    two_sets(&env, "/a.md").await;
    rm(&env, "/a.md", false, true).await;
    assert!(set_paths(&env).await.is_empty(), "{:?}", set_paths(&env).await);
}

/// SPEC-0101/E2E-103: a move over an existing file.
#[tokio::test(start_paused = true)]
async fn e2e_0101_103_a_move_over_an_existing_file() {
    let env = env().await;
    env.seed("/a.md", "# A\n\nalpha\n").await;
    env.seed("/b.md", "# B\n\nbeta\n").await;
    let moved = snapshot(&env, "/a.md").await;
    mv(&env, "/a.md", "/b.md", true).await;
    assert_eq!(snapshot(&env, "/b.md").await, moved);
    assert_eq!(set_paths(&env).await, ["/b.md"]);
    assert_refused(
        list_sets(&env, ALICE, PROJ, "/a.md").await,
        "ERR_NOT_FOUND: file not found: /a.md",
    );
}

/// SPEC-0101/E2E-104: a project deleted, then a project of the same id.
#[tokio::test(start_paused = true)]
async fn e2e_0101_104_a_project_deleted_then_restored() {
    let env = env().await;
    env.seed("/a.md", "# A\n\nalpha\n").await;
    assert_eq!(set_paths(&env).await, ["/a.md"]);
    assert!(env.state.admin.soft_delete_project(PROJ).await.unwrap());
    let e = list_sets(&env, ALICE, PROJ, "/a.md").await.unwrap_err();
    assert!(e.contains("ERR_PROJECT_NOT_FOUND"), "{e}");
    assert!(set_paths(&env).await.is_empty(), "the sets go with the project");
    let a = args(json!({"project_id": PROJ}));
    tool_json(env.server(ALICE).admin_undelete_project(a).await).unwrap();
    assert!(set_paths(&env).await.is_empty(), "no set of the deleted files reappears");
    let v = sets_of(&env, "/a.md").await;
    assert_eq!(set_count(&v), 0, "no set until the next rebuild: {v}");
}

/// SPEC-0101/E2E-105: a folder move.
#[tokio::test(start_paused = true)]
async fn e2e_0101_105_a_folder_move() {
    let env = env().await;
    env.seed("/docs/a.md", "# A\n\nalpha\n").await;
    env.seed("/docs/b.md", "# B\n\nbeta\n").await;
    let (a, b) = (snapshot(&env, "/docs/a.md").await, snapshot(&env, "/docs/b.md").await);
    mv(&env, "/docs", "/manual", false).await;
    assert_eq!(snapshot(&env, "/manual/a.md").await, a);
    assert_eq!(snapshot(&env, "/manual/b.md").await, b);
    assert_eq!(set_paths(&env).await, ["/manual/a.md", "/manual/b.md"]);
}

/// SPEC-0101/E2E-146: copying and deleting a folder.
#[tokio::test(start_paused = true)]
async fn e2e_0101_146_copying_and_deleting_a_folder() {
    let env = env_hard().await;
    env.seed("/docs/a.md", "# A\n\nalpha\n").await;
    env.seed("/docs/b.md", "# B\n\nbeta\n").await;
    cp(&env, "/docs", "/copy").await;
    env.at(15).await;
    for p in ["/copy/a.md", "/copy/b.md"] {
        let v = sets_of(&env, p).await;
        assert_eq!(set_count(&v), 1, "{p}: {v}");
        assert_eq!(v["sets"][0]["default"], true);
    }
    rm(&env, "/docs", true, false).await;
    assert_refused(
        list_sets(&env, ALICE, PROJ, "/docs/a.md").await,
        "ERR_NOT_FOUND: file not found: /docs/a.md",
    );
    assert_eq!(set_paths(&env).await, ["/copy/a.md", "/copy/b.md"]);
}

// ── US-0282: recalculating chunks by hand ───────────────────────────────────

/// `fs.recalculate` as `person` with an optional `set_id` (empty is absent).
async fn recalc(
    env: &Env,
    person: &str,
    mount: &str,
    path: &str,
    kind: &str,
    set_id: &str,
) -> Result<Value, String> {
    let mut a = json!({"mount_id": mount, "path": path, "kind": kind});
    if !set_id.is_empty() {
        a["set_id"] = json!(set_id);
    }
    tool_json(env.server(person).fs_recalculate(args(a)).await)
}

/// The set `set_id` of `path` as `fs.list_chunk_sets` shows it.
async fn set_of_id(env: &Env, path: &str, set_id: &str) -> Value {
    let v = sets_of(env, path).await;
    v["sets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["set_id"] == set_id)
        .cloned()
        .unwrap_or_else(|| panic!("{set_id} is not in {v}"))
}

/// No row of `derived_job` of kind `chunks` for any project.
async fn no_chunks_job(env: &Env) {
    let rows = env.jobs.load_unfinished(None).await.unwrap();
    assert!(rows.iter().all(|j| j.kind.as_str() != "chunks"), "no chunks job started: {rows:?}");
}

/// SPEC-0101/E2E-110: recalculating the default set by hand.
#[tokio::test(start_paused = true)]
async fn e2e_0101_110_recalculating_the_default_set_by_hand() {
    let env = Env::new(Options { delay: 30, ..Options::default() }).await;
    env.seed("/guide.md", "# Guide\n\nolden words\n").await;
    let id = default_of(&env, "/guide.md").await.unwrap()["set_id"].as_str().unwrap().to_string();
    put(&env, "/guide.md", "# Guide\n\nmarmoset words\n").await;
    env.at(5).await;
    assert_eq!(default_of(&env, "/guide.md").await.unwrap()["state"], "out_of_date");
    let job = recalc(&env, ALICE, PROJ, "/guide.md", "chunks", "").await.unwrap();
    assert_eq!(job["kind"], "chunks", "{job}");
    assert_eq!(job["cause"], "manual");
    assert!(["running", "done"].contains(&job["state"].as_str().unwrap()), "{job}");
    assert_eq!(job["set_id"], id.as_str());
    env.at(12).await;
    let after = default_of(&env, "/guide.md").await.unwrap();
    assert_eq!(after["set_id"], id.as_str());
    assert_eq!(after["state"], "current");
    let chunks = all_chunks(&env, "/guide.md", &id).await;
    assert!(chunks[0]["text"].as_str().unwrap().contains("marmoset"), "{chunks:?}");
    assert!(env.finds("marmoset", "/guide.md").await, "the entries follow the default set");
    assert!(!env.finds("olden", "/guide.md").await);
    let jobs = env.jobs("/guide.md").await;
    let kinds: Vec<(&str, &str)> =
        jobs.iter().map(|j| (j["kind"].as_str().unwrap(), j["state"].as_str().unwrap())).collect();
    assert!(kinds.contains(&("search_index", "waiting")), "{jobs:?}");
    let waiting = jobs.iter().find(|j| j["kind"] == "search_index").unwrap();
    assert_eq!(waiting["seconds_left"], 18, "the waiting rebuild keeps its time: {waiting}");
}

/// SPEC-0101/E2E-111: recalculating an extra set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_111_recalculating_an_extra_set() {
    let env = env().await;
    env.seed("/guide.md", "# Guide\n\nolden words\n").await;
    let extra = create(&env, ALICE, "/guide.md", "fixed", json!({})).await.unwrap();
    let id = extra["set_id"].as_str().unwrap().to_string();
    let default_before = default_of(&env, "/guide.md").await.unwrap();
    put(&env, "/guide.md", "# Guide\n\nmarmoset words\n").await;
    let job = recalc(&env, ALICE, PROJ, "/guide.md", "chunks", &id).await.unwrap();
    assert_eq!((job["kind"].as_str(), job["set_id"].as_str()), (Some("chunks"), Some(id.as_str())));
    env.at(1).await;
    let after = set_of_id(&env, "/guide.md", &id).await;
    assert_eq!(after["state"], "current", "{after}");
    let chunks = all_chunks(&env, "/guide.md", &id).await;
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0]["text"], "# Guide\n\nmarmoset words\n");
    let default_after = default_of(&env, "/guide.md").await.unwrap();
    assert_eq!(default_after["built_at"], default_before["built_at"]);
    assert_eq!(default_after["state"], "out_of_date", "the default set is untouched");
    assert!(!env.finds("marmoset", "/guide.md").await, "the search entries are not rebuilt");
    assert!(env.finds("olden", "/guide.md").await);
}

/// SPEC-0101/E2E-112: recalculating an unknown set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_112_recalculating_an_unknown_set() {
    let env = env().await;
    env.seed("/guide.md", "# Guide\n\nwords\n").await;
    let r = recalc(&env, ALICE, PROJ, "/guide.md", "chunks", "set-999").await;
    assert_refused(r, "ERR_NOT_FOUND: chunk set not found: set-999");
    no_chunks_job(&env).await;
    assert!(env.jobs("/guide.md").await.is_empty());
}

/// SPEC-0101/E2E-113: recalculating when there is no default set.
#[tokio::test(start_paused = true)]
async fn e2e_0101_113_recalculating_with_no_default_set() {
    let env = env().await;
    env.state.admin.create_project("plain", ALICE).await.unwrap();
    env.state.stores.provision_volume("plain").await.unwrap();
    env.save_in("plain", "/guide.md", "# Guide\n\nwords\n").await;
    let r = recalc(&env, ALICE, "plain", "/guide.md", "chunks", "").await;
    assert_refused(r, "ERR_NOT_FOUND: the file has no default chunk set");
    no_chunks_job(&env).await;
}

/// SPEC-0101/E2E-114: `set_id` with the wrong kind.
#[tokio::test(start_paused = true)]
async fn e2e_0101_114_set_id_with_the_wrong_kind() {
    let env = env().await;
    env.seed("/guide.md", "# Guide\n\nwords\n").await;
    let id = default_of(&env, "/guide.md").await.unwrap()["set_id"].as_str().unwrap().to_string();
    let r = recalc(&env, ALICE, PROJ, "/guide.md", "search_index", &id).await;
    assert_refused(r, "ERR_INVALID_ARGUMENT: set_id applies to kind 'chunks' only");
    assert!(env.jobs.load_unfinished(None).await.unwrap().is_empty());
}

/// SPEC-0101/E2E-115: an unknown kind.
#[tokio::test(start_paused = true)]
async fn e2e_0101_115_an_unknown_kind() {
    let env = env().await;
    env.seed("/guide.md", "# Guide\n\nwords\n").await;
    let r = recalc(&env, ALICE, PROJ, "/guide.md", "concepts", "").await;
    assert_refused(r, "ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'");
    assert!(env.jobs.load_unfinished(None).await.unwrap().is_empty());
}

/// SPEC-0101/E2E-138: recalculating with search not enabled.
#[tokio::test(start_paused = true)]
async fn e2e_0101_138_recalculating_with_search_not_enabled() {
    let env = Env::new(Options { search: false, delay: 3, ..Options::default() }).await;
    env.state.admin.create_project("plain", ALICE).await.unwrap();
    env.state.stores.provision_volume("plain").await.unwrap();
    let client = env.state.stores.client("plain").await.unwrap();
    client.write_text_atomic("/guide.md", "# Guide\n\nolden words\n").await.unwrap();
    let sha = client.stat("/guide.md").await.unwrap().sha256.unwrap_or_default();
    let store = client.meta.chunk_store().unwrap();
    let req = crate::chunking::service::Requested::parse(
        "fixed",
        &serde_json::Map::new(),
        &env.state.config.search.chunking(),
    )
    .unwrap();
    let made = crate::chunking::service::create_set(
        &env.state,
        &client,
        &store,
        "plain",
        "/guide.md",
        &sha,
        req,
    )
    .await
    .unwrap();
    client.write_text_atomic("/guide.md", "# Guide\n\nmarmoset words\n").await.unwrap();
    let job = recalc(&env, ALICE, "plain", "/guide.md", "chunks", &made.set_id).await.unwrap();
    assert_eq!(job["kind"], "chunks", "{job}");
    env.at(7).await;
    let sets = store.list_sets("/guide.md").await.unwrap();
    let new_sha = client.stat("/guide.md").await.unwrap().sha256.unwrap_or_default();
    assert_eq!(sets[0].source_sha, new_sha, "the set is current");
    let chunks = store.get_chunk_window(&made.set_id, 1, u32::MAX).await.unwrap();
    assert!(chunks[0].text.contains("marmoset"), "{chunks:?}");
    let r = recalc(&env, ALICE, "plain", "/guide.md", "search_index", "").await;
    assert_refused(
        r,
        "ERR_NOT_SUPPORTED: search is not enabled; set search.enabled: true in config",
    );
    let a = args(json!({"mount_id": "plain", "query": "marmoset", "mode": "bm25"}));
    let found = tool_json(env.server(ALICE).search_query(a).await);
    assert!(found.is_err() || found.unwrap()["results"].as_array().is_none_or(Vec::is_empty));
}

/// SPEC-0101/E2E-052: a converted document that changed.
#[tokio::test(start_paused = true)]
async fn e2e_0101_052_a_converted_document_that_changed() {
    use crate::docs::service::{Conversion, StubDocService};
    let env = env().await;
    let client = env.state.stores.client(PROJ).await.unwrap();
    let s = &env.state;
    let convert = |markdown: &str, overwrite: bool| {
        let conversion =
            Conversion { markdown: markdown.into(), files: Default::default(), manifest: None };
        let converting = crate::state::AppState {
            config: s.config.clone(),
            admin: s.admin.clone(),
            stores: s.stores.clone(),
            safety: s.safety.clone(),
            identity: s.identity.clone(),
            editors: s.editors.clone(),
            doc_service: Some(std::sync::Arc::new(StubDocService::bundle(conversion))),
            search: None,
        };
        let server =
            crate::mcp::server::McpServer::new(std::sync::Arc::new(converting), ALICE.into());
        let a = args(json!({"mount_id": PROJ, "path": "/report.pdf", "overwrite": overwrite}));
        async move { tool_json(server.fs_documentize(a).await).unwrap() }
    };
    client.write_bytes_atomic("/report.pdf", b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\nv1\n").await.unwrap();
    convert("# Report\n\nolden numbers\n", false).await;
    let set = create(&env, ALICE, "/report.pdf", "fixed", json!({})).await.unwrap();
    let id = set["set_id"].as_str().unwrap().to_string();
    assert_eq!(set["state"], "current");
    assert!(
        all_chunks(&env, "/report.pdf", &id).await[0]["text"].as_str().unwrap().contains("olden")
    );

    client.write_bytes_atomic("/report.pdf", b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\nv2\n").await.unwrap();
    convert("# Report\n\nmarmoset numbers\n", true).await;
    let before = set_of_id(&env, "/report.pdf", &id).await;
    assert_eq!(before["state"], "out_of_date", "{before}");
    assert!(
        all_chunks(&env, "/report.pdf", &id).await[0]["text"].as_str().unwrap().contains("olden")
    );

    recalc(&env, ALICE, PROJ, "/report.pdf", "chunks", &id).await.unwrap();
    env.at(2).await;
    let after = set_of_id(&env, "/report.pdf", &id).await;
    assert_eq!(after["state"], "current", "{after}");
    let text = all_chunks(&env, "/report.pdf", &id).await[0]["text"].as_str().unwrap().to_string();
    assert!(text.contains("marmoset") && !text.contains("olden"), "{text}");
}

/// SPEC-0101/E2E-MOD-003 (was SPEC-0100/E2E-069): a rebuild by hand with search disabled.
#[tokio::test(start_paused = true)]
async fn e2e_0101_mod003_a_rebuild_by_hand_with_search_disabled() {
    let env = Env::new(Options { search: false, ..Options::default() }).await;
    env.save("/notes.md", "# Notes\n").await;
    let r = recalc(&env, ALICE, PROJ, "/notes.md", "search_index", "").await;
    assert_refused(
        r,
        "ERR_NOT_SUPPORTED: search is not enabled; set search.enabled: true in config",
    );
    let r = recalc(&env, ALICE, PROJ, "/ghost.md", "search_index", "").await;
    assert_refused(r, "ERR_NOT_FOUND: file not found: /ghost.md");
    assert!(env.jobs.load_unfinished(None).await.unwrap().is_empty());
}

/// US-0282 refusal order for the kind `chunks`: unknown set, then no default
/// set, then no text; and `search_index` with a `set_id` before search checks.
#[tokio::test(start_paused = true)]
async fn e2e_0101_us0282_refusal_order() {
    let env = env().await;
    env.seed("/guide.md", "# G\n\nwords\n").await;
    // A binary file that is not a converted document: no text, but also no set.
    put_bytes(&env, "/logo.png", &[PNG, &[0u8; 64]].concat()).await;
    let r = recalc(&env, ALICE, PROJ, "/logo.png", "chunks", "").await;
    assert_refused(r, "ERR_NOT_FOUND: the file has no default chunk set");
    let r = recalc(&env, ALICE, PROJ, "/logo.png", "chunks", "set-999").await;
    assert_refused(r, "ERR_NOT_FOUND: chunk set not found: set-999");
    // A set of another file is unknown for this one.
    let other =
        default_of(&env, "/guide.md").await.unwrap()["set_id"].as_str().unwrap().to_string();
    let r = recalc(&env, ALICE, PROJ, "/logo.png", "chunks", &other).await;
    assert_refused(r, &format!("ERR_NOT_FOUND: chunk set not found: {other}"));
    // Kind before path, path before the rest.
    let r = recalc(&env, ALICE, PROJ, "/ghost.md", "concepts", "").await;
    assert_refused(r, "ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'");
    let r = recalc(&env, ALICE, PROJ, "/ghost.md", "chunks", "").await;
    assert_refused(r, "ERR_NOT_FOUND: file not found: /ghost.md");
    no_chunks_job(&env).await;
}

/// A `chunks` job is one per file whatever the set: a second call supersedes
/// the running one and the waiting `search_index` rebuild is not cancelled.
#[tokio::test(start_paused = true)]
async fn e2e_0101_us0282_one_chunks_job_per_file() {
    let env = env().await;
    env.seed("/guide.md", "# G\n\nwords\n").await;
    let extra = create(&env, ALICE, "/guide.md", "fixed", json!({})).await.unwrap();
    let id = extra["set_id"].as_str().unwrap().to_string();
    put(&env, "/guide.md", "# G\n\nmore words\n").await;
    let a = recalc(&env, ALICE, PROJ, "/guide.md", "chunks", &id).await.unwrap();
    let b = recalc(&env, ALICE, PROJ, "/guide.md", "chunks", "").await.unwrap();
    assert_ne!(a["job_id"], b["job_id"]);
    env.at(2).await;
    let jobs = env.jobs("/guide.md").await;
    let chunks: Vec<&Value> = jobs.iter().filter(|j| j["kind"] == "chunks").collect();
    assert_eq!(chunks.len(), 1, "one finished chunks job per file: {jobs:?}");
    assert!(jobs.iter().any(|j| j["kind"] == "search_index" && j["state"] == "waiting"));
}
