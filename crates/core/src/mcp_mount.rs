//! Where the MCP route can be mounted (SPEC-0116 FR-NEW-046, FR-NEW-048).
//!
//! The MCP service is nested at `server.mcp_path` next to every other route of the
//! server. The router panics when the path collides with one of them, and a stored
//! path that panics would stop every later start with nothing in the tools to undo
//! it, so a path that cannot be mounted is refused as a value (DEC-006, D23, D24).
//!
//! The table below holds every route the server can serve, with every optional
//! family (REST plane, git) on, whatever the running configuration holds; the test
//! `table_matches_the_routers` in `app.rs` checks it against the real routers.

/// The routes of the server as written in its routers. A `{name}` segment is a
/// variable part, a `{*name}` segment takes the rest of the path.
pub(crate) const ROUTES: &[&str] = &[
    "/health",
    "/exports/{token}",
    "/git/{mount_id}/git-receive-pack",
    "/git/{mount_id}/git-upload-pack",
    "/git/{mount_id}/info/refs",
    "/api/docs",
    "/api/docs/",
    "/api/docs/{*asset}",
    "/api/swagger.json",
    "/api/admin/converter-default",
    "/api/admin/converters",
    "/api/admin/converters/{converter_id}",
    "/api/admin/defaults",
    "/api/admin/defaults/{kind}",
    "/api/admin/models",
    "/api/admin/models/{model_id}",
    "/api/admin/projects/{project_id}/converter",
    "/api/admin/projects/{project_id}/disk-usage",
    "/api/admin/projects/{project_id}/folder-usage",
    "/api/admin/projects/{project_id}/quota",
    "/api/admin/providers",
    "/api/admin/providers/{provider_id}",
    "/api/admin/setting-changes",
    "/api/admin/settings",
    "/api/admin/settings/{section}",
    "/api/fs/diff",
    "/api/fs/roots",
    "/api/fs/{mount_id}/{op}",
    "/app/converters",
    "/app/converters/{group}/{action}",
    "/app/deleted-projects",
    "/app/deleted-projects/undelete",
    "/app/file-metadata",
    "/app/file-metadata/recalculate",
    "/app/models",
    "/app/models/{group}/{action}",
    "/app/settings",
    "/app/settings/set",
    "/app/tokens",
    "/app/tokens/revoke",
    "/app/trash",
    "/app/trash/restore",
];

/// The refusal for `path`, `None` when the MCP service can be mounted there.
pub(crate) fn refusal(path: &str) -> Option<String> {
    let cannot = !plain(path) || path.ends_with('/') || collides(path);
    cannot.then(|| format!("server.mcp_path '{path}' cannot be served: choose another path"))
}

/// A path of ASCII letters and digits and `-_.~`, segments between single slashes,
/// no `.` or `..` segment.
fn plain(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains("//")
        && path.chars().all(|c| c.is_ascii_alphanumeric() || "-_.~/".contains(c))
        && path.split('/').all(|s| s != "." && s != "..")
}

fn is_var(seg: &str) -> bool {
    seg.starts_with('{') && seg.ends_with('}')
}

/// Whether nesting the service at `path` collides with a route or shadows it: the path
/// is a route, a wildcard route covers it, or it is the beginning of a route whose
/// part up to the next segment holds a variable part (the tail route of the nest
/// overlaps it, or the nest answers in place of a project or a token).
fn collides(path: &str) -> bool {
    let p: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    ROUTES.iter().any(|route| {
        let r: Vec<&str> = route.trim_start_matches('/').split('/').collect();
        let same = |a: &[&str], b: &[&str]| a.iter().zip(b).all(|(x, y)| is_var(y) || x == y);
        if let Some(i) = r.iter().position(|s| s.starts_with("{*")) {
            return p.len() > i && same(&p[..i], &r[..i]);
        }
        (p.len() == r.len() && same(&p, &r))
            || (p.len() < r.len()
                && same(&p, &r[..p.len()])
                && r[..=p.len()].iter().any(|s| is_var(s)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_examples_of_the_spec() {
        for p in ["/", "/health", "/exports", "/git", "/api/docs", "/app/settings", "/api/fs/roots"]
        {
            assert!(refusal(p).is_some(), "{p}");
        }
        for p in
            ["/git/foo", "/api/fs/foo", "/app/converters/default", "/exports/mcp", "/api/docs/x"]
        {
            assert!(refusal(p).is_some(), "{p}");
        }
        for p in ["/mcp", "/rpc", "/app", "/api", "/a/b", "/gitx", "/a-b_c.d~e/MCP"] {
            assert!(refusal(p).is_none(), "{p}");
        }
    }
}
