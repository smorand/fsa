//! `fs.list_derived_jobs` (SPEC-0100 FR-NEW-010): the rebuild jobs of derived
//! data of a project or of one file; `fs.recalculate` (FR-NEW-008): a rebuild
//! by hand.
//!
//! Typed functions only: the `#[tool]` methods in `mcp::server` call
//! [`list_derived_jobs`] after `authorize`, and [`recalculate`], which owns its
//! refusal order. MCP only, no REST twin (DEC-012).

use crate::errors::{Result, ToolError};
use crate::search::derived::{Job, JobKind, JobState, Marker};
use crate::state::AppState;
#[cfg(test)]
use crate::tools::authorize_only;
#[cfg(test)]
use crate::tools::registry_support::{ToolRegistry, ToolSchema, handler};
use serde_json::{Value, json};

/// Must equal the `#[tool]` description in `mcp::server`, a literal there.
#[cfg(test)]
const LIST_DESCRIPTION: &str = "List the rebuild jobs of derived data (search entries) of a project or of one file: waiting, running and the last finished one per file.";

/// Must equal the `#[tool]` description in `mcp::server`, a literal there.
#[cfg(test)]
const RECALCULATE_DESCRIPTION: &str =
    "Rebuild the search entries of one file now, without waiting for the settle delay.";

/// Jobs per answer (FR-NEW-010).
const PAGE: usize = 100;

/// Test dispatch glue mirroring `McpServer::fs_list_derived_jobs`, so the
/// golden contract regeneration sees the schema the live surface serves.
#[cfg(test)]
pub(crate) fn register(reg: &mut ToolRegistry) {
    reg.add(
        ToolSchema::new("fs.list_derived_jobs", LIST_DESCRIPTION)
            .req_str("mount_id", "Project/volume id the operation targets.")
            .opt_str(
                "path",
                "",
                "A file, or a folder for the files under it; empty lists the whole project.",
            )
            .opt_str("marker", "", "The marker of the previous answer, to read the next page.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            let mount = authorize_only(&ctx, &a).await?;
            list_derived_jobs(&ctx.state, &mount, &a.str_or("path", ""), &a.str_or("marker", ""))
                .await
        }),
    );
    reg.add(
        ToolSchema::new("fs.recalculate", RECALCULATE_DESCRIPTION)
            .req_str("mount_id", "Project/volume id the operation targets.")
            .req_str("path", "Absolute POSIX path of the file to rebuild.")
            .req_str("kind", "What to rebuild; only 'search_index' today.")
            .read_only(false)
            .destructive(false)
            .idempotent(false)
            .open_world(false),
        handler(|ctx, a| async move {
            recalculate(
                &ctx.state,
                &ctx.person,
                &a.str("mount_id")?,
                &a.str("path")?,
                &a.str("kind")?,
            )
            .await
        }),
    );
}

/// A listed job with its sort key inside its group.
struct Listed {
    job: Job,
    seconds_left: u64,
}

impl Listed {
    /// The marker naming this job, so the next page starts strictly after it.
    fn marker(&self) -> Marker {
        let path = self.job.path.clone();
        let kind = self.job.kind.as_str().to_string();
        match self.job.state {
            JobState::Waiting => Marker::Waiting { seconds_left: self.seconds_left, path, kind },
            JobState::Running => {
                Marker::Running { created_at: self.job.created_at.clone(), path, kind }
            }
            _ => Marker::Finished {
                ended_at: self.job.ended_at.clone().unwrap_or_default(),
                path,
                kind,
            },
        }
    }
}

/// The caller has already passed `authorize`. Order: waiting (fewest seconds
/// left first), running (oldest first), finished (newest end first), ties by
/// path then kind; at most [`PAGE`] jobs, plus a marker when more follow.
///
/// Waiting and running jobs come from the scheduler's memory, which alone knows
/// a due time, so `seconds_left` is exact; hidden entries (delay 0) are not
/// listed (DEC-007). Finished jobs come from the table. A backend without a
/// scheduler (tests that build an `AppState` by hand) reads every job from the
/// table, where a waiting row reports `seconds_left` 0 (DEC-005). Cost: one pass
/// over the scheduler's live keys (at most one waiting and one running job per
/// file), one keyset query of at most 101 finished rows on `idx_derived_job_page`.
pub(crate) async fn list_derived_jobs(
    state: &AppState,
    mount_id: &str,
    path: &str,
    marker: &str,
) -> Result<Value> {
    // Nothing is ever scheduled when search is off, so the honest answer is an
    // empty list, not a refusal (E2E-070).
    let (Some(search), Some(store)) = (state.search.as_ref(), state.admin.derived_jobs()) else {
        return Ok(json!({"jobs": []}));
    };
    let after = if marker.is_empty() { None } else { Some(Marker::decode(marker)?) };
    let scope = if path.is_empty() { None } else { Some(state.safety.normalize_path(path)?) };
    let scope = scope.filter(|p| p != "/");
    let in_scope = |p: &str| match &scope {
        None => true,
        Some(s) => p == s || p.strip_prefix(s.as_str()).is_some_and(|rest| rest.starts_with('/')),
    };
    let group = after.as_ref().map_or(0, Marker::group);
    let want = PAGE + 1;
    let mut out: Vec<Listed> = Vec::with_capacity(want);

    if group <= 1 {
        let live: Vec<Listed> = match search.scheduler() {
            Some(scheduler) => scheduler
                .list(mount_id, &in_scope)
                .await
                .into_iter()
                .map(|l| Listed { job: l.job, seconds_left: l.seconds_left })
                .collect(),
            None => store
                .load_unfinished(Some(mount_id))
                .await?
                .into_iter()
                .filter(|j| !j.hidden && in_scope(&j.path))
                .map(|job| Listed { job, seconds_left: 0 })
                .collect(),
        };
        let mut waiting: Vec<Listed> = Vec::new();
        let mut running: Vec<Listed> = Vec::new();
        for l in live {
            match l.job.state {
                JobState::Waiting => waiting.push(l),
                _ => running.push(l),
            }
        }
        waiting.sort_by(|a, b| {
            (a.seconds_left, &a.job.path, a.job.kind.as_str()).cmp(&(
                b.seconds_left,
                &b.job.path,
                b.job.kind.as_str(),
            ))
        });
        running.sort_by(|a, b| {
            (&a.job.created_at, &a.job.path, a.job.kind.as_str()).cmp(&(
                &b.job.created_at,
                &b.job.path,
                b.job.kind.as_str(),
            ))
        });
        if group == 0 {
            out.extend(waiting.into_iter().filter(|l| match &after {
                Some(Marker::Waiting { seconds_left, path, kind }) => {
                    (l.seconds_left, l.job.path.as_str(), l.job.kind.as_str())
                        > (*seconds_left, path.as_str(), kind.as_str())
                }
                _ => true,
            }));
        }
        out.extend(running.into_iter().filter(|l| match &after {
            Some(Marker::Running { created_at, path, kind }) => {
                (l.job.created_at.as_str(), l.job.path.as_str(), l.job.kind.as_str())
                    > (created_at.as_str(), path.as_str(), kind.as_str())
            }
            _ => true,
        }));
    }

    if out.len() < want {
        let keyset = match &after {
            Some(Marker::Finished { ended_at, path, kind }) => {
                Some((ended_at.as_str(), path.as_str(), kind.as_str()))
            }
            _ => None,
        };
        let finished =
            store.list_finished_page(mount_id, scope.as_deref(), keyset, want - out.len()).await?;
        out.extend(finished.into_iter().map(|job| Listed { job, seconds_left: 0 }));
    }

    let next = (out.len() > PAGE).then(|| {
        out.truncate(PAGE);
        out.last().expect("a full page").marker().encode()
    });
    let jobs: Vec<Value> = out.iter().map(|l| l.job.to_json(Some(l.seconds_left))).collect();
    let mut v = json!({"jobs": jobs});
    if let Some(m) = next {
        v["marker"] = json!(m);
    }
    Ok(v)
}

/// `fs.recalculate` (FR-NEW-008, FR-NEW-009): rebuild the search entries of one
/// file now. Every refusal comes before any change, in the contract order:
/// project absent or deleted, not a member, search off, kind, path absent,
/// path is a folder, indexing off (design 5.2), so a refused request leaves the
/// waiting rebuild as it was. Cost: three lookups (membership, node, mode),
/// then the scheduler's few row writes.
pub(crate) async fn recalculate(
    state: &AppState,
    person: &str,
    mount_id: &str,
    path: &str,
    kind: &str,
) -> Result<Value> {
    state.authorize(mount_id, person).await?;
    let not_enabled =
        || ToolError::not_supported("search is not enabled; set search.enabled: true in config");
    let search = state.search.as_ref().ok_or_else(not_enabled)?;
    let _kind: JobKind = kind.parse()?;
    let path = state.safety.normalize_path(path)?;
    let client = state.stores.client(mount_id).await?;
    // The root has no node of its own, yet it is a folder.
    if path != "/" && !client.exists(&path).await? {
        return Err(ToolError::not_found(format!("file not found: {path}")));
    }
    if path == "/" || client.is_dir(&path).await? {
        return Err(ToolError::invalid_argument("path must be a file"));
    }
    if !state.admin.get_index_mode(mount_id).await?.is_active() {
        return Err(ToolError::not_supported(format!(
            "search indexing is not active for project {mount_id}"
        )));
    }
    // `app.rs` always wraps the engine in the scheduler when search is on.
    let scheduler = search
        .scheduler()
        .ok_or_else(|| ToolError::internal("the rebuild scheduler is not running"))?;
    Ok(scheduler.recalculate(mount_id, &path).await)
}
