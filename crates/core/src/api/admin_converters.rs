//! The REST interface of the document converters (SPEC-0106 DEC-015, DEC-017):
//! ten routes under `/api/admin`, on the functions of `tools::converters` that
//! the `admin.*converter*` tools call, so a body, an answer and a refusal equal
//! the tools' (FR-NEW-018). A handler only reads the caller and shapes the input.

use super::dataplane::{Q, body_args, error_response, person_of, unauthorized};
use crate::errors::Result;
use crate::settings::ConverterInput;
use crate::state::AppState;
use crate::tools::converters;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/admin/converters", post(create).get(list))
        .route("/api/admin/converters/{converter_id}", patch(update).delete(delete))
        .route(
            "/api/admin/converter-default",
            get(get_default).put(set_default).delete(clear_default),
        )
        .route(
            "/api/admin/projects/{project_id}/converter",
            get(get_project).put(set_project).delete(clear_project),
        )
        .with_state(state)
}

/// Bearer, then the administrator rule, then the operation: the access rule
/// wins over every shape error, as for the registry routes.
async fn run<F, Fut>(state: Arc<AppState>, headers: HeaderMap, op: F) -> Response
where
    F: FnOnce(Arc<AppState>, String) -> Fut,
    Fut: Future<Output = Result<Value>>,
{
    let person = match person_of(&state, &headers) {
        Ok(p) => p,
        Err(e) => return unauthorized(&e),
    };
    if let Err(e) = state.require_admin(&person) {
        return error_response(&e);
    }
    match op(state, person).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(&e),
    }
}

/// The body as the untyped input of the tools: a `null` field is not given, and
/// the rule module words a wrong type itself. No `Debug` anywhere: `secret`.
fn input_of(body: &Bytes) -> Result<ConverterInput> {
    let a = body_args(body)?.0;
    let field = |k: &str| a.get(k).filter(|v| !v.is_null()).cloned();
    Ok(ConverterInput {
        name: field("name"),
        form: field("form"),
        command: field("command"),
        url: field("url"),
        auth_header: field("auth_header"),
        secret: field("secret"),
        file_field: field("file_field"),
        response_field: field("response_field"),
        extensions: field("extensions"),
        max_input_bytes: field("max_input_bytes"),
        timeout_seconds: field("timeout_seconds"),
        converter_id: field("converter_id"),
        clear_secret: field("clear_secret"),
    })
}

async fn create(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    run(state, headers, |state, person| async move {
        // A creation has no target id: the tool never reads one.
        let input = ConverterInput { converter_id: None, clear_secret: None, ..input_of(&body)? };
        converters::create_converter(&state, &person, input).await
    })
    .await
}

async fn update(
    State(state): State<Arc<AppState>>,
    Path(converter_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    run(state, headers, |state, person| async move {
        converters::update_converter(&state, &person, &converter_id, input_of(&body)?).await
    })
    .await
}

async fn delete(
    State(state): State<Arc<AppState>>,
    Path(converter_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    run(state, headers, |state, person| async move {
        converters::delete_converter(&state, &person, &converter_id).await
    })
    .await
}

async fn list(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<Vec<(String, String)>>,
) -> Response {
    let q = Q(q);
    run(state, headers, |state, person| async move {
        let marker = Some(q.str_or("marker", "")).filter(|m| !m.is_empty()).map(Value::from);
        converters::list_converters(&state, &person, marker).await
    })
    .await
}

async fn get_default(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    run(state, headers, |state, person| async move {
        converters::get_default_converter(&state, &person).await
    })
    .await
}

async fn set_default(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    run(state, headers, |state, person| async move {
        let id = body_args(&body)?.str("converter_id")?;
        converters::set_default_converter(&state, &person, &id).await
    })
    .await
}

async fn clear_default(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    run(state, headers, |state, person| async move {
        converters::clear_default_converter(&state, &person).await
    })
    .await
}

async fn get_project(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    run(state, headers, |state, person| async move {
        converters::get_project_converter(&state, &person, &project_id).await
    })
    .await
}

async fn set_project(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    run(state, headers, |state, person| async move {
        let id = body_args(&body)?.str("converter_id")?;
        converters::set_project_converter(&state, &person, &project_id, &id).await
    })
    .await
}

async fn clear_project(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    run(state, headers, |state, person| async move {
        converters::clear_project_converter(&state, &person, &project_id).await
    })
    .await
}
