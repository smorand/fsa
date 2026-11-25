//! `/app/converters*`: the converters screen for platform administrators
//! (SPEC-0106 US-0320, DEC-013, DEC-015, DEC-020).
//!
//! It shows the converters, the platform default, the projects that have a
//! converter and the latest converter changes, with one form per operation.
//! Every action calls the typed functions of `tools::converters`, the ones the
//! `admin.*` tools call, so the admin check and every rule stay in one place:
//! this module adds the HTML and the translation of form text to tool input.
//!
//! Same structure as the models screen: identity, then the administrator
//! check, then the one time token, with its own [`CsrfStore`]. No secret is
//! rendered: the answers carry `has_secret` only, the secret input has no
//! value, and an empty secret on an edit keeps the stored one.

use crate::errors::{Result, ToolError};
use crate::settings::ConverterInput;
use crate::state::AppState;
use crate::token_screen::{CsrfStore, check_csrf, escape_html, resolve_person};
use crate::tools::converters;
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

/// The converters screen router, always mounted. Owns its own [`CsrfStore`].
pub fn router(state: Arc<AppState>) -> Router {
    let screen = Screen { app: state, csrf: Arc::new(CsrfStore::default()) };
    Router::new()
        .route("/app/converters", get(show))
        .route("/app/converters/{group}/{action}", post(act))
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
    let app = &s.app;
    let done = match (group.as_str(), action.as_str()) {
        ("converter", "create") => converters::create_converter(app, &person, input(&fields)).await,
        ("converter", "update") => {
            converters::update_converter(
                app,
                &person,
                &field(&fields, "converter_id"),
                input(&fields),
            )
            .await
        }
        ("converter", "delete") => {
            converters::delete_converter(app, &person, &field(&fields, "converter_id")).await
        }
        ("default", "set") => {
            converters::set_default_converter(app, &person, &field(&fields, "converter_id")).await
        }
        ("default", "clear") => converters::clear_default_converter(app, &person).await,
        ("project", "set") => {
            converters::set_project_converter(
                app,
                &person,
                &field(&fields, "project_id"),
                &field(&fields, "converter_id"),
            )
            .await
        }
        ("project", "clear") => {
            converters::clear_project_converter(app, &person, &field(&fields, "project_id")).await
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    match done {
        Ok(_) => (StatusCode::SEE_OTHER, [(header::LOCATION, "/app/converters")]).into_response(),
        Err(e) => render(&s, &person, status_of(&e), Some(&e)).await,
    }
}

fn field(f: &Fields, name: &str) -> String {
    f.get(name).map(|v| v.trim().to_string()).unwrap_or_default()
}

/// A text field as the tool input: absent or empty is not given, anything
/// else goes untrimmed so the rule module words its own refusal (a name of
/// spaces is "must be 1 to 64 characters", not "missing").
fn text_input(f: &Fields, name: &str) -> Option<Value> {
    f.get(name).filter(|v| !v.is_empty()).map(|v| json!(v))
}

/// A list typed in a text area: one item per line, blank lines dropped.
fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

/// Extensions typed with commas, spaces or both.
fn extensions(text: &str) -> Vec<String> {
    text.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|e| !e.is_empty())
        .map(String::from)
        .collect()
}

/// A whole number when the text is one, else the text, so the rule module
/// words the refusal.
fn number_input(f: &Fields, name: &str) -> Option<Value> {
    let v = field(f, name);
    if v.is_empty() {
        return None;
    }
    Some(v.parse::<i64>().map_or_else(|_| json!(v), |n| json!(n)))
}

/// The tool input of a create or an update form. `response_field` and
/// `extensions` are sent whenever the form carries them, even empty, so an
/// edit can blank them (the registry keeps only a field not given). The
/// secret is sent only when typed; `clear_secret` is the box ticked.
fn input(f: &Fields) -> ConverterInput {
    let command = f.get("command").map(|c| json!(lines(c))).filter(|c| c != &json!([]));
    ConverterInput {
        name: text_input(f, "name"),
        form: text_input(f, "form"),
        command,
        url: text_input(f, "url"),
        auth_header: text_input(f, "auth_header"),
        secret: text_input(f, "secret"),
        file_field: text_input(f, "file_field"),
        response_field: f.get("response_field").map(|v| json!(v.trim())),
        extensions: f.get("extensions").map(|v| json!(extensions(v))),
        max_input_bytes: number_input(f, "max_input_bytes"),
        timeout_seconds: number_input(f, "timeout_seconds"),
        converter_id: None,
        clear_secret: f
            .get("clear_secret")
            .filter(|v| !v.is_empty() && *v != "false")
            .map(|_| json!(true)),
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

/// Everything the page shows.
struct Rows {
    converters: Vec<Value>,
    default: Value,
    /// `(project_id, converter)` for each project that has a converter.
    projects: Vec<(String, Value)>,
    changes: Vec<crate::settings::Change>,
}

/// Every converter (the tool answers 100 at a time, so one loop), the default, the projects with a converter (one
/// read per project) and the latest converter changes. The administrator
/// check is the first call's.
async fn read_rows(app: &AppState, person: &str) -> Result<Rows> {
    let mut converters_ = Vec::new();
    let mut marker: Option<Value> = None;
    loop {
        let answer = converters::list_converters(app, person, marker.take()).await?;
        converters_.extend(answer["converters"].as_array().cloned().unwrap_or_default());
        match answer["marker"].as_str() {
            Some(m) => marker = Some(json!(m)),
            None => break,
        }
    }
    let default = converters::get_default_converter(app, person).await?["converter"].clone();
    let mut projects = Vec::new();
    for p in app.admin.list_all_projects().await? {
        let c = converters::get_project_converter(app, person, &p.id).await;
        // A project removed for good between the two reads is not worth a refusal.
        if let Ok(v) = c
            && !v["converter"].is_null()
        {
            projects.push((p.id, v["converter"].clone()));
        }
    }
    let changes = converters::list_converter_changes(app, person, CHANGES_SHOWN).await?;
    Ok(Rows { converters: converters_, default, projects, changes })
}

/// The page, or the refusal of the registry when the caller may not read it.
/// `refused` is the text of the action that failed, shown above the forms.
async fn render(
    s: &Screen,
    person: &str,
    status: StatusCode,
    refused: Option<&ToolError>,
) -> Response {
    match read_rows(&s.app, person).await {
        Ok(rows) => {
            // Issued once the page is about to render, so a refused read never
            // leaves a token behind.
            let token = s.csrf.issue(person);
            html(status, page(person, &token, refused, &rows))
        }
        Err(e) => html(status_of(&e), error_page(&e)),
    }
}

fn error_page(e: &ToolError) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Converters</title></head>\n\
         <body>\n<h1>Converters</h1>\n<p class=\"error\">{}</p>\n</body>\n</html>\n",
        escape_html(&e.to_string())
    )
}

fn text(v: &Value, key: &str) -> String {
    escape_html(v[key].as_str().unwrap_or_default())
}

/// The strings of a JSON list, for a text field.
fn list_of(v: &Value, key: &str) -> Vec<String> {
    v[key]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default()
}

fn page(person: &str, token: &str, refused: Option<&ToolError>, rows: &Rows) -> String {
    let csrf =
        format!("<input type=\"hidden\" name=\"csrf_token\" value=\"{}\">", escape_html(token));
    let mut out = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Converters</title></head>\n\
         <body>\n<h1>Converters</h1>\n<p>Signed in as {}</p>\n",
        escape_html(person)
    );
    if let Some(e) = refused {
        out.push_str(&format!("<p class=\"error\">{}</p>\n", escape_html(&e.to_string())));
    }
    let who = if rows.default.is_null() { "none".to_string() } else { text(&rows.default, "name") };
    out.push_str(&format!(
        "<h2>Default</h2>\n<p data-default=\"converter\">{who}</p>\n\
         <form method=\"post\" action=\"/app/converters/default/clear\">{csrf}<button type=\"submit\">Clear</button></form>\n\
         <h2>Converters</h2>\n"
    ));
    for c in &rows.converters {
        converter_section(&mut out, &csrf, c);
    }
    out.push_str(&format!(
        "<h2>New converter</h2>\n<form method=\"post\" action=\"/app/converters/converter/create\">{csrf}\
         <input name=\"name\"><select name=\"form\"><option>program</option><option>http</option></select>\
         <textarea name=\"command\"></textarea><input name=\"url\"><input name=\"auth_header\">\
         <input name=\"secret\" type=\"password\" autocomplete=\"new-password\">\
         <input name=\"file_field\"><input name=\"response_field\"><input name=\"extensions\">\
         <input name=\"max_input_bytes\"><input name=\"timeout_seconds\">\
         <button type=\"submit\">Create converter</button></form>\n<h2>Projects</h2>\n<ul>\n"
    ));
    for (id, c) in &rows.projects {
        let pid = escape_html(id);
        out.push_str(&format!(
            "<li data-project=\"{pid}\">{pid}: {}\n<form method=\"post\" action=\"/app/converters/project/clear\">{csrf}\
             <input type=\"hidden\" name=\"project_id\" value=\"{pid}\"><button type=\"submit\">Clear</button></form></li>\n",
            text(c, "name"),
        ));
    }
    out.push_str(&format!(
        "</ul>\n<form method=\"post\" action=\"/app/converters/project/set\">{csrf}\
         <input name=\"project_id\"><select name=\"converter_id\">"
    ));
    for c in &rows.converters {
        out.push_str(&format!(
            "<option value=\"{}\">{}</option>",
            text(c, "converter_id"),
            text(c, "name")
        ));
    }
    out.push_str("</select><button type=\"submit\">Set project converter</button></form>\n");
    out.push_str("<h2>Latest changes</h2>\n<table>\n<tbody>\n");
    for c in &rows.changes {
        out.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} {}</td><td>{}</td></tr>\n",
            escape_html(&c.at),
            escape_html(&c.person),
            escape_html(&c.action),
            escape_html(&c.target_kind),
            escape_html(&c.target_name),
            escape_html(&c.fields.join(", ")),
        ));
    }
    out.push_str("</tbody>\n</table>\n</body>\n</html>\n");
    out
}

/// One converter: its summary, its edit form (the fields of its own form
/// only), delete and make default.
fn converter_section(out: &mut String, csrf: &str, c: &Value) {
    let id = text(c, "converter_id");
    let http = c["form"] == "http";
    let secret = if http {
        if c["has_secret"].as_bool() == Some(true) { " Secret stored" } else { " No secret" }
    } else {
        ""
    };
    out.push_str(&format!(
        "<section data-converter=\"{id}\">\n<h3>{}</h3>\n<p>{}{secret}</p>\n\
         <form method=\"post\" action=\"/app/converters/converter/update\">{csrf}\
         <input type=\"hidden\" name=\"converter_id\" value=\"{id}\"><input name=\"name\" value=\"{}\">",
        text(c, "name"),
        text(c, "form"),
        text(c, "name"),
    ));
    if http {
        out.push_str(&format!(
            "<input name=\"url\" value=\"{}\"><input name=\"auth_header\" value=\"{}\">\
             <input name=\"secret\" type=\"password\" autocomplete=\"new-password\">\
             <label><input type=\"checkbox\" name=\"clear_secret\">Clear the secret</label>\
             <input name=\"file_field\" value=\"{}\"><input name=\"response_field\" value=\"{}\">",
            text(c, "url"),
            text(c, "auth_header"),
            text(c, "file_field"),
            text(c, "response_field"),
        ));
    } else {
        out.push_str(&format!(
            "<textarea name=\"command\">{}</textarea>",
            escape_html(&list_of(c, "command").join("\n"))
        ));
    }
    out.push_str(&format!(
        "<input name=\"extensions\" value=\"{}\"><input name=\"max_input_bytes\" value=\"{}\">\
         <input name=\"timeout_seconds\" value=\"{}\"><button type=\"submit\">Save</button></form>\n\
         <form method=\"post\" action=\"/app/converters/converter/delete\">{csrf}<input type=\"hidden\" name=\"converter_id\" value=\"{id}\"><button type=\"submit\">Delete converter</button></form>\n\
         <form method=\"post\" action=\"/app/converters/default/set\">{csrf}<input type=\"hidden\" name=\"converter_id\" value=\"{id}\"><button type=\"submit\">Make default</button></form>\n</section>\n",
        escape_html(&list_of(c, "extensions").join(", ")),
        c["max_input_bytes"],
        c["timeout_seconds"],
    ));
}
