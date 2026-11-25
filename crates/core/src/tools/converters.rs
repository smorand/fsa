//! The converter tools of SPEC-0106: `admin.create_converter`,
//! `admin.update_converter` and `admin.list_converters`, platform administrator only, reading included
//! (FR-NEW-017).
//!
//! Typed functions only: the `#[tool]` methods in `mcp::server` call them, and
//! they add nothing to [`Registry`] but the admin check, made first so it comes
//! before any body shape or existence check (DEC-015), and the JSON shape.
//! Nothing here logs an argument: a secret may travel in the input (DEC-011).

use crate::errors::Result;
use crate::settings::{ConverterInput, Registry, converter_page_json};
use crate::state::AppState;
#[cfg(test)]
use crate::tools::registry_support::{ToolRegistry, ToolSchema, handler};
use serde_json::Value;

/// Must equal the `#[tool]` descriptions in `mcp::server`, literals there.
#[cfg(test)]
pub(crate) const CREATE_CONVERTER: &str = "Register a document converter: a name and a form, 'program' (a command, a list of arguments holding {document} once and {outdir} at most once, never run through a shell) or 'http' (an address, an optional header and secret stored encrypted and never returned), with optional accepted extensions, answer cap in bytes and timeout in seconds (platform admin only).";
#[cfg(test)]
pub(crate) const UPDATE_CONVERTER: &str = "Change the fields given of a document converter, never its form; a field not given keeps its value; a secret not given or blank keeps the stored one, clear_secret removes it (platform admin only).";
#[cfg(test)]
pub(crate) const LIST_CONVERTERS: &str = "List the document converters ordered by name without regard to case, 100 per answer, each with has_secret instead of its secret (platform admin only).";
#[cfg(test)]
pub(crate) const DELETE_CONVERTER: &str = "Delete a document converter and its secret, unless it is the platform default or the converter of a project (platform admin only).";
#[cfg(test)]
pub(crate) const SET_DEFAULT_CONVERTER: &str = "Make a document converter the platform default, used by every project that has no converter of its own (platform admin only).";
#[cfg(test)]
pub(crate) const CLEAR_DEFAULT_CONVERTER: &str =
    "Remove the platform default converter; answers converter null (platform admin only).";
#[cfg(test)]
pub(crate) const GET_DEFAULT_CONVERTER: &str =
    "Read the platform default converter, null when none is set (platform admin only).";
#[cfg(test)]
pub(crate) const SET_PROJECT_CONVERTER: &str = "Make a document converter the converter of a project, a project in its grace period included; gives no access to the files of the project (platform admin only).";
#[cfg(test)]
pub(crate) const CLEAR_PROJECT_CONVERTER: &str = "Remove the converter of a project, which then uses the platform default; answers converter null (platform admin only).";
#[cfg(test)]
pub(crate) const GET_PROJECT_CONVERTER: &str =
    "Read the converter of a project, null when it has none (platform admin only).";

/// The parameter descriptions, shared with the `#[tool]` argument structs.
#[cfg(test)]
const CREATE_PARAMS: [(&str, &str); 11] = [
    ("name", "Name of the converter, 1 to 64 characters, unique without regard to case."),
    ("form", "Form of the converter: 'program' or 'http'."),
    (
        "command",
        "Program form: the command as a list of text arguments, holding '{document}' exactly once and '{outdir}' at most once. Never run through a shell.",
    ),
    ("url", "Http form: the http or https address the document is sent to."),
    ("auth_header", "Http form: the header carrying the secret. Default Authorization."),
    ("secret", "Http form: the value of the auth_header header, stored encrypted. Never returned."),
    ("file_field", "Http form: the form field holding the document. Default file."),
    (
        "response_field",
        "Http form: the field of a JSON answer holding the Markdown; empty means the whole answer.",
    ),
    (
        "extensions",
        "Accepted extensions without a dot, up to 64 names of 1 to 16 letters or digits; empty means the built in document list.",
    ),
    (
        "max_input_bytes",
        "Largest document and answer in bytes, 1 to 4294967296. Default 536870912.",
    ),
    ("timeout_seconds", "Longest conversion in seconds, 1 to 86400. Default 900."),
];

/// The update parameters: the creation ones but the form, reworded as changes.
#[cfg(test)]
const UPDATE_PARAMS: [(&str, &str); 12] = [
    ("name", "New name, 1 to 64 characters, unique without regard to case."),
    ("form", "The form the converter has; it cannot be changed."),
    (
        "command",
        "Program form: the new command, a list of text arguments, holding '{document}' exactly once and '{outdir}' at most once.",
    ),
    ("url", "Http form: the new http or https address."),
    ("auth_header", "Http form: the new header carrying the secret."),
    (
        "secret",
        "Http form: the new secret, stored encrypted; empty or not given keeps the stored one. Never returned.",
    ),
    ("file_field", "Http form: the new form field holding the document."),
    ("response_field", "Http form: the new field of a JSON answer holding the Markdown."),
    (
        "extensions",
        "New accepted extensions without a dot, up to 64 names of 1 to 16 letters or digits.",
    ),
    ("max_input_bytes", "New largest document and answer in bytes, 1 to 4294967296."),
    ("timeout_seconds", "New longest conversion in seconds, 1 to 86400."),
    ("clear_secret", "True removes the stored secret of an http converter. Default false."),
];

/// Test dispatch glue mirroring the `#[tool]` methods, so the golden contract
/// regeneration sees the schemas the live surface serves.
#[cfg(test)]
pub(crate) fn register(reg: &mut ToolRegistry) {
    let mut create = ToolSchema::new("admin.create_converter", CREATE_CONVERTER);
    for (name, desc) in CREATE_PARAMS {
        create = create.opt_any(name, desc);
    }
    reg.add(
        create.read_only(false).destructive(false).idempotent(false).open_world(false),
        handler(|ctx, a| async move {
            let v = |k: &str| a.raw(k).cloned();
            let input = ConverterInput {
                name: v("name"),
                form: v("form"),
                command: v("command"),
                url: v("url"),
                auth_header: v("auth_header"),
                secret: v("secret"),
                file_field: v("file_field"),
                response_field: v("response_field"),
                extensions: v("extensions"),
                max_input_bytes: v("max_input_bytes"),
                timeout_seconds: v("timeout_seconds"),
                ..Default::default()
            };
            create_converter(&ctx.state, &ctx.person, input).await
        }),
    );
    let mut update = ToolSchema::new("admin.update_converter", UPDATE_CONVERTER)
        .req_str("converter_id", "Id of the converter to change.");
    for (name, desc) in UPDATE_PARAMS {
        update = update.opt_any(name, desc);
    }
    reg.add(
        update.read_only(false).destructive(false).idempotent(true).open_world(false),
        handler(|ctx, a| async move {
            let v = |k: &str| a.raw(k).cloned();
            let input = ConverterInput {
                name: v("name"),
                form: v("form"),
                command: v("command"),
                url: v("url"),
                auth_header: v("auth_header"),
                secret: v("secret"),
                file_field: v("file_field"),
                response_field: v("response_field"),
                extensions: v("extensions"),
                max_input_bytes: v("max_input_bytes"),
                timeout_seconds: v("timeout_seconds"),
                clear_secret: v("clear_secret"),
                converter_id: None,
            };
            update_converter(&ctx.state, &ctx.person, &a.str("converter_id")?, input).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.list_converters", LIST_CONVERTERS)
            .opt_any("marker", "The marker of the previous answer, to read the next page.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            list_converters(&ctx.state, &ctx.person, a.raw("marker").cloned()).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.delete_converter", DELETE_CONVERTER)
            .req_str("converter_id", "Id of the converter.")
            .read_only(false)
            .destructive(true)
            .idempotent(false)
            .open_world(false),
        handler(|ctx, a| async move {
            delete_converter(&ctx.state, &ctx.person, &a.str("converter_id")?).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.set_default_converter", SET_DEFAULT_CONVERTER)
            .req_str("converter_id", "Id of the converter.")
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            set_default_converter(&ctx.state, &ctx.person, &a.str("converter_id")?).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.clear_default_converter", CLEAR_DEFAULT_CONVERTER)
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, _a| async move { clear_default_converter(&ctx.state, &ctx.person).await }),
    );
    reg.add(
        ToolSchema::new("admin.get_default_converter", GET_DEFAULT_CONVERTER)
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, _a| async move { get_default_converter(&ctx.state, &ctx.person).await }),
    );
    reg.add(
        ToolSchema::new("admin.set_project_converter", SET_PROJECT_CONVERTER)
            .req_str("project_id", "Id of the project.")
            .req_str("converter_id", "Id of the converter.")
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            set_project_converter(
                &ctx.state,
                &ctx.person,
                &a.str("project_id")?,
                &a.str("converter_id")?,
            )
            .await
        }),
    );
    reg.add(
        ToolSchema::new("admin.clear_project_converter", CLEAR_PROJECT_CONVERTER)
            .req_str("project_id", "Id of the project.")
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            clear_project_converter(&ctx.state, &ctx.person, &a.str("project_id")?).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.get_project_converter", GET_PROJECT_CONVERTER)
            .req_str("project_id", "Id of the project.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            get_project_converter(&ctx.state, &ctx.person, &a.str("project_id")?).await
        }),
    );
}

/// The registry of the caller, refused before any read for a non admin.
fn registry(state: &AppState, person: &str) -> Result<Registry> {
    state.require_admin(person)?;
    Registry::of(state.admin.as_ref(), state.config.search.embedding.dimensions)
}

pub(crate) async fn create_converter(
    state: &AppState,
    person: &str,
    input: ConverterInput,
) -> Result<Value> {
    let reg = registry(state, person)?;
    let c = reg.create_converter(person, input).await?;
    Ok(serde_json::to_value(c).expect("a converter serializes"))
}

pub(crate) async fn update_converter(
    state: &AppState,
    person: &str,
    converter_id: &str,
    input: ConverterInput,
) -> Result<Value> {
    let reg = registry(state, person)?;
    let c = reg.update_converter(person, converter_id, input).await?;
    Ok(serde_json::to_value(c).expect("a converter serializes"))
}

pub(crate) async fn list_converters(
    state: &AppState,
    person: &str,
    marker: Option<Value>,
) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(converter_page_json(&reg.list_converters(&marker).await?))
}

/// The `limit` newest records of converter actions (the converter, the default
/// converter and the project converter targets), newest first: what the
/// converters screen shows, never the model registry's records (DEC-013).
pub(crate) async fn list_converter_changes(
    state: &AppState,
    person: &str,
    limit: usize,
) -> Result<Vec<crate::settings::Change>> {
    let reg = registry(state, person)?;
    reg.list_changes_of_kinds(&["converter", "default_converter", "project_converter"], limit).await
}

/// `{converter}`, the converter or null (FR-NEW-008).
fn default_json(c: Option<crate::settings::Converter>) -> Value {
    serde_json::json!({"converter": c})
}

/// `{project_id, converter}`, the converter or null (FR-NEW-009).
fn project_json(project_id: &str, c: Option<crate::settings::Converter>) -> Value {
    serde_json::json!({"project_id": project_id, "converter": c})
}

pub(crate) async fn delete_converter(
    state: &AppState,
    person: &str,
    converter_id: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    reg.delete_converter(person, converter_id).await?;
    Ok(serde_json::json!({"converter_id": converter_id}))
}

pub(crate) async fn set_default_converter(
    state: &AppState,
    person: &str,
    converter_id: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(default_json(Some(reg.set_default_converter(person, converter_id).await?)))
}

pub(crate) async fn clear_default_converter(state: &AppState, person: &str) -> Result<Value> {
    let reg = registry(state, person)?;
    reg.clear_default_converter(person).await?;
    Ok(default_json(None))
}

pub(crate) async fn get_default_converter(state: &AppState, person: &str) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(default_json(reg.default_converter().await?))
}

pub(crate) async fn set_project_converter(
    state: &AppState,
    person: &str,
    project_id: &str,
    converter_id: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    let c = reg.set_project_converter(person, project_id, converter_id).await?;
    Ok(project_json(project_id, Some(c)))
}

pub(crate) async fn clear_project_converter(
    state: &AppState,
    person: &str,
    project_id: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    reg.clear_project_converter(person, project_id).await?;
    Ok(project_json(project_id, None))
}

pub(crate) async fn get_project_converter(
    state: &AppState,
    person: &str,
    project_id: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(project_json(project_id, reg.project_converter(project_id).await?))
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
            ("admin.create_converter", super::CREATE_CONVERTER),
            ("admin.update_converter", super::UPDATE_CONVERTER),
            ("admin.list_converters", super::LIST_CONVERTERS),
            ("admin.delete_converter", super::DELETE_CONVERTER),
            ("admin.set_default_converter", super::SET_DEFAULT_CONVERTER),
            ("admin.clear_default_converter", super::CLEAR_DEFAULT_CONVERTER),
            ("admin.get_default_converter", super::GET_DEFAULT_CONVERTER),
            ("admin.set_project_converter", super::SET_PROJECT_CONVERTER),
            ("admin.clear_project_converter", super::CLEAR_PROJECT_CONVERTER),
            ("admin.get_project_converter", super::GET_PROJECT_CONVERTER),
        ] {
            let tool = tools.iter().find(|t| t.name == name).expect(name);
            assert_eq!(tool.description.as_deref(), Some(desc), "{name}");
        }
        let create = tools.iter().find(|t| t.name == "admin.create_converter").unwrap();
        for (param, desc) in super::CREATE_PARAMS {
            let live = &create.input_schema["properties"][param]["description"];
            assert_eq!(live.as_str(), Some(desc), "{param}");
        }
        let update = tools.iter().find(|t| t.name == "admin.update_converter").unwrap();
        for (param, desc) in super::UPDATE_PARAMS {
            let live = &update.input_schema["properties"][param]["description"];
            assert_eq!(live.as_str(), Some(desc), "{param}");
        }
    }
}
