//! SPEC-0118 assets: E2E-021 to 032.

use super::testkit::{Kit, run};
use axum::http::StatusCode;

const FILES: &[&str] = &[
    "css/tokens.css",
    "css/app.css",
    "js/app.js",
    "js/icons-data.js",
    "js/icons.js",
    "logo.svg",
    "sprite.svg",
];

fn header<'a>(a: &'a super::testkit::Answer, name: &str) -> &'a str {
    a.headers.get(name).and_then(|v| v.to_str().ok()).unwrap_or("")
}

#[test]
fn e2e_0118_021_css_is_served_with_type_cache_and_nosniff() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        let a = k.get("/app/assets/css/tokens.css", None, false).await;
        assert_eq!(a.status, StatusCode::OK);
        assert!(header(&a, "content-type").starts_with("text/css"));
        assert_eq!(header(&a, "cache-control"), "public, max-age=3600");
        assert_eq!(header(&a, "x-content-type-options"), "nosniff");
        assert!(a.body.contains("--primary"));
    });
}

#[test]
fn e2e_0118_022_script_is_served_as_javascript() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        let a = k.get("/app/assets/js/app.js", None, false).await;
        assert_eq!(a.status, StatusCode::OK);
        assert!(header(&a, "content-type").starts_with("text/javascript"));
    });
}

#[test]
fn e2e_0118_023_sprite_is_served_as_svg() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        let a = k.get("/app/assets/sprite.svg", None, false).await;
        assert_eq!(a.status, StatusCode::OK);
        assert_eq!(header(&a, "content-type"), "image/svg+xml");
    });
}

#[test]
fn e2e_0118_024_unknown_asset_is_404() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        let a = k.get("/app/assets/css/nope.css", None, false).await;
        assert_eq!(a.status, StatusCode::NOT_FOUND);
    });
}

#[test]
fn e2e_0118_025_to_028_traversal_is_404() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        for uri in [
            "/app/assets/../Cargo.toml",
            "/app/assets/css/../../../Cargo.toml",
            "/app/assets/%2e%2e/Cargo.toml",
            "/app/assets/css/%2e%2e/%2e%2e/Cargo.toml",
            "/app/assets//css/app.css",
            "/app/assets/css//app.css",
            "/app/assets/css%5capp.css",
            "/app/assets/css\\app.css",
            "/app/assets/%2fetc/passwd",
        ] {
            let a = k.get(uri, None, false).await;
            assert_eq!(a.status, StatusCode::NOT_FOUND, "{uri}");
        }
    });
}

#[test]
fn e2e_0118_029_embedded_copies_equal_the_manifest_and_the_design_files() {
    use sha2::{Digest, Sha256};
    let manifest =
        String::from_utf8(super::assets::Assets::get("MANIFEST.sha256").unwrap().data.into_owned())
            .unwrap();
    for f in FILES {
        let got = super::assets::Assets::get(f).unwrap_or_else(|| panic!("{f} not embedded"));
        let sum = hex::encode(Sha256::digest(got.data.as_ref()));
        assert!(
            manifest.contains(&format!("{sum}  {f}")),
            "{f} differs from MANIFEST.sha256: run scripts/sync-design.sh"
        );
    }
    let dir = std::env::var("APP_DESIGN_DIR").unwrap_or_else(|_| {
        format!("{}/projects/perso/app-design", std::env::var("HOME").unwrap())
    });
    let root = std::path::Path::new(&dir).join("assets");
    if !root.is_dir() {
        eprintln!("skipped: no design folder at {}", root.display());
        return;
    }
    for f in FILES {
        let want = std::fs::read(root.join(f)).unwrap();
        let got = super::assets::Assets::get(f).unwrap_or_else(|| panic!("{f} not embedded"));
        assert_eq!(got.data.as_ref(), want.as_slice(), "{f} drifted: run scripts/sync-design.sh");
    }
}

#[test]
fn e2e_0118_030_every_file_is_embedded_and_served_with_no_web_folder_beside() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        // `debug-embed` makes this build read the table in the binary, not the folder: the
        // bytes below come from memory, whatever the working directory holds.
        let mut seen = Vec::new();
        for f in FILES {
            assert!(super::assets::Assets::get(f).is_some(), "{f}");
            let a = k.get(&format!("/app/assets/{f}"), None, false).await;
            seen.push((a.status, a.body.len()));
        }
        assert!(seen.iter().all(|(s, n)| *s == StatusCode::OK && *n > 0), "{seen:?}");
    });
}

#[test]
fn e2e_0118_031_assets_need_no_identity() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let a = k.get("/app/assets/css/app.css", None, false).await;
        assert_eq!(a.status, StatusCode::OK);
    });
}

#[test]
fn e2e_0118_032_sync_script_is_idempotent_and_refuses_a_missing_source() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = root.join("scripts/sync-design.sh");
    let missing = std::process::Command::new(&script)
        .env("APP_DESIGN_DIR", "/nonexistent-design-dir")
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("app-design not found"));
    let bad = std::process::Command::new(&script).arg("--nope").output().unwrap();
    assert_eq!(bad.status.code(), Some(2));
    let design = std::env::var("APP_DESIGN_DIR").unwrap_or_else(|_| {
        format!("{}/projects/perso/app-design", std::env::var("HOME").unwrap())
    });
    if !std::path::Path::new(&design).join("assets").is_dir() {
        return;
    }
    let before = std::fs::read(root.join("crates/core/web/assets/css/app.css")).unwrap();
    for _ in 0..2 {
        assert!(std::process::Command::new(&script).status().unwrap().success());
    }
    let after = std::fs::read(root.join("crates/core/web/assets/css/app.css")).unwrap();
    assert_eq!(before, after);
}
