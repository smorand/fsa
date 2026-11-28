//! `/app/assets/{path}`: the design system files, embedded in the binary (SPEC-0118).
//!
//! The folder `crates/core/web/assets/` is copied from `app-design` by
//! `scripts/sync-design.sh` and embedded with `rust-embed` (feature `debug-embed`, so a
//! debug build and a test read the same bytes as a release build, never the disk). A file is
//! found by its exact key in the embedded table, never by a path on disk, so a traversal
//! has nothing to reach; the checks below only make the refusal explicit.

use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rust_embed::RustEmbed;

/// The embedded design files.
#[derive(RustEmbed)]
#[folder = "web/assets/"]
pub(crate) struct Assets;

/// Browsers may keep an asset for an hour; the page links carry the version.
const CACHE_CONTROL: &str = "public, max-age=3600";

/// The route, mounted with no authentication: the files are public and read only.
pub fn router() -> Router {
    Router::new().route("/app/assets/{*path}", get(serve))
}

async fn serve(Path(path): Path<String>) -> Response {
    if !is_plain_key(&path) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(file) = Assets::get(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    (
        [
            (header::CONTENT_TYPE, content_type(&path)),
            (header::CACHE_CONTROL, CACHE_CONTROL),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        file.data.into_owned(),
    )
        .into_response()
}

/// A key made of non empty segments with no `..`, no `.` and no backslash.
fn is_plain_key(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && path.split('/').all(|seg| !seg.is_empty() && seg != ".." && seg != ".")
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
