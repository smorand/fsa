//! The recalculation tools of SPEC-0108: `admin.recalculate_start`,
//! `admin.recalculate_status` and `admin.recalculate_cancel`.
//!
//! Typed functions only: the `#[tool]` methods in `mcp::server` call them. They
//! add the access check and the shape checks; the job itself is the scheduler's
//! (`search::derived::scheduler`, kind `vectors`). MCP only, no REST twin.

use crate::errors::{Result, ToolError};
use crate::search::derived::JobCause;
use crate::search::derived::scheduler::WHOLE_PROJECT;
use crate::state::AppState;
#[cfg(test)]
use crate::tools::registry_support::{ToolRegistry, ToolSchema, handler};
use serde_json::{Value, json};
use std::sync::Arc;

/// Must equal the `#[tool]` descriptions in `mcp::server`, literals there.
#[cfg(test)]
pub(crate) const START: &str = "Start the recalculation of the vectors of a project, or of one file when a path is given, with the models of its profile; one runs per project at a time; answers the job (owner or platform admin).";
#[cfg(test)]
pub(crate) const STATUS: &str = "Read the running recalculation of a project, else the last finished one, with its progress in files, and whether the vectors are out of date (member or platform admin).";
#[cfg(test)]
pub(crate) const CANCEL: &str = "Stop the running recalculation of a project after its current file; answers whether one was running (owner or platform admin).";
#[cfg(test)]
pub(crate) const PATH_PARAM: &str =
    "Absolute POSIX path of one file to recalculate; empty recalculates every file of the project.";

#[cfg(test)]
pub(crate) fn register(reg: &mut ToolRegistry) {
    reg.add(
        ToolSchema::new("admin.recalculate_start", START)
            .req_str("project_id", "Id of the project.")
            .opt_str("path", "", PATH_PARAM)
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            recalculate_start(&ctx.state, &ctx.person, &a.str("project_id")?, &a.str_or("path", ""))
                .await
        }),
    );
    reg.add(
        ToolSchema::new("admin.recalculate_status", STATUS)
            .req_str("project_id", "Id of the project.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            recalculate_status(&ctx.state, &ctx.person, &a.str("project_id")?).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.recalculate_cancel", CANCEL)
            .req_str("project_id", "Id of the project.")
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            recalculate_cancel(&ctx.state, &ctx.person, &a.str("project_id")?).await
        }),
    );
}

fn scheduler(state: &AppState) -> Result<Arc<crate::search::derived::Scheduler>> {
    let search = state.search.as_ref().ok_or_else(|| {
        ToolError::not_supported("search is not enabled; set search.enabled: true in config")
    })?;
    search.scheduler().ok_or_else(|| ToolError::not_supported("the rebuild jobs are not available"))
}

async fn out_of_date(state: &AppState, project_id: &str) -> Result<bool> {
    Ok(crate::tools::profile::registry(state)?
        .project_profile(project_id)
        .await?
        .vectors_stale_at
        .is_some())
}

/// Order of the refusals: access, search off, a mode without vectors, then the
/// path (`ERR_NOT_FOUND` for what is not a file of the project), then a job of
/// another scope already running. Cost: one file check, one row write.
pub(crate) async fn recalculate_start(
    state: &AppState,
    person: &str,
    project_id: &str,
    raw_path: &str,
) -> Result<Value> {
    state.require_owner_or_admin(project_id, person).await?;
    let scheduler = scheduler(state)?;
    if !state.admin.get_index_mode(project_id).await?.needs_embedding() {
        return Err(ToolError::not_supported("the index mode of the project has no vectors"));
    }
    let scope = if raw_path.is_empty() {
        WHOLE_PROJECT.to_string()
    } else {
        let path = state.safety.normalize_path(raw_path)?;
        if path != WHOLE_PROJECT {
            let client = state.stores.client(project_id).await?;
            if !client.is_file(&path).await? {
                return Err(ToolError::not_found(format!("not a file: {path}")));
            }
        }
        path
    };
    let (job, already_running) =
        scheduler.start_vectors(project_id, &scope, JobCause::Manual).await?;
    Ok(json!({
        "project_id": project_id,
        "job": job.to_json(None),
        "already_running": already_running,
    }))
}

pub(crate) async fn recalculate_status(
    state: &AppState,
    person: &str,
    project_id: &str,
) -> Result<Value> {
    crate::tools::profile::require_reader(state, person, project_id).await?;
    let job = match state.admin.derived_jobs() {
        Some(store) => store.latest_vectors(project_id).await?,
        None => None,
    };
    Ok(json!({
        "project_id": project_id,
        "job": job.map(|j| j.to_json(None)),
        "vectors_out_of_date": out_of_date(state, project_id).await?,
    }))
}

pub(crate) async fn recalculate_cancel(
    state: &AppState,
    person: &str,
    project_id: &str,
) -> Result<Value> {
    state.require_owner_or_admin(project_id, person).await?;
    let cancelled = match state.search.as_ref().and_then(|s| s.scheduler()) {
        Some(s) => s.cancel_vectors(project_id),
        None => false,
    };
    Ok(json!({"project_id": project_id, "cancelled": cancelled}))
}

#[cfg(test)]
mod tests {
    /// The test registry and the live router describe each tool and parameter
    /// alike, so the golden contract regenerated from the one is what the other
    /// serves.
    #[test]
    fn the_live_descriptions_equal_the_registry_ones() {
        let router = crate::mcp::server::McpServer::tool_router();
        let tools = router.list_all();
        for (name, desc) in [
            ("admin.recalculate_start", super::START),
            ("admin.recalculate_status", super::STATUS),
            ("admin.recalculate_cancel", super::CANCEL),
        ] {
            let tool = tools.iter().find(|t| t.name == name).expect(name);
            assert_eq!(tool.description.as_deref(), Some(desc), "{name}");
        }
        let start = tools.iter().find(|t| t.name == "admin.recalculate_start").unwrap();
        assert_eq!(
            start.input_schema["properties"]["path"]["description"].as_str(),
            Some(super::PATH_PARAM)
        );
    }
}
