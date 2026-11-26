//! The recalculation of vectors (SPEC-0108): a `vectors` job over every file of
//! a project, or over one file, run by a task of its own (DEC-005). The task
//! rebuilds each file with the same code as a settled change (`rebuild_file`:
//! the default set's entries, then the images of a converted document), one
//! file at a time, writing its progress after each file.
//!
//! One job runs per project. A change of the file being read raises its stale
//! flag, so the job drops its result for that file and the change's own rebuild
//! indexes the new content (FR-NEW-015). A cancel is read between two files and
//! by the guard of the file in flight.

use super::*;
use crate::errors::Result;

/// The scope that means every file of the project.
pub const WHOLE_PROJECT: &str = "/";

pub(super) struct VectorRun {
    job: Job,
    cancel: Arc<AtomicBool>,
}

/// How a pass over the files ended without an error.
enum Pass {
    Complete,
    Cancelled,
}

fn scope_text(scope: &str) -> String {
    if scope == WHOLE_PROJECT { "the project".into() } else { scope.to_string() }
}

impl Scheduler {
    /// Starts a recalculation of `scope` (`/` or one file) and answers the job as
    /// its row reads, with whether it was already running (FR-NEW-013): the same
    /// scope answers the running job, another scope is refused. The caller has
    /// checked the mode, the access and that a file scope is a file.
    pub async fn start_vectors(
        self: &Arc<Self>,
        project: &str,
        scope: &str,
        cause: JobCause,
    ) -> Result<(Job, bool)> {
        let mut job = Job::new_waiting(project, scope, JobKind::Vectors, cause);
        job.state = JobState::Running;
        job.created_at = self.now_secs();
        job.done = Some(0);
        let cancel = Arc::new(AtomicBool::new(false));
        // The lock is released before any await: the map decides alone who starts.
        let running = {
            let mut runs = self.vector_runs.lock().expect("vector runs mutex");
            match runs.get(project) {
                Some(run) if run.job.path != scope => {
                    return Err(ToolError::invalid_argument(format!(
                        "a recalculation of {} is already running",
                        scope_text(&run.job.path)
                    )));
                }
                Some(run) => Some(run.job.clone()),
                None => {
                    runs.insert(
                        project.to_string(),
                        VectorRun { job: job.clone(), cancel: cancel.clone() },
                    );
                    None
                }
            }
        };
        if let Some(running) = running {
            let row = self.store.latest_vectors(project).await?;
            return Ok((row.unwrap_or(running), true));
        }
        if let Err(e) = self.store.insert(&job).await {
            self.vector_runs.lock().expect("vector runs mutex").remove(project);
            return Err(e);
        }
        // A supervisor, so a task that panics still ends its row and frees the project.
        let me = self.clone();
        let supervised = job.clone();
        tokio::spawn(async move {
            let run = tokio::spawn(me.clone().run_vectors(supervised.clone(), cancel));
            if run.await.is_err() {
                let why = "the recalculation stopped unexpectedly";
                me.end_vectors(&supervised, JobState::Failed, Some(why)).await;
            }
        });
        Ok((job, false))
    }

    /// Asks the running recalculation of `project` to stop after its current
    /// file; false when none runs (FR-NEW-012).
    pub fn cancel_vectors(&self, project: &str) -> bool {
        match self.vector_runs.lock().expect("vector runs mutex").get(project) {
            Some(run) => {
                run.cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    /// The running recalculation of `project`, else the newest finished one.
    pub async fn vectors_job(&self, project: &str) -> Result<Option<Job>> {
        self.store.latest_vectors(project).await
    }

    /// A change or a delete of `path` makes the result of the recalculation now
    /// reading it useless.
    pub(super) fn mark_inflight_stale(&self, project: &str, path: &str) {
        let inflight = self.vector_inflight.lock().expect("vector inflight mutex");
        if let Some((reading, flag)) = inflight.get(project)
            && reading == path
        {
            flag.store(true, Ordering::SeqCst);
        }
    }

    /// Ends the recalculations a restart interrupted as failed and remembers the
    /// scopes of those whose project still indexes vectors.
    pub(super) async fn recover_vectors(&self, rows: Vec<Job>) -> Result<()> {
        for job in rows {
            self.store
                .finish(&job.job_id, JobState::Failed, &self.now_secs(), Some(RESTART_REASON))
                .await?;
            if job.state == JobState::Running && self.store.wants_vectors(&job.project_id).await? {
                self.vector_resume
                    .lock()
                    .expect("vector resume mutex")
                    .push((job.project_id, job.path));
            }
        }
        Ok(())
    }

    /// Starts again every recalculation [`Self::recover`] found interrupted
    /// (FR-NEW-016): each file is rebuilt idempotently, so resuming is restarting.
    pub async fn resume_vectors(self: &Arc<Self>) {
        let scopes: Vec<(String, String)> =
            std::mem::take(&mut *self.vector_resume.lock().expect("vector resume mutex"));
        for (project, scope) in scopes {
            if let Err(e) = self.start_vectors(&project, &scope, JobCause::Change).await {
                tracing::warn!(project, scope, error = %e, "cannot resume a recalculation");
            }
        }
    }

    async fn run_vectors(self: Arc<Self>, job: Job, cancel: Arc<AtomicBool>) {
        let outcome = self.vectors_pass(&job, &cancel).await;
        let (state, reason) = match outcome {
            Ok(Pass::Complete) => (JobState::Done, None),
            Ok(Pass::Cancelled) => (JobState::Cancelled, None),
            Err(e) => {
                tracing::warn!(project = %job.project_id, scope = %job.path, error = %e,
                    "recalculation failed");
                (JobState::Failed, Some(failure_reason(&e)))
            }
        };
        self.end_vectors(&job, state, reason.as_deref()).await;
    }

    /// Ends the row first, then frees the project: a start that follows finds the
    /// finished row, never a second running one beside it.
    async fn end_vectors(&self, job: &Job, state: JobState, reason: Option<&str>) {
        if let Err(e) = self.store.finish(&job.job_id, state, &self.now_secs(), reason).await {
            tracing::warn!(job_id = %job.job_id, error = %e, "cannot record a job end");
        }
        self.vector_inflight.lock().expect("vector inflight mutex").remove(&job.project_id);
        self.vector_runs.lock().expect("vector runs mutex").remove(&job.project_id);
    }

    /// The files of the scope, rebuilt one by one. The first error ends the job:
    /// the files before it are rebuilt, the ones after keep their old vectors.
    async fn vectors_pass(&self, job: &Job, cancel: &Arc<AtomicBool>) -> Result<Pass> {
        let project = &job.project_id;
        let seen = self.store.project_stale_at(project).await?;
        let paths = self.files_of(project, &job.path).await?;
        let total = i64::try_from(paths.len()).unwrap_or(i64::MAX);
        self.store.set_progress(&job.job_id, 0, total).await?;
        for (i, path) in paths.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                return Ok(Pass::Cancelled);
            }
            // A file that is gone (a resumed or late single file job) keeps no vector.
            if job.path != WHOLE_PROJECT && !self.file_exists(project, path).await {
                self.backend.delete_path(project, path).await?;
                continue;
            }
            let stale = Arc::new(AtomicBool::new(false));
            self.vector_inflight
                .lock()
                .expect("vector inflight mutex")
                .insert(project.clone(), (path.clone(), stale.clone()));
            let guard = || !stale.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst);
            let rebuilt = self.rebuild_file(project, path, &guard).await;
            self.vector_inflight.lock().expect("vector inflight mutex").remove(project);
            rebuilt?;
            let done = i64::try_from(i + 1).unwrap_or(i64::MAX);
            if let Err(e) = self.store.set_progress(&job.job_id, done, total).await {
                tracing::warn!(job_id = %job.job_id, error = %e, "cannot record progress");
            }
        }
        if cancel.load(Ordering::SeqCst) {
            return Ok(Pass::Cancelled);
        }
        // Only a whole project pass, with the marker as the job found it, makes the
        // vectors current: a model changed meanwhile keeps the marker (DEC-006).
        if job.path == WHOLE_PROJECT
            && let Some(seen) = seen
        {
            self.store.clear_stale_if(project, &seen).await?;
        }
        Ok(Pass::Complete)
    }

    async fn file_exists(&self, project: &str, path: &str) -> bool {
        match self.stores.client(project).await {
            Ok(client) => client.is_file(path).await.unwrap_or(true),
            Err(_) => true,
        }
    }

    /// Every file of the project outside the trash, in path order, or the one
    /// file of the scope.
    async fn files_of(&self, project: &str, scope: &str) -> Result<Vec<String>> {
        if scope != WHOLE_PROJECT {
            return Ok(vec![scope.to_string()]);
        }
        let client = self.stores.client(project).await?;
        let trash_dir = self.stores.config().safety.trash_dir.clone();
        let mut paths: Vec<String> = crate::core::fs_ops::iter_files(&client, "/", &[])
            .await?
            .into_iter()
            .map(|(p, _)| p)
            .filter(|p| !crate::search::indexer::is_trash_path(p, &trash_dir))
            .collect();
        paths.sort();
        Ok(paths)
    }

    /// The image vectors of a converted document, computed with the project's
    /// image model; an empty set when it has none (clearing what an earlier
    /// model left). Engines without vectors have nothing to do.
    pub(super) async fn rebuild_images(
        &self,
        project: &str,
        path: &str,
        guard: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        if !self.backend.supported_modes().contains(&"rag") {
            return Ok(Some(0));
        }
        let Ok(client) = self.stores.client(project).await else { return Ok(Some(0)) };
        let mut images = Vec::new();
        if self.backend.embeds_images(project).await?
            && let Some(meta) = client.trash.clone()
            && let Some(set_id) = meta.current_artifact_set(path).await?
        {
            const PAGE: i64 = 100;
            let mut offset = 0;
            loop {
                let rows = meta.list_artifact_images(&set_id, PAGE, offset).await?;
                for row in &rows {
                    let Some(image) = meta.artifact_image(&set_id, row.seq).await? else {
                        continue;
                    };
                    let bytes = client.blob.get(&image.sha256, 0, None).await?;
                    let text = match &image.ocr_text {
                        Some(ocr) => format!("{}\n{ocr}", image.caption),
                        None => image.caption.clone(),
                    };
                    images.push(crate::search::ImageInput {
                        number: u32::try_from(row.seq).unwrap_or(0),
                        text,
                        data_uri: format!(
                            "data:image/{};base64,{}",
                            image.format,
                            base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                bytes
                            )
                        ),
                    });
                }
                offset += i64::try_from(rows.len()).unwrap_or(PAGE);
                if rows.len() < PAGE as usize {
                    break;
                }
            }
        }
        self.backend.index_images_checked(project, path, &images, guard).await
    }
}
