//! SPEC-0112 (BL-0177): the git operations schedule the indexing of the files
//! they create or change and let the files they delete leave the index.
//!
//! Run on the `search::derived_e2e::Env` of the recalculation tests: search on,
//! the engine behind the SPEC-0100 scheduler, a paused clock. The git tools are
//! called through their typed `tool_*` functions with a store of the test
//! (the process wide store is fixed to its first caller). Volume files are put
//! with raw client writes, which index nothing, and the index is aligned on the
//! volume by hand before an operation (`G::align`), so a test reads what the
//! operation alone did.

use crate::errors::Result;
use crate::git::GitRepoStore;
use crate::mcp::server::{
    GitBranchCreateArgs, GitBranchResetArgs, GitBranchSwitchArgs, GitCheckoutFileArgs,
    GitCherryPickArgs, GitCommitArgs, GitMergeArgs, GitResetArgs, GitRevertArgs, GitStashIdArgs,
    GitStashSaveArgs, MountOnlyArgs,
};
use crate::search::derived_e2e::{ALICE, Env, Options, PROJ};
use crate::tools::registry_support::ToolCtx;
use serde_json::Value;
use std::sync::Arc;

pub(crate) struct G {
    pub env: Env,
    pub store: Arc<GitRepoStore>,
}

fn git_on(c: &mut crate::config::ServerConfig) {
    c.git.enabled = true;
}

impl G {
    pub async fn new() -> G {
        Self::with(Options { config: git_on, ..Options::default() }).await
    }

    pub async fn with(opts: Options) -> G {
        let env = Env::new(opts).await;
        let store = Arc::new(GitRepoStore::new(
            env.state.config.clone(),
            env.state.stores.relational().clone(),
        ));
        let g = G { env, store };
        crate::tools::git::tool_init(g.ctx(), MountOnlyArgs { mount_id: PROJ.into() }, g.g())
            .await
            .unwrap();
        g
    }

    pub fn ctx(&self) -> ToolCtx {
        ToolCtx { person: ALICE.into(), state: self.env.state.clone() }
    }

    pub fn g(&self) -> Option<Arc<GitRepoStore>> {
        Some(self.store.clone())
    }

    /// A raw write: the volume changes, nothing is scheduled.
    pub async fn put(&self, path: &str, text: &str) {
        let c = self.env.state.stores.client(PROJ).await.unwrap();
        c.write_text_atomic(path, text).await.unwrap();
    }

    pub async fn rm(&self, path: &str) {
        let c = self.env.state.stores.client(PROJ).await.unwrap();
        c.delete_file(path).await.unwrap();
    }

    pub async fn text(&self, path: &str) -> Option<String> {
        let c = self.env.state.stores.client(PROJ).await.unwrap();
        c.read_bytes(path).await.ok().map(|b| String::from_utf8(b).unwrap())
    }

    /// The index of `files` as an earlier rebuild would have left it, and no
    /// entry for `gone`: the state before the operation under test.
    pub async fn align(&self, files: &[(&str, &str)], gone: &[&str]) {
        for (p, t) in files {
            self.env.seed(p, t).await;
        }
        let search = self.env.state.search.as_ref().unwrap();
        for p in gone {
            search.delete_path(PROJ, p).await.unwrap();
        }
    }

    pub async fn commit(&self, message: &str) -> String {
        let a = GitCommitArgs {
            mount_id: PROJ.into(),
            message: message.into(),
            author_name: None,
            author_email: None,
        };
        let out = crate::tools::git::tool_commit(self.ctx(), a, self.g()).await.unwrap();
        out["commit_sha"].as_str().expect("a commit sha").to_string()
    }

    pub async fn branch(&self, name: &str, start: &str, checkout: bool) -> Result<Value> {
        let a = GitBranchCreateArgs {
            mount_id: PROJ.into(),
            name: name.into(),
            start_point: start.into(),
            checkout,
        };
        crate::tools::git::tool_branch_create(self.ctx(), a, self.g()).await
    }

    pub async fn switch(&self, name: &str) -> Result<Value> {
        let a = GitBranchSwitchArgs { mount_id: PROJ.into(), name: name.into() };
        crate::tools::git::tool_branch_switch(self.ctx(), a, self.g()).await
    }

    pub async fn merge(&self, source: &str) -> Result<Value> {
        let a = GitMergeArgs {
            mount_id: PROJ.into(),
            source_ref: source.into(),
            squash: false,
            message: None,
        };
        crate::tools::git::tool_merge(self.ctx(), a, self.g()).await
    }

    pub async fn reset(&self, target: &str, mode: &str) -> Result<Value> {
        let a =
            GitResetArgs { mount_id: PROJ.into(), target_ref: target.into(), mode: mode.into() };
        crate::tools::git::tool_reset(self.ctx(), a, self.g()).await
    }

    pub async fn branch_reset(&self, name: &str, target: &str) -> Result<Value> {
        let a = GitBranchResetArgs {
            mount_id: PROJ.into(),
            name: name.into(),
            target_commit: target.into(),
            force: true,
        };
        crate::tools::git::tool_branch_reset(self.ctx(), a, self.g()).await
    }

    pub async fn cherry_pick(&self, sha: &str) -> Result<Value> {
        let a = GitCherryPickArgs { mount_id: PROJ.into(), commit_sha: sha.into(), mainline: 0 };
        crate::tools::git::tool_cherry_pick(self.ctx(), a, 0, self.g()).await
    }

    pub async fn revert(&self, sha: &str) -> Result<Value> {
        let a = GitRevertArgs { mount_id: PROJ.into(), commit_sha: sha.into(), mainline: 0 };
        crate::tools::git::tool_revert(self.ctx(), a, 0, self.g()).await
    }

    pub async fn stash_save(&self) -> Result<Value> {
        let a = GitStashSaveArgs { mount_id: PROJ.into(), message: None };
        crate::tools::git::tool_stash_save(self.ctx(), a, self.g()).await
    }

    pub async fn stash_apply(&self, id: &str, pop: bool) -> Result<Value> {
        let a = GitStashIdArgs { mount_id: PROJ.into(), stash_id: id.into() };
        let name = if pop { "git.stash_pop" } else { "git.stash_apply" };
        crate::tools::git::tool_stash_apply_or_pop(self.ctx(), a, self.g(), name, pop).await
    }

    pub async fn checkout_file(&self, sha: &str, path: &str) -> Result<Value> {
        let a = GitCheckoutFileArgs {
            mount_id: PROJ.into(),
            commit_sha: sha.into(),
            path: path.into(),
        };
        crate::tools::git::tool_checkout_file(self.ctx(), a, self.g()).await
    }

    pub async fn pull(&self, url: &str) -> Result<Value> {
        crate::tools::git::test_support::pull_from(&self.ctx(), &self.store, PROJ, url, "main")
            .await
    }

    pub async fn clone_from(&self, url: &str) -> Result<Value> {
        crate::tools::git::test_support::clone_from(&self.ctx(), self.store.clone(), PROJ, url)
            .await
    }

    /// Time of the test, in seconds since its start, past the settle delay of
    /// everything scheduled so far.
    pub async fn settle(&self, at: u64) {
        self.env.at(at).await;
    }

    pub async fn has(&self, word: &str, path: &str) -> bool {
        self.env.finds(word, path).await
    }

    /// Every scheduled or finished job of `path`.
    pub async fn jobs(&self, path: &str) -> Vec<Value> {
        self.env.jobs(path).await
    }
}

/// A bare repository whose `main` holds `files`, as a `file://` url.
pub(crate) fn bare_remote(dir: &std::path::Path, files: &[(&str, &[u8])]) -> String {
    let repo = git2::Repository::init_bare(dir).unwrap();
    commit_bare(&repo, files, &[], &[]);
    repo.set_head("refs/heads/main").unwrap();
    format!("file://{}", dir.display())
}

/// One commit on `main` of `repo`: the parent's tree with `files` put and
/// `removed` taken out.
pub(crate) fn commit_bare(
    repo: &git2::Repository,
    files: &[(&str, &[u8])],
    removed: &[&str],
    parents: &[&git2::Commit<'_>],
) {
    let base = parents.first().map(|p| p.tree().unwrap());
    let mut builder = repo.treebuilder(base.as_ref()).unwrap();
    for name in removed {
        builder.remove(name).unwrap();
    }
    for (name, bytes) in files {
        builder.insert(*name, repo.blob(bytes).unwrap(), 0o100_644).unwrap();
    }
    let tree = repo.find_tree(builder.write().unwrap()).unwrap();
    let sig =
        git2::Signature::new("Origin", "o@t.com", &git2::Time::new(1_700_000_000, 0)).unwrap();
    repo.commit(Some("refs/heads/main"), &sig, &sig, "remote\n", &tree, parents).unwrap();
}

/// The tip commit of `main` of a bare repository.
pub(crate) fn tip(repo: &git2::Repository) -> git2::Commit<'_> {
    repo.find_reference("refs/heads/main").unwrap().peel_to_commit().unwrap()
}

// ── the tests ───────────────────────────────────────────────────────────────

use crate::storage::traits::IndexMode;

const A: [(&str, &str); 2] = [("/f.md", "orig"), ("/h.md", "hword")];
const B: [(&str, &str); 2] = [("/f.md", "changed"), ("/g.md", "newfile")];

/// Two commits: C1 holds `A`, C2 (on branch `feat`) holds `B`: `f.md` changed,
/// `g.md` created, `h.md` deleted. Leaves HEAD on `feat`, the volume at `B`.
async fn two_commits(g: &G) -> (String, String) {
    for (p, t) in A {
        g.put(p, t).await;
    }
    let c1 = g.commit("c1").await;
    g.branch("feat", &c1, true).await.unwrap();
    for (p, t) in B {
        g.put(p, t).await;
    }
    g.rm("/h.md").await;
    let c2 = g.commit("c2").await;
    (c1, c2)
}

/// The waiting jobs of `path`: what an operation left to rebuild.
async fn waiting(g: &G, path: &str) -> Vec<Value> {
    g.jobs(path).await.into_iter().filter(|j| j["state"] == "waiting").collect()
}

/// The volume and the index at `A`, HEAD on `main` (at C1).
async fn at_a(g: &G) {
    g.switch("main").await.unwrap();
    assert_eq!(g.text("/f.md").await.as_deref(), Some("orig"));
    g.align(&A, &["/g.md"]).await;
}

/// The volume and the index at `B`, HEAD on `feat` (at C2).
async fn at_b(g: &G) {
    assert_eq!(g.text("/f.md").await.as_deref(), Some("changed"));
    g.align(&B, &["/h.md"]).await;
}

async fn assert_forward(g: &G, what: &str) {
    g.settle(100).await;
    for (w, p) in [("changed", "/f.md"), ("newfile", "/g.md")] {
        assert!(g.has(w, p).await, "{what}: {w} in {p} is searchable");
    }
    for (w, p) in [("orig", "/f.md"), ("hword", "/h.md")] {
        assert!(!g.has(w, p).await, "{what}: {w} left {p}");
    }
}

async fn assert_backward(g: &G, what: &str) {
    g.settle(100).await;
    for (w, p) in [("orig", "/f.md"), ("hword", "/h.md")] {
        assert!(g.has(w, p).await, "{what}: {w} in {p} is searchable");
    }
    for (w, p) in [("changed", "/f.md"), ("newfile", "/g.md")] {
        assert!(!g.has(w, p).await, "{what}: {w} left {p}");
    }
}

/// SPEC-0112/E2E-001: a pull of 3 changed files makes them searchable.
#[tokio::test(start_paused = true)]
async fn e2e_0112_001_a_pull_of_three_changed_files_is_searchable() {
    let g = G::new().await;
    let dir = g.env.state.config.infra.meta.dir.clone() + "/../remote-001";
    let url = bare_remote(
        std::path::Path::new(&dir),
        &[("a.md", b"aone"), ("b.md", b"bone"), ("c.md", b"cone")],
    );
    g.clone_from(&url).await.unwrap();
    g.settle(40).await;
    let remote = git2::Repository::open_bare(&dir).unwrap();
    let parent = tip(&remote);
    commit_bare(
        &remote,
        &[("a.md", b"atwo"), ("b.md", b"btwo"), ("c.md", b"ctwo")],
        &[],
        &[&parent],
    );
    g.pull(&url).await.unwrap();
    g.settle(45).await;
    assert!(!g.has("ctwo", "/c.md").await, "not before the settle delay");
    g.settle(90).await;
    for (w, p) in [("atwo", "/a.md"), ("btwo", "/b.md"), ("ctwo", "/c.md")] {
        assert!(g.has(w, p).await, "{w} in {p}");
    }
    for (w, p) in [("aone", "/a.md"), ("bone", "/b.md"), ("cone", "/c.md")] {
        assert!(!g.has(w, p).await, "{w} left {p}");
    }
}

/// SPEC-0112/E2E-002: a merge that deletes one file and renames another.
#[tokio::test(start_paused = true)]
async fn e2e_0112_002_a_merge_drops_a_deleted_file_and_moves_a_renamed_one() {
    let g = G::new().await;
    g.put("/keep.md", "kword").await;
    g.put("/gone.md", "gword").await;
    g.put("/old.md", "oword").await;
    let c1 = g.commit("c1").await;
    g.branch("feat", &c1, true).await.unwrap();
    g.rm("/gone.md").await;
    let moved = g.text("/old.md").await.unwrap();
    g.rm("/old.md").await;
    g.put("/new.md", &moved).await;
    g.commit("c2").await;
    g.switch("main").await.unwrap();
    g.put("/side.md", "sword").await;
    g.commit("c3").await;
    g.align(
        &[
            ("/keep.md", "kword"),
            ("/gone.md", "gword"),
            ("/old.md", "oword"),
            ("/side.md", "sword"),
        ],
        &["/new.md"],
    )
    .await;
    let out = g.merge("feat").await.unwrap();
    assert_eq!(out["status"], "merged", "{out}");
    g.settle(100).await;
    assert!(g.has("oword", "/new.md").await, "the new name is searchable");
    assert!(!g.has("oword", "/old.md").await, "the old name left the index");
    assert!(!g.has("gword", "/gone.md").await, "the deleted file left the index");
    assert!(g.has("kword", "/keep.md").await && g.has("sword", "/side.md").await);
}

/// SPEC-0112/E2E-003: a conflicting merge changes nothing, so schedules nothing.
#[tokio::test(start_paused = true)]
async fn e2e_0112_003_a_conflicting_merge_schedules_nothing() {
    let g = G::new().await;
    g.put("/f.md", "base").await;
    g.put("/o.md", "other").await;
    let c1 = g.commit("c1").await;
    g.branch("feat", &c1, true).await.unwrap();
    g.put("/f.md", "theirs").await;
    g.put("/n.md", "newer").await;
    g.commit("c2").await;
    g.switch("main").await.unwrap();
    g.put("/f.md", "ours").await;
    g.commit("c3").await;
    g.align(&[("/f.md", "ours"), ("/o.md", "other")], &["/n.md"]).await;
    let before = g.jobs("/f.md").await.len();
    let out = g.merge("feat").await.unwrap();
    assert_eq!(out["status"], "conflict", "{out}");
    for p in ["/f.md", "/n.md", "/o.md"] {
        assert_eq!(g.jobs(p).await.len(), if p == "/f.md" { before } else { 0 }, "{p}");
    }
    g.settle(100).await;
    assert!(g.has("ours", "/f.md").await && !g.has("newer", "/n.md").await);
}

/// SPEC-0112/E2E-004: a project that indexes nothing schedules nothing.
#[tokio::test(start_paused = true)]
async fn e2e_0112_004_mode_none_schedules_nothing_for_git() {
    let g = G::new().await;
    let (_c1, c2) = two_commits(&g).await;
    at_a(&g).await;
    g.settle(100).await;
    g.env.state.admin.set_index_mode(PROJ, IndexMode::None).await.unwrap();
    g.cherry_pick(&c2).await.unwrap();
    for p in ["/f.md", "/g.md", "/h.md"] {
        assert!(waiting(&g, p).await.is_empty(), "{p}: {:?}", g.jobs(p).await);
    }
    g.settle(100).await;
    assert!(!g.has("newfile", "/g.md").await);
}

/// SPEC-0112/E2E-005: a second operation before the delay is one rebuild.
#[tokio::test(start_paused = true)]
async fn e2e_0112_005_two_operations_before_the_delay_collapse_to_one_rebuild() {
    let g = G::new().await;
    let (c1, c2) = two_commits(&g).await;
    at_b(&g).await;
    g.settle(100).await;
    let base = g.env.starts_of("/f.md").len();
    g.reset(&c1, "hard").await.unwrap();
    g.settle(120).await;
    g.reset(&c2, "hard").await.unwrap();
    g.settle(125).await;
    let jobs = g.jobs("/f.md").await;
    assert_eq!(jobs.iter().filter(|j| j["state"] == "waiting").count(), 1, "{jobs:?}");
    g.settle(200).await;
    assert_eq!(g.env.starts_of("/f.md").len() - base, 1, "one rebuild for two operations");
    assert!(g.has("changed", "/f.md").await);
}

/// SPEC-0112/E2E-006: a rename in a commit is a delete plus a create.
#[tokio::test(start_paused = true)]
async fn e2e_0112_006_a_rename_in_a_commit_moves_the_entries() {
    let g = G::new().await;
    g.put("/old.md", "rword").await;
    let c1 = g.commit("c1").await;
    g.rm("/old.md").await;
    g.put("/new.md", "rword").await;
    let c2 = g.commit("c2").await;
    g.align(&[("/new.md", "rword")], &["/old.md"]).await;
    g.reset(&c1, "hard").await.unwrap();
    g.settle(100).await;
    assert!(g.has("rword", "/old.md").await && !g.has("rword", "/new.md").await);
    g.reset(&c2, "hard").await.unwrap();
    g.settle(250).await;
    assert!(g.has("rword", "/new.md").await, "the new path is searchable");
    assert!(!g.has("rword", "/old.md").await, "the old path left the index");
}

/// SPEC-0112/E2E-007: a binary file is scheduled and finishes with no entry.
#[tokio::test(start_paused = true)]
async fn e2e_0112_007_a_binary_file_ends_without_entries_and_without_failure() {
    let g = G::new().await;
    let c = g.env.state.stores.client(PROJ).await.unwrap();
    c.write_bytes_atomic("/b.bin", &[0xff, 0xfe, 0x00, 0x80]).await.unwrap();
    g.put("/t.md", "tword").await;
    let c1 = g.commit("c1").await;
    c.delete_file("/b.bin").await.unwrap();
    g.rm("/t.md").await;
    g.commit("c2").await;
    g.reset(&c1, "hard").await.unwrap();
    g.settle(100).await;
    let jobs = g.jobs("/b.bin").await;
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_eq!(jobs[0]["state"], "done", "{jobs:?}");
    assert!(g.has("tword", "/t.md").await);
}

/// SPEC-0112/E2E-008: the entries equal those of a write of the same text.
#[tokio::test(start_paused = true)]
async fn e2e_0112_008_the_entries_equal_those_of_a_write() {
    let g = G::new().await;
    let text = "# Title\n\nbody wword one\n\n# Second\n\nmore wword two\n";
    g.put("/w.md", text).await;
    let c1 = g.commit("c1").await;
    g.rm("/w.md").await;
    g.commit("c2").await;
    g.reset(&c1, "hard").await.unwrap();
    g.settle(100).await;
    let by_git = g.env.found("wword").await;
    g.env.save("/same.md", text).await;
    g.settle(250).await;
    let by_write: Vec<_> =
        g.env.found("wword").await.into_iter().filter(|(p, _)| p == "/same.md").collect();
    let by_git: Vec<_> = by_git.into_iter().filter(|(p, _)| p == "/w.md").collect();
    assert!(!by_git.is_empty());
    assert_eq!(by_git.len(), by_write.len());
    let cause = g.jobs("/w.md").await[0]["cause"].clone();
    assert_eq!(cause, "change");
}

/// SPEC-0112/E2E-009: the audit lines are those of today.
#[tokio::test(start_paused = true)]
async fn e2e_0112_009_audit_lines_are_unchanged() {
    let g = G::new().await;
    let (c1, _c2) = two_commits(&g).await;
    let before = g.env.state.safety.audit(ALICE, PROJ).len();
    g.reset(&c1, "hard").await.unwrap();
    let log = g.env.state.safety.audit(ALICE, PROJ);
    let added: Vec<String> =
        log[before..].iter().map(|e| format!("{}|{}|{}", e.op, e.path, e.detail)).collect();
    assert_eq!(added.len(), 1, "{added:?}");
    assert!(added[0].starts_with("git.reset|"), "{added:?}");
    assert!(added[0].contains("hard"), "{added:?}");
}

/// SPEC-0112/E2E-010: every operation of the scope, one row each.
#[tokio::test(start_paused = true)]
async fn e2e_0112_010_every_operation_follows_the_index() {
    // forward: the volume goes from A to B
    for case in ["merge", "cherry_pick", "switch", "reset", "branch_reset", "branch_create"] {
        let g = G::new().await;
        let (c1, c2) = two_commits(&g).await;
        let _ = c1;
        at_a(&g).await;
        match case {
            "merge" => drop(g.merge("feat").await.unwrap()),
            "cherry_pick" => drop(g.cherry_pick(&c2).await.unwrap()),
            "switch" => drop(g.switch("feat").await.unwrap()),
            "reset" => drop(g.reset(&c2, "hard").await.unwrap()),
            "branch_reset" => drop(g.branch_reset("main", &c2).await.unwrap()),
            _ => drop(g.branch("fresh", &c2, true).await.unwrap()),
        }
        assert_forward(&g, case).await;
    }
    // backward: the volume goes from B to A
    for case in ["revert", "reset", "switch", "branch_reset", "branch_create"] {
        let g = G::new().await;
        let (c1, c2) = two_commits(&g).await;
        at_b(&g).await;
        match case {
            "revert" => drop(g.revert(&c2).await.unwrap()),
            "reset" => drop(g.reset(&c1, "hard").await.unwrap()),
            "switch" => drop(g.switch("main").await.unwrap()),
            "branch_reset" => drop(g.branch_reset("feat", &c1).await.unwrap()),
            _ => drop(g.branch("back", &c1, true).await.unwrap()),
        }
        assert_backward(&g, case).await;
    }
    // one file restored from a commit
    let g = G::new().await;
    let (c1, _c2) = two_commits(&g).await;
    at_b(&g).await;
    g.checkout_file(&c1, "/f.md").await.unwrap();
    g.settle(100).await;
    assert!(g.has("orig", "/f.md").await && !g.has("changed", "/f.md").await, "checkout_file");
    assert!(g.has("newfile", "/g.md").await, "an untouched file keeps its entries");
    // stash save, apply and pop
    for case in ["save", "apply", "pop"] {
        let g = G::new().await;
        g.put("/f.md", "orig").await;
        g.commit("c1").await;
        g.put("/f.md", "wip").await;
        g.align(&[("/f.md", "wip")], &[]).await;
        let saved = g.stash_save().await.unwrap();
        if case == "save" {
            g.settle(100).await;
            assert!(g.has("orig", "/f.md").await && !g.has("wip", "/f.md").await, "stash_save");
            continue;
        }
        g.align(&[("/f.md", "orig")], &[]).await;
        g.stash_apply(saved["stash_id"].as_str().unwrap(), case == "pop").await.unwrap();
        g.settle(100).await;
        assert!(g.has("wip", "/f.md").await && !g.has("orig", "/f.md").await, "stash_{case}");
    }
}

/// SPEC-0112/E2E-011: an operation the quota refuses schedules nothing.
#[tokio::test(start_paused = true)]
async fn e2e_0112_011_a_quota_refusal_schedules_nothing() {
    let g = G::new().await;
    g.put("/f.md", "orig").await;
    let c1 = g.commit("c1").await;
    g.branch("feat", &c1, true).await.unwrap();
    g.put("/f.md", &"x ".repeat(4000)).await;
    g.put("/g.md", "newfile").await;
    g.commit("c2").await;
    g.switch("main").await.unwrap();
    g.align(&[("/f.md", "orig")], &["/g.md"]).await;
    g.settle(100).await;
    g.env.state.admin.set_quota(PROJ, Some(100)).await.unwrap();
    let err = g.merge("feat").await.unwrap_err();
    assert!(
        err.message.to_lowercase().contains("quota") || err.message.contains("limit"),
        "{}",
        err.message
    );
    for p in ["/f.md", "/g.md"] {
        assert!(waiting(&g, p).await.is_empty(), "{p}: {:?}", g.jobs(p).await);
    }
    g.settle(300).await;
    assert!(g.has("orig", "/f.md").await && !g.has("newfile", "/g.md").await);
}

/// SPEC-0112/E2E-018: a cherry pick and an extraction follow the index with the
/// admin store and the volume metadata on PostgreSQL. Skips without
/// `MCPFS_TEST_PG_DSN`. The clock is the real one (settle delay 1 s): a paused
/// clock jumps while a socket answer is in flight and the pool times out.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn e2e_0112_018_git_and_extraction_follow_the_index_on_postgres() {
    if std::env::var("MCPFS_TEST_PG_DSN").map_or(true, |v| v.trim().is_empty()) {
        eprintln!("skipping e2e_0112_018: MCPFS_TEST_PG_DSN is unset");
        return;
    }
    let g = G::with(Options { config: git_on, pg: true, delay: 1, ..Options::default() }).await;
    let (_c1, c2) = two_commits(&g).await;
    at_a(&g).await;
    g.cherry_pick(&c2).await.unwrap();
    g.settle(6).await;
    for (w, p) in [("changed", "/f.md"), ("newfile", "/g.md")] {
        assert!(g.has(w, p).await, "{w} in {p} is searchable on postgres");
    }
    for (w, p) in [("orig", "/f.md"), ("hword", "/h.md")] {
        assert!(!g.has(w, p).await, "{w} left {p} on postgres");
    }

    let c = g.env.state.stores.client(PROJ).await.unwrap();
    let zip = crate::tools::archive::tests::build_zip(&[("z.md", b"zipword")], None);
    c.write_bytes_atomic("/pack.zip", &zip).await.unwrap();
    crate::tools::archive::extract_archive(
        &g.env.state,
        PROJ,
        ALICE,
        "/pack.zip",
        None,
        false,
        None,
    )
    .await
    .unwrap();
    g.settle(12).await;
    assert!(g.has("zipword", "/pack/z.md").await, "the extracted file is searchable");
    g.env.drop_schema().await;
}
