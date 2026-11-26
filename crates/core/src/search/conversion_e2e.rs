//! SPEC-0113 (BL-0176) acceptance tests: a converted document is rebuilt after
//! its source changes. Run on the SPEC-0100 `Env` (search on, the engine behind
//! the scheduler, a paused clock) with a scripted converter: its answer is
//! built from the bytes it receives, so a rebuild is visible in the companion,
//! the images and the tables. Names are `e2e_0113_NNN_<slug>`, so
//! `cargo test -p fsa-core --lib -- e2e_0113_` runs the whole spec.

use crate::docs::service::Conversion;
use crate::errors::{Result, ToolError};
use crate::search::derived_e2e::{ALICE, Env, Options, PROJ, args, tool_json};
use async_trait::async_trait;
use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A converter whose answer is a function of the source bytes: the text
/// `T` gives `# Converted T`, a picture captioned `Fig T`, a table captioned
/// `Tab T`. It counts its calls, can be down, and can hold a conversion
/// mid flight so a test changes the source while it runs.
pub(crate) struct Scripted {
    calls: AtomicUsize,
    down: AtomicBool,
    hold: AtomicBool,
    pad: AtomicUsize,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    extensions: Vec<String>,
}

impl Scripted {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            down: AtomicBool::new(false),
            hold: AtomicBool::new(false),
            pad: AtomicUsize::new(0),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            extensions: vec![".pdf".into(), ".docx".into()],
        })
    }
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    /// Every following conversion adds `bytes` of padding to its Markdown.
    pub fn set_pad(&self, bytes: usize) {
        self.pad.store(bytes, Ordering::SeqCst);
    }
    pub fn set_down(&self, down: bool) {
        self.down.store(down, Ordering::SeqCst);
    }
    /// The next conversion waits until [`Self::release`].
    pub fn hold_next(&self) {
        self.hold.store(true, Ordering::SeqCst);
    }
    pub async fn entered(&self) {
        let wait = std::time::Duration::from_secs(120);
        tokio::time::timeout(wait, self.entered.notified()).await.expect("no conversion started");
    }
    pub fn release(&self) {
        self.release.notify_one();
    }
}

#[async_trait]
impl crate::docs::DocService for Scripted {
    async fn convert(&self, bytes: &[u8], _file_name: &str) -> Result<Conversion> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.hold.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        if self.down.load(Ordering::SeqCst) {
            return Err(ToolError::internal("document service unreachable: connection refused"));
        }
        let t = String::from_utf8_lossy(bytes).to_string();
        let manifest = json!({
            "version": 2,
            "pictures": {"p1.png": {"caption": format!("Fig {t}"), "page": 1}},
            "tables": [{"caption": format!("Tab {t}")}],
            "headings": [{"level": 1, "title": format!("Converted {t}")}],
        });
        Ok(Conversion {
            markdown: format!(
                "# Converted {t}\n\n![](p1.png)\n\n| a | b |\n| --- | --- |\n| {t} | 2 |\n{}",
                "x".repeat(self.pad.load(Ordering::SeqCst))
            ),
            files: [("p1.png".to_string(), b"PNG".to_vec())].into_iter().collect(),
            manifest: Some(manifest),
        })
    }

    fn accepted_extensions(&self) -> &[String] {
        &self.extensions
    }

    fn max_input_bytes(&self) -> u64 {
        1 << 20
    }
}

pub(crate) fn sha_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// One server with a scripted converter.
pub(crate) struct W {
    pub env: Env,
    pub doc: Arc<Scripted>,
}

impl W {
    pub async fn new() -> W {
        Self::with(Options::default()).await
    }

    pub async fn with(opts: Options) -> W {
        let doc = Scripted::new();
        let env = Env::new(Options { doc: Some(doc.clone()), ..opts }).await;
        W { env, doc }
    }

    pub async fn client(&self) -> Arc<crate::storage::VolumeClient> {
        self.env.state.stores.client(PROJ).await.unwrap()
    }

    /// `fs.write_bytes` by Alice, overwriting; `flag` asks the conversion.
    pub async fn put(
        &self,
        path: &str,
        text: &str,
        flag: bool,
    ) -> std::result::Result<Value, String> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let a = args(json!({"mount_id": PROJ, "path": path, "base64": b64, "overwrite": true,
            "trigger_documentation_service": flag}));
        tool_json(self.env.server(ALICE).fs_write_bytes(a).await)
    }

    /// `fs.documentize` by Alice with overwrite.
    pub async fn documentize(&self, path: &str) -> std::result::Result<Value, String> {
        let a = args(json!({"mount_id": PROJ, "path": path, "overwrite": true}));
        tool_json(self.env.server(ALICE).fs_documentize(a).await)
    }

    /// `fs.write` by Alice with overwrite (a hand edit of a text file).
    pub async fn edit(&self, path: &str, text: &str) {
        self.env.save(path, text).await;
    }

    pub async fn text(&self, path: &str) -> Option<String> {
        let c = self.client().await;
        c.read_bytes(path).await.ok().map(|b| String::from_utf8(b).unwrap())
    }

    pub async fn origin(&self, md: &str) -> Option<crate::storage::meta::CompanionOrigin> {
        let c = self.client().await;
        c.trash.as_ref().unwrap().companion_origin(md).await.unwrap()
    }

    /// The captions of the images and tables of the source document.
    /// The artifact listing refusal of a changed document (SPEC-0019: a changed
    /// source has no readable images or tables until it is converted again).
    pub async fn artifacts_gone(&self, path: &str) -> bool {
        let server = self.env.server(ALICE);
        let r = server.fs_list_images(args(json!({"mount_id": PROJ, "path": path}))).await;
        tool_json(r).is_err_and(|e| e.contains("not extracted"))
    }

    pub async fn artifacts(&self, path: &str) -> (Vec<String>, Vec<String>) {
        let s = self.env.server(ALICE);
        let images =
            tool_json(s.fs_list_images(args(json!({"mount_id": PROJ, "path": path}))).await)
                .unwrap();
        let tables =
            tool_json(s.fs_list_tables(args(json!({"mount_id": PROJ, "path": path}))).await)
                .unwrap();
        let caps = |v: &Value, k: &str| -> Vec<String> {
            v[k].as_array()
                .map(|a| {
                    a.iter().map(|x| x["caption"].as_str().unwrap_or("").to_string()).collect()
                })
                .unwrap_or_default()
        };
        (caps(&images, "images"), caps(&tables, "tables"))
    }
}

// ── US-0338: the origin of a companion ─────────────────────────────────────

/// FR-NEW-001: every conversion records the source, its sha256 and the sha256
/// of the companion it wrote; a conversion of another source to the same
/// companion path replaces the record.
#[tokio::test(start_paused = true)]
async fn e2e_0113_001_a_conversion_records_its_origin() {
    let w = W::new().await;
    w.put("/a.pdf", "one", false).await.unwrap();
    w.documentize("/a.pdf").await.unwrap();
    let o = w.origin("/a.md").await.expect("the origin of /a.md is recorded");
    assert_eq!(o.source_path, "/a.pdf");
    assert_eq!(o.source_sha, sha_hex(b"one"));
    assert_eq!(o.md_sha, sha_hex(w.text("/a.md").await.unwrap().as_bytes()));

    // The upload flag records it too.
    w.put("/b.pdf", "two", true).await.unwrap();
    let b = w.origin("/b.md").await.expect("the upload flag records the origin");
    assert_eq!(b.source_path, "/b.pdf");
    assert_eq!(b.source_sha, sha_hex(b"two"));

    // Another source of the same companion path takes the record over.
    w.put("/a.docx", "three", false).await.unwrap();
    w.documentize("/a.docx").await.unwrap();
    let o = w.origin("/a.md").await.unwrap();
    assert_eq!(o.source_path, "/a.docx");
    assert_eq!(o.source_sha, sha_hex(b"three"));
}

/// FR-NEW-002: a companion is converted only while it holds the bytes the
/// conversion wrote; a record is no proof for a file edited since, and a file
/// with no record is manual.
#[tokio::test(start_paused = true)]
async fn e2e_0113_002_a_hand_written_or_edited_companion_is_manual() {
    let w = W::new().await;
    let c = w.client().await;
    // No record at all: manual.
    w.edit("/m.md", "my own notes").await;
    assert!(crate::core::fs_ops::converted_origin(&c, "/m.md").await.unwrap().is_none());
    // Converted: the origin is read.
    w.put("/a.pdf", "one", false).await.unwrap();
    w.documentize("/a.pdf").await.unwrap();
    let o = crate::core::fs_ops::converted_origin(&c, "/a.md").await.unwrap();
    assert_eq!(o.map(|o| o.source_path), Some("/a.pdf".to_string()));
    // Edited by hand: manual, whatever the record says.
    w.edit("/a.md", "edited by a member").await;
    assert!(w.origin("/a.md").await.is_some(), "the record is still there");
    assert!(crate::core::fs_ops::converted_origin(&c, "/a.md").await.unwrap().is_none());
    // Converted again with overwrite: converted again.
    w.documentize("/a.pdf").await.unwrap();
    assert!(crate::core::fs_ops::converted_origin(&c, "/a.md").await.unwrap().is_some());
}

/// FR-NEW-013 / X8: a database from before this version has no origin table.
/// Opening it creates the table, no companion has a record, every one reads as
/// manual, and a record can then be written.
#[tokio::test]
async fn e2e_0113_003_an_old_database_opens_with_every_companion_manual() {
    use crate::storage::meta::{CompanionOrigin, RelationalMetaStore, schema};
    use crate::storage::rel::{RelationalDb, SqliteRelationalDb};
    let db = Arc::new(SqliteRelationalDb::open_in_memory().unwrap());
    let mut old = schema();
    old.tables.retain(|t| t.name != "companion_origin");
    db.migrate(&old).await.unwrap();
    let store = RelationalMetaStore::open(db.clone(), "proj").await.unwrap();
    assert!(store.companion_origin("/a.md").await.unwrap().is_none());
    let origin = CompanionOrigin {
        md_path: "/a.md".into(),
        source_path: "/a.pdf".into(),
        source_sha: "s".into(),
        md_sha: "m".into(),
    };
    store.set_companion_origin(&origin).await.unwrap();
    assert_eq!(store.companion_origin("/a.md").await.unwrap(), Some(origin));
}

// ── US-0339: the conversion job after a source change ──────────────────────

use crate::core::fs_ops::{self, Rebuilt};
use crate::search::derived::JobCause;

impl W {
    /// The conversion jobs of `path` as `fs.list_derived_jobs` shows them.
    pub async fn conv_jobs(&self, path: &str) -> Vec<Value> {
        self.env
            .jobs(path)
            .await
            .into_iter()
            .filter(|j| j["kind"] == "conversion" && j["path"] == path)
            .collect()
    }

    /// Converts `path` with `text` as its bytes and records the origin.
    pub async fn converted(&self, path: &str, text: &str) {
        self.put(path, text, false).await.unwrap();
        self.documentize(path).await.unwrap();
    }
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// S1: a converted source replaced: after the settle delay the companion, the
/// images and the tables are those of the new bytes, the job is listed.
#[tokio::test(start_paused = true)]
async fn e2e_0113_004_a_replaced_source_is_converted_again_after_the_delay() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    assert_eq!(w.artifacts("/a.pdf").await, (names(&["Fig one"]), names(&["Tab one"])));
    w.put("/a.pdf", "two", false).await.unwrap();
    let jobs = w.conv_jobs("/a.pdf").await;
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_eq!(
        (jobs[0]["state"].as_str(), jobs[0]["cause"].as_str()),
        (Some("waiting"), Some("change"))
    );
    w.env.at(29).await;
    assert!(w.text("/a.md").await.unwrap().contains("Converted one"), "not before the delay");
    w.env.at(32).await;
    assert!(w.text("/a.md").await.unwrap().contains("Converted two"));
    assert_eq!(w.artifacts("/a.pdf").await, (names(&["Fig two"]), names(&["Tab two"])));
    let jobs = w.conv_jobs("/a.pdf").await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["state"], "done");
    assert_eq!(w.doc.calls(), 2);
    // A server job has no session budget: its session charge was refunded.
    assert_eq!(w.env.state.safety.bytes_written(fs_ops::CONVERSION_PERSON, PROJ), 0);
    // The new companion is indexed like one fs.documentize wrote.
    w.env.at(34).await;
    assert!(w.env.finds("two", "/a.md").await);
}

/// S3 and X1: `/a.pdf` and `/a.docx` share `/a.md`; only the source that
/// converted last triggers a rebuild.
#[tokio::test(start_paused = true)]
async fn e2e_0113_005_only_the_source_that_converted_last_triggers() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.converted("/a.docx", "two").await;
    w.put("/a.pdf", "three", false).await.unwrap();
    assert!(w.conv_jobs("/a.pdf").await.is_empty(), "the older source schedules nothing");
    w.env.at(40).await;
    assert_eq!(w.doc.calls(), 2);
    assert!(w.text("/a.md").await.unwrap().contains("Converted two"));
    w.put("/a.docx", "four", false).await.unwrap();
    assert_eq!(w.conv_jobs("/a.docx").await.len(), 1);
    w.env.at(80).await;
    assert!(w.text("/a.md").await.unwrap().contains("Converted four"));
    assert_eq!(w.origin("/a.md").await.unwrap().source_path, "/a.docx");
}

/// X2: a companion edited by hand, then its source replaced: nothing is
/// scheduled and the companion stays.
#[tokio::test(start_paused = true)]
async fn e2e_0113_006_a_hand_edited_companion_is_left_alone() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.edit("/a.md", "edited by a member").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    assert!(w.conv_jobs("/a.pdf").await.is_empty());
    w.env.at(40).await;
    assert_eq!(w.text("/a.md").await.as_deref(), Some("edited by a member"));
    assert_eq!(w.doc.calls(), 1);
}

/// X6: the source written again with the bytes the companion was made from
/// never calls the converter: no job at all, and a waiting one is cancelled.
/// At run time the same rule holds for the cause `change`, never for `manual`.
#[tokio::test(start_paused = true)]
async fn e2e_0113_007_an_unchanged_source_never_calls_the_converter() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "one", false).await.unwrap();
    assert!(w.conv_jobs("/a.pdf").await.is_empty());
    w.put("/a.pdf", "two", false).await.unwrap();
    assert_eq!(w.conv_jobs("/a.pdf").await.len(), 1);
    w.put("/a.pdf", "one", false).await.unwrap();
    assert!(
        w.conv_jobs("/a.pdf").await.iter().all(|j| j["state"] != "waiting"),
        "the waiting job was cancelled"
    );
    w.env.at(40).await;
    assert_eq!(w.doc.calls(), 1);
    // The run time check, called as the job calls it.
    let c = w.client().await;
    let safety = &w.env.state.safety;
    let run = |cause| {
        fs_ops::rebuild_companion(&c, safety, Some(&*w.doc), PROJ, "/a.pdf", cause, &|| true)
    };
    assert_eq!(run(JobCause::Change).await.unwrap(), Rebuilt::Unchanged);
    assert_eq!(w.doc.calls(), 1);
    assert!(matches!(run(JobCause::Manual).await.unwrap(), Rebuilt::Done { .. }));
    assert_eq!(w.doc.calls(), 2);
}

/// F1: the converter is down: the job fails with the reason, the old
/// companion, images and tables stay; the next change converts again.
#[tokio::test(start_paused = true)]
async fn e2e_0113_008_a_converter_down_fails_the_job_and_keeps_the_old_companion() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    w.doc.set_down(true);
    w.env.at(32).await;
    let jobs = w.conv_jobs("/a.pdf").await;
    assert_eq!(jobs[0]["state"], "failed");
    assert!(jobs[0]["reason"].as_str().unwrap().contains("unreachable"), "{jobs:?}");
    assert!(w.text("/a.md").await.unwrap().contains("Converted one"));
    assert!(w.artifacts_gone("/a.pdf").await, "as for any changed document (SPEC-0019)");
    w.doc.set_down(false);
    w.put("/a.pdf", "three", false).await.unwrap();
    w.env.at(64).await;
    assert!(w.text("/a.md").await.unwrap().contains("Converted three"));
    assert_eq!(w.conv_jobs("/a.pdf").await[0]["state"], "done");
}

/// F2: the companion became manual between the change and the run: the job
/// ends failed with a note, the file is untouched, the converter not called.
#[tokio::test(start_paused = true)]
async fn e2e_0113_009_a_manual_companion_is_never_overwritten() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    w.edit("/a.md", "hand written since").await;
    w.env.at(32).await;
    let jobs = w.conv_jobs("/a.pdf").await;
    assert_eq!(jobs[0]["state"], "failed");
    assert!(jobs[0]["reason"].as_str().unwrap().contains("left untouched"), "{jobs:?}");
    assert_eq!(w.text("/a.md").await.as_deref(), Some("hand written since"));
    assert_eq!(w.doc.calls(), 1);
}

/// X9: the converter of an earlier conversion is no longer configured: the
/// job fails with a clear reason and the companion stays.
#[tokio::test(start_paused = true)]
async fn e2e_0113_010_no_converter_configured_fails_the_job_clearly() {
    // A companion an earlier configuration converted, then the converter went.
    let w = W { env: Env::new(Options::default()).await, doc: Scripted::new() };
    let c = w.client().await;
    w.put("/a.pdf", "one", false).await.unwrap();
    c.write_text_atomic("/a.md", "old companion").await.unwrap();
    let origin = crate::storage::meta::CompanionOrigin {
        md_path: "/a.md".into(),
        source_path: "/a.pdf".into(),
        source_sha: fs_ops::node_sha(b"one"),
        md_sha: fs_ops::node_sha(b"old companion"),
    };
    c.trash.as_ref().unwrap().set_companion_origin(&origin).await.unwrap();
    w.put("/a.pdf", "two", false).await.unwrap();
    assert_eq!(w.conv_jobs("/a.pdf").await.len(), 1);
    w.env.at(32).await;
    let jobs = w.conv_jobs("/a.pdf").await;
    assert_eq!(jobs[0]["state"], "failed");
    assert!(jobs[0]["reason"].as_str().unwrap().contains("document service is not configured"));
    assert_eq!(w.text("/a.md").await.as_deref(), Some("old companion"));
}

/// F3: the project quota would be exceeded: nothing is written, the job fails
/// with the quota message, the old companion and artifacts stay.
#[tokio::test(start_paused = true)]
async fn e2e_0113_011_a_quota_refusal_writes_nothing() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    let c = w.client().await;
    let used = crate::quota::used_bytes(&c).await.unwrap();
    w.env.state.admin.set_quota(PROJ, Some(used + 100)).await.unwrap();
    w.doc.set_pad(5000);
    w.put("/a.pdf", "two", false).await.unwrap();
    w.env.at(32).await;
    let jobs = w.conv_jobs("/a.pdf").await;
    assert_eq!(jobs[0]["state"], "failed", "{jobs:?}");
    assert!(jobs[0]["reason"].as_str().unwrap().to_lowercase().contains("quota"), "{jobs:?}");
    assert!(w.text("/a.md").await.unwrap().contains("Converted one"));
    assert!(w.artifacts_gone("/a.pdf").await, "nothing was written, none was built");
}

/// F4: the source changes while the conversion runs: the result is discarded
/// (never written), and the change's own job converts the new bytes.
#[tokio::test(start_paused = true)]
async fn e2e_0113_012_a_source_changed_during_the_run_discards_the_result() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    w.doc.hold_next();
    w.env.at(31).await;
    w.doc.entered().await;
    w.put("/a.pdf", "three", false).await.unwrap();
    w.doc.release();
    w.env.at(33).await;
    let md = w.text("/a.md").await.unwrap();
    assert!(md.contains("Converted one"), "the discarded result was written: {md}");
    w.env.at(70).await;
    let md = w.text("/a.md").await.unwrap();
    assert!(md.contains("Converted three"), "{md}");
    assert_eq!(w.artifacts("/a.pdf").await, (names(&["Fig three"]), names(&["Tab three"])));
    assert_eq!(w.doc.calls(), 3);
}

/// X3: `fs.documentize` during the delay converts now and cancels the waiting
/// job, so the converter is not called a second time.
#[tokio::test(start_paused = true)]
async fn e2e_0113_013_documentize_cancels_the_waiting_rebuild() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    w.env.at(10).await;
    w.documentize("/a.pdf").await.unwrap();
    assert!(w.text("/a.md").await.unwrap().contains("Converted two"));
    assert!(w.conv_jobs("/a.pdf").await.iter().all(|j| j["state"] != "waiting"));
    w.env.at(40).await;
    assert_eq!(w.doc.calls(), 3 - 1);
}

/// X4: an upload with the conversion flag converts in the request and leaves
/// no waiting rebuild.
#[tokio::test(start_paused = true)]
async fn e2e_0113_014_an_upload_with_the_flag_cancels_the_waiting_rebuild() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", true).await.unwrap();
    assert!(w.text("/a.md").await.unwrap().contains("Converted two"));
    assert!(w.conv_jobs("/a.pdf").await.iter().all(|j| j["state"] != "waiting"));
    w.env.at(40).await;
    assert_eq!(w.doc.calls(), 2);
}

/// X5: a source moved or deleted gets no new rebuild (SPEC-0105 rules); its
/// waiting job goes with it.
#[tokio::test(start_paused = true)]
async fn e2e_0113_015_a_moved_or_trashed_source_schedules_nothing() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    assert_eq!(w.conv_jobs("/a.pdf").await.len(), 1);
    let mv = args(json!({"mount_id": PROJ, "source": "/a.pdf", "destination": "/b.pdf"}));
    tool_json(w.env.server(ALICE).fs_move(mv).await).unwrap();
    assert!(w.conv_jobs("/a.pdf").await.is_empty());
    assert!(w.conv_jobs("/b.pdf").await.is_empty());
    w.env.state.safety.record_read(ALICE, PROJ, "/b.pdf");
    w.put("/b.pdf", "three", false).await.unwrap();
    assert!(w.conv_jobs("/b.pdf").await.is_empty(), "the moved file has no origin");
    let rm = args(json!({"mount_id": PROJ, "path": "/b.pdf"}));
    tool_json(w.env.server(ALICE).fs_delete(rm).await).unwrap();
    w.env.at(40).await;
    assert_eq!(w.doc.calls(), 1);
    assert!(w.text("/a.md").await.unwrap().contains("Converted one"));
}

/// X7: a rebuild and an upload racing at the quota boundary go through the
/// one admission lock: the limit is never exceeded and one of them is refused.
#[tokio::test(start_paused = true)]
async fn e2e_0113_016_a_rebuild_and_an_upload_never_exceed_the_quota_together() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    let c = w.client().await;
    let used = crate::quota::used_bytes(&c).await.unwrap();
    let limit = used + 4000;
    w.env.state.admin.set_quota(PROJ, Some(limit)).await.unwrap();
    w.doc.set_pad(3000);
    let safety = &w.env.state.safety;
    let rebuild = fs_ops::rebuild_companion(
        &c,
        safety,
        Some(&*w.doc),
        PROJ,
        "/a.pdf",
        JobCause::Manual,
        &|| true,
    );
    let big = "n".repeat(2500);
    let upload = w.put("/n.bin", &big, false);
    let (r, u) = tokio::join!(rebuild, upload);
    assert!(r.is_ok() != u.is_ok(), "exactly one is admitted: {r:?} {u:?}");
    let refusal = r.err().map(|e| e.message).or(u.err()).unwrap();
    assert!(refusal.to_lowercase().contains("quota"), "the refusal is the quota: {refusal}");
    assert!(crate::quota::used_bytes(&c).await.unwrap() <= limit);
}

/// X10: a restart keeps a waiting conversion job (full delay again) and ends a
/// running one failed, then queues the source again.
#[tokio::test(start_paused = true)]
async fn e2e_0113_017_a_restart_keeps_waiting_and_requeues_running_conversions() {
    let mut w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    w.env.restart().await;
    assert_eq!(w.conv_jobs("/a.pdf").await[0]["state"], "waiting");
    w.doc.hold_next();
    w.env.at(31).await;
    w.doc.entered().await;
    w.env.restart().await;
    let jobs = w.conv_jobs("/a.pdf").await;
    let states: Vec<_> = jobs.iter().map(|j| j["state"].as_str().unwrap()).collect();
    assert!(states.contains(&"failed") && states.contains(&"waiting"), "{jobs:?}");
    let failed = jobs.iter().find(|j| j["state"] == "failed").unwrap();
    assert_eq!(failed["reason"], "interrupted by a server restart");
    w.env.at(70).await;
    assert!(w.text("/a.md").await.unwrap().contains("Converted two"));
}

/// FR-NEW-007: a result whose guard reads false is dropped before anything is
/// charged or written, whether or not the node revision moved.
#[tokio::test(start_paused = true)]
async fn e2e_0113_022_a_false_guard_drops_the_result() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    let c = w.client().await;
    let safety = &w.env.state.safety;
    let r = fs_ops::rebuild_companion(
        &c,
        safety,
        Some(&*w.doc),
        PROJ,
        "/a.pdf",
        JobCause::Manual,
        &|| false,
    )
    .await
    .unwrap();
    assert_eq!(r, Rebuilt::Superseded);
    assert_eq!(w.doc.calls(), 2, "the converter answered, the result was dropped");
    assert!(w.text("/a.md").await.unwrap().contains("Converted one"));
    assert_eq!(safety.bytes_written(fs_ops::CONVERSION_PERSON, PROJ), 0);
}

// ── US-0340: fs.recalculate kind conversion, PostgreSQL ─────────────────────

impl W {
    /// `fs.recalculate` by Alice.
    pub async fn recalc(
        &self,
        path: &str,
        kind: &str,
        set_id: &str,
    ) -> std::result::Result<Value, String> {
        let a = args(json!({"mount_id": PROJ, "path": path, "kind": kind, "set_id": set_id}));
        tool_json(self.env.server(ALICE).fs_recalculate(a).await)
    }
}

/// S2: a member asks for the conversion by hand: it starts at once (cause
/// `manual`), converts even when the bytes did not change, and ends `done`.
/// A manual companion is left untouched with a note; an absent one is created.
#[tokio::test(start_paused = true)]
async fn e2e_0113_018_a_member_recalculates_the_conversion() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    let job = w.recalc("/a.pdf", "conversion", "").await.unwrap();
    assert_eq!(job["kind"], "conversion");
    assert_eq!(job["state"], "running");
    assert_eq!(job["cause"], "manual");
    assert_eq!(job["path"], "/a.pdf");
    w.env.at(2).await;
    assert_eq!(w.doc.calls(), 2, "a manual rebuild converts even unchanged bytes");
    let jobs = w.conv_jobs("/a.pdf").await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["state"], "done");
    assert_eq!(w.artifacts("/a.pdf").await, (names(&["Fig one"]), names(&["Tab one"])));

    // A companion nobody converted is never overwritten: the job says why.
    w.put("/m.pdf", "x", false).await.unwrap();
    w.edit("/m.md", "mine").await;
    w.recalc("/m.pdf", "conversion", "").await.unwrap();
    w.env.at(4).await;
    let jobs = w.conv_jobs("/m.pdf").await;
    assert_eq!(jobs[0]["state"], "failed");
    assert!(jobs[0]["reason"].as_str().unwrap().contains("left untouched"), "{jobs:?}");
    assert_eq!(w.text("/m.md").await.as_deref(), Some("mine"));

    // An absent companion is created, with its origin.
    w.put("/n.pdf", "y", false).await.unwrap();
    w.recalc("/n.pdf", "conversion", "").await.unwrap();
    w.env.at(6).await;
    assert!(w.text("/n.md").await.unwrap().contains("Converted y"));
    assert_eq!(w.origin("/n.md").await.unwrap().source_path, "/n.pdf");
}

/// FR-NEW-010: the refusals keep the contract order and leave a waiting job as
/// it was.
#[tokio::test(start_paused = true)]
async fn e2e_0113_019_recalculate_conversion_refusals() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    let refused = |r: std::result::Result<Value, String>, text: &str| {
        let e = r.expect_err("refused");
        assert!(e.ends_with(text), "expected {text:?}, got {e:?}");
    };
    refused(
        w.recalc("/a.pdf", "conversion", "set-1").await,
        "set_id applies to kind 'chunks' only",
    );
    refused(w.recalc("/nope.pdf", "conversion", "").await, "file not found: /nope.pdf");
    refused(w.recalc("/", "conversion", "").await, "path must be a file");
    refused(
        w.recalc("/a.pdf", "concepts", "").await,
        "kind must be 'search_index', 'chunks' or 'conversion'",
    );
    assert_eq!(w.conv_jobs("/a.pdf").await[0]["state"], "waiting", "the waiting job is untouched");
    // Search off: no scheduler, so nothing can run the job.
    let off = W::with(Options { search: false, ..Options::default() }).await;
    off.put("/a.pdf", "one", false).await.unwrap();
    refused(
        off.recalc("/a.pdf", "conversion", "").await,
        "search is not enabled; set search.enabled: true in config",
    );
    // A non member is refused first.
    let a = args(json!({"mount_id": PROJ, "path": "/a.pdf", "kind": "conversion"}));
    let e = tool_json(w.env.server(crate::search::derived_e2e::DAVE).fs_recalculate(a).await);
    assert!(e.unwrap_err().contains("is not a member"));
}

/// A manual rebuild supersedes a conversion that is running (its result would
/// be the older one) and replaces the waiting job: one rebuild lands.
#[tokio::test(start_paused = true)]
async fn e2e_0113_020_a_recalculation_supersedes_the_running_conversion() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    w.doc.hold_next();
    w.env.at(31).await;
    w.doc.entered().await;
    let job = w.recalc("/a.pdf", "conversion", "").await.unwrap();
    assert_eq!(job["state"], "waiting", "behind the running job: {job}");
    w.doc.release();
    w.env.at(34).await;
    let jobs = w.conv_jobs("/a.pdf").await;
    assert!(jobs.iter().all(|j| j["state"] != "waiting" && j["state"] != "running"), "{jobs:?}");
    assert!(w.text("/a.md").await.unwrap().contains("Converted two"));
    assert_eq!(w.doc.calls(), 3, "the discarded run, then the manual one");
}

/// X11: the same flow on PostgreSQL (jobs in the admin schema, origins in the
/// volume schema). Real clock, one second of settle delay: a paused clock jumps
/// while a socket answer is in flight (AUTO-029).
#[cfg(feature = "postgres")]
#[tokio::test]
async fn e2e_0113_021_the_conversion_rebuild_on_postgres() {
    if std::env::var("MCPFS_TEST_PG_DSN").map_or(true, |v| v.trim().is_empty()) {
        eprintln!("skipping e2e_0113_021: MCPFS_TEST_PG_DSN is unset");
        return;
    }
    let w = W::with(Options { pg: true, delay: 1, ..Options::default() }).await;
    w.converted("/a.pdf", "one").await;
    w.edit("/m.md", "mine").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    let wait = std::time::Duration::from_secs(6);
    let started = std::time::Instant::now();
    loop {
        let md = w.text("/a.md").await.unwrap();
        if md.contains("Converted two") || started.elapsed() > wait {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    assert!(w.text("/a.md").await.unwrap().contains("Converted two"), "rebuilt on postgres");
    assert_eq!(w.artifacts("/a.pdf").await, (names(&["Fig two"]), names(&["Tab two"])));
    // A manual companion is never overwritten on postgres either.
    w.put("/m.pdf", "x", false).await.unwrap();
    w.recalc("/m.pdf", "conversion", "").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert_eq!(w.text("/m.md").await.as_deref(), Some("mine"));
    let jobs = w.conv_jobs("/m.pdf").await;
    assert_eq!(jobs[0]["state"], "failed", "{jobs:?}");
    w.env.drop_schema().await;
}

/// FR-NEW-001, FR-NEW-002, X7 on PostgreSQL, with no timer in play: the origin
/// round trip, a hand edit read as manual, and the admission lock at the quota
/// boundary (one of a rebuild and an upload is refused).
#[cfg(feature = "postgres")]
#[tokio::test]
async fn e2e_0113_023_origin_and_admission_on_postgres() {
    if std::env::var("MCPFS_TEST_PG_DSN").map_or(true, |v| v.trim().is_empty()) {
        eprintln!("skipping e2e_0113_023: MCPFS_TEST_PG_DSN is unset");
        return;
    }
    let w = W::with(Options { pg: true, ..Options::default() }).await;
    w.converted("/a.pdf", "one").await;
    let o = w.origin("/a.md").await.expect("the origin is stored on postgres");
    assert_eq!(
        (o.source_path.as_str(), o.source_sha.as_str()),
        ("/a.pdf", sha_hex(b"one").as_str())
    );
    let c = w.client().await;
    assert!(fs_ops::converted_origin(&c, "/a.md").await.unwrap().is_some());
    w.edit("/a.md", "edited").await;
    assert!(fs_ops::converted_origin(&c, "/a.md").await.unwrap().is_none());
    // The admission lock: a rebuild and an upload at the boundary.
    w.documentize("/a.pdf").await.unwrap();
    let used = crate::quota::used_bytes(&c).await.unwrap();
    let limit = used + 4000;
    w.env.state.admin.set_quota(PROJ, Some(limit)).await.unwrap();
    w.doc.set_pad(3000);
    let safety = &w.env.state.safety;
    let rebuild = fs_ops::rebuild_companion(
        &c,
        safety,
        Some(&*w.doc),
        PROJ,
        "/a.pdf",
        JobCause::Manual,
        &|| true,
    );
    let big = "n".repeat(2500);
    let (r, u) = tokio::join!(rebuild, w.put("/n.bin", &big, false));
    assert!(r.is_ok() != u.is_ok(), "exactly one is admitted: {r:?} {u:?}");
    assert!(crate::quota::used_bytes(&c).await.unwrap() <= limit);
    w.env.drop_schema().await;
}

/// Review finding, FR-NEW-005: a member edits the companion while the
/// conversion runs. The check under the admission lock refuses the landing:
/// the edit stays, the job ends `failed` with the note.
#[tokio::test(start_paused = true)]
async fn e2e_0113_024_a_hand_edit_during_the_conversion_is_kept() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    w.doc.hold_next();
    w.env.at(31).await;
    w.doc.entered().await;
    w.edit("/a.md", "edited while converting").await;
    w.doc.release();
    w.env.at(33).await;
    assert_eq!(w.text("/a.md").await.as_deref(), Some("edited while converting"));
    let jobs = w.conv_jobs("/a.pdf").await;
    assert_eq!(jobs[0]["state"], "failed", "{jobs:?}");
    assert!(jobs[0]["reason"].as_str().unwrap().contains("left untouched"), "{jobs:?}");
    assert_eq!(w.env.state.safety.bytes_written(fs_ops::CONVERSION_PERSON, PROJ), 0);
}

/// Review finding, DEC-007: the write hook reads the companion and its origin
/// under the admission lock a landing conversion holds, so it never sees new
/// bytes with the old origin. While the lock is held the hook waits; when it is
/// released the job is scheduled.
#[tokio::test(start_paused = true)]
async fn e2e_0113_025_the_write_hook_waits_for_a_landing_conversion() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    let c = w.client().await;
    let c2 = c.clone();
    c.write_bytes_atomic("/a.pdf", b"two").await.unwrap();
    let window = c.admission.clone().lock_owned().await;
    let state = w.env.state.clone();
    let hook = tokio::spawn(async move {
        crate::search::indexer::after_write_reread(&state, PROJ, "/a.pdf", &c2).await;
    });
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    assert!(!hook.is_finished(), "the hook must wait for the landing window");
    assert!(w.conv_jobs("/a.pdf").await.is_empty());
    drop(window);
    hook.await.unwrap();
    assert_eq!(w.conv_jobs("/a.pdf").await.len(), 1);
}

/// Review finding: a source reverted to the bytes the companion was made from,
/// while a conversion of the other bytes runs, ends that run `superseded`, not
/// `failed` on the revision check.
#[tokio::test(start_paused = true)]
async fn e2e_0113_026_a_revert_during_the_run_supersedes_it() {
    let w = W::new().await;
    w.converted("/a.pdf", "one").await;
    w.put("/a.pdf", "two", false).await.unwrap();
    w.doc.hold_next();
    w.env.at(31).await;
    w.doc.entered().await;
    w.put("/a.pdf", "one", false).await.unwrap();
    w.doc.release();
    w.env.at(40).await;
    let jobs = w.conv_jobs("/a.pdf").await;
    assert!(jobs.iter().all(|j| j["state"] == "superseded"), "{jobs:?}");
    assert!(w.text("/a.md").await.unwrap().contains("Converted one"));
    assert_eq!(w.doc.calls(), 2);
}
