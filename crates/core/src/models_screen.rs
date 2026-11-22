//! `/app/models*`: the models screen for platform administrators (SPEC-0102
//! US-0289, DEC-006, DEC-014).
//!
//! It shows the providers, the models under them, the server defaults and the
//! latest changes, and has one form per registry operation. Every action calls
//! the typed functions of `tools::models`, the ones the `admin.*` tools call,
//! so the admin check and every rule stay in one place: this module adds the
//! HTML and nothing else.
//!
//! Identity and anti forgery are the token screen's own: the same
//! [`resolve_person`] (header, bearer, read only `mcpfs_token` cookie), the same
//! single use [`CsrfStore`] token (one instance per router), required only when
//! the cookie authenticated the request.
//!
//! No secret is ever rendered: the answers carry `has_secret` only, secret
//! inputs are password fields without a value, and an empty secret on an edit
//! keeps the stored one (the registry's rule for an absent secret).

use crate::errors::{Result, ToolError};
use crate::settings::{ModelInput, ModelUpdate, ProviderUpdate, SecretText};
use crate::state::AppState;
use crate::token_screen::{CsrfStore, check_csrf, escape_html, resolve_person};
use crate::tools::models;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

/// How many changes the page lists, newest first.
const CHANGES_SHOWN: usize = 20;

/// The models screen router, always mounted. Owns its own [`CsrfStore`].
pub fn router(state: Arc<AppState>) -> Router {
    let screen = Screen { app: state, csrf: Arc::new(CsrfStore::default()) };
    Router::new()
        .route("/app/models", get(show))
        .route("/app/models/{group}/{action}", post(act))
        .with_state(screen)
}

#[derive(Clone)]
struct Screen {
    app: Arc<AppState>,
    csrf: Arc<CsrfStore>,
}

/// The form fields by name: the names of the tool inputs plus `csrf_token`.
type Fields = HashMap<String, String>;

async fn show(State(s): State<Screen>, headers: HeaderMap) -> Response {
    let (person, _) = match resolve_person(&s.app.identity, &headers) {
        Ok(p) => p,
        Err(e) => return unauthorized(&e),
    };
    render(&s, &person, StatusCode::OK, None).await
}

async fn act(
    State(s): State<Screen>,
    Path((group, action)): Path<(String, String)>,
    headers: HeaderMap,
    Form(mut fields): Form<Fields>,
) -> Response {
    let (person, source) = match resolve_person(&s.app.identity, &headers) {
        Ok(p) => p,
        Err(e) => return unauthorized(&e),
    };
    // Identity, then the admin refusal, then the one time token: a non admin
    // cookie post is told it is not an admin, not that its token is wrong.
    if let Err(e) = s.app.require_admin(&person) {
        return html(status_of(&e), error_page(&e));
    }
    let token = fields.remove("csrf_token");
    if let Some(refusal) = check_csrf(&s.csrf, source, &person, token.as_deref()) {
        return refusal;
    }
    let done = match (group.as_str(), action.as_str()) {
        ("provider", "create") => provider_create(&s.app, &person, &fields).await,
        ("provider", "update") => provider_update(&s.app, &person, &fields).await,
        ("provider", "delete") => {
            models::delete_provider(&s.app, &person, &field(&fields, "provider_id")).await
        }
        ("model", "create") => model_create(&s.app, &person, &fields).await,
        ("model", "update") => model_update(&s.app, &person, &fields).await,
        ("model", "delete") => {
            models::delete_model(&s.app, &person, &field(&fields, "model_id")).await
        }
        ("default", "set") => {
            models::set_default_model(
                &s.app,
                &person,
                &field(&fields, "kind"),
                &field(&fields, "model_id"),
            )
            .await
        }
        ("default", "clear") => {
            models::clear_default_model(&s.app, &person, &field(&fields, "kind")).await
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    match done {
        Ok(_) => (StatusCode::SEE_OTHER, [(header::LOCATION, "/app/models")]).into_response(),
        Err(e) => render(&s, &person, status_of(&e), Some(&e)).await,
    }
}

fn field(f: &Fields, name: &str) -> String {
    f.get(name).map(|v| v.trim().to_string()).unwrap_or_default()
}

/// A field left empty is a field not given: the registry keeps its value.
fn given(f: &Fields, name: &str) -> Option<String> {
    Some(field(f, name)).filter(|v| !v.is_empty())
}

/// An optional whole number; text that is not one is the refusal of the
/// contract, worded from the field name only.
fn number(f: &Fields, name: &str) -> Result<Option<i64>> {
    match given(f, name) {
        None => Ok(None),
        Some(v) => v
            .parse::<i64>()
            .map(Some)
            .map_err(|_| ToolError::invalid_argument(format!("{name} must be a whole number"))),
    }
}

async fn provider_create(app: &AppState, person: &str, f: &Fields) -> Result<Value> {
    models::create_provider(
        app,
        person,
        field(f, "name"),
        field(f, "endpoint"),
        // Only an absent field defaults to none; the form always sends one.
        f.get("auth_scheme").map_or_else(|| "none".to_string(), |v| v.trim().to_string()),
        SecretText::new(f.get("secret").cloned().unwrap_or_default()),
    )
    .await
}

async fn provider_update(app: &AppState, person: &str, f: &Fields) -> Result<Value> {
    let update = ProviderUpdate {
        name: given(f, "name"),
        endpoint: given(f, "endpoint"),
        auth_scheme: given(f, "auth_scheme"),
        // Empty keeps the stored secret, so only a typed value is a new one.
        secret: f.get("secret").filter(|v| !v.is_empty()).cloned().map(SecretText::new),
        clear_secret: f.get("clear_secret").is_some_and(|v| !v.is_empty() && v != "false"),
    };
    models::update_provider(app, person, &field(f, "provider_id"), update).await
}

async fn model_create(app: &AppState, person: &str, f: &Fields) -> Result<Value> {
    let input = ModelInput {
        provider_id: field(f, "provider_id"),
        name: field(f, "name"),
        kind: field(f, "kind"),
        dimension: number(f, "dimension")?,
        max_input_tokens: number(f, "max_input_tokens")?,
    };
    models::create_model(app, person, input).await
}

async fn model_update(app: &AppState, person: &str, f: &Fields) -> Result<Value> {
    let update = ModelUpdate {
        name: given(f, "name"),
        dimension: number(f, "dimension")?,
        max_input_tokens: number(f, "max_input_tokens")?,
    };
    models::update_model(app, person, &field(f, "model_id"), update).await
}

fn status_of(e: &ToolError) -> StatusCode {
    StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

fn unauthorized(e: &ToolError) -> Response {
    (StatusCode::UNAUTHORIZED, Json(json!({"error": e.code, "detail": e.message}))).into_response()
}

fn html(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

/// Every provider and every model: the tools answer 100 at a time. Two plain
/// loops, because a closure taking the marker is not provably `Send`.
async fn registry_rows(app: &AppState, person: &str) -> Result<(Vec<Value>, Vec<Value>)> {
    let (mut providers, mut models_) = (Vec::new(), Vec::new());
    let mut marker = String::new();
    loop {
        let answer = models::list_providers(app, person, &marker).await?;
        providers.extend(answer["providers"].as_array().cloned().unwrap_or_default());
        match answer["marker"].as_str() {
            Some(m) => marker = m.to_string(),
            None => break,
        }
    }
    marker.clear();
    loop {
        let answer = models::list_models(app, person, "", "", &marker).await?;
        models_.extend(answer["models"].as_array().cloned().unwrap_or_default());
        match answer["marker"].as_str() {
            Some(m) => marker = m.to_string(),
            None => break,
        }
    }
    Ok((providers, models_))
}

/// The page, or the refusal of the registry when the caller may not read it.
/// `refused` is the text of the action that failed, shown above the forms.
async fn render(
    s: &Screen,
    person: &str,
    status: StatusCode,
    refused: Option<&ToolError>,
) -> Response {
    let app = &s.app;
    let read = async {
        let (providers, models_) = registry_rows(app, person).await?;
        let defaults = models::get_default_models(app, person).await?;
        let changes = models::list_setting_changes(app, person, "").await?;
        Ok::<_, ToolError>((providers, models_, defaults, changes))
    };
    match read.await {
        Ok((providers, models_, defaults, changes)) => {
            // Issued once the page is about to render, so a refused read never
            // leaves a token behind.
            let token = s.csrf.issue(person);
            let changes: Vec<Value> = changes["changes"].as_array().cloned().unwrap_or_default();
            let body = page(person, &token, refused, &providers, &models_, &defaults, &changes);
            html(status, body)
        }
        Err(e) => html(status_of(&e), error_page(&e)),
    }
}

fn error_page(e: &ToolError) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Models</title></head>\n\
         <body>\n<h1>Models</h1>\n<p class=\"error\">{}</p>\n</body>\n</html>\n",
        escape_html(&e.to_string())
    )
}

fn text(v: &Value, key: &str) -> String {
    escape_html(v[key].as_str().unwrap_or_default())
}

fn number_text(v: &Value, key: &str) -> String {
    v[key].as_i64().map(|n| n.to_string()).unwrap_or_default()
}

fn page(
    person: &str,
    token: &str,
    refused: Option<&ToolError>,
    providers: &[Value],
    models_: &[Value],
    defaults: &Value,
    changes: &[Value],
) -> String {
    let csrf =
        format!("<input type=\"hidden\" name=\"csrf_token\" value=\"{}\">", escape_html(token));
    let mut out = String::new();
    out.push_str(&format!(
        "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Models</title></head>\n\
         <body>\n<h1>Models</h1>\n<p>Signed in as {}</p>\n",
        escape_html(person)
    ));
    if let Some(e) = refused {
        out.push_str(&format!("<p class=\"error\">{}</p>\n", escape_html(&e.to_string())));
    }
    out.push_str("<h2>Defaults</h2>\n<ul>\n");
    for kind in ["text_embedding", "reranker"] {
        let d = &defaults[kind];
        let who = if d.is_null() { "none".to_string() } else { text(d, "name") };
        out.push_str(&format!("<li data-default=\"{kind}\">{kind}: {who}\n"));
        out.push_str(&format!(
            "<form method=\"post\" action=\"/app/models/default/clear\">{csrf}<input type=\"hidden\" name=\"kind\" value=\"{kind}\"><button type=\"submit\">Clear</button></form></li>\n"
        ));
    }
    out.push_str("</ul>\n<h2>Providers</h2>\n");
    for p in providers {
        let id = text(p, "provider_id");
        let secret =
            if p["has_secret"].as_bool() == Some(true) { "Secret stored" } else { "No secret" };
        out.push_str(&format!(
            "<section data-provider=\"{id}\">\n<h3>{}</h3>\n<p>{} ({}) {secret}</p>\n",
            text(p, "name"),
            text(p, "endpoint"),
            text(p, "auth_scheme"),
        ));
        out.push_str(&format!(
            "<form method=\"post\" action=\"/app/models/provider/update\">{csrf}<input type=\"hidden\" name=\"provider_id\" value=\"{id}\">\
             <input name=\"name\" value=\"{}\"><input name=\"endpoint\" value=\"{}\"><input name=\"auth_scheme\" value=\"{}\">\
             <input name=\"secret\" type=\"password\" autocomplete=\"new-password\"><label><input type=\"checkbox\" name=\"clear_secret\" value=\"true\">Clear secret</label>\
             <button type=\"submit\">Save</button></form>\n",
            text(p, "name"),
            text(p, "endpoint"),
            text(p, "auth_scheme"),
        ));
        out.push_str(&format!(
            "<form method=\"post\" action=\"/app/models/provider/delete\">{csrf}<input type=\"hidden\" name=\"provider_id\" value=\"{id}\"><button type=\"submit\">Delete provider</button></form>\n<ul>\n"
        ));
        for m in models_.iter().filter(|m| m["provider_id"] == p["provider_id"]) {
            let mid = text(m, "model_id");
            let kind = text(m, "kind");
            let star = if m["is_default"].as_bool() == Some(true) { " (default)" } else { "" };
            out.push_str(&format!(
                "<li data-model=\"{mid}\">{} {kind} dimension {} max tokens {}{star}\n",
                text(m, "name"),
                number_text(m, "dimension"),
                number_text(m, "max_input_tokens"),
            ));
            out.push_str(&format!(
                "<form method=\"post\" action=\"/app/models/model/update\">{csrf}<input type=\"hidden\" name=\"model_id\" value=\"{mid}\">\
                 <input name=\"name\" value=\"{}\"><input name=\"dimension\" value=\"{}\"><input name=\"max_input_tokens\" value=\"{}\"><button type=\"submit\">Save</button></form>\n",
                text(m, "name"),
                number_text(m, "dimension"),
                number_text(m, "max_input_tokens"),
            ));
            out.push_str(&format!(
                "<form method=\"post\" action=\"/app/models/model/delete\">{csrf}<input type=\"hidden\" name=\"model_id\" value=\"{mid}\"><button type=\"submit\">Delete model</button></form>\n"
            ));
            if matches!(m["kind"].as_str(), Some("text_embedding" | "reranker")) {
                out.push_str(&format!(
                    "<form method=\"post\" action=\"/app/models/default/set\">{csrf}<input type=\"hidden\" name=\"kind\" value=\"{kind}\"><input type=\"hidden\" name=\"model_id\" value=\"{mid}\"><button type=\"submit\">Make default</button></form>\n"
                ));
            }
            out.push_str("</li>\n");
        }
        out.push_str(&format!(
            "</ul>\n<form method=\"post\" action=\"/app/models/model/create\">{csrf}<input type=\"hidden\" name=\"provider_id\" value=\"{id}\">\
             <input name=\"name\"><select name=\"kind\"><option>text_embedding</option><option>image_embedding</option><option>reranker</option><option>ocr</option><option>chat</option></select>\
             <input name=\"dimension\"><input name=\"max_input_tokens\"><button type=\"submit\">Add model</button></form>\n</section>\n"
        ));
    }
    out.push_str(&format!(
        "<h2>New provider</h2>\n<form method=\"post\" action=\"/app/models/provider/create\">{csrf}\
         <input name=\"name\"><input name=\"endpoint\"><select name=\"auth_scheme\"><option>none</option><option>bearer</option></select>\
         <input name=\"secret\" type=\"password\" autocomplete=\"new-password\"><button type=\"submit\">Create provider</button></form>\n"
    ));
    out.push_str("<h2>Latest changes</h2>\n<table>\n<tbody>\n");
    for c in changes.iter().take(CHANGES_SHOWN) {
        let fields: Vec<&str> = c["fields"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        out.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} {}</td><td>{}</td></tr>\n",
            text(c, "at"),
            text(c, "person"),
            text(c, "action"),
            text(c, "target_kind"),
            text(c, "target_name"),
            escape_html(&fields.join(", ")),
        ));
    }
    out.push_str("</tbody>\n</table>\n</body>\n</html>\n");
    out
}
