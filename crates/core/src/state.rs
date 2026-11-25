//! Shared server state, handed to every tool handler and REST endpoint.
//!
//! Cheap to clone (everything behind `Arc`). Assembled once in `app::build`.

use crate::config::ServerConfig;
use crate::docs::DocService;
use crate::errors::{Result, ToolError};
use crate::identity::IdentityResolver;
use crate::safety::SafetyManager;
use crate::search::SearchBackend;
use crate::storage::StoreManager;
use crate::storage::traits::AdminBackend;
use crate::tools::editor::EditorRegistry;
use std::sync::Arc;

pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub admin: Arc<dyn AdminBackend>,
    pub stores: Arc<StoreManager>,
    pub safety: Arc<SafetyManager>,
    pub identity: Arc<IdentityResolver>,
    pub editors: Arc<EditorRegistry>,
    /// The external document to Markdown converter, `None` when `doc_service` is
    /// disabled. Built once at boot so its HTTP connection pool is reused.
    pub doc_service: Option<Arc<dyn DocService>>,
    /// The search backend, `None` when `search.enabled` is false.
    pub search: Option<Arc<dyn SearchBackend>>,
}

impl AppState {
    /// Caseless platform-admin check.
    pub fn is_admin(&self, person: &str) -> bool {
        self.config.is_admin(person)
    }

    /// `ERR_FORBIDDEN` unless `person` is a platform admin.
    pub fn require_admin(&self, person: &str) -> Result<()> {
        if self.is_admin(person) {
            Ok(())
        } else {
            Err(ToolError::forbidden(format!("'{person}' is not a platform admin")))
        }
    }

    /// Authorize as project owner OR platform admin (the C# `RequireOwnerOrAdminAsync`).
    /// A platform admin still needs the project to exist.
    pub async fn require_owner_or_admin(&self, project_id: &str, person: &str) -> Result<()> {
        if self.is_admin(person) {
            if self.admin.get_project(project_id).await?.is_none() {
                return Err(ToolError::project_not_found(project_id));
            }
            return Ok(());
        }
        self.admin.require_owner(project_id, person).await.map(|_| ())
    }

    /// The converter a conversion in `mount_id` uses, resolved at call time
    /// (SPEC-0106 DEC-006): the project converter, else the platform default,
    /// else the configuration file converter. Built from the row at every call,
    /// never cached, so a change applies to the next conversion (FR-NEW-011);
    /// the caller holds the answer for the whole request.
    pub async fn doc_service_for(&self, mount_id: &str) -> Result<Option<Arc<dyn DocService>>> {
        match crate::settings::converters::resolved_converter(self.admin.as_ref(), mount_id).await?
        {
            Some((converter, secret)) => crate::docs::resolve::build(&converter, secret).map(Some),
            None => Ok(self.doc_service.clone()),
        }
    }

    /// Membership gate for every `fs.*` tool. Deliberately does NOT bypass for
    /// platform admins: managing the platform is not the same as reading data.
    pub async fn authorize(&self, mount_id: &str, person: &str) -> Result<()> {
        self.admin.require_member(mount_id, person).await
    }
}
