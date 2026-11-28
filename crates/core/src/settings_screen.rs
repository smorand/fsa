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

use crate::app_shell::{self, Page, Screen as Nav, Viewer};
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
        return refused_page(&s.app, &person, &e).await;
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

/// A refusal: the shell with the alert, the status of the error, the text the bare error
/// page held.
async fn refused_page(app: &AppState, person: &str, e: &ToolError) -> Response {
    let viewer = Viewer::load(app, person).await;
    app_shell::refusal(&viewer, Nav::Settings, status_of(e), &e.to_string())
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
            let viewer = Viewer::load(app, person).await;
            html(status, page(&viewer, &token, refused, &sections, &changes))
        }
        Err(e) => refused_page(app, person, &e).await,
    }
}

fn text(v: &Value, key: &str) -> String {
    escape_html(v[key].as_str().unwrap_or_default())
}

fn page(
    viewer: &Viewer,
    token: &str,
    refused: Option<&ToolError>,
    sections: &[Value],
    changes: &[Value],
) -> String {
    let csrf =
        format!("<input type=\"hidden\" name=\"csrf_token\" value=\"{}\">", escape_html(token));
    let mut tabs = String::new();
    let mut panels = String::new();
    for (i, sec) in sections.iter().enumerate() {
        let name = text(sec, "section");
        let active = if i == 0 { " active" } else { "" };
        tabs.push_str(&format!(
            "<button type=\"button\" class=\"tab{active}\" data-tab=\"s-{name}\">{name}</button>"
        ));
        let restart = if sec["restart_required"].as_bool() == Some(true) {
            "restart required"
        } else {
            "applied"
        };
        let mut panel = format!(
            "<section class=\"panel{active}\" id=\"s-{name}\" data-section=\"{name}\">\n<div class=\"card\"><div class=\"card-h\"><h2>{name}</h2></div><div class=\"card-b col\">\n<p class=\"muted\">Source: {} ({restart})</p>\n",
            text(sec, "source")
        );
        if let Some(secrets) = sec["secrets"].as_object() {
            for (path, s) in secrets {
                let state = if s["has_value"].as_bool() == Some(true) {
                    format!("set, ends with {}", escape_html(s["last4"].as_str().unwrap_or("")))
                } else {
                    "not set".to_string()
                };
                panel.push_str(&format!("<p>Secret {}: {state}</p>\n", escape_html(path)));
            }
        }
        let value = serde_json::to_string_pretty(&sec["value"]).unwrap_or_default();
        panel.push_str(&format!(
            "<pre class=\"code\">{}</pre>\n<form class=\"col\" method=\"post\" action=\"/app/settings/set\">{csrf}\
             <input type=\"hidden\" name=\"section\" value=\"{name}\">\
             <div class=\"field\"><label>Change</label><textarea class=\"textarea\" name=\"value\" rows=\"4\" cols=\"60\">{{}}</textarea></div>\
             <div><button type=\"submit\" class=\"btn primary\">Save</button></div></form>\n</div></div></section>\n",
            escape_html(&value)
        ));
        panels.push_str(&panel);
    }
    tabs.push_str(
        "<button type=\"button\" class=\"tab\" data-tab=\"s-changes\">Latest changes</button>",
    );
    let mut rows = String::new();
    for c in changes.iter().take(CHANGES_SHOWN) {
        let fields: Vec<&str> = c["fields"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            text(c, "at"),
            text(c, "person"),
            text(c, "action"),
            text(c, "target_name"),
            escape_html(&fields.join(", ")),
        ));
    }
    panels.push_str(&format!(
        "<section class=\"panel\" id=\"s-changes\"><div class=\"card\"><div class=\"card-h\"><h2>Latest changes</h2></div><div class=\"table-wrap\"><table>\n<tbody>\n{rows}</tbody>\n</table></div></div></section>\n"
    ));
    let body = format!(
        "<div class=\"vtabs\"><div class=\"tabs\" data-tabs=\"st\">{tabs}</div><div id=\"st\">{panels}</div></div>\n"
    );
    let alert = refused.map(|e| app_shell::alert(&e.to_string()));
    app_shell::page(viewer, Page { screen: Nav::Settings, title: "Settings", body, alert })
}
