//! The conversion host of the rebuild scheduler (SPEC-0113).
//!
//! The scheduler holds no application state, so a `conversion` job runs through
//! this trait, implemented once over a weak reference to the state (the state
//! holds the search backend which holds the scheduler: a strong one would be a
//! cycle).

use super::JobCause;
use crate::errors::{Result, ToolError};
use crate::state::AppState;
use async_trait::async_trait;
use std::sync::{Arc, Weak};

/// Runs the conversion of one source for the scheduler. `Ok(None)` means the
/// result was discarded (the source changed or went away), `Ok(Some(n))` a
/// finished job (`n` is 1 when the companion was replaced, 0 when it was
/// already current).
#[async_trait]
pub trait ConversionHost: Send + Sync {
    async fn rebuild(
        &self,
        project: &str,
        path: &str,
        cause: JobCause,
        guard: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>>;
}

struct StateHost(Weak<AppState>);

#[async_trait]
impl ConversionHost for StateHost {
    async fn rebuild(
        &self,
        project: &str,
        path: &str,
        cause: JobCause,
        guard: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Option<usize>> {
        use crate::core::fs_ops::{Rebuilt, rebuild_companion};
        let state =
            self.0.upgrade().ok_or_else(|| ToolError::internal("the server is shutting down"))?;
        let client = state.stores.client(project).await?;
        // Resolved at run time, as every conversion does (SPEC-0106 DEC-006).
        let service = state.doc_service_for(project).await?;
        let done = rebuild_companion(
            &client,
            &state.safety,
            service.as_deref(),
            project,
            path,
            cause,
            guard,
        )
        .await?;
        match done {
            Rebuilt::Done { md_path } => {
                // The companion is text worth indexing, as after `fs.documentize`.
                crate::search::indexer::index_companion(&state, project, &md_path, &client).await;
                Ok(Some(1))
            }
            Rebuilt::Unchanged => Ok(Some(0)),
            Rebuilt::Superseded => Ok(None),
        }
    }
}

/// Gives the scheduler of `state` the means to convert a document. Called by
/// `app.rs` before the recovery of the waiting jobs, so a due job always finds
/// its host. A state without a scheduler (search off) has nothing to attach to.
pub fn attach(state: &Arc<AppState>) {
    if let Some(scheduler) = state.search.as_ref().and_then(|s| s.scheduler()) {
        scheduler.set_conversion_host(Arc::new(StateHost(Arc::downgrade(state))));
    }
}
