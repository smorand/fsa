//! SPEC-0118 shell, navigation and theme: E2E-001 to 020, 057, 062 to 067.

use super::testkit::{Kit, run};
use axum::http::StatusCode;

/// The seven screens.
pub const ALL: &[&str] = &[
    "/app/tokens",
    "/app/trash",
    "/app/deleted-projects",
    "/app/file-metadata",
    "/app/models",
    "/app/converters",
    "/app/settings",
];

fn links(body: &str) -> Vec<String> {
    let nav = body
        .split("<nav class=\"nav\"")
        .nth(1)
        .and_then(|s| s.split("</nav>").next())
        .unwrap_or("");
    nav.split("<a ")
        .skip(1)
        .filter_map(|a| a.split("href=\"").nth(1)?.split('"').next().map(str::to_string))
        .collect()
}

#[test]
fn e2e_0118_001_every_live_screen_has_the_shell() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        for u in ALL {
            let a = k.get(u, Some("root@t.c"), false).await;
            assert_eq!(a.status, StatusCode::OK, "{u}");
            for need in [
                "<!doctype html>",
                "<html lang=\"en\"",
                "class=\"shell\"",
                "class=\"sidebar\"",
                "class=\"topbar\"",
                "class=\"page-head\"",
                "class=\"content\"",
            ] {
                assert!(a.body.contains(need), "{u} lacks {need}: {}", a.body);
            }
        }
    });
}

#[test]
fn e2e_0118_002_head_links_embedded_assets_only() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let a = k.get("/app/trash", Some("root@t.c"), false).await;
        let head = a.body.split("</head>").next().unwrap();
        assert!(head.contains("href=\"/app/assets/css/tokens.css"), "{head}");
        assert!(head.contains("href=\"/app/assets/css/app.css"), "{head}");
        assert!(head.contains("<script src=\"/app/assets/js/app.js"), "{head}");
        let tail = a.body.split("</head>").nth(1).unwrap();
        assert!(
            tail.contains("/app/assets/js/icons-data.js")
                && tail.contains("/app/assets/js/icons.js")
        );
        assert!(!a.body.contains("http://") && !a.body.contains("https://"), "{}", a.body);
    });
}

#[test]
fn e2e_0118_003_topbar_shows_the_person() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        for u in ALL {
            let a = k.get(u, Some("root@t.c"), false).await;
            let bar =
                a.body.split("class=\"topbar\"").nth(1).unwrap().split("</header>").next().unwrap();
            assert!(bar.contains("root@t.c") && bar.contains("class=\"avatar"), "{u}: {bar}");
        }
    });
}

#[test]
fn e2e_0118_004_title_and_heading_name_the_screen() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        for (u, name) in [
            ("/app/tokens", "Git tokens"),
            ("/app/trash", "Trash"),
            ("/app/deleted-projects", "Deleted projects"),
            ("/app/file-metadata", "File metadata"),
            ("/app/models", "Models"),
            ("/app/converters", "Converters"),
            ("/app/settings", "Settings"),
        ] {
            let a = k.get(u, Some("root@t.c"), false).await;
            assert!(a.body.contains(&format!("<title>{name}")), "{u}");
            assert!(a.body.contains("<h1>"), "{u}");
        }
    });
}

#[test]
fn e2e_0118_005_no_identity_keeps_401_json() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        for u in ALL {
            let a = k.get(u, None, true).await;
            assert_eq!(a.status, StatusCode::UNAUTHORIZED, "{u}");
            assert!(!a.body.contains("<html"), "{u}");
            assert!(a.body.contains("\"error\""), "{u}");
        }
    });
}

#[test]
fn e2e_0118_008_009_077_no_inline_style_script_colour_or_confirm() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        k.project("p1", "root@t.c", &[]).await;
        for u in ALL {
            for url in [u.to_string(), format!("{u}?mount_id=p1")] {
                let a = k.get(&url, Some("root@t.c"), false).await;
                if a.status != StatusCode::OK {
                    continue;
                }
                let b = &a.body;
                assert!(!b.contains(" style=") && !b.contains("<style"), "{url}");
                assert!(!b.contains("<script>") && !b.contains("<script type"), "{url}");
                assert!(!b.contains("onclick=") && !b.contains("onsubmit="), "{url}");
                assert!(!b.contains("confirm("), "{url}");
                let bytes = b.as_bytes();
                let hex = (0..bytes.len()).any(|i| {
                    bytes[i] == b'#'
                        && bytes.len() >= i + 7
                        && bytes[i + 1..i + 7].iter().all(u8::is_ascii_hexdigit)
                        && !bytes[i + 7..].first().is_some_and(|c| c.is_ascii_alphanumeric())
                        && !b[..i].ends_with("href=\"")
                });
                assert!(!hex, "{url} has a hex colour");
            }
        }
    });
}

#[test]
fn e2e_0118_010_links_and_forms_work_without_script() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let a = k.get("/app/trash", Some("root@t.c"), false).await;
        for l in links(&a.body) {
            assert!(l.starts_with("/app/"), "{l}");
        }
        for form in a.body.split("<form").skip(1) {
            let tag = form.split('>').next().unwrap();
            assert!(tag.contains("method=\"post\"") && tag.contains("action=\""), "{tag}");
        }
    });
}

#[test]
fn e2e_0118_011_member_sees_four_links() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        k.project("p1", "bob@t.c", &["mem@t.c"]).await;
        let a = k.get("/app/trash", Some("mem@t.c"), false).await;
        assert_eq!(
            links(&a.body),
            ["/app/tokens", "/app/trash", "/app/deleted-projects", "/app/file-metadata"]
        );
    });
}

#[test]
fn e2e_0118_012_platform_admin_sees_every_screen() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        k.project("p1", "root@t.c", &[]).await;
        let a = k.get("/app/trash", Some("root@t.c"), false).await;
        assert_eq!(
            links(&a.body),
            [
                "/app/tokens",
                "/app/trash",
                "/app/deleted-projects",
                "/app/file-metadata",
                "/app/models",
                "/app/converters",
                "/app/settings"
            ]
        );
    });
}

#[test]
fn e2e_0118_013_owner_who_is_not_admin_sees_the_member_list() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        k.project("p1", "own@t.c", &[]).await;
        let a = k.get("/app/trash", Some("own@t.c"), false).await;
        assert_eq!(
            links(&a.body),
            ["/app/tokens", "/app/trash", "/app/deleted-projects", "/app/file-metadata"]
        );
    });
}

#[test]
fn e2e_0118_014_non_admin_has_no_admin_links() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let a = k.get("/app/trash", Some("x@t.c"), false).await;
        for l in links(&a.body) {
            assert!(
                !["/app/models", "/app/converters", "/app/settings"].contains(&l.as_str()),
                "{l}"
            );
        }
    });
}

#[test]
fn e2e_0118_015_person_with_no_project_has_no_file_metadata_link() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let a = k.get("/app/trash", Some("x@t.c"), false).await;
        assert_eq!(links(&a.body), ["/app/tokens", "/app/trash", "/app/deleted-projects"]);
    });
}

#[test]
fn e2e_0118_018_git_disabled_has_no_tokens_link_and_route() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        let a = k.get("/app/trash", Some("root@t.c"), false).await;
        assert!(!links(&a.body).contains(&"/app/tokens".to_string()));
        let t = k.get("/app/tokens", Some("root@t.c"), false).await;
        assert_eq!(t.status, StatusCode::NOT_FOUND);
    });
}

#[test]
fn e2e_0118_019_020_one_current_marker_on_the_screen_shown() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        k.project("p1", "root@t.c", &[]).await;
        for u in ALL {
            let a = k.get(u, Some("root@t.c"), false).await;
            let nav =
                a.body.split("<nav class=\"nav\"").nth(1).unwrap().split("</nav>").next().unwrap();
            assert_eq!(nav.matches("aria-current=\"page\"").count(), 1, "{u}");
            let current = nav.split("aria-current=\"page\"").next().unwrap();
            assert!(
                current.rsplit("<a ").next().unwrap().contains(&format!("href=\"{u}\"")),
                "{u}"
            );
            assert!(nav.contains("class=\"active\"") || nav.contains("active"), "{u}");
        }
    });
}

#[test]
fn e2e_0118_057_person_name_is_escaped_in_the_shell() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let who = "<b>x</b>@t.c";
        for u in ALL {
            let a = k.get(u, Some(who), false).await;
            assert!(!a.body.contains("<b>x</b>"), "{u}");
            assert!(a.body.contains("&lt;b&gt;x&lt;/b&gt;@t.c"), "{u}");
        }
    });
}

#[test]
fn e2e_0118_062_064_theme_script_is_in_the_head_and_set_by_the_asset() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let a = k.get("/app/trash", Some("root@t.c"), false).await;
        let (head, rest) = a.body.split_once("</head>").unwrap();
        assert!(head.contains("<script src=\"/app/assets/js/app.js"));
        assert!(!rest.contains("app.js"));
        assert!(!a.body.contains("data-theme"), "the asset sets the theme, not the page");
    });
}

#[test]
fn e2e_0118_063_065_066_067_theme_and_rail_live_in_the_assets() {
    let get = |f: &str| {
        String::from_utf8(super::assets::Assets::get(f).unwrap().data.into_owned()).unwrap()
    };
    let js = get("js/app.js");
    assert!(
        js.contains("localStorage.getItem('pyr-theme')")
            && js.contains("localStorage.setItem('pyr-theme'")
    );
    assert!(get("css/tokens.css").contains("[data-theme=\"dark\"]"));
    let css = get("css/app.css");
    assert!(css.contains(".shell.collapsed") && css.contains("max-width:900px"));
    assert!(js.contains("data-sidebar-toggle"));
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let a = k.get("/app/trash", Some("root@t.c"), false).await;
        assert!(a.body.contains("data-sidebar-toggle"));
    });
}
