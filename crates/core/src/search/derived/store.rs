//! `JobStore`: the `derived_job` rows of the admin store (DEC-004).
//!
//! Speaks only [`RelationalDb`], so SQLite and PostgreSQL share every query.

use super::{Job, JobCause, JobKind, JobState};
use crate::errors::Result;
use crate::storage::meta::SubtreeMatch;
use crate::storage::rel::{Query, RelationalDb, RowValues};
use std::str::FromStr;
use std::sync::Arc;

const COLS: &str =
    "job_id, project_id, path, kind, state, cause, hidden, created_at, ended_at, reason, set_id";

/// The states a finished row can hold, inlined as literals (never user text).
const FINISHED: &str = "('done', 'failed', 'superseded')";

#[derive(Clone)]
pub struct JobStore {
    db: Arc<dyn RelationalDb>,
}

impl JobStore {
    /// No IO: the table is created with the admin schema at `connect()`.
    pub fn new(db: Arc<dyn RelationalDb>) -> Self {
        Self { db }
    }

    fn read(r: &RowValues) -> Result<Job> {
        Ok(Job {
            job_id: r.text(0)?,
            project_id: r.text(1)?,
            path: r.text(2)?,
            kind: JobKind::from_str(&r.text(3)?)?,
            state: JobState::from_str(&r.text(4)?)?,
            cause: JobCause::from_str(&r.text(5)?)?,
            hidden: r.i64(6)? != 0,
            created_at: r.text(7)?,
            ended_at: r.opt_text(8)?,
            reason: r.opt_text(9)?,
            set_id: r.opt_text(10)?,
        })
    }

    pub async fn insert(&self, job: &Job) -> Result<()> {
        self.db
            .execute(
                &Query::new(format!(
                    "INSERT INTO derived_job ({COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
                ))
                .bind(&job.job_id)
                .bind(&job.project_id)
                .bind(&job.path)
                .bind(job.kind.as_str())
                .bind(job.state.as_str())
                .bind(job.cause.as_str())
                .bind(i64::from(job.hidden))
                .bind(&job.created_at)
                .bind(job.ended_at.clone())
                .bind(job.reason.clone())
                .bind(job.set_id.clone()),
            )
            .await?;
        Ok(())
    }

    pub async fn set_state(&self, job_id: &str, state: JobState) -> Result<()> {
        self.db
            .execute(
                &Query::new("UPDATE derived_job SET state=?1 WHERE job_id=?2")
                    .bind(state.as_str())
                    .bind(job_id),
            )
            .await?;
        Ok(())
    }

    /// Ends a job and, in the same transaction, removes the earlier finished
    /// row of its key, so the table keeps one finished job per file and kind
    /// (DEC-006) and never grows with history.
    pub async fn finish(
        &self,
        job_id: &str,
        state: JobState,
        ended_at: &str,
        reason: Option<&str>,
    ) -> Result<()> {
        let mut tx = self.db.begin().await?;
        tx.execute(
            &Query::new(format!(
                "DELETE FROM derived_job WHERE job_id<>?1 AND state IN {FINISHED} AND \
                 (project_id, path, kind) IN \
                 (SELECT project_id, path, kind FROM derived_job WHERE job_id=?1)"
            ))
            .bind(job_id),
        )
        .await?;
        tx.execute(
            &Query::new("UPDATE derived_job SET state=?1, ended_at=?2, reason=?3 WHERE job_id=?4")
                .bind(state.as_str())
                .bind(ended_at)
                .bind(reason)
                .bind(job_id),
        )
        .await?;
        tx.commit().await
    }

    pub async fn delete(&self, job_id: &str) -> Result<()> {
        self.db
            .execute(&Query::new("DELETE FROM derived_job WHERE job_id=?1").bind(job_id))
            .await?;
        Ok(())
    }

    /// Removes every row of one file of a project, finished ones included
    /// (FR-NEW-005).
    pub async fn delete_path(&self, project_id: &str, path: &str) -> Result<()> {
        self.db
            .execute(
                &Query::new("DELETE FROM derived_job WHERE project_id=?1 AND path=?2")
                    .bind(project_id)
                    .bind(path),
            )
            .await?;
        Ok(())
    }

    /// Makes every row of `from` name `to` (FR-NEW-006). The caller has removed
    /// the rows of `to` first, so the table still keeps one finished row per key.
    pub async fn move_path(&self, project_id: &str, from: &str, to: &str) -> Result<()> {
        self.db
            .execute(
                &Query::new("UPDATE derived_job SET path=?1 WHERE project_id=?2 AND path=?3")
                    .bind(to)
                    .bind(project_id)
                    .bind(from),
            )
            .await?;
        Ok(())
    }

    /// Removes every row of a project. A hard delete already cascades; this is
    /// for a soft delete, which keeps the `project` row (DEC-016).
    pub async fn delete_project(&self, project_id: &str) -> Result<()> {
        self.db
            .execute(&Query::new("DELETE FROM derived_job WHERE project_id=?1").bind(project_id))
            .await?;
        Ok(())
    }

    /// Every waiting and running row, of one project or of all of them (boot
    /// recovery). Few by construction: at most one of each per file and kind.
    pub async fn load_unfinished(&self, project_id: Option<&str>) -> Result<Vec<Job>> {
        let q = match project_id {
            Some(p) => Query::new(format!(
                "SELECT {COLS} FROM derived_job \
                 WHERE project_id=?1 AND state IN ('waiting', 'running')"
            ))
            .bind(p),
            None => Query::new(format!(
                "SELECT {COLS} FROM derived_job WHERE state IN ('waiting', 'running')"
            )),
        };
        self.db.query(&q).await?.iter().map(Self::read).collect()
    }

    /// One page of finished rows ordered `ended_at DESC, path ASC, kind ASC`,
    /// strictly after `after` when given (DEC-022). `scope` is a path: the row
    /// of that path plus every row below it; `None` is the whole project.
    pub async fn list_finished_page(
        &self,
        project_id: &str,
        scope: Option<&str>,
        after: Option<(&str, &str, &str)>,
        limit: usize,
    ) -> Result<Vec<Job>> {
        let mut sql =
            format!("SELECT {COLS} FROM derived_job WHERE project_id=?1 AND state IN {FINISHED}");
        let mut q = Query::new("").bind(project_id);
        let mut n = 1;
        if let Some(path) = scope {
            let sm = SubtreeMatch::new(self.db.dialect(), path);
            sql.push_str(&format!(" AND (path=?{} OR {})", n + 1, sm.clause("path", n + 2, n + 3)));
            q = q.bind(path).bind(&sm.like).bind(&sm.prefix);
            n += 3;
        }
        if let Some((ended, path, kind)) = after {
            let (e, p, k) = (n + 1, n + 2, n + 3);
            sql.push_str(&format!(
                " AND (ended_at<?{e} OR (ended_at=?{e} AND (path>?{p} OR (path=?{p} AND kind>?{k}))))"
            ));
            q = q.bind(ended).bind(path).bind(kind);
        }
        sql.push_str(&format!(" ORDER BY ended_at DESC, path ASC, kind ASC LIMIT {limit}"));
        q.sql = sql;
        self.db.query(&q).await?.iter().map(Self::read).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::traits::AdminBackend;

    async fn store() -> JobStore {
        let db = Arc::new(crate::storage::rel::SqliteRelationalDb::open_in_memory().unwrap());
        let admin = crate::storage::admin::RelationalAdminStore::new(db.clone());
        admin.connect().await.unwrap();
        admin.create_project("proj", "a@x.y").await.unwrap();
        JobStore::new(db)
    }

    fn finished(path: &str, ended: &str) -> Job {
        let mut j = Job::new_waiting("proj", path, JobKind::SearchIndex, JobCause::Change);
        j.state = JobState::Done;
        j.ended_at = Some(ended.into());
        j
    }

    #[tokio::test]
    async fn finishing_a_job_keeps_one_finished_row_per_key() {
        let s = store().await;
        s.insert(&finished("/a.md", "2026-11-20T10:00:00Z")).await.unwrap();
        let next = Job::new_waiting("proj", "/a.md", JobKind::SearchIndex, JobCause::Change);
        s.insert(&next).await.unwrap();
        s.set_state(&next.job_id, JobState::Running).await.unwrap();
        s.finish(&next.job_id, JobState::Failed, "2026-11-20T10:01:00Z", Some("boom"))
            .await
            .unwrap();
        let page = s.list_finished_page("proj", None, None, 10).await.unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].job_id, next.job_id);
        assert_eq!(page[0].reason.as_deref(), Some("boom"));
    }

    #[tokio::test]
    async fn a_chunks_job_keeps_its_set_id_and_a_finished_row_per_kind() {
        let s = store().await;
        let mut job = Job::new_waiting("proj", "/a.md", JobKind::Chunks, JobCause::Manual);
        job.set_id = Some("set-1a2b3c4d".into());
        s.insert(&job).await.unwrap();
        s.insert(&finished("/a.md", "2026-11-20T10:00:00Z")).await.unwrap();
        s.finish(&job.job_id, JobState::Done, "2026-11-20T10:01:00Z", None).await.unwrap();
        let page = s.list_finished_page("proj", None, None, 10).await.unwrap();
        assert_eq!(page.len(), 2, "one finished row per file and kind");
        let chunks = page.iter().find(|j| j.kind == JobKind::Chunks).unwrap();
        assert_eq!(chunks.set_id.as_deref(), Some("set-1a2b3c4d"));
        assert_eq!(page.iter().find(|j| j.kind == JobKind::SearchIndex).unwrap().set_id, None);
    }

    #[tokio::test]
    async fn the_scope_selects_the_path_and_what_is_below_it_only() {
        let s = store().await;
        for p in ["/d/a.md", "/d/sub/b.md", "/d_x.md", "/dx/c.md", "/d"] {
            s.insert(&finished(p, "2026-11-20T10:00:00Z")).await.unwrap();
        }
        let page = s.list_finished_page("proj", Some("/d"), None, 10).await.unwrap();
        let paths: Vec<&str> = page.iter().map(|j| j.path.as_str()).collect();
        assert_eq!(paths, ["/d", "/d/a.md", "/d/sub/b.md"]);
    }

    #[tokio::test]
    async fn a_hard_project_delete_cascades_to_its_jobs() {
        let db = Arc::new(crate::storage::rel::SqliteRelationalDb::open_in_memory().unwrap());
        let admin = crate::storage::admin::RelationalAdminStore::new(db.clone());
        admin.connect().await.unwrap();
        admin.create_project("proj", "a@x.y").await.unwrap();
        let s = JobStore::new(db);
        s.insert(&finished("/a.md", "2026-11-20T10:00:00Z")).await.unwrap();
        admin.delete_project("proj").await.unwrap();
        assert!(s.list_finished_page("proj", None, None, 10).await.unwrap().is_empty());
    }
}
