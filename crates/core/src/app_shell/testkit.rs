//! Test harness for the screen tests of SPEC-0118: a built server, minted tokens, seeded
//! projects, plain request helpers.

use crate::config::ServerConfig;
use crate::keys;
use crate::state::AppState;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

/// What a request answered.
pub struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

pub struct Kit {
    pub app: Router,
    pub state: Arc<AppState>,
    key: PathBuf,
    _dir: tempfile::TempDir,
}

const REFERENCE_HOSTS: &[(&str, &str)] =
    &[("github.com", "github"), ("gitlab.acme.corp", "gitlab")];

/// Run an async test body under the git host lock, as the token screen tests do (the host
/// map is a process wide static).
pub fn run<F: std::future::Future>(f: F) -> F::Output {
    let _guard = crate::git::remote::tests::lock_for_test();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
}

impl Kit {
    /// A server with `admin` as platform administrator, git enabled or not.
    pub async fn new(admin: &str, git: bool) -> Kit {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let (key, public) = keys::write_keypair(root.join("keys")).unwrap();
        let mut c = ServerConfig::default();
        c.auth.jwt.public_key_path = public.display().to_string();
        c.infra.meta.dir = root.join("volumes").display().to_string();
        c.infra.blob.dir = root.join("blobs").display().to_string();
        c.infra.admin.path = root.join("admin.db").display().to_string();
        c.auth.admins = vec![admin.to_string()];
        c.git.enabled = git;
        if git {
            let mut cfg = crate::config::GitConfig::default();
            cfg.hosts.0 =
                REFERENCE_HOSTS.iter().map(|(h, p)| (h.to_string(), p.to_string())).collect();
            crate::git::remote::validate_hosts(&cfg).expect("a valid host map");
        }
        let (app, state) = crate::app::build_with_state(c).await.unwrap();
        Kit { app, state, key, _dir: dir }
    }

    pub fn token(&self, email: &str) -> String {
        keys::mint_token_from_file(
            &self.key,
            email,
            keys::DEFAULT_ISSUER,
            keys::DEFAULT_CLAIM,
            3600,
        )
        .unwrap()
    }

    /// A project owned by `owner`, with `members` added.
    pub async fn project(&self, id: &str, owner: &str, members: &[&str]) {
        self.state.admin.create_project(id, owner).await.unwrap();
        for m in members {
            self.state.admin.add_member(id, m, owner).await.unwrap();
        }
    }

    /// GET with a bearer token (or none), optionally asking for HTML.
    pub async fn get(&self, uri: &str, who: Option<&str>, html: bool) -> Answer {
        let mut b = Request::builder().method("GET").uri(uri);
        if let Some(w) = who {
            b = b.header("Authorization", format!("Bearer {}", self.token(w)));
        }
        if html {
            b = b.header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8");
        }
        self.send(b.body(Body::empty()).unwrap()).await
    }

    /// POST a form as the person's session cookie (so CSRF applies) or by bearer.
    pub async fn post(&self, uri: &str, who: &str, cookie: bool, form: &str, html: bool) -> Answer {
        let mut b = Request::builder()
            .method("POST")
            .uri(uri)
            .header("Content-Type", "application/x-www-form-urlencoded");
        b = if cookie {
            b.header("Cookie", format!("mcpfs_token={}", self.token(who)))
        } else {
            b.header("Authorization", format!("Bearer {}", self.token(who)))
        };
        if html {
            b = b.header("Accept", "text/html");
        }
        self.send(b.body(Body::from(form.to_string())).unwrap()).await
    }

    /// GET as the cookie session, so the page carries a one time token.
    pub async fn get_cookie(&self, uri: &str, who: &str) -> Answer {
        let b = Request::builder()
            .method("GET")
            .uri(uri)
            .header("Cookie", format!("mcpfs_token={}", self.token(who)));
        self.send(b.body(Body::empty()).unwrap()).await
    }

    /// POST a JSON body to the REST data plane as `who` (seeds files, deletes to the trash).
    pub async fn api(&self, uri: &str, who: &str, json: &str) -> Answer {
        let b = Request::builder()
            .method("POST")
            .uri(uri)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.token(who)));
        self.send(b.body(Body::from(json.to_string())).unwrap()).await
    }

    /// A file written then deleted, so it sits in the trash of `project`.
    pub async fn trashed(&self, project: &str, who: &str, path: &str) {
        let w = format!(
            "{{\"path\":{path:?},\"content\":\"x\",\"overwrite\":true,\"create_parents\":true}}"
        );
        assert_eq!(
            self.api(&format!("/api/fs/{project}/write"), who, &w).await.status,
            StatusCode::OK
        );
        let d = format!("{{\"path\":{path:?},\"recursive\":false}}");
        assert_eq!(
            self.api(&format!("/api/fs/{project}/delete"), who, &d).await.status,
            StatusCode::OK
        );
    }

    pub async fn send(&self, req: Request<Body>) -> Answer {
        let r = self.app.clone().oneshot(req).await.unwrap();
        let status = r.status();
        let headers = r.headers().clone();
        let bytes = to_bytes(r.into_body(), usize::MAX).await.unwrap();
        Answer { status, headers, body: String::from_utf8_lossy(&bytes).into_owned() }
    }
}

/// The `csrf_token` value the page renders first.
pub fn csrf_of(body: &str) -> String {
    body.split("name=\"csrf_token\" value=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("a rendered csrf_token")
        .to_string()
}
