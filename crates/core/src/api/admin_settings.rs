//! The REST interface of the registry (SPEC-0102 DEC-013): providers, models,
//! server defaults and the record of changes, under `/api/admin`.
//!
//! A handler only reads the caller, shapes the input and maps the answer or the
//! refusal. The access rule (platform administrators only, reading included) and
//! every other rule live in `tools::models`, the functions the `admin.*` tools
//! call, so a body, an answer and a refusal equal the tools' (DEC-006).

use super::dataplane::{Q, body_args, error_response, person_of, unauthorized};
use crate::errors::Result;
use crate::settings::{ModelInput, ModelUpdate, ProviderUpdate, SecretText};
use crate::state::AppState;
use crate::tools::models;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/admin/providers", post(create_provider).get(list_providers))
        .route(
            "/api/admin/providers/{provider_id}",
            axum::routing::patch(update_provider).delete(delete_provider),
        )
        .route("/api/admin/models", post(create_model).get(list_models))
        .route(
            "/api/admin/models/{model_id}",
            axum::routing::patch(update_model).delete(delete_model),
        )
        .route("/api/admin/defaults", get(get_defaults))
        .route("/api/admin/defaults/{kind}", axum::routing::put(set_default).delete(clear_default))
        .route("/api/admin/setting-changes", get(list_changes))
        .with_state(state)
}

/// Bearer, then the operation: the 401 body of the plane, then the tool's own
/// refusals through the plane's error envelope.
async fn run<F, Fut>(state: Arc<AppState>, headers: HeaderMap, op: F) -> Response
where
    F: FnOnce(Arc<AppState>, String) -> Fut,
    Fut: Future<Output = Result<Value>>,
{
    let person = match person_of(&state, &headers) {
        Ok(p) => p,
        Err(e) => return unauthorized(&e),
    };
    // The access rule wins over every shape error: a non admin with a malformed
    // body is refused as a non admin (FR-NEW-020).
    if let Err(e) = state.require_admin(&person) {
        return error_response(&e);
    }
    match op(state, person).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(&e),
    }
}

async fn create_provider(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    run(state, headers, |state, person| async move {
        let a = body_args(&body)?;
        let secret = SecretText::new(a.str_or("secret", ""));
        let (name, endpoint) = (a.str("name")?, a.str("endpoint")?);
        models::create_provider(
            &state,
            &person,
            name,
            endpoint,
            a.str_or("auth_scheme", "none"),
            secret,
        )
        .await
    })
    .await
}

async fn update_provider(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    run(state, headers, |state, person| async move {
        let a = body_args(&body)?;
        let update = ProviderUpdate {
            name: a.opt_str("name"),
            endpoint: a.opt_str("endpoint"),
            auth_scheme: a.opt_str("auth_scheme"),
            secret: a.opt_str("secret").map(SecretText::new),
            clear_secret: a.bool_or("clear_secret", false),
        };
        models::update_provider(&state, &person, &provider_id, update).await
    })
    .await
}

async fn delete_provider(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    run(state, headers, |state, person| async move {
        models::delete_provider(&state, &person, &provider_id).await
    })
    .await
}

async fn list_providers(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<Vec<(String, String)>>,
) -> Response {
    let q = Q(q);
    run(state, headers, |state, person| async move {
        models::list_providers(&state, &person, q.str_or("marker", "")).await
    })
    .await
}

async fn create_model(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    run(state, headers, |state, person| async move {
        let a = body_args(&body)?;
        let input = ModelInput {
            provider_id: a.str("provider_id")?,
            name: a.str("name")?,
            kind: a.str("kind")?,
            dimension: a.opt_i64("dimension")?,
            max_input_tokens: a.opt_i64("max_input_tokens")?,
        };
        models::create_model(&state, &person, input).await
    })
    .await
}

async fn update_model(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    run(state, headers, |state, person| async move {
        let a = body_args(&body)?;
        let update = ModelUpdate {
            name: a.opt_str("name"),
            dimension: a.opt_i64("dimension")?,
            max_input_tokens: a.opt_i64("max_input_tokens")?,
        };
        models::update_model(&state, &person, &model_id, update).await
    })
    .await
}

async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    run(state, headers, |state, person| async move {
        models::delete_model(&state, &person, &model_id).await
    })
    .await
}

async fn list_models(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<Vec<(String, String)>>,
) -> Response {
    let q = Q(q);
    run(state, headers, |state, person| async move {
        models::list_models(
            &state,
            &person,
            q.str_or("provider_id", ""),
            q.str_or("kind", ""),
            q.str_or("marker", ""),
        )
        .await
    })
    .await
}

async fn get_defaults(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    run(
        state,
        headers,
        |state, person| async move { models::get_default_models(&state, &person).await },
    )
    .await
}

async fn set_default(
    State(state): State<Arc<AppState>>,
    Path(kind): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    run(state, headers, |state, person| async move {
        let model_id = body_args(&body)?.str("model_id")?;
        models::set_default_model(&state, &person, &kind, &model_id).await
    })
    .await
}

async fn clear_default(
    State(state): State<Arc<AppState>>,
    Path(kind): Path<String>,
    headers: HeaderMap,
) -> Response {
    run(state, headers, |state, person| async move {
        models::clear_default_model(&state, &person, &kind).await
    })
    .await
}

async fn list_changes(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<Vec<(String, String)>>,
) -> Response {
    let q = Q(q);
    run(state, headers, |state, person| async move {
        models::list_setting_changes(&state, &person, q.str_or("marker", "")).await
    })
    .await
}
