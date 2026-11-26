//! `fs.list_derived_jobs` (SPEC-0100 FR-NEW-010): the rebuild jobs of derived
//! data of a project or of one file; `fs.recalculate` (FR-NEW-008): a rebuild
//! by hand.
//!
//! Typed functions only: the `#[tool]` methods in `mcp::server` call
//! [`list_derived_jobs`] after `authorize`, and [`recalculate`], which owns its
//! refusal order. MCP only, no REST twin (DEC-012).

use crate::errors::{Result, ToolError};
use crate::search::derived::scheduler::{ChunkWork, rebuild_chunk_set};
use crate::search::derived::{Job, JobCause, JobKind, JobState, Marker};
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
const RECALCULATE_DESCRIPTION: &str = "Rebuild derived data of one file now, without waiting for the settle delay: its search entries (kind 'search_index') or one chunk set (kind 'chunks').";

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
            .req_str(
                "kind",
                "What to rebuild: 'search_index' (the search entries) or 'chunks' (one chunk set).",
            )
            .opt_str(
                "set_id",
                "",
                "For kind 'chunks': the set to rebuild; empty means the default set.",
            )
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
                &a.str_or("set_id", ""),
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

/// `fs.recalculate` (FR-NEW-008, FR-NEW-009, SPEC-0101 FR-NEW-019): rebuild one
/// derived datum of one file now. Every refusal comes before any change, in the
/// contract order: project absent or deleted, not a member, kind, path absent,
/// path is a folder; then for `search_index`: `set_id` given, search off,
/// indexing off (design 5.2); for `chunks`: set unknown, no default set, no
/// text. So a refused request leaves the waiting rebuild as it was. `chunks`
/// works with search off and with a project that does not index. Cost: three
/// lookups (membership, node, mode), then the scheduler's few row writes.
pub(crate) async fn recalculate(
    state: &AppState,
    person: &str,
    mount_id: &str,
    path: &str,
    kind: &str,
    set_id: &str,
) -> Result<Value> {
    state.authorize(mount_id, person).await?;
    let kind: JobKind = kind.parse()?;
    let path = state.safety.normalize_path(path)?;
    let client = state.stores.client(mount_id).await?;
    // The root has no node of its own, yet it is a folder.
    if path != "/" && !client.exists(&path).await? {
        return Err(ToolError::not_found(format!("file not found: {path}")));
    }
    if path == "/" || client.is_dir(&path).await? {
        return Err(ToolError::invalid_argument("path must be a file"));
    }
    match kind {
        JobKind::SearchIndex => recalculate_search_index(state, mount_id, &path, set_id).await,
        JobKind::Chunks => recalculate_chunks(state, &client, mount_id, &path, set_id).await,
        // Never parsed from the argument: `admin.recalculate_start` owns this kind.
        JobKind::Vectors => {
            Err(ToolError::invalid_argument("kind must be 'search_index' or 'chunks'"))
        }
    }
}

async fn recalculate_search_index(
    state: &AppState,
    mount_id: &str,
    path: &str,
    set_id: &str,
) -> Result<Value> {
    if !set_id.is_empty() {
        return Err(ToolError::invalid_argument("set_id applies to kind 'chunks' only"));
    }
    let search = state.search.as_ref().ok_or_else(|| {
        ToolError::not_supported("search is not enabled; set search.enabled: true in config")
    })?;
    if !state.admin.get_index_mode(mount_id).await?.is_active() {
        return Err(ToolError::not_supported(format!(
            "search indexing is not active for project {mount_id}"
        )));
    }
    // `app.rs` always wraps the engine in the scheduler when search is on.
    let scheduler = search
        .scheduler()
        .ok_or_else(|| ToolError::internal("the rebuild scheduler is not running"))?;
    Ok(scheduler.recalculate(mount_id, path).await)
}

/// The kind `chunks`: the set is resolved and the text read now, so the refusals
/// precede any job; the scheduler then cuts it at once. With search off (no
/// scheduler) the same work runs on a task of its own, its job unrecorded and so
/// not listed, since `fs.list_derived_jobs` reads the scheduler.
async fn recalculate_chunks(
    state: &AppState,
    client: &crate::storage::volume::VolumeClient,
    mount_id: &str,
    path: &str,
    set_id: &str,
) -> Result<Value> {
    let store = client.meta.chunk_store();
    let set = match (&store, set_id.is_empty()) {
        (Some(store), true) => store
            .default_set(path)
            .await?
            .ok_or_else(|| ToolError::not_found("the file has no default chunk set"))?,
        (Some(store), false) => store
            .get_set(set_id)
            .await?
            .filter(|s| s.path == path)
            .ok_or_else(|| ToolError::not_found(format!("chunk set not found: {set_id}")))?,
        (None, true) => return Err(ToolError::not_found("the file has no default chunk set")),
        (None, false) => {
            return Err(ToolError::not_found(format!("chunk set not found: {set_id}")));
        }
    };
    let text = crate::chunking::service::source_text(state, client, mount_id, path).await?;
    let sha = client.meta.get(path).await?.and_then(|n| n.sha256).unwrap_or_default();
    let indexes = state.search.is_some() && state.admin.get_index_mode(mount_id).await?.is_active();
    let work =
        ChunkWork { set_id: set.set_id.clone(), sha, text, reindex: set.is_default && indexes };
    if let Some(scheduler) = state.search.as_ref().and_then(|s| s.scheduler()) {
        return Ok(scheduler.recalculate_chunks(mount_id, path, work).await);
    }
    let mut job = Job::new_waiting(mount_id, path, JobKind::Chunks, JobCause::Manual);
    job.state = JobState::Running;
    job.set_id = Some(set.set_id);
    let answer = job.to_json(None);
    let (stores, defaults) = (state.stores.clone(), state.config.search.chunking());
    let (backend, project, path) = (state.search.clone(), mount_id.to_string(), path.to_string());
    tokio::spawn(async move {
        let engine = backend.as_deref();
        if let Err(e) =
            rebuild_chunk_set(&stores, engine, &project, &path, &work, defaults, &|| true).await
        {
            tracing::warn!(project, path, error = %e, "rebuild of a chunk set failed");
        }
    });
    Ok(answer)
}
