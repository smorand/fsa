//! The server settings tools of SPEC-0114: `admin.list_settings`,
//! `admin.get_setting` and `admin.set_setting`, platform administrator only,
//! reading included.
//!
//! Typed functions only: the `#[tool]` methods in `mcp::server`, the REST
//! routes and the `/app/settings` screen call them. They add the access check
//! (first, so it comes before any shape check); every rule lives in
//! `settings::server_settings`. A secret travels as a JSON text and is never
//! logged here.

use crate::errors::Result;
use crate::settings::ServerSettings;
use crate::state::AppState;
#[cfg(test)]
use crate::tools::registry_support::{ToolRegistry, ToolSchema, handler};
use serde_json::Value;

/// Must equal the `#[tool]` descriptions in `mcp::server`, literals there.
#[cfg(test)]
pub(crate) const LIST_SETTINGS: &str = "List the server setting sections (safety, api, extract, doc, sqlite, db, doc_service, search, git, web, context7, server, auth, infra) with where each value comes from (database, yaml or default), whether a restart is needed to apply it, and who changed it last (platform admin only).";
#[cfg(test)]
pub(crate) const GET_SETTING: &str = "Read one server setting section: its value without secrets, for each secret whether it is set and its last 4 characters, where it comes from, and whether a restart is needed to apply it (platform admin only).";
#[cfg(test)]
pub(crate) const SET_SETTING: &str = "Change keys of one server setting section: the keys given are merged over the section, an unknown key or a value that today's configuration check refuses is refused, a secret key set to an empty text is cleared; the change applies at the next start (platform admin only).";

#[cfg(test)]
pub(crate) const SECTION_PARAM: &str = "Section name: safety, api, extract, doc, sqlite, db, doc_service, search, git, web, context7, server, auth or infra.";
#[cfg(test)]
pub(crate) const VALUE_PARAM: &str = "Object of the keys to change, named as in the YAML configuration, for example {\"max_read_lines\": 500}; nested keys are nested objects; a key not given keeps its value.";

#[cfg(test)]
pub(crate) fn register(reg: &mut ToolRegistry) {
    reg.add(
        ToolSchema::new("admin.list_settings", LIST_SETTINGS)
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, _a| async move { list_settings(&ctx.state, &ctx.person).await }),
    );
    reg.add(
        ToolSchema::new("admin.get_setting", GET_SETTING)
            .req_str("section", SECTION_PARAM)
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(
            |ctx, a| async move { get_setting(&ctx.state, &ctx.person, &a.str("section")?).await },
        ),
    );
    reg.add(
        ToolSchema::new("admin.set_setting", SET_SETTING)
            .req_str("section", SECTION_PARAM)
            .opt_any("value", VALUE_PARAM)
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            set_setting(
                &ctx.state,
                &ctx.person,
                &a.str("section")?,
                a.raw("value").cloned().unwrap_or(Value::Null),
            )
            .await
        }),
    );
}

/// The settings of the caller, refused before any read for a non admin.
fn settings(state: &AppState, person: &str) -> Result<ServerSettings> {
    state.require_admin(person)?;
    ServerSettings::of(state.admin.as_ref(), state.config.clone())
}

pub(crate) async fn list_settings(state: &AppState, person: &str) -> Result<Value> {
    settings(state, person)?.list().await
}

pub(crate) async fn get_setting(state: &AppState, person: &str, section: &str) -> Result<Value> {
    settings(state, person)?.get(section).await
}

/// The newest `limit` changes of the settings, newest first (the screen's list).
pub(crate) async fn recent_changes(
    state: &AppState,
    person: &str,
    limit: usize,
) -> Result<Vec<Value>> {
    state.require_admin(person)?;
    let reg = crate::settings::Registry::of(
        state.admin.as_ref(),
        state.config.search.embedding.dimensions,
    )?;
    let changes = reg.list_changes_of_kinds(&["setting"], limit).await?;
    Ok(changes.iter().map(|c| serde_json::to_value(c).expect("a change serializes")).collect())
}

pub(crate) async fn set_setting(
    state: &AppState,
    person: &str,
    section: &str,
    value: Value,
) -> Result<Value> {
    settings(state, person)?.set(person, section, value).await
}

#[cfg(test)]
mod tests {
    /// The test registry and the live router describe each tool alike, so the
    /// golden contract regenerated from the one is what the other serves.
    #[test]
    fn the_live_descriptions_equal_the_registry_ones() {
        let router = crate::mcp::server::McpServer::tool_router();
        let tools = router.list_all();
        for (name, desc) in [
            ("admin.list_settings", super::LIST_SETTINGS),
            ("admin.get_setting", super::GET_SETTING),
            ("admin.set_setting", super::SET_SETTING),
        ] {
            let tool = tools.iter().find(|t| t.name == name).expect(name);
            assert_eq!(tool.description.as_deref(), Some(desc), "{name}");
        }
        for name in ["admin.get_setting", "admin.set_setting"] {
            let tool = tools.iter().find(|t| t.name == name).unwrap();
            let live = &tool.input_schema["properties"]["section"]["description"];
            assert_eq!(live.as_str(), Some(super::SECTION_PARAM), "{name}");
        }
        let set = tools.iter().find(|t| t.name == "admin.set_setting").unwrap();
        let live = &set.input_schema["properties"]["value"]["description"];
        assert_eq!(live.as_str(), Some(super::VALUE_PARAM));
    }
}
