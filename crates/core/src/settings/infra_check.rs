//! The checks of a change of the `infra` section made before it is saved (SPEC-0117):
//! the new places are probed, and a place the data lives in must already hold what the
//! current place holds. Nothing here copies anything (hard stop S-90): a probe opens a
//! store and applies its schema, as any open does, and the comparisons only read.

use crate::config::{ServerConfig, backend};
use crate::errors::{Result, ToolError};
use crate::migrate::{count_rows, shas_of};
use crate::storage::RelationalRegistry;
use crate::storage::blob::s3::S3BlobStore;
use crate::storage::traits::BlobBackend;
use std::cell::Cell;
use std::time::Duration;

thread_local! {
    /// How long a blob probe or an existence request waits, in milliseconds. A cell of
    /// the thread so a test lowers it for itself only; production never changes it.
    pub(crate) static PROBE_TIMEOUT_MS: Cell<u64> = const { Cell::new(15_000) };
    /// How many stores were probed on this thread (test observation).
    static PROBES: Cell<usize> = const { Cell::new(0) };
}

/// How many stores this thread probed so far.
#[cfg(test)]
pub(crate) fn probes() -> usize {
    PROBES.with(Cell::get)
}

fn probed() {
    PROBES.with(|p| p.set(p.get() + 1));
}

const HOLD: &str = "the server never moves data itself";
const REDACTED: &str = "<redacted>";

/// `text` with every secret, and the password inside a DSN, replaced.
pub(crate) fn redact(text: &str, secrets: &[String]) -> String {
    let mut all: Vec<String> = Vec::new();
    for s in secrets.iter().filter(|s| !s.is_empty()) {
        all.push(s.clone());
        all.extend(dsn_passwords(s));
    }
    all.sort_by_key(|s| std::cmp::Reverse(s.len()));
    all.dedup();
    let mut out = text.to_string();
    for s in all {
        out = out.replace(&s, REDACTED);
    }
    out
}

/// The passwords inside a DSN, as typed and percent decoded: the one of
/// `scheme://user:password@host/...` and a `password=` parameter of its query.
fn dsn_passwords(dsn: &str) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(rest) = dsn.split_once("://").map(|p| p.1) {
        if let Some(userinfo) = rest.split_once('@').map(|p| p.0)
            && let Some(password) = userinfo.split_once(':').map(|p| p.1)
        {
            found.push(password.to_string());
        }
        if let Some(query) = rest.split_once('?').map(|p| p.1) {
            for pair in query.split('&') {
                if let Some(v) = pair.strip_prefix("password=") {
                    found.push(v.to_string());
                }
            }
        }
    }
    let decoded: Vec<String> = found.iter().map(|p| percent_decode(p)).collect();
    found.extend(decoded);
    found.retain(|p| !p.is_empty());
    found
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn secrets_of(c: &ServerConfig) -> Vec<String> {
    let i = &c.infra;
    vec![
        i.blob.access_key.clone(),
        i.blob.secret_key.clone(),
        i.meta.dsn.expose().to_string(),
        i.git.dsn.expose().to_string(),
        i.oauth.dsn.expose().to_string(),
    ]
}

#[derive(Clone, Copy, PartialEq)]
enum Store {
    Meta,
    Git,
    Oauth,
}

impl Store {
    fn name(self) -> &'static str {
        match self {
            Store::Meta => "meta",
            Store::Git => "git",
            Store::Oauth => "oauth",
        }
    }

    /// Where the store lives: the engine and its address (FR-NEW-005).
    fn place(self, c: &ServerConfig) -> String {
        let i = &c.infra;
        let (engine, dsn, schema) = match self {
            Store::Meta => (&i.meta.backend, &i.meta.dsn, &i.meta.schema),
            Store::Git => (&i.git.backend, &i.git.dsn, &i.git.schema),
            Store::Oauth => (&i.oauth.backend, &i.oauth.dsn, &i.oauth.schema),
        };
        if engine == backend::POSTGRES {
            return format!("postgres|{}|{schema}", dsn.expose());
        }
        match self {
            Store::Meta => format!("sqlite|{}", i.meta.dir),
            _ => format!("sqlite|{}", c.state_root().display()),
        }
    }

    fn is_sqlite(self, c: &ServerConfig) -> bool {
        let engine = match self {
            Store::Meta => &c.infra.meta.backend,
            Store::Git => &c.infra.git.backend,
            Store::Oauth => &c.infra.oauth.backend,
        };
        engine != backend::POSTGRES
    }
}

/// Probes the places that differ from the running ones, then compares their data.
/// `projects` are the ids the admin store holds, in ascending order.
pub(crate) async fn run(
    running: &ServerConfig,
    candidate: &ServerConfig,
    projects: &[String],
) -> Result<()> {
    let secrets: Vec<String> = [secrets_of(running), secrets_of(candidate)].concat();
    // Whatever failed, no DSN or key leaves in its text.
    let hide = |mut e: ToolError| {
        e.message = redact(&e.message, &secrets);
        e
    };
    for store in [Store::Meta, Store::Git, Store::Oauth] {
        if store.place(running) != store.place(candidate) {
            relational(store, running, candidate, projects, &secrets).await.map_err(hide)?;
        }
    }
    blob(running, candidate, projects, &secrets).await.map_err(hide)
}

fn unreachable(store: Store, e: impl std::fmt::Display, secrets: &[String]) -> ToolError {
    ToolError::invalid_argument(format!(
        "infra.{} cannot be reached: {}",
        store.name(),
        redact(&e.to_string(), secrets)
    ))
}

/// Not an internal error: the settings retry those, and a store that cannot be read stays
/// unreadable.
fn unreadable(store: Store, e: impl std::fmt::Display, secrets: &[String]) -> ToolError {
    ToolError::invalid_argument(format!(
        "the current infra.{} store cannot be read: {}",
        store.name(),
        redact(&e.to_string(), secrets)
    ))
}

fn not_held(store: Store, what: String) -> ToolError {
    ToolError::invalid_argument(format!(
        "infra.{} points at a store that does not hold the data of the current one ({what}): \
         run fsa migrate --from <current config> --to <new config> first, {HOLD}",
        store.name()
    ))
}

/// `Some("<table> a here, b there")` when the counts differ.
fn differ(table: &str, here: i64, there: i64) -> Option<String> {
    (here != there).then(|| format!("{table} {here} here, {there} there"))
}

async fn relational(
    store: Store,
    running: &ServerConfig,
    candidate: &ServerConfig,
    projects: &[String],
    secrets: &[String],
) -> Result<()> {
    probed();
    if store.is_sqlite(candidate) {
        let place = match store {
            Store::Meta => std::path::PathBuf::from(&candidate.infra.meta.dir),
            _ => candidate.state_root(),
        };
        std::fs::create_dir_all(&place).map_err(|e| unreachable(store, e, secrets))?;
    }
    match store {
        Store::Oauth => {
            let (old, new) = (RelationalRegistry::new(), RelationalRegistry::new());
            let schema = crate::git::oauth::persistence::schema();
            let src = crate::storage::open_oauth_db(running, &old)
                .await
                .map_err(|e| unreadable(store, e.message, secrets))?;
            let dst = crate::storage::open_oauth_db(candidate, &new)
                .await
                .map_err(|e| unreachable(store, e.message, secrets))?;
            dst.migrate(&schema).await.map_err(|e| unreachable(store, e.message, secrets))?;
            src.migrate(&schema).await.map_err(|e| unreadable(store, e.message, secrets))?;
            let (here, there) = (
                count_rows(&*src, "oauth_tokens", None).await?,
                count_rows(&*dst, "oauth_tokens", None).await?,
            );
            if let Some(what) = differ("oauth_tokens", here, there) {
                return Err(not_held(store, what));
            }
        }
        Store::Meta | Store::Git => {
            if projects.is_empty() && !store.is_sqlite(candidate) {
                // One open proves the server answers and the schema applies.
                let new = RelationalRegistry::new();
                let dst = open_one(store, candidate, &new, "probe")
                    .await
                    .map_err(|e| unreachable(store, e.message, secrets))?;
                apply(store, &*dst).await.map_err(|e| unreachable(store, e.message, secrets))?;
            }
            for p in projects {
                // One pair of pools at a time: a server with many projects keeps few
                // connections open while it is probed.
                let (old, new) = (RelationalRegistry::new(), RelationalRegistry::new());
                let dst = open_one(store, candidate, &new, p)
                    .await
                    .map_err(|e| unreachable(store, e.message, secrets))?;
                apply(store, &*dst).await.map_err(|e| unreachable(store, e.message, secrets))?;
                let src = open_one(store, running, &old, p)
                    .await
                    .map_err(|e| unreadable(store, e.message, secrets))?;
                apply(store, &*src).await.map_err(|e| unreadable(store, e.message, secrets))?;
                compare(store, p, &*src, &*dst).await?;
            }
        }
    }
    Ok(())
}

async fn open_one(
    store: Store,
    config: &ServerConfig,
    registry: &RelationalRegistry,
    project: &str,
) -> Result<std::sync::Arc<dyn crate::storage::rel::RelationalDb>> {
    match store {
        Store::Meta => crate::storage::open_meta_db(config, registry, project).await,
        _ => crate::storage::open_git_db(config, registry, project).await,
    }
}

async fn apply(store: Store, db: &dyn crate::storage::rel::RelationalDb) -> Result<()> {
    match store {
        Store::Meta => db.migrate(&crate::storage::meta::schema()).await,
        _ => db.migrate(&crate::git::db::schema()).await,
    }
}

/// The first difference of one project's rows, in the order of FR-NEW-006.
async fn compare(
    store: Store,
    project: &str,
    src: &dyn crate::storage::rel::RelationalDb,
    dst: &dyn crate::storage::rel::RelationalDb,
) -> Result<()> {
    let tables: &[&str] =
        if store == Store::Meta { &["nodes", "blob_refs"] } else { &crate::git::db::TABLES };
    for table in tables {
        let here = count_rows(src, table, Some(project)).await?;
        let there = count_rows(dst, table, Some(project)).await?;
        if let Some(what) = differ(table, here, there) {
            return Err(not_held(store, format!("project '{project}': {what}")));
        }
    }
    if store == Store::Meta {
        let here = shas_of(src, project).await?;
        let there = shas_of(dst, project).await?;
        if here != there {
            let missing = here.difference(&there).count();
            let what = if missing > 0 {
                format!("project '{project}': {missing} content addresses missing there")
            } else {
                format!("project '{project}': the content addresses differ")
            };
            return Err(not_held(store, what));
        }
    }
    Ok(())
}

fn object_store(b: &str) -> bool {
    b == "minio" || b == "s3"
}

async fn blob(
    running: &ServerConfig,
    candidate: &ServerConfig,
    projects: &[String],
    secrets: &[String],
) -> Result<()> {
    let (old, new) = (&running.infra.blob, &candidate.infra.blob);
    let backend_moved = old.backend != new.backend;
    let moved = backend_moved
        || if object_store(&new.backend) {
            old.endpoint != new.endpoint
                || old.bucket_prefix != new.bucket_prefix
                || old.region != new.region
        } else {
            old.dir != new.dir
        };
    let keys_changed = object_store(&new.backend)
        && (old.access_key != new.access_key || old.secret_key != new.secret_key);
    if !moved && !keys_changed {
        return Ok(());
    }
    probed();
    let unusable = |e: String| {
        ToolError::invalid_argument(format!("infra.blob cannot be used: {}", redact(&e, secrets)))
    };
    let wait = Duration::from_millis(PROBE_TIMEOUT_MS.with(Cell::get));
    if object_store(&new.backend) {
        let probe = S3BlobStore::new(new, format!("{}probe", new.bucket_prefix))
            .map_err(|e| unusable(e.message))?;
        match tokio::time::timeout(wait, probe.exists("fsa-probe")).await {
            Err(_) => return Err(unusable("timed out".into())),
            Ok(Err(e)) => return Err(unusable(e.message)),
            Ok(Ok(_)) => {}
        }
    } else {
        std::fs::create_dir_all(&new.dir).map_err(|e| unusable(e.to_string()))?;
    }
    if !moved {
        return Ok(());
    }
    let (mut missing, mut total) = (0usize, 0usize);
    let registry = RelationalRegistry::new();
    for p in projects {
        let db = crate::storage::open_meta_db(running, &registry, p)
            .await
            .map_err(|e| unreadable(Store::Meta, e.message, secrets))?;
        db.migrate(&crate::storage::meta::schema())
            .await
            .map_err(|e| unreadable(Store::Meta, e.message, secrets))?;
        let shas = shas_of(&*db, p).await?;
        let target =
            crate::storage::build_blob_store(candidate, p).map_err(|e| unusable(e.message))?;
        for sha in shas {
            total += 1;
            match tokio::time::timeout(wait, target.exists(&sha)).await {
                Err(_) => return Err(unusable("timed out".into())),
                Ok(Err(e)) => return Err(unusable(e.message)),
                Ok(Ok(false)) => missing += 1,
                Ok(Ok(true)) => {}
            }
        }
    }
    if missing > 0 {
        return Err(ToolError::invalid_argument(format!(
            "infra.blob points at a store that does not hold {missing} of {total} blobs of the \
             current one: copy them first (fsa migrate --from <current config> --to <new \
             config> lists the missing ones), {HOLD}"
        )));
    }
    Ok(())
}
