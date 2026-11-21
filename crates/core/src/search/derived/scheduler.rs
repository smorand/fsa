//! The rebuild scheduler (SPEC-0100 design section 3.1, DEC-003).
//!
//! A change of a file of an indexing project schedules one rebuild of its
//! search entries, due when the file has been quiet for the settle delay. Each
//! further change pushes the due time back on the same waiting job. One
//! dispatcher task sleeps until the earliest due job whose key has nothing
//! running, moves it to running and spawns its rebuild; the rebuild reads the
//! file when it starts (DEC-021), so it indexes the content of the last save.
//!
//! Every state change and row write happens under one async mutex; the lock is
//! never held while a rebuild runs, so a write that schedules waits at most for
//! one short row write, never for an embedding round trip (FR-NEW-004).
//!
//! Only `tokio::time` is used for due times, so a test can pause the clock.

use super::{Job, JobCause, JobKind, JobState, JobStore};
use crate::errors::ToolError;
use crate::search::SearchBackend;
use crate::search::indexer::{AUTO_CHUNK_OVERLAP, AUTO_CHUNK_SIZE};
use crate::storage::StoreManager;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, Notify};
use tokio::time::{Duration, Instant};

/// The reason of a rebuild that was running when the server stopped (FR-NEW-017).
const RESTART_REASON: &str = "interrupted by a server restart";

/// One file and kind of one project: at most one waiting and one running job.
type Key = (String, String, JobKind);

struct Waiting {
    job: Job,
    due: Instant,
}

struct Running {
    job: Job,
    /// Set by a change during the run: the result must not be stored (FR-NEW-013).
    stale: Arc<AtomicBool>,
    /// Aborted by `stop`, so a stopped scheduler cannot end a row a later
    /// recovery already ended.
    task: tokio::task::JoinHandle<()>,
    /// Once removed, whether its end clears the entries of its path (DEC-010).
    /// False when a mode change between two active modes removed it: the full
    /// index then writes the path again, and a clear would undo it (FR-NEW-015).
    clear_at_end: bool,
}

#[derive(Default)]
struct Slot {
    waiting: Option<Waiting>,
    running: Option<Running>,
    /// Rebuilds of a file deleted (or replaced by a move) while they ran: the
    /// design's `removed` flag (DEC-010). Kept apart from `running` so a move
    /// can bring its own running job onto a key that still holds one of them;
    /// they block a new start of the key like `running` does, so their final
    /// entry clear cannot land after a newer rebuild's commit.
    removed: Vec<Running>,
}

impl Slot {
    fn busy(&self) -> bool {
        self.running.is_some() || !self.removed.is_empty()
    }

    fn is_empty(&self) -> bool {
        self.waiting.is_none() && !self.busy()
    }
}

/// A waiting or running job as the list shows it, with its whole seconds left.
pub struct Live {
    pub job: Job,
    pub seconds_left: u64,
}

pub struct Scheduler {
    /// The engine itself, never the decorator, so a rebuild indexes at once.
    backend: Arc<dyn SearchBackend>,
    store: JobStore,
    stores: Arc<StoreManager>,
    delay: Duration,
    slots: Mutex<HashMap<Key, Slot>>,
    wake: Arc<Notify>,
    /// Job times follow the tokio clock from this wall clock origin, so the
    /// times a member reads agree with the delays the scheduler applies, also on
    /// a paused test clock. Drift against the wall clock stays the monotonic
    /// clock's, a few milliseconds a day.
    origin_wall: chrono::DateTime<chrono::Utc>,
    origin: Instant,
    dispatcher: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Scheduler {
    /// Builds the scheduler and starts its dispatcher; needs a tokio runtime.
    pub fn new(
        backend: Arc<dyn SearchBackend>,
        store: JobStore,
        stores: Arc<StoreManager>,
        delay_secs: u64,
    ) -> Arc<Self> {
        let s = Arc::new(Self {
            backend,
            store,
            stores,
            delay: Duration::from_secs(delay_secs),
            slots: Mutex::new(HashMap::new()),
            wake: Arc::new(Notify::new()),
            origin_wall: chrono::Utc::now(),
            origin: Instant::now(),
            dispatcher: std::sync::Mutex::new(None),
        });
        let handle = tokio::spawn(dispatch(Arc::downgrade(&s), s.wake.clone()));
        *s.dispatcher.lock().expect("dispatcher mutex") = Some(handle);
        s
    }

    /// Stops the dispatcher and the running rebuilds, as the end of the process
    /// would. Rows stay as they are: they are the truth a later start recovers
    /// from (DEC-020).
    pub fn stop(&self) {
        if let Some(h) = self.dispatcher.lock().expect("dispatcher mutex").take() {
            h.abort();
        }
        if let Ok(slots) = self.slots.try_lock() {
            for s in slots.values() {
                for r in s.running.iter().chain(&s.removed) {
                    r.task.abort();
                }
            }
        }
    }

    /// Boot recovery (DEC-020, FR-NEW-007, FR-NEW-017). Rows are the truth, the
    /// dispatcher is stateless: every waiting row is queued again with the full
    /// delay (`due` is never stored, DEC-005), then every running row, whose run
    /// died with the process, ends as failed and gets a new rebuild through the
    /// ordinary `schedule`, so a file with both rows keeps one waiting job.
    /// Waiting rows go first for that reason. Cost: one read, then one row write
    /// per interrupted job.
    pub async fn recover(&self) -> crate::errors::Result<()> {
        let rows = self.store.load_unfinished(None).await?;
        let (running, waiting): (Vec<Job>, Vec<Job>) =
            rows.into_iter().partition(|j| j.state == JobState::Running);
        {
            let mut slots = self.slots.lock().await;
            let due = Instant::now() + self.delay;
            for mut job in waiting {
                let key = (job.project_id.clone(), job.path.clone(), job.kind);
                let slot = slots.entry(key).or_default();
                if slot.waiting.is_none() {
                    // The row may predate a change of the configured delay.
                    job.hidden = self.delay.is_zero();
                    slot.waiting = Some(Waiting { job, due });
                }
            }
        }
        for job in running {
            self.store
                .finish(&job.job_id, JobState::Failed, &self.now_secs(), Some(RESTART_REASON))
                .await?;
            // FR-NEW-001: a job a restart creates is a `change` one, even
            // when the interrupted job was manual.
            self.schedule(&job.project_id, &job.path, JobCause::Change).await;
        }
        self.wake.notify_one();
        Ok(())
    }

    /// RFC 3339 UTC seconds of the tokio clock (see `origin_wall`).
    fn now_secs(&self) -> String {
        let elapsed = chrono::Duration::from_std(self.origin.elapsed()).unwrap_or_default();
        (self.origin_wall + elapsed).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    /// Rule 1: a change creates the waiting job, or pushes its due time back
    /// without a row write; a running job of the key becomes stale. The caller
    /// has checked that the project's index mode is active.
    pub async fn schedule(&self, project: &str, path: &str, cause: JobCause) {
        let key = (project.to_string(), path.to_string(), JobKind::SearchIndex);
        let due = Instant::now() + self.delay;
        let mut slots = self.slots.lock().await;
        let slot = slots.entry(key).or_default();
        if let Some(r) = &slot.running {
            r.stale.store(true, Ordering::SeqCst);
        }
        match &mut slot.waiting {
            Some(w) => w.due = due,
            None => slot.waiting = Some(self.new_waiting(project, path, cause, due).await),
        }
        drop(slots);
        self.wake.notify_one();
    }

    /// FR-NEW-008: a member asks for a rebuild of `path` now. The waiting job of
    /// the key goes without a record (DEC-007), and a new manual job is created.
    /// With nothing running it starts in this call, so the answer reads
    /// `running`; behind a running job, which becomes stale and so ends
    /// `superseded` (FR-NEW-013), it is a listed waiting job due at once, which
    /// the dispatcher starts when the running one ends. Returns the job as the
    /// member sees it. Cost: at most three row writes under the lock.
    pub async fn recalculate(self: &Arc<Self>, project: &str, path: &str) -> serde_json::Value {
        let key = (project.to_string(), path.to_string(), JobKind::SearchIndex);
        let now = Instant::now();
        let mut slots = self.slots.lock().await;
        let slot = slots.entry(key.clone()).or_default();
        if let Some(old) = slot.waiting.take()
            && let Err(e) = self.store.delete(&old.job.job_id).await
        {
            tracing::warn!(job_id = %old.job.job_id, error = %e, "cannot remove a replaced job");
        }
        if let Some(r) = &slot.running {
            r.stale.store(true, Ordering::SeqCst);
        }
        let mut job = Job::new_waiting(project, path, JobKind::SearchIndex, JobCause::Manual);
        job.created_at = self.now_secs();
        if slot.busy() {
            if let Err(e) = self.store.insert(&job).await {
                tracing::warn!(project, path, error = %e, "cannot record a rebuild job");
            }
            let answer = job.to_json(Some(0));
            slot.waiting = Some(Waiting { job, due: now });
            drop(slots);
            self.wake.notify_one();
            return answer;
        }
        // Written as running at once: a row never shows a job that already ran
        // as waiting, even for the instant between two writes.
        job.state = JobState::Running;
        if let Err(e) = self.store.insert(&job).await {
            tracing::warn!(project, path, error = %e, "cannot record a rebuild job");
        }
        let stale = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn(self.clone().rebuild(key, job.clone(), stale.clone()));
        let answer = job.to_json(None);
        slot.running = Some(Running { job, stale, task, clear_at_end: true });
        answer
    }

    /// A new waiting job, its row written. Called under the slots lock.
    async fn new_waiting(
        &self,
        project: &str,
        path: &str,
        cause: JobCause,
        due: Instant,
    ) -> Waiting {
        let mut job = Job::new_waiting(project, path, JobKind::SearchIndex, cause);
        job.created_at = self.now_secs();
        // A delay of 0 starts at once: no member ever sees it wait (DEC-007).
        job.hidden = self.delay.is_zero();
        if let Err(e) = self.store.insert(&job).await {
            // The rebuild still runs from memory; only its record is missing.
            tracing::warn!(project, path, error = %e, "cannot record a rebuild job");
        }
        Waiting { job, due }
    }

    /// FR-NEW-005: a file is deleted. Its waiting job is dropped, its running
    /// job is listed `superseded` until its computation ends and then leaves no
    /// record, and every row of the path goes, finished ones included. Called
    /// before the caller clears the entries, so a commit the guard let through
    /// is cleared by the rebuild's epilogue (DEC-010). Cost: one row delete.
    pub async fn forget_path(&self, project: &str, path: &str) {
        let mut slots = self.slots.lock().await;
        self.forget_locked(&mut slots, project, path).await;
    }

    async fn forget_locked(&self, slots: &mut HashMap<Key, Slot>, project: &str, path: &str) {
        let key = (project.to_string(), path.to_string(), JobKind::SearchIndex);
        if let Some(mut slot) = slots.remove(&key) {
            slot.waiting = None;
            if let Some(mut r) = slot.running.take() {
                r.stale.store(true, Ordering::SeqCst);
                r.job.state = JobState::Superseded;
                slot.removed.push(r);
            }
            // The file is gone now: a rebuild an earlier mode change left
            // unclearing must clear it after all.
            for r in &mut slot.removed {
                r.clear_at_end = true;
            }
            if !slot.removed.is_empty() {
                slots.insert(key, slot);
            }
        }
        if let Err(e) = self.store.delete_path(project, path).await {
            tracing::warn!(project, path, error = %e, "cannot remove the jobs of a deleted file");
        }
    }

    /// FR-NEW-015, FR-NEW-018, FR-NEW-019: every job of a project goes, as for
    /// a delete of each of its files: waiting jobs are dropped, running ones are
    /// listed `superseded` until their computation ends and then leave no record
    /// (their epilogue clears the entries of their path, DEC-010), and every row
    /// of the project goes, finished ones included. Called on an index mode
    /// change (after the wipe, before the full index), on a soft delete and on an
    /// undelete (DEC-016). `clear_entries_at_end` is false only for a mode
    /// change whose new mode is active, as the full index rewrites every path.
    /// Cost: one pass over the slots, one row delete.
    pub async fn forget_project(&self, project: &str, clear_entries_at_end: bool) {
        let mut slots = self.slots.lock().await;
        let keys: Vec<Key> = slots.keys().filter(|k| k.0 == project).cloned().collect();
        for key in keys {
            let Some(mut slot) = slots.remove(&key) else { continue };
            slot.waiting = None;
            if let Some(mut r) = slot.running.take() {
                r.stale.store(true, Ordering::SeqCst);
                r.job.state = JobState::Superseded;
                r.clear_at_end = clear_entries_at_end;
                slot.removed.push(r);
            }
            // A clear asked for once stays asked for (a removal of the project
            // after a mode change still clears).
            for r in &mut slot.removed {
                r.clear_at_end |= clear_entries_at_end;
            }
            if !slot.removed.is_empty() {
                slots.insert(key, slot);
            }
        }
        if let Err(e) = self.store.delete_project(project).await {
            tracing::warn!(project, error = %e, "cannot remove the jobs of a project");
        }
    }

    /// The hook the admin store calls when a project is soft deleted, restored
    /// or removed. Holds the scheduler weakly: the store outlives a stopped one.
    pub fn project_hook(self: &Arc<Self>) -> crate::storage::traits::ProjectGoneHook {
        let weak = Arc::downgrade(self);
        Arc::new(move |project: String| {
            let weak = weak.clone();
            Box::pin(async move {
                if let Some(s) = weak.upgrade() {
                    s.forget_project(&project, true).await;
                }
            })
        })
    }

    /// FR-NEW-006: a file moves from `from` to `to`. Jobs of a replaced `to`
    /// go first, as for a delete; the waiting job keeps its due time under the
    /// new path; a running job is superseded, renamed and followed by a new
    /// waiting job with the full delay, which it blocks until it ends; finished
    /// rows are renamed. Cost: at most three row writes.
    pub async fn move_path(&self, project: &str, from: &str, to: &str) {
        if from == to {
            return;
        }
        let mut slots = self.slots.lock().await;
        self.forget_locked(&mut slots, project, to).await;
        if let Err(e) = self.store.move_path(project, from, to).await {
            tracing::warn!(project, from, to, error = %e, "cannot move the jobs of a file");
        }
        let from_key = (project.to_string(), from.to_string(), JobKind::SearchIndex);
        let Some(mut src) = slots.remove(&from_key) else { return };
        // A rebuild already removed keeps clearing the old path when it ends.
        let removed = std::mem::take(&mut src.removed);
        if !removed.is_empty() {
            slots.insert(from_key, Slot { removed, ..Slot::default() });
        }
        let to_key = (project.to_string(), to.to_string(), JobKind::SearchIndex);
        let mut dest = Slot::default();
        if let Some(mut w) = src.waiting {
            w.job.path = to.to_string();
            dest.waiting = Some(w);
        }
        if let Some(mut r) = src.running {
            r.stale.store(true, Ordering::SeqCst);
            r.job.state = JobState::Superseded;
            r.job.path = to.to_string();
            dest.running = Some(r);
            if dest.waiting.is_none() {
                let due = Instant::now() + self.delay;
                dest.waiting = Some(self.new_waiting(project, to, JobCause::Change, due).await);
            }
        }
        // `to` was just forgotten, so only its removed rebuilds can be there.
        let slot = slots.entry(to_key).or_default();
        slot.waiting = dest.waiting;
        slot.running = dest.running;
        drop(slots);
        self.wake.notify_one();
    }

    /// The listed waiting and running jobs of a project whose path passes
    /// `in_scope`; hidden waiting entries are left out (DEC-007).
    pub async fn list(&self, project: &str, in_scope: &(dyn Fn(&str) -> bool + Sync)) -> Vec<Live> {
        let now = Instant::now();
        let slots = self.slots.lock().await;
        let mut out = Vec::new();
        for ((p, path, _), slot) in slots.iter() {
            if p != project || !in_scope(path) {
                continue;
            }
            if let Some(w) = slot.waiting.as_ref().filter(|w| !w.job.hidden) {
                let left = w.due.saturating_duration_since(now);
                // Rounded up, so a job never reads 0 seconds while it still waits.
                let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
                out.push(Live { job: w.job.clone(), seconds_left: secs });
            }
            for r in slot.running.iter().chain(&slot.removed) {
                out.push(Live { job: r.job.clone(), seconds_left: 0 });
            }
        }
        out
    }

    /// Rule 2, one pass: start every due waiting job whose key has nothing
    /// running, and answer the earliest due time still pending.
    async fn start_due(self: &Arc<Self>) -> Option<Instant> {
        let now = Instant::now();
        let mut slots = self.slots.lock().await;
        let mut next: Option<Instant> = None;
        for (key, slot) in slots.iter_mut() {
            if slot.busy() {
                continue;
            }
            let Some(due) = slot.waiting.as_ref().map(|w| w.due) else { continue };
            if due > now {
                next = Some(next.map_or(due, |n| n.min(due)));
                continue;
            }
            let Waiting { mut job, .. } = slot.waiting.take().expect("a waiting job");
            job.state = JobState::Running;
            if let Err(e) = self.store.set_state(&job.job_id, JobState::Running).await {
                tracing::warn!(job_id = %job.job_id, error = %e, "cannot record a job start");
            }
            let stale = Arc::new(AtomicBool::new(false));
            let task = tokio::spawn(self.clone().rebuild(key.clone(), job.clone(), stale.clone()));
            slot.running = Some(Running { job, stale, task, clear_at_end: true });
        }
        next
    }

    /// Rule 3: rebuild, then record the outcome and free the key.
    async fn rebuild(self: Arc<Self>, key: Key, job: Job, stale: Arc<AtomicBool>) {
        let (project, path, _) = &key;
        let outcome = match self.read(project, path).await {
            // Not text, or gone: nothing to index, which is a finished rebuild.
            None => Ok(Some(0)),
            Some(text) => {
                let guard = || !stale.load(Ordering::SeqCst);
                self.backend
                    .index_path_checked(
                        project,
                        path,
                        &text,
                        AUTO_CHUNK_SIZE,
                        AUTO_CHUNK_OVERLAP,
                        &guard,
                    )
                    .await
            }
        };
        let (state, reason) = match outcome {
            Ok(None) => (JobState::Superseded, None),
            Ok(Some(_)) if stale.load(Ordering::SeqCst) => (JobState::Superseded, None),
            Ok(Some(_)) => (JobState::Done, None),
            Err(e) => {
                tracing::warn!(project, path, error = %e, "rebuild of search entries failed");
                (JobState::Failed, Some(failure_reason(&e)))
            }
        };

        let mut slots = self.slots.lock().await;
        // A move may have carried the job to another key: found by its id.
        let at = if slots.get(&key).is_some_and(|s| holds(s, &job.job_id)) {
            Some(key.clone())
        } else {
            slots.iter().find(|(_, s)| holds(s, &job.job_id)).map(|(k, _)| k.clone())
        };
        let mut was_removed = false;
        let mut clear_at_end = true;
        if let Some(k) = &at {
            let slot = slots.get_mut(k).expect("the slot just found");
            if slot.running.as_ref().is_some_and(|r| r.job.job_id == job.job_id) {
                slot.running = None;
            } else {
                if let Some(r) = slot.removed.iter().find(|r| r.job.job_id == job.job_id) {
                    clear_at_end = r.clear_at_end;
                }
                slot.removed.retain(|r| r.job.job_id != job.job_id);
                was_removed = true;
            }
        }
        let recorded = if was_removed {
            // FR-NEW-005: a job removed while running leaves no record.
            self.store.delete(&job.job_id).await
        } else {
            self.store.finish(&job.job_id, state, &self.now_secs(), reason.as_deref()).await
        };
        if let Err(e) = recorded {
            tracing::warn!(job_id = %job.job_id, error = %e, "cannot record a job end");
        }
        // DEC-010: a commit may have slipped between the guard and the clear of
        // a delete or a move, so the old path is cleared once more, unless a
        // newer job of that path will write its entries again.
        let clear_old = ((was_removed && clear_at_end)
            || at.as_ref().is_some_and(|k| k.1 != *path))
            && slots.get(&key).is_none_or(|s| s.waiting.is_none() && s.running.is_none());
        for k in at.iter().chain(std::iter::once(&key)) {
            if slots.get(k).is_some_and(Slot::is_empty) {
                slots.remove(k);
            }
        }
        drop(slots);
        if clear_old && let Err(e) = self.backend.delete_path(project, path).await {
            tracing::warn!(project, path, error = %e, "cannot clear the entries of a removed file");
        }
        // A waiting job blocked behind this one may now be due.
        self.wake.notify_one();
    }

    async fn read(&self, project: &str, path: &str) -> Option<String> {
        let client = self.stores.client(project).await.ok()?;
        crate::search::indexer::read_utf8(&client, path).await
    }
}

/// True when `slot` holds the running or removed job `job_id`.
fn holds(slot: &Slot, job_id: &str) -> bool {
    slot.running.iter().chain(&slot.removed).any(|r| r.job.job_id == job_id)
}

/// The dispatcher loop. Holds the scheduler weakly so dropping the last owner
/// ends it.
async fn dispatch(scheduler: Weak<Scheduler>, wake: Arc<Notify>) {
    loop {
        let next = match scheduler.upgrade() {
            Some(s) => s.start_due().await,
            None => return,
        };
        match next {
            Some(at) => {
                tokio::select! {
                    _ = tokio::time::sleep_until(at) => {}
                    _ = wake.notified() => {}
                }
            }
            None => wake.notified().await,
        }
    }
}

/// DEC-019: the two failure texts of the embedding call read as the service
/// being unreachable; any other error keeps its code and message.
pub fn failure_reason(e: &ToolError) -> String {
    if e.message.starts_with("embedding request failed")
        || e.message.starts_with("embedding endpoint returned")
    {
        return "embedding service unreachable".into();
    }
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_embedding_failure_reads_as_the_service_unreachable() {
        for msg in ["embedding request failed: refused", "embedding endpoint returned HTTP 503"] {
            assert_eq!(failure_reason(&ToolError::internal(msg)), "embedding service unreachable");
        }
        let other = ToolError::invalid_argument("bad");
        assert_eq!(failure_reason(&other), "ERR_INVALID_ARGUMENT: bad");
    }
}
