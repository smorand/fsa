//! `/app/file-metadata`: the file metadata screen (SPEC-0111).
//!
//! A member opens a file of a project and sees what the server holds about it:
//! the conversion text, the images (caption and OCR text), the tables, the
//! headings, the chunk sets and their chunks one page or one chunk at a time,
//! and the jobs of the file. Every datum comes from the typed function the
//! matching tool calls (`tools::artifacts`, `tools::chunks`, `tools::derived`),
//! so this module adds the HTML and nothing else (S-2B).
//!
//! Identity is the token screen's own [`resolve_person`] (forwarded header,
//! bearer, read only `mcpfs_token` cookie). Every value that comes from storage
//! or from the request passes [`escape_html`] before it enters the page, and
//! the page carries no script.

use crate::app_shell::{self, Page, Screen as Nav, Viewer};
use crate::errors::{Result, ToolError, code};
use crate::state::AppState;
use crate::token_screen::{CsrfStore, check_csrf, escape_html, resolve_person};
use crate::tools::{artifacts, chunks, derived};
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// Characters of the conversion text the page shows (DEC-004).
const TEXT_SHOWN: usize = 20_000;

/// The file metadata screen router, always mounted. Owns its own [`CsrfStore`]:
/// the one time tokens of the recalculate forms are never shared with another
/// screen's.
pub fn router(state: Arc<AppState>) -> Router {
    let screen = Screen { app: state, csrf: Arc::new(CsrfStore::default()) };
    Router::new()
        .route("/app/file-metadata", get(show))
        .route("/app/file-metadata/recalculate", post(recalculate))
        .with_state(screen)
}

#[derive(Clone)]
struct Screen {
    app: Arc<AppState>,
    csrf: Arc<CsrfStore>,
}

/// The query of the page: the names of the form fields.
type Params = HashMap<String, String>;

fn param(p: &Params, name: &str) -> String {
    p.get(name).map(|v| v.trim().to_string()).unwrap_or_default()
}

async fn show(
    State(s): State<Screen>,
    Query(params): Query<Params>,
    headers: HeaderMap,
) -> Response {
    let (person, _) = match resolve_person(&s.app.identity, &headers) {
        Ok(p) => p,
        Err(e) => return unauthorized(&e),
    };
    let mount = param(&params, "mount_id");
    if mount.is_empty() {
        return picker(&s.app, &person).await;
    }
    render(&s, &person, &mount, &params, None).await
}

/// The page, or the refusal of the tool that failed, with its status.
async fn render(
    s: &Screen,
    person: &str,
    mount: &str,
    q: &Params,
    answer: Option<&Value>,
) -> Response {
    match read_page(s, person, mount, q, answer).await {
        Ok(body) => html(StatusCode::OK, body),
        Err(e) => refused_page(&s.app, person, &e).await,
    }
}

/// FR-NEW-008, FR-NEW-009: identity, then membership, then the one time token,
/// then `fs.recalculate`'s own function. A cookie post a third party could
/// forge needs the token; a header identity carries no ambient credential.
async fn recalculate(
    State(s): State<Screen>,
    headers: HeaderMap,
    Form(mut fields): Form<Params>,
) -> Response {
    let (person, source) = match resolve_person(&s.app.identity, &headers) {
        Ok(p) => p,
        Err(e) => return unauthorized(&e),
    };
    let mount = param(&fields, "mount_id");
    if let Err(e) = s.app.authorize(&mount, &person).await {
        return refused_page(&s.app, &person, &e).await;
    }
    let token = fields.remove("csrf_token");
    if let Some(refusal) = check_csrf(&s.csrf, source, &person, token.as_deref()) {
        return refusal;
    }
    let done = derived::recalculate(
        &s.app,
        &person,
        &mount,
        &param(&fields, "path"),
        &param(&fields, "kind"),
        &param(&fields, "set_id"),
    )
    .await;
    match done {
        Ok(job) => render(&s, &person, &mount, &fields, Some(&job)).await,
        Err(e) => refused_page(&s.app, &person, &e).await,
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
    app_shell::refusal(&viewer, Nav::FileMetadata, status_of(e), &e.to_string())
}

/// The project picker of FR-NEW-004: the caller's own projects, no metadata.
async fn picker(app: &AppState, person: &str) -> Response {
    match app.admin.list_projects_for(person).await {
        Ok(projects) => {
            let ids: Vec<String> = projects.into_iter().map(|p| p.id).collect();
            let viewer = Viewer::load(app, person).await;
            html(StatusCode::OK, page_shell(&viewer, &open_form(&ids, "", ""), ""))
        }
        Err(e) => refused_page(app, person, &e).await,
    }
}

/// The form that opens a file: a project of the caller and a path.
fn open_form(projects: &[String], mount: &str, path: &str) -> String {
    let mut out = String::from(
        "<div class=\"card\"><form class=\"card-b row\" method=\"get\" action=\"/app/file-metadata\">",
    );
    out.push_str("<select class=\"select\" name=\"mount_id\">");
    for id in projects {
        let sel = if id == mount { " selected" } else { "" };
        let id = escape_html(id);
        let _ = write!(out, "<option value=\"{id}\"{sel}>{id}</option>");
    }
    let _ = writeln!(
        out,
        "</select><input class=\"input\" name=\"path\" value=\"{}\"><button type=\"submit\" class=\"btn primary\">Open</button></form></div>",
        escape_html(path)
    );
    out
}

fn page_shell(viewer: &Viewer, head: &str, body: &str) -> String {
    app_shell::page(
        viewer,
        Page {
            screen: Nav::FileMetadata,
            title: "File metadata",
            body: format!("{head}{body}"),
            alert: None,
        },
    )
}

/// Percent encoding of one query value, so a path with `&` or `#` survives.
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// An escaped link to this page for `mount` and `path` with `extra` query pairs.
fn link(mount: &str, path: &str, extra: &[(&str, &str)]) -> String {
    let mut url = format!("/app/file-metadata?mount_id={}&path={}", enc(mount), enc(path));
    for (k, v) in extra.iter().filter(|(_, v)| !v.is_empty()) {
        let _ = write!(url, "&{k}={}", enc(v));
    }
    escape_html(&url)
}

fn text(v: &Value, key: &str) -> String {
    escape_html(v[key].as_str().unwrap_or_default())
}

fn number(v: &Value, key: &str) -> String {
    v[key].as_i64().map(|n| n.to_string()).unwrap_or_default()
}

/// The file the page is about, refused before any metadata is read: not a
/// member, a path that cannot be normalized, a folder, an unknown file.
async fn locate(app: &AppState, person: &str, mount: &str, raw: &str) -> Result<String> {
    app.authorize(mount, person).await?;
    let path = app.safety.normalize_path(raw)?;
    let client = app.stores.client(mount).await?;
    if client.is_file(&path).await? {
        return Ok(path);
    }
    if path == "/" || client.is_dir(&path).await? {
        return Err(ToolError::invalid_argument("path must be a file"));
    }
    Err(ToolError::not_found(format!("file not found: {path}")))
}

/// A datum a conversion carries: a file with no conversion has none, which is
/// an empty state of the page and not an error.
fn or_empty(r: Result<Value>) -> Result<Option<Value>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.code == code::NOT_FOUND => Ok(None),
        Err(e) => Err(e),
    }
}

const NO_CONVERSION: &str = "<p class=\"muted\">No conversion yet</p>\n";

async fn read_page(
    s: &Screen,
    person: &str,
    mount: &str,
    q: &Params,
    answer: Option<&Value>,
) -> Result<String> {
    let app = &s.app;
    let path = locate(app, person, mount, &param(q, "path")).await?;
    let marker = |name: &str| Some(param(q, name)).filter(|m| !m.is_empty());
    let mut body = String::new();
    if let Some(job) = answer {
        body.push_str(&answer_section(job));
    }
    body.push_str(&text_section(app, mount, &path).await?);
    let images = or_empty(
        artifacts::list_images(app, mount, &path, marker("images_marker").as_deref()).await,
    )?;
    body.push_str(&images_section(images.as_ref(), mount, &path));
    let tables = or_empty(
        artifacts::list_tables(app, mount, &path, marker("tables_marker").as_deref()).await,
    )?;
    body.push_str(&tables_section(tables.as_ref(), mount, &path));
    let headings = or_empty(
        artifacts::document_structure(app, mount, &path, marker("headings_marker").as_deref())
            .await,
    )?;
    body.push_str(&headings_section(headings.as_ref(), mount, &path));
    let (chunk_html, shown_set) = chunks_section(app, person, mount, &path, q).await?;
    body.push_str(&chunk_html);
    body.push_str(&jobs_section(app, mount, &path).await?);
    // Issued once everything is read, so a refused read leaves no token behind.
    let token = s.csrf.issue(person);
    body.push_str(&actions_section(&token, mount, &path, shown_set.as_deref()));
    let projects: Vec<String> =
        app.admin.list_projects_for(person).await?.into_iter().map(|p| p.id).collect();
    let head = format!(
        "{}<h2>{}</h2>\n",
        open_form(&projects, mount, &path),
        escape_html(&format!("{mount}:{path}"))
    );
    let viewer = Viewer::load(app, person).await;
    Ok(page_shell(&viewer, &head, &body))
}

async fn text_section(app: &AppState, mount: &str, path: &str) -> Result<String> {
    let mut out = String::from(
        "<section class=\"card\" data-section=\"text\">\n<div class=\"card-h\"><h3>Text</h3></div><div class=\"card-b\">\n",
    );
    let view =
        or_empty(artifacts::document_view(app, mount, path, false, None, Some("text")).await)?;
    match view {
        None => out.push_str(NO_CONVERSION),
        Some(v) => {
            let full = v["text"].as_str().unwrap_or_default();
            let total = full.chars().count();
            let shown: String = full.chars().take(TEXT_SHOWN).collect();
            let _ = writeln!(out, "<p>{total} characters</p>");
            if total > TEXT_SHOWN {
                let _ = writeln!(out, "<p>Showing the first {TEXT_SHOWN}</p>");
            }
            let _ = writeln!(out, "<pre>{}</pre>", escape_html(&shown));
        }
    }
    out.push_str("</div></section>\n");
    Ok(out)
}

/// The "more" link of a list page, when the tool returned a marker.
fn more_link(answer: &Value, mount: &str, path: &str, param: &str) -> String {
    match answer["marker"].as_str() {
        Some(m) => format!(
            "<p><a data-nav=\"more-{param}\" href=\"{}\">More</a></p>\n",
            link(mount, path, &[(param, m)])
        ),
        None => String::new(),
    }
}

fn images_section(images: Option<&Value>, mount: &str, path: &str) -> String {
    let mut out = String::from(
        "<section class=\"card\" data-section=\"images\">\n<div class=\"card-h\"><h3>Images</h3></div><div class=\"card-b\">\n",
    );
    let list = images.and_then(|v| v["images"].as_array()).filter(|l| !l.is_empty());
    match (images, list) {
        (None, _) => out.push_str(NO_CONVERSION),
        (Some(_), None) => out.push_str("<p>No image</p>\n"),
        (Some(all), Some(list)) => {
            out.push_str("<div class=\"table-wrap\"><table>\n<tbody>\n");
            for i in list {
                let _ = writeln!(
                    out,
                    "<tr data-image=\"{id}\"><td>{id}</td><td>page {}</td><td>{}</td><td>{}</td></tr>",
                    number(i, "page"),
                    text(i, "caption"),
                    text(i, "ocr_text"),
                    id = text(i, "id"),
                );
            }
            out.push_str("</tbody>\n</table></div>\n");
            out.push_str(&more_link(all, mount, path, "images_marker"));
        }
    }
    out.push_str("</div></section>\n");
    out
}

fn tables_section(tables: Option<&Value>, mount: &str, path: &str) -> String {
    let mut out = String::from(
        "<section class=\"card\" data-section=\"tables\">\n<div class=\"card-h\"><h3>Tables</h3></div><div class=\"card-b\">\n",
    );
    let list = tables.and_then(|v| v["tables"].as_array()).filter(|l| !l.is_empty());
    match (tables, list) {
        (None, _) => out.push_str(NO_CONVERSION),
        (Some(_), None) => out.push_str("<p>No table</p>\n"),
        (Some(all), Some(list)) => {
            out.push_str("<div class=\"table-wrap\"><table>\n<tbody>\n");
            for t in list {
                let _ = writeln!(
                    out,
                    "<tr data-table=\"{id}\"><td>{id}</td><td>{}</td><td>{} rows, {} columns</td><td>{}</td></tr>",
                    text(t, "caption"),
                    number(t, "rows"),
                    number(t, "columns"),
                    text(t, "csv_quality"),
                    id = text(t, "id"),
                );
            }
            out.push_str("</tbody>\n</table></div>\n");
            out.push_str(&more_link(all, mount, path, "tables_marker"));
        }
    }
    out.push_str("</div></section>\n");
    out
}

fn headings_section(headings: Option<&Value>, mount: &str, path: &str) -> String {
    let mut out = String::from(
        "<section class=\"card\" data-section=\"headings\">\n<div class=\"card-h\"><h3>Headings</h3></div><div class=\"card-b\">\n",
    );
    let list = headings.and_then(|v| v["headings"].as_array()).filter(|l| !l.is_empty());
    match (headings, list) {
        (None, _) => out.push_str(NO_CONVERSION),
        (Some(_), None) => out.push_str("<p>No heading</p>\n"),
        (Some(all), Some(list)) => {
            out.push_str("<ol>\n");
            for h in list {
                let _ = writeln!(
                    out,
                    "<li data-level=\"{}\">{}</li>",
                    number(h, "level"),
                    text(h, "title")
                );
            }
            out.push_str("</ol>\n");
            out.push_str(&more_link(all, mount, path, "headings_marker"));
        }
    }
    out.push_str("</div></section>\n");
    out
}

/// The sets of the file, then the chunk of `chunk` or the page of chunks of
/// the set shown (the one named by `set_id`, else the default set).
async fn chunks_section(
    app: &AppState,
    person: &str,
    mount: &str,
    path: &str,
    q: &Params,
) -> Result<(String, Option<String>)> {
    let sets = chunks::list_chunk_sets(app, person, mount, path).await?;
    let sets = sets["sets"].as_array().cloned().unwrap_or_default();
    let mut out = String::from(
        "<section class=\"card\" data-section=\"chunk-sets\">\n<div class=\"card-h\"><h3>Chunk sets</h3></div><div class=\"card-b\">\n",
    );
    if sets.is_empty() {
        out.push_str("<p>No chunk set</p>\n</div></section>\n");
        return Ok((out, None));
    }
    out.push_str("<div class=\"table-wrap\"><table>\n<tbody>\n");
    for s in &sets {
        let raw_id = s["set_id"].as_str().unwrap_or_default();
        let default = if s["default"].as_bool() == Some(true) { "default" } else { "" };
        let _ = writeln!(
            out,
            "<tr data-set=\"{id}\"><td><a href=\"{}\">{id}</a></td><td>{}</td><td>{}</td><td>{default}</td>\
             <td>{} chunks</td><td>{} tokens</td><td>{}</td><td>{}</td></tr>",
            link(mount, path, &[("set_id", raw_id)]),
            text(s, "strategy"),
            escape_html(&s["settings"].to_string()),
            number(s, "chunk_count"),
            number(s, "token_count"),
            text(s, "state"),
            text(s, "built_at"),
            id = escape_html(raw_id),
        );
    }
    out.push_str("</tbody>\n</table></div>\n");
    let wanted = param(q, "set_id");
    let shown = sets
        .iter()
        .find(|s| s["set_id"].as_str() == Some(wanted.as_str()))
        .or_else(|| sets.iter().find(|s| s["default"].as_bool() == Some(true)))
        .unwrap_or(&sets[0]);
    let set_id = shown["set_id"].as_str().unwrap_or_default();
    let chunk = param(q, "chunk");
    if chunk.is_empty() {
        out.push_str(&chunk_list(app, person, mount, path, set_id, &param(q, "marker")).await?);
    } else {
        out.push_str(&chunk_view(app, person, mount, path, set_id, &chunk).await?);
    }
    out.push_str("</div></section>\n");
    Ok((out, Some(set_id.to_string())))
}

/// One page of the chunks of a set: 100 rows, never the whole set.
async fn chunk_list(
    app: &AppState,
    person: &str,
    mount: &str,
    path: &str,
    set_id: &str,
    marker: &str,
) -> Result<String> {
    let page = chunks::list_chunks(app, person, mount, path, set_id, marker).await?;
    let mut out = format!(
        "<h4>Chunks of {}</h4>\n<div class=\"table-wrap\"><table>\n<tbody>\n",
        escape_html(set_id)
    );
    for c in page["chunks"].as_array().cloned().unwrap_or_default() {
        let n = number(&c, "number");
        let headings: Vec<&str> = c["heading_path"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "<tr data-chunk=\"{n}\"><td><a href=\"{}\">{n}</a></td><td>{}</td><td>{} tokens</td><td>{}</td></tr>",
            link(mount, path, &[("set_id", set_id), ("chunk", &n)]),
            escape_html(&headings.join(" > ")),
            number(&c, "token_count"),
            text(&c, "preview"),
        );
    }
    out.push_str("</tbody>\n</table></div>\n");
    if let Some(next) = page["marker"].as_str() {
        let _ = writeln!(
            out,
            "<p><a data-nav=\"more-chunks\" href=\"{}\">More chunks</a></p>",
            link(mount, path, &[("set_id", set_id), ("marker", next)])
        );
    }
    Ok(out)
}

/// One chunk in full, with its neighbours' links (S2).
async fn chunk_view(
    app: &AppState,
    person: &str,
    mount: &str,
    path: &str,
    set_id: &str,
    raw: &str,
) -> Result<String> {
    let n: i64 =
        raw.parse().map_err(|_| ToolError::invalid_argument("chunk must be a whole number"))?;
    let window = chunks::get_chunk(app, person, mount, path, set_id, n, 1, 1).await?;
    let rows = window["chunks"].as_array().cloned().unwrap_or_default();
    let has = |k: i64| rows.iter().any(|c| c["number"].as_i64() == Some(k));
    let Some(c) = rows.iter().find(|c| c["number"].as_i64() == Some(n)) else {
        return Err(ToolError::not_found(format!("chunk {n} not found in set {set_id}")));
    };
    let headings: Vec<&str> = c["heading_path"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut out = format!(
        "<h4>Chunk {n} of {}</h4>\n<p>{}</p>\n<p>pages {} to {}, {} tokens</p>\n<pre data-chunk-text=\"{n}\">{}</pre>\n<p>",
        escape_html(set_id),
        escape_html(&headings.join(" > ")),
        number(c, "page_start"),
        number(c, "page_end"),
        number(c, "token_count"),
        text(c, "text"),
    );
    if has(n - 1) {
        let at = (n - 1).to_string();
        let _ = write!(
            out,
            "<a data-nav=\"previous\" href=\"{}\">Previous</a> ",
            link(mount, path, &[("set_id", set_id), ("chunk", &at)])
        );
    }
    let _ = write!(
        out,
        "<a data-nav=\"list\" href=\"{}\">Chunk list</a>",
        link(mount, path, &[("set_id", set_id)])
    );
    if has(n + 1) {
        let at = (n + 1).to_string();
        let _ = write!(
            out,
            " <a data-nav=\"next\" href=\"{}\">Next</a>",
            link(mount, path, &[("set_id", set_id), ("chunk", &at)])
        );
    }
    out.push_str("</p>\n");
    Ok(out)
}

/// The job a recalculation answered (FR-NEW-008).
fn answer_section(job: &Value) -> String {
    format!(
        "<section class=\"card\" data-section=\"answer\" data-answer-job=\"{}\">\n<div class=\"card-h\"><h3>Recalculation requested</h3></div><div class=\"table-wrap\"><div class=\"table-wrap\"><table>\n<tbody>\n{}</tbody>\n</table></div>\n</div></section>\n",
        text(job, "job_id"),
        job_row(job)
    )
}

/// The recalculate forms: one per kind a person can ask, all carrying the
/// token of this page. The chunks form names the set shown.
fn actions_section(token: &str, mount: &str, path: &str, set_id: Option<&str>) -> String {
    let form = |kind: &str, label: &str, set: Option<&str>| {
        let set = set.map_or(String::new(), |id| {
            format!("<input type=\"hidden\" name=\"set_id\" value=\"{}\">", escape_html(id))
        });
        format!(
            "<form method=\"post\" action=\"/app/file-metadata/recalculate\">\
             <input type=\"hidden\" name=\"csrf_token\" value=\"{}\">\
             <input type=\"hidden\" name=\"mount_id\" value=\"{}\">\
             <input type=\"hidden\" name=\"path\" value=\"{}\">\
             <input type=\"hidden\" name=\"kind\" value=\"{kind}\">{set}\
             <button type=\"submit\" class=\"btn\">{label}</button></form>\n",
            escape_html(token),
            escape_html(mount),
            escape_html(path),
        )
    };
    let mut out = String::from(
        "<section class=\"card\" data-section=\"actions\">\n<div class=\"card-h\"><h3>Recalculate</h3></div><div class=\"card-b\">\n",
    );
    out.push_str(&form("search_index", "Recalculate the search entries", None));
    if let Some(id) = set_id {
        out.push_str(&form("chunks", "Recalculate the chunk set shown", Some(id)));
    }
    out.push_str("</div></section>\n");
    out
}

fn job_row(j: &Value) -> String {
    let left = match j["seconds_left"].as_u64() {
        Some(s) => format!("{s} s left"),
        None => String::new(),
    };
    format!(
        "<tr data-job=\"{}\"><td>{}</td><td>{}</td><td>{}</td><td>{left}</td></tr>\n",
        text(j, "job_id"),
        text(j, "kind"),
        text(j, "state"),
        text(j, "cause"),
    )
}

async fn jobs_section(app: &AppState, mount: &str, path: &str) -> Result<String> {
    let jobs = derived::list_derived_jobs(app, mount, path, "").await?;
    let jobs = jobs["jobs"].as_array().cloned().unwrap_or_default();
    let mut out = String::from(
        "<section class=\"card\" data-section=\"jobs\">\n<div class=\"card-h\"><h3>Jobs</h3></div><div class=\"card-b\">\n",
    );
    if jobs.is_empty() {
        out.push_str("<p>No job</p>\n");
    } else {
        out.push_str("<div class=\"table-wrap\"><table>\n<tbody>\n");
        for j in &jobs {
            out.push_str(&job_row(j));
        }
        out.push_str("</tbody>\n</table></div>\n");
    }
    out.push_str("</div></section>\n");
    Ok(out)
}

#[cfg(test)]
mod e2e;
