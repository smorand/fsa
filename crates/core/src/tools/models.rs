//! The registry tools of SPEC-0102: `admin.create_provider`, `update_provider`,
//! `delete_provider`, `list_providers`, `create_model`, `update_model`,
//! `delete_model`, `list_models`, `set_default_model`, `clear_default_model`,
//! `get_default_models` and `list_setting_changes`, platform
//! administrator only, reading included (FR-NEW-020).
//!
//! Typed functions only: the `#[tool]` methods in `mcp::server` call them, and
//! they add nothing to [`Registry`] but the admin check and the JSON shape
//! (DEC-006). Nothing here logs an argument: a secret travels as [`SecretText`]
//! (DEC-005).

use crate::errors::Result;
use crate::settings::{
    ModelInput, ModelUpdate, ProviderInput, ProviderUpdate, Registry, SecretText,
};
use crate::state::AppState;
#[cfg(test)]
use crate::tools::registry_support::{ToolRegistry, ToolSchema, handler};
use serde_json::{Value, json};

/// Must equal the `#[tool]` descriptions in `mcp::server`, literals there.
#[cfg(test)]
pub(crate) const CREATE_PROVIDER: &str = "Register a model provider: a name, the base address of its service, an authentication scheme ('none' or 'bearer') and, for 'bearer', a secret that is stored encrypted and never returned (platform admin only).";
#[cfg(test)]
pub(crate) const UPDATE_PROVIDER: &str = "Change the fields given of a model provider; a field not given keeps its value; clear_secret removes the secret (platform admin only).";
#[cfg(test)]
pub(crate) const DELETE_PROVIDER: &str =
    "Delete a model provider that has no model, with its secret (platform admin only).";
#[cfg(test)]
pub(crate) const LIST_PROVIDERS: &str = "List the model providers ordered by name without regard to case, 100 per answer, each with has_secret instead of its secret (platform admin only).";
#[cfg(test)]
pub(crate) const CREATE_MODEL: &str = "Register a model of a provider: a name, a kind (text_embedding, image_embedding, reranker, ocr or chat), for the embedding kinds a dimension, and optionally a limit of input tokens (platform admin only).";
#[cfg(test)]
pub(crate) const UPDATE_MODEL: &str = "Change the name, the dimension or the input token limit of a model; a field not given keeps its value; max_input_tokens 0 removes the limit (platform admin only).";
#[cfg(test)]
pub(crate) const DELETE_MODEL: &str =
    "Delete a model that is not a server default (platform admin only).";
#[cfg(test)]
pub(crate) const LIST_MODELS: &str = "List the models ordered by provider name then model name without regard to case, 100 per answer, optionally of one provider or one kind (platform admin only).";
#[cfg(test)]
pub(crate) const SET_DEFAULT_MODEL: &str = "Make a model the server default of its kind, text_embedding or reranker, replacing the previous default; a text embedding model must have the dimension of the vector store (platform admin only).";
#[cfg(test)]
pub(crate) const CLEAR_DEFAULT_MODEL: &str = "Remove the server default of a kind, text_embedding or reranker; a kind with no default is not an error (platform admin only).";
#[cfg(test)]
pub(crate) const GET_DEFAULT_MODELS: &str = "Read the server default text embedding model and the server default reranker, null for a kind with none (platform admin only).";
#[cfg(test)]
pub(crate) const LIST_SETTING_CHANGES: &str = "List the record of changes of the platform settings, newest first, 100 per answer: who changed what, when, and which fields (platform admin only).";

/// Test dispatch glue mirroring the `#[tool]` methods, so the golden contract
/// regeneration sees the schemas the live surface serves.
#[cfg(test)]
pub(crate) fn register(reg: &mut ToolRegistry) {
    reg.add(
        ToolSchema::new("admin.create_provider", CREATE_PROVIDER)
            .req_str("name", "Name of the provider, 1 to 64 characters, unique without regard to case.")
            .req_str("endpoint", "Base address of the service, http or https, for example http://localhost:9010/v1.")
            .opt_str("auth_scheme", "none", "Authentication scheme: 'none' or 'bearer'.")
            .opt_str("secret", "", "Secret sent as a bearer token; required for 'bearer', refused for 'none'. Never returned.")
            .read_only(false)
            .destructive(false)
            .idempotent(false)
            .open_world(false),
        handler(|ctx, a| async move {
            create_provider(
                &ctx.state,
                &ctx.person,
                a.str("name")?,
                a.str("endpoint")?,
                a.str_or("auth_scheme", "none"),
                SecretText::new(a.str_or("secret", "")),
            )
            .await
        }),
    );
    reg.add(
        ToolSchema::new("admin.update_provider", UPDATE_PROVIDER)
            .req_str("provider_id", "Id of the provider to change.")
            .opt_str_null("name", "New name, 1 to 64 characters.")
            .opt_str_null("endpoint", "New base address of the service.")
            .opt_str_null("auth_scheme", "New authentication scheme: 'none' or 'bearer'.")
            .opt_str_null("secret", "New secret; empty keeps the stored one. Never returned.")
            .opt_bool("clear_secret", false, "True removes the stored secret.")
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            let update = ProviderUpdate {
                name: a.opt_str("name"),
                endpoint: a.opt_str("endpoint"),
                auth_scheme: a.opt_str("auth_scheme"),
                secret: a.opt_str("secret").map(SecretText::new),
                clear_secret: a.bool_or("clear_secret", false),
            };
            update_provider(&ctx.state, &ctx.person, &a.str("provider_id")?, update).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.delete_provider", DELETE_PROVIDER)
            .req_str("provider_id", "Id of the provider to delete.")
            .read_only(false)
            .destructive(true)
            .idempotent(false)
            .open_world(false),
        handler(|ctx, a| async move {
            delete_provider(&ctx.state, &ctx.person, &a.str("provider_id")?).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.list_providers", LIST_PROVIDERS)
            .opt_str("marker", "", "The marker of the previous answer, to read the next page.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            list_providers(&ctx.state, &ctx.person, &a.str_or("marker", "")).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.create_model", CREATE_MODEL)
            .req_str("provider_id", "Id of the provider of the model.")
            .req_str("name", "Name of the model, 1 to 128 characters, unique within its provider without regard to case.")
            .req_str("kind", "Kind: text_embedding, image_embedding, reranker, ocr or chat.")
            .opt_nullable_int("dimension", "Size of the vectors, 1 to 8192; required for the two embedding kinds, refused for the others.")
            .opt_nullable_int("max_input_tokens", "Largest input in tokens, 1 to 1000000; absent means no limit.")
            .read_only(false)
            .destructive(false)
            .idempotent(false)
            .open_world(false),
        handler(|ctx, a| async move {
            let input = ModelInput {
                provider_id: a.str("provider_id")?,
                name: a.str("name")?,
                kind: a.str("kind")?,
                dimension: a.opt_i64("dimension")?,
                max_input_tokens: a.opt_i64("max_input_tokens")?,
            };
            create_model(&ctx.state, &ctx.person, input).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.update_model", UPDATE_MODEL)
            .req_str("model_id", "Id of the model to change.")
            .opt_str_null("name", "New name, 1 to 128 characters.")
            .opt_nullable_int(
                "dimension",
                "New size of the vectors, 1 to 8192; embedding kinds only.",
            )
            .opt_nullable_int(
                "max_input_tokens",
                "New largest input in tokens, 1 to 1000000; 0 removes the limit.",
            )
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            let update = ModelUpdate {
                name: a.opt_str("name"),
                dimension: a.opt_i64("dimension")?,
                max_input_tokens: a.opt_i64("max_input_tokens")?,
            };
            update_model(&ctx.state, &ctx.person, &a.str("model_id")?, update).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.delete_model", DELETE_MODEL)
            .req_str("model_id", "Id of the model to delete.")
            .read_only(false)
            .destructive(true)
            .idempotent(false)
            .open_world(false),
        handler(|ctx, a| async move {
            delete_model(&ctx.state, &ctx.person, &a.str("model_id")?).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.list_models", LIST_MODELS)
            .opt_str("provider_id", "", "Only the models of this provider.")
            .opt_str("kind", "", "Only the models of this kind.")
            .opt_str("marker", "", "The marker of the previous answer, to read the next page.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            list_models(
                &ctx.state,
                &ctx.person,
                &a.str_or("provider_id", ""),
                &a.str_or("kind", ""),
                &a.str_or("marker", ""),
            )
            .await
        }),
    );
    reg.add(
        ToolSchema::new("admin.set_default_model", SET_DEFAULT_MODEL)
            .req_str("kind", "Kind of the default: text_embedding or reranker.")
            .req_str("model_id", "Id of the model, which must be of that kind.")
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            set_default_model(&ctx.state, &ctx.person, &a.str("kind")?, &a.str("model_id")?).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.clear_default_model", CLEAR_DEFAULT_MODEL)
            .req_str("kind", "Kind whose default is removed: text_embedding or reranker.")
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            clear_default_model(&ctx.state, &ctx.person, &a.str("kind")?).await
        }),
    );
    reg.add(
        ToolSchema::new("admin.get_default_models", GET_DEFAULT_MODELS)
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, _a| async move { get_default_models(&ctx.state, &ctx.person).await }),
    );
    reg.add(
        ToolSchema::new("admin.list_setting_changes", LIST_SETTING_CHANGES)
            .opt_str("marker", "", "The marker of the previous answer, to read the next page.")
            .read_only(true)
            .idempotent(true)
            .open_world(false),
        handler(|ctx, a| async move {
            list_setting_changes(&ctx.state, &ctx.person, &a.str_or("marker", "")).await
        }),
    );
}

/// The registry of the caller, refused before any read for a non admin.
fn registry(state: &AppState, person: &str) -> Result<Registry> {
    state.require_admin(person)?;
    Registry::of(state.admin.as_ref(), state.config.search.embedding.dimensions)
}

fn to_json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("a registry answer serializes")
}

pub(crate) async fn create_provider(
    state: &AppState,
    person: &str,
    name: String,
    endpoint: String,
    auth_scheme: String,
    secret: SecretText,
) -> Result<Value> {
    let reg = registry(state, person)?;
    let input = ProviderInput { name, endpoint, auth_scheme, secret };
    Ok(to_json(&reg.create_provider(person, input).await?))
}

pub(crate) async fn update_provider(
    state: &AppState,
    person: &str,
    provider_id: &str,
    update: ProviderUpdate,
) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(to_json(&reg.update_provider(person, provider_id, update).await?))
}

pub(crate) async fn delete_provider(
    state: &AppState,
    person: &str,
    provider_id: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    reg.delete_provider(person, provider_id).await?;
    Ok(json!({"deleted": true}))
}

pub(crate) async fn list_providers(state: &AppState, person: &str, marker: &str) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(reg.list_providers(marker).await?.to_json("providers"))
}

pub(crate) async fn create_model(
    state: &AppState,
    person: &str,
    input: ModelInput,
) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(to_json(&reg.create_model(person, input).await?))
}

pub(crate) async fn update_model(
    state: &AppState,
    person: &str,
    model_id: &str,
    update: ModelUpdate,
) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(to_json(&reg.update_model(person, model_id, update).await?))
}

pub(crate) async fn delete_model(state: &AppState, person: &str, model_id: &str) -> Result<Value> {
    let reg = registry(state, person)?;
    reg.delete_model(person, model_id).await?;
    Ok(json!({"deleted": true}))
}

pub(crate) async fn list_models(
    state: &AppState,
    person: &str,
    provider_id: &str,
    kind: &str,
    marker: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(reg.list_models(provider_id, kind, marker).await?.to_json("models"))
}

pub(crate) async fn set_default_model(
    state: &AppState,
    person: &str,
    kind: &str,
    model_id: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(to_json(&reg.set_default_model(person, kind, model_id).await?))
}

pub(crate) async fn clear_default_model(
    state: &AppState,
    person: &str,
    kind: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    reg.clear_default_model(person, kind).await?;
    Ok(json!({"cleared": true}))
}

pub(crate) async fn get_default_models(state: &AppState, person: &str) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(to_json(&reg.default_models().await?))
}

pub(crate) async fn list_setting_changes(
    state: &AppState,
    person: &str,
    marker: &str,
) -> Result<Value> {
    let reg = registry(state, person)?;
    Ok(reg.list_changes(marker).await?.to_json("changes"))
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
            ("admin.create_provider", super::CREATE_PROVIDER),
            ("admin.update_provider", super::UPDATE_PROVIDER),
            ("admin.delete_provider", super::DELETE_PROVIDER),
            ("admin.list_providers", super::LIST_PROVIDERS),
            ("admin.create_model", super::CREATE_MODEL),
            ("admin.update_model", super::UPDATE_MODEL),
            ("admin.delete_model", super::DELETE_MODEL),
            ("admin.list_models", super::LIST_MODELS),
            ("admin.set_default_model", super::SET_DEFAULT_MODEL),
            ("admin.clear_default_model", super::CLEAR_DEFAULT_MODEL),
            ("admin.get_default_models", super::GET_DEFAULT_MODELS),
            ("admin.list_setting_changes", super::LIST_SETTING_CHANGES),
        ] {
            let tool = tools.iter().find(|t| t.name == name).expect(name);
            assert_eq!(tool.description.as_deref(), Some(desc), "{name}");
        }
    }
}
