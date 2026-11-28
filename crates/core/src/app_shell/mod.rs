//! The shared page shell of the browser screens and the design assets (SPEC-0118).
//!
//! Every screen builds its document through [`page`] (head, sidebar, top bar, page head) and
//! its parts through [`alert`], [`empty_state`] and [`confirm`]; no screen writes a
//! `<!doctype>` of its own. The look comes from the files of `app-design` embedded in the
//! binary ([`assets`]); a page carries no inline style or script. Only the look lives here:
//! routes, identity, anti forgery and authorization stay in the screens.

pub mod assets;

use crate::errors::ToolError;
use crate::state::AppState;
use crate::token_screen::escape_html;
use axum::Json;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// The seven screens, in the order the sidebar lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Tokens,
    Trash,
    DeletedProjects,
    FileMetadata,
    Models,
    Converters,
    Settings,
}

impl Screen {
    const ALL: [Screen; 7] = [
        Screen::Tokens,
        Screen::Trash,
        Screen::DeletedProjects,
        Screen::FileMetadata,
        Screen::Models,
        Screen::Converters,
        Screen::Settings,
    ];

    /// The route, with no query: what the sidebar link points at.
    pub fn href(self) -> &'static str {
        match self {
            Screen::Tokens => "/app/tokens",
            Screen::Trash => "/app/trash",
            Screen::DeletedProjects => "/app/deleted-projects",
            Screen::FileMetadata => "/app/file-metadata",
            Screen::Models => "/app/models",
            Screen::Converters => "/app/converters",
            Screen::Settings => "/app/settings",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Screen::Tokens => "Git tokens",
            Screen::Trash => "Trash",
            Screen::DeletedProjects => "Deleted projects",
            Screen::FileMetadata => "File metadata",
            Screen::Models => "Models",
            Screen::Converters => "Converters",
            Screen::Settings => "Settings",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Screen::Tokens => "key",
            Screen::Trash => "trash",
            Screen::DeletedProjects => "folder",
            Screen::FileMetadata => "file",
            Screen::Models => "cpu",
            Screen::Converters => "layers",
            Screen::Settings => "settings",
        }
    }

    /// Sidebar group label.
    fn group(self) -> &'static str {
        match self {
            Screen::Tokens | Screen::Trash | Screen::DeletedProjects | Screen::FileMetadata => {
                "Workspace"
            }
            _ => "Administration",
        }
    }
}

/// What the sidebar needs to know about the person: computed once from the server state,
/// never from the request, so a person never sees a link to a screen she cannot open.
#[derive(Clone, Debug)]
pub struct Viewer {
    person: String,
    is_admin: bool,
    git: bool,
    member: bool,
}

impl Viewer {
    /// Read the facts of `person` from the state. A failing project lookup counts as no
    /// project: the link is left out, the page still renders.
    pub async fn load(state: &AppState, person: &str) -> Viewer {
        let member =
            state.admin.list_projects_for(person).await.map(|p| !p.is_empty()).unwrap_or(false);
        Viewer {
            person: person.to_string(),
            is_admin: state.is_admin(person),
            git: state.config.git.enabled,
            member,
        }
    }

    /// The screens this person can open, in sidebar order.
    pub fn screens(&self) -> Vec<Screen> {
        Screen::ALL
            .into_iter()
            .filter(|s| match s {
                Screen::Tokens => self.git,
                Screen::Trash | Screen::DeletedProjects => true,
                Screen::FileMetadata => self.member,
                Screen::Models | Screen::Converters | Screen::Settings => self.is_admin,
            })
            .collect()
    }
}

/// One page: which screen it is, the heading, the body (already escaped HTML) and an
/// optional alert shown above it.
pub struct Page<'a> {
    pub screen: Screen,
    pub title: &'a str,
    pub body: String,
    pub alert: Option<String>,
}

/// An icon from the embedded sprite (injected by `icons.js`).
pub fn icon(name: &str) -> String {
    format!("<svg class=\"ico\" aria-hidden=\"true\"><use href=\"#i-{name}\"/></svg>")
}

/// The initials shown in the avatar: the first two letters of the name before the `@`.
fn initials(person: &str) -> String {
    let name = person.split('@').next().unwrap_or(person);
    let s: String = name.chars().filter(|c| c.is_alphanumeric()).take(2).collect();
    if s.is_empty() { "?".to_string() } else { s.to_uppercase() }
}

fn sidebar(v: &Viewer, current: Screen) -> String {
    let mut out = String::new();
    let mut group = "";
    for s in v.screens() {
        if s.group() != group {
            group = s.group();
            out.push_str(&format!("<div class=\"nav-label\">{group}</div>"));
        }
        let mark = if s == current { " class=\"active\" aria-current=\"page\"" } else { "" };
        out.push_str(&format!(
            "<a href=\"{}\" title=\"{}\"{mark}>{}<span class=\"t\">{}</span></a>",
            s.href(),
            s.label(),
            icon(s.icon()),
            s.label()
        ));
    }
    out
}

/// The whole document of a screen.
pub fn page(v: &Viewer, p: Page<'_>) -> String {
    let who = escape_html(&v.person);
    let title = escape_html(p.title);
    let alert = p.alert.unwrap_or_default();
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{title}</title>\
         <link rel=\"stylesheet\" href=\"/app/assets/css/tokens.css\">\
         <link rel=\"stylesheet\" href=\"/app/assets/css/app.css\">\
         <script src=\"/app/assets/js/app.js\"></script>\
         <noscript><link rel=\"stylesheet\" href=\"/app/assets/css/nojs.css\"></noscript></head>\n<body>\n\
         <div class=\"shell\"><div class=\"side-backdrop\" data-sidebar-toggle></div>\
         <aside class=\"sidebar\"><div class=\"side-head\">\
         <a class=\"brand\" href=\"/app/trash\"><span data-logo></span><span class=\"t\">fsa</span></a>\
         <button type=\"button\" class=\"side-toggle\" data-sidebar-toggle \
         aria-label=\"Collapse or expand the menu\" title=\"Collapse or expand the menu\">{open}{rail}</button></div>\
         <nav class=\"nav\">{nav}</nav></aside>\n\
         <div class=\"main\"><header class=\"topbar\">\
         <button type=\"button\" class=\"btn icon ghost only-mobile\" data-sidebar-toggle aria-label=\"Open the menu\">{menu}</button>\
         <div class=\"grow\"></div>\
         <span class=\"row\"><span class=\"avatar sm\">{init}</span><span class=\"small\">{who}</span></span></header>\n\
         <main class=\"content\"><div class=\"page-head\"><div><h1>{title}</h1></div></div>\n\
         {alert}{body}</main></div></div>\n\
         <script src=\"/app/assets/js/icons-data.js\"></script>\
         <script src=\"/app/assets/js/icons.js\"></script></body>\n</html>\n",
        open = "<svg class=\"ico if-open\" aria-hidden=\"true\"><use href=\"#i-panel-left-close\"/></svg>",
        rail = "<svg class=\"ico if-rail\" aria-hidden=\"true\"><use href=\"#i-panel-left-open\"/></svg>",
        menu = icon("menu"),
        nav = sidebar(v, p.screen),
        init = escape_html(&initials(&v.person)),
        body = p.body,
    )
}

/// A page answer with a status.
pub fn html(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

/// A danger alert; `message` is escaped here.
pub fn alert(message: &str) -> String {
    format!(
        "<div class=\"alert danger\" role=\"alert\">{}<div>{}</div></div>\n",
        icon("alert"),
        escape_html(message)
    )
}

/// The design empty state: an icon, a title and a hint (both escaped).
pub fn empty_state(icon_name: &str, title: &str, hint: &str) -> String {
    format!(
        "<div class=\"empty\">{}<h3>{}</h3><p>{}</p></div>\n",
        icon(icon_name),
        escape_html(title),
        escape_html(hint)
    )
}

/// A confirmation before a risky post: a `type="button"` trigger and a modal that holds the
/// original form. `fields` is the hidden inputs of the form (already escaped, the one time
/// `csrf_token` among them), so the confirm button posts exactly what the plain form posted.
/// Without script the trigger does nothing: the actions that go through here need script.
pub fn confirm(
    id: &str,
    trigger: &str,
    heading: &str,
    text: &str,
    action: &str,
    fields: &str,
) -> String {
    let id = escape_html(id);
    format!(
        "<button type=\"button\" class=\"btn sm danger\" data-open=\"{id}\">{trigger}</button>\
         <div class=\"overlay\" id=\"{id}\"><div class=\"modal\">\
         <form method=\"post\" action=\"{action}\"><div class=\"modal-b\"><h2>{heading}</h2>\
         <p class=\"muted\">{text}</p>{fields}</div>\
         <div class=\"modal-f\"><button type=\"button\" class=\"btn\" data-close>Cancel</button>\
         <button type=\"submit\" class=\"btn danger\">{trigger}</button></div></form></div></div>",
        trigger = escape_html(trigger),
        heading = escape_html(heading),
        text = escape_html(text),
    )
}

/// Whether the caller is a browser asking for a page (`Accept` names `text/html`).
pub fn wants_html(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|a| a.to_ascii_lowercase().contains("text/html"))
}

/// A refusal for a browser: the shell with the alert, same status, same message. Callers
/// keep answering JSON to a client that did not ask for HTML.
pub fn refusal(v: &Viewer, screen: Screen, status: StatusCode, message: &str) -> Response {
    let title = screen.label();
    html(status, page(v, Page { screen, title, body: String::new(), alert: Some(alert(message)) }))
}

/// The answer to a refused call: the JSON every client got before this shell existed
/// (`{"error", "detail"}` with the error's status), or, for a browser that asked for HTML,
/// the shell with an alert carrying the same status and the same message.
pub async fn refuse(
    state: &AppState,
    person: &str,
    headers: &HeaderMap,
    screen: Screen,
    err: &ToolError,
) -> Response {
    let status =
        StatusCode::from_u16(err.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    if !wants_html(headers) {
        return (status, Json(json!({"error": err.code, "detail": err.message}))).into_response();
    }
    refusal(&Viewer::load(state, person).await, screen, status, &err.message)
}

#[cfg(test)]
mod assets_e2e;
#[cfg(test)]
mod screens_e2e;
#[cfg(test)]
mod shell_e2e;
#[cfg(test)]
mod testkit;
