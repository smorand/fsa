//! The indexing profile tools of SPEC-0107: `admin.get_indexing_profile` and
//! `admin.set_indexing_profile`.
//!
//! Typed functions only: the `#[tool]` methods in `mcp::server` call them. They
//! add the access check (made first, so it comes before any shape check) and the
//! reading of the loose JSON arguments; every rule lives in `settings::profile`.

use crate::errors::{Result, ToolError};
use crate::settings::{ConfiguredModel, ProfileChange, Registry};
use crate::state::AppState;
#[cfg(test)]
use crate::tools::registry_support::{ToolRegistry, ToolSchema, handler};
use serde_json::Value;

/// Must equal the `#[tool]` descriptions in `mcp::server`, literals there.
#[cfg(test)]
pub(crate) const GET_INDEXING_PROFILE: &str = "Read the indexing profile of a project: the default chunk strategy with its settings, the text embedding model and the image embedding model, each with where it comes from, and whether the vectors are out of date (member or platform admin).";
#[cfg(test)]
pub(crate) const SET_INDEXING_PROFILE: &str = "Change the indexing profile of a project: the fields given only; an empty text clears a field; changing a model marks the vectors of the project out of date; answers the whole profile (owner or platform admin).";

/// The parameter descriptions, shared with the `#[tool]` argument structs.
#[cfg(test)]
pub(crate) const SET_PARAMS: [(&str, &str); 4] = [
    (
        "chunk_strategy",
        "Default chunk strategy of the project: 'fixed' or 'structural'; an empty text clears it and its settings.",
    ),
    (
        "chunk_settings",
        "Settings of the strategy as in fs.create_chunk_set, an object; omitted settings take the server defaults; an empty object clears them. Needs a chunk strategy.",
    ),
    (
        "text_model_id",
        "Id of a text_embedding model of the registry; an empty text clears it, the project then uses the server default.",
    ),
    ("image_model_id", "Id of an image_embedding model of the registry; an empty text clears it."),
];

#[cfg(test)]
pub(crate) fn register(reg: &mut ToolRegistry) {
    reg.add(
        ToolSchema::new("admin.get_indexing_profile", GET_INDEXING_PROFILE)
            .req_str("project_id", "Id of the project.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            get_indexing_profile(&ctx.state, &ctx.person, &a.str("project_id")?).await
        }),
    );
    let mut set = ToolSchema::new("admin.set_indexing_profile", SET_INDEXING_PROFILE)
        .req_str("project_id", "Id of the project.");
    for (name, desc) in SET_PARAMS {
        set = set.opt_any(name, desc);
    }
    reg.add(
        set.read_only(false).destructive(false).idempotent(true).open_world(false),
        handler(|ctx, a| async move {
            let v = |k: &str| a.raw(k).cloned();
            set_indexing_profile(
                &ctx.state,
                &ctx.person,
                &a.str("project_id")?,
                v("chunk_strategy"),
                v("chunk_settings"),
                v("text_model_id"),
                v("image_model_id"),
            )
            .await
        }),
    );
}

pub(crate) fn registry(state: &AppState) -> Result<Registry> {
    Registry::of(state.admin.as_ref(), state.config.search.embedding.dimensions)
}

/// The model of `search.embedding`, when its endpoint is set (FR-NEW-008).
pub(crate) fn configured_model(state: &AppState) -> Option<ConfiguredModel> {
    let e = &state.config.search.embedding;
    (!e.endpoint.is_empty())
        .then(|| ConfiguredModel { name: e.model.clone(), dimension: i64::from(e.dimensions) })
}

/// Member or platform administrator; the project must exist.
pub(crate) async fn require_reader(state: &AppState, person: &str, project_id: &str) -> Result<()> {
    if state.is_admin(person) {
        if state.admin.get_project(project_id).await?.is_none() {
            return Err(ToolError::project_not_found(project_id));
        }
        Ok(())
    } else {
        state.admin.require_member(project_id, person).await
    }
}

pub(crate) async fn get_indexing_profile(
    state: &AppState,
    person: &str,
    project_id: &str,
) -> Result<Value> {
    require_reader(state, person, project_id).await?;
    registry(state)?
        .indexing_profile_json(
            project_id,
            &state.config.search.chunking(),
            configured_model(state).as_ref(),
        )
        .await
}

/// Every project with its profile, for the models screen (platform admin only).
pub(crate) async fn list_profiles(state: &AppState, person: &str) -> Result<Vec<Value>> {
    state.require_admin(person)?;
    registry(state)?
        .list_profiles_json(&state.config.search.chunking(), configured_model(state).as_ref())
        .await
}

/// A text argument: `null` and absent are one, any other type is refused.
fn text_arg(v: Option<Value>, name: &str) -> Result<Option<String>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(ToolError::invalid_argument(format!("{name} must be text"))),
    }
}

pub(crate) async fn set_indexing_profile(
    state: &AppState,
    person: &str,
    project_id: &str,
    chunk_strategy: Option<Value>,
    chunk_settings: Option<Value>,
    text_model_id: Option<Value>,
    image_model_id: Option<Value>,
) -> Result<Value> {
    state.require_owner_or_admin(project_id, person).await?;
    let chunk_settings = match chunk_settings {
        None | Some(Value::Null) => None,
        Some(Value::Object(m)) => Some(m),
        Some(_) => return Err(ToolError::invalid_argument("chunk_settings must be an object")),
    };
    let change = ProfileChange {
        chunk_strategy: text_arg(chunk_strategy, "chunk_strategy")?,
        chunk_settings,
        text_model_id: text_arg(text_model_id, "text_model_id")?,
        image_model_id: text_arg(image_model_id, "image_model_id")?,
    };
    let reg = registry(state)?;
    let defaults = state.config.search.chunking();
    reg.set_project_profile(person, project_id, change, &defaults).await?;
    reg.indexing_profile_json(project_id, &defaults, configured_model(state).as_ref()).await
}

#[cfg(test)]
mod tests {
    /// The test registry and the live router describe each tool and each
    /// parameter alike, so the golden contract regenerated from the one is what
    /// the other serves.
    #[test]
    fn the_live_descriptions_equal_the_registry_ones() {
        let router = crate::mcp::server::McpServer::tool_router();
        let tools = router.list_all();
        for (name, desc) in [
            ("admin.get_indexing_profile", super::GET_INDEXING_PROFILE),
            ("admin.set_indexing_profile", super::SET_INDEXING_PROFILE),
        ] {
            let tool = tools.iter().find(|t| t.name == name).expect(name);
            assert_eq!(tool.description.as_deref(), Some(desc), "{name}");
        }
        let set = tools.iter().find(|t| t.name == "admin.set_indexing_profile").unwrap();
        for (param, desc) in super::SET_PARAMS {
            let live = &set.input_schema["properties"][param]["description"];
            assert_eq!(live.as_str(), Some(desc), "{param}");
        }
    }
}
