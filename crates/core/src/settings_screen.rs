//! `/app/settings`: the server settings screen for platform administrators
//! (SPEC-0114 FR-NEW-008).
//!
//! One page lists the sections with where each value comes from and whether a
//! restart applies it, shows the value without secrets and, for each secret,
//! whether it is set and its last 4 characters; one form per section takes a
//! JSON object of the keys to change. Every action calls the typed functions of
//! `tools::settings`, the ones the `admin.*` tools call, so the admin check and
//! every rule stay in one place: this module adds the HTML and nothing else.
//!
//! Identity and anti forgery are the models screen's own: `resolve_person`, a
//! single use `CsrfStore` token (one instance per router), required only when
//! the cookie authenticated the request. No secret is rendered or prefilled.

use crate::errors::ToolError;
use crate::state::AppState;
use crate::token_screen::{CsrfStore, check_csrf, escape_html, resolve_person};
use crate::tools::settings;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

/// How many changes the page lists, newest first.
const CHANGES_SHOWN: usize = 20;

/// The settings screen router, always mounted. Owns its own [`CsrfStore`].
pub fn router(state: Arc<AppState>) -> Router {
    let screen = Screen { app: state, csrf: Arc::new(CsrfStore::default()) };
    Router::new()
        .route("/app/settings", get(show))
        .route("/app/settings/set", post(set))
        .with_state(screen)
}

#[derive(Clone)]
struct Screen {
    app: Arc<AppState>,
    csrf: Arc<CsrfStore>,
}

type Fields = HashMap<String, String>;

async fn show(State(s): State<Screen>, headers: HeaderMap) -> Response {
    let (person, _) = match resolve_person(&s.app.identity, &headers) {
        Ok(p) => p,
        Err(e) => return unauthorized(&e),
    };
    render(&s, &person, StatusCode::OK, None).await
}

async fn set(
    State(s): State<Screen>,
    headers: HeaderMap,
    Form(mut fields): Form<Fields>,
) -> Response {
    let (person, source) = match resolve_person(&s.app.identity, &headers) {
        Ok(p) => p,
        Err(e) => return unauthorized(&e),
    };
    // Identity, then the admin refusal, then the one time token.
    if let Err(e) = s.app.require_admin(&person) {
        return html(status_of(&e), error_page(&e));
    }
    let token = fields.remove("csrf_token");
    if let Some(refusal) = check_csrf(&s.csrf, source, &person, token.as_deref()) {
        return refusal;
    }
    let section = fields.get("section").map(|v| v.trim().to_string()).unwrap_or_default();
    let raw = fields.get("value").map(|v| v.trim().to_string()).unwrap_or_default();
    let done = match serde_json::from_str::<Value>(if raw.is_empty() { "{}" } else { &raw }) {
        Ok(value) => settings::set_setting(&s.app, &person, &section, value).await,
        Err(_) => Err(ToolError::invalid_argument("the value must be a JSON object")),
    };
    match done {
        Ok(_) => (StatusCode::SEE_OTHER, [(header::LOCATION, "/app/settings")]).into_response(),
        Err(e) => render(&s, &person, status_of(&e), Some(&e)).await,
    }
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

fn error_page(e: &ToolError) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Settings</title></head>\n\
         <body>\n<h1>Settings</h1>\n<p class=\"error\">{}</p>\n</body>\n</html>\n",
        escape_html(&e.to_string())
    )
}

async fn render(
    s: &Screen,
    person: &str,
    status: StatusCode,
    refused: Option<&ToolError>,
) -> Response {
    let app = &s.app;
    let read = async {
        let list = settings::list_settings(app, person).await?;
        let mut sections = Vec::new();
        for e in list["settings"].as_array().cloned().unwrap_or_default() {
            let name = e["section"].as_str().unwrap_or_default().to_string();
            sections.push(settings::get_setting(app, person, &name).await?);
        }
        let changes = settings::recent_changes(app, person, CHANGES_SHOWN).await?;
        Ok::<_, ToolError>((sections, changes))
    };
    match read.await {
        Ok((sections, changes)) => {
            let token = s.csrf.issue(person);
            html(status, page(person, &token, refused, &sections, &changes))
        }
        Err(e) => html(status_of(&e), error_page(&e)),
    }
}

fn text(v: &Value, key: &str) -> String {
    escape_html(v[key].as_str().unwrap_or_default())
}

fn page(
    person: &str,
    token: &str,
    refused: Option<&ToolError>,
    sections: &[Value],
    changes: &[Value],
) -> String {
    let csrf =
        format!("<input type=\"hidden\" name=\"csrf_token\" value=\"{}\">", escape_html(token));
    let mut out = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Settings</title></head>\n\
         <body>\n<h1>Settings</h1>\n<p>Signed in as {}</p>\n",
        escape_html(person)
    );
    if let Some(e) = refused {
        out.push_str(&format!("<p class=\"error\">{}</p>\n", escape_html(&e.to_string())));
    }
    for sec in sections {
        let name = text(sec, "section");
        let restart = if sec["restart_required"].as_bool() == Some(true) {
            "restart required"
        } else {
            "applied"
        };
        out.push_str(&format!(
            "<section data-section=\"{name}\">\n<h2>{name}</h2>\n<p>Source: {} ({restart})</p>\n",
            text(sec, "source")
        ));
        if let Some(secrets) = sec["secrets"].as_object() {
            for (path, s) in secrets {
                let state = if s["has_value"].as_bool() == Some(true) {
                    format!("set, ends with {}", escape_html(s["last4"].as_str().unwrap_or("")))
                } else {
                    "not set".to_string()
                };
                out.push_str(&format!("<p>Secret {}: {state}</p>\n", escape_html(path)));
            }
        }
        let value = serde_json::to_string_pretty(&sec["value"]).unwrap_or_default();
        out.push_str(&format!(
            "<pre>{}</pre>\n<form method=\"post\" action=\"/app/settings/set\">{csrf}\
             <input type=\"hidden\" name=\"section\" value=\"{name}\">\
             <textarea name=\"value\" rows=\"4\" cols=\"60\">{{}}</textarea>\
             <button type=\"submit\">Save</button></form>\n</section>\n",
            escape_html(&value)
        ));
    }
    out.push_str("<h2>Latest changes</h2>\n<table>\n<tbody>\n");
    for c in changes.iter().take(CHANGES_SHOWN) {
        let fields: Vec<&str> = c["fields"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        out.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            text(c, "at"),
            text(c, "person"),
            text(c, "action"),
            text(c, "target_name"),
            escape_html(&fields.join(", ")),
        ));
    }
    out.push_str("</tbody>\n</table>\n</body>\n</html>\n");
    out
}
