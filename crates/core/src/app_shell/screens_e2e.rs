//! SPEC-0118 screens: E2E-006, 007, 016, 017, 033 to 061, 068 to 077.

use super::testkit::{Kit, csrf_of, run};
use axum::http::StatusCode;

const GHP: &str = "ghp_AAAABBBBCCCCDDDDEEEEFFFFGGGGHHHH";

#[test]
fn e2e_0118_033_034_076_tokens_list_in_design_and_remove_goes_through_a_modal() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let seeded = k
            .post("/app/tokens", "root@t.c", false, &format!("host=github.com&token={GHP}"), false)
            .await;
        assert_eq!(seeded.status, StatusCode::SEE_OTHER);
        let a = k.get("/app/tokens", Some("root@t.c"), false).await;
        assert!(
            a.body.contains("class=\"card\"") && a.body.contains("class=\"table-wrap\""),
            "{}",
            a.body
        );
        assert!(a.body.contains("class=\"btn primary\""));
        assert!(
            a.body.contains("data-open=\"m-remove-0\"") && a.body.contains("id=\"m-remove-0\"")
        );
        assert!(a.body.contains("action=\"/app/tokens/revoke\""));
        let r = k.post("/app/tokens/revoke", "root@t.c", false, "host=github.com", false).await;
        assert_eq!(r.status, StatusCode::SEE_OTHER);
    });
}

#[test]
fn e2e_0118_035_036_trash_rows_in_design_and_restore_redirects() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        k.project("p1", "root@t.c", &[]).await;
        k.trashed("p1", "root@t.c", "/a.txt").await;
        let a = k.get("/app/trash?mount_id=p1", Some("root@t.c"), false).await;
        assert!(
            a.body.contains("class=\"table-wrap\"") && a.body.contains("class=\"btn sm\">Restore"),
            "{}",
            a.body
        );
        let page = k.get_cookie("/app/trash?mount_id=p1", "root@t.c").await;
        let tp = page
            .body
            .split("name=\"trash_path\" value=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string();
        let form = format!("csrf_token={}&mount_id=p1&trash_path={tp}", csrf_of(&page.body));
        let r = k.post("/app/trash/restore", "root@t.c", true, &form, false).await;
        assert_eq!(r.status, StatusCode::SEE_OTHER);
    });
}

#[test]
fn e2e_0118_037_038_deleted_projects_in_design_and_undelete_redirects() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        k.project("p1", "root@t.c", &[]).await;
        assert!(k.state.admin.soft_delete_project("p1").await.unwrap());
        let a = k.get("/app/deleted-projects", Some("root@t.c"), false).await;
        assert!(
            a.body.contains("class=\"table-wrap\"") && a.body.contains("class=\"btn sm\">Undelete"),
            "{}",
            a.body
        );
        let r = k
            .post("/app/deleted-projects/undelete", "root@t.c", false, "project_id=p1", false)
            .await;
        assert_eq!(r.status, StatusCode::SEE_OTHER);
    });
}

#[test]
fn e2e_0118_047_048_049_empty_states() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        k.project("p1", "root@t.c", &[]).await;
        for u in ["/app/trash?mount_id=p1", "/app/deleted-projects", "/app/tokens"] {
            let a = k.get(u, Some("root@t.c"), false).await;
            assert!(a.body.contains("class=\"empty\""), "{u}: {}", a.body);
        }
    });
}

#[test]
fn e2e_0118_051_056_a_refusal_shows_an_alert_to_a_browser_and_json_otherwise() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        k.project("p1", "bob@t.c", &[]).await;
        let json = k.get("/app/trash?mount_id=p1", Some("eve@t.c"), false).await;
        assert!(
            json.status == StatusCode::FORBIDDEN || json.status == StatusCode::NOT_FOUND,
            "{}",
            json.status
        );
        assert!(json.body.contains("\"detail\""));
        let page = k.get("/app/trash?mount_id=p1", Some("eve@t.c"), true).await;
        assert_eq!(page.status, json.status);
        assert!(
            page.body.contains("class=\"alert danger\"") && page.body.contains("class=\"shell\"")
        );
        let detail =
            json.body.split("\"detail\":\"").nth(1).unwrap().split('"').next().unwrap().to_string();
        assert!(page.body.contains(&crate::token_screen::escape_html(&detail)), "{detail}");
        let t = k.post("/app/tokens", "root@t.c", false, "host=nope.example&token=x", true).await;
        assert!(t.status.is_client_error());
        assert!(t.body.contains("class=\"alert danger\""), "{}", t.body);
    });
}

#[test]
fn e2e_0118_006_072_a_bad_token_keeps_403_json() {
    run(async {
        let k = Kit::new("root@t.c", true).await;
        let r = k
            .post("/app/tokens/revoke", "root@t.c", true, "host=github.com&csrf_token=nope", true)
            .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN);
        assert!(r.body.contains("missing or invalid csrf_token") && !r.body.contains("<html"));
    });
}

#[test]
fn e2e_0118_058_060_markup_in_a_project_id_and_a_trash_path_stays_escaped() {
    run(async {
        let k = Kit::new("root@t.c", false).await;
        k.project("p1", "root@t.c", &[]).await;
        k.trashed("p1", "root@t.c", "/<b>x</b>.txt").await;
        let a = k.get("/app/trash?mount_id=p1", Some("root@t.c"), false).await;
        assert!(
            !a.body.contains("<b>x</b>") && a.body.contains("&lt;b&gt;x&lt;/b&gt;.txt"),
            "{}",
            a.body
        );
    });
}

// ── models, converters, settings, file metadata, modals ──────────────────────

const ADMIN: &str = "root@t.c";

async fn with_provider(k: &Kit, name: &str) -> String {
    let form = format!("name={name}&endpoint=http%3A%2F%2Flocalhost%3A9010%2Fv1&auth_scheme=none");
    let r = k.post("/app/models/provider/create", ADMIN, false, &form, false).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
    let page = k.get("/app/models", Some(ADMIN), false).await;
    page.body.split("data-provider=\"").nth(1).unwrap().split('"').next().unwrap().to_string()
}

async fn with_model(k: &Kit, provider: &str, name: &str) {
    let form = format!("provider_id={provider}&name={name}&kind=chat");
    let r = k.post("/app/models/model/create", ADMIN, false, &form, false).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
}

#[test]
fn e2e_0118_007_016_017_a_non_admin_is_refused_on_the_admin_screens_as_before() {
    run(async {
        let k = Kit::new(ADMIN, true).await;
        k.project("p1", "bob@t.c", &[]).await;
        let want: &[(&str, &str, StatusCode, StatusCode)] = &[
            ("/app/tokens", "bob@t.c", StatusCode::OK, StatusCode::OK),
            ("/app/trash", "bob@t.c", StatusCode::OK, StatusCode::OK),
            ("/app/deleted-projects", "bob@t.c", StatusCode::OK, StatusCode::OK),
            ("/app/file-metadata", "bob@t.c", StatusCode::OK, StatusCode::OK),
            ("/app/models", "bob@t.c", StatusCode::FORBIDDEN, StatusCode::OK),
            ("/app/converters", "bob@t.c", StatusCode::FORBIDDEN, StatusCode::OK),
            ("/app/settings", "bob@t.c", StatusCode::FORBIDDEN, StatusCode::OK),
        ];
        for (u, who, plain, _) in want {
            assert_eq!(k.get(u, Some(who), false).await.status, *plain, "{u}");
            assert_eq!(k.get(u, Some(ADMIN), false).await.status, StatusCode::OK, "{u} admin");
        }
        let a = k.get("/app/models", Some("bob@t.c"), true).await;
        assert_eq!(a.status, StatusCode::FORBIDDEN);
        assert!(a.body.contains("class=\"alert danger\"") && a.body.contains("class=\"shell\""));
        assert!(
            a.body.contains("ERR_FORBIDDEN: &#39;bob@t.c&#39; is not a platform admin")
                || a.body.contains("is not a platform admin"),
            "{}",
            a.body
        );
    });
}

#[test]
fn e2e_0118_039_040_050_models_in_cards_with_an_empty_state_then_a_create_redirect() {
    run(async {
        let k = Kit::new(ADMIN, false).await;
        let empty = k.get("/app/models", Some(ADMIN), false).await;
        assert!(empty.body.contains("class=\"empty\"") && empty.body.contains("No provider"));
        let pid = with_provider(&k, "Local").await;
        with_model(&k, &pid, "m1").await;
        let a = k.get("/app/models", Some(ADMIN), false).await;
        assert!(
            a.body.contains("class=\"card\"") && a.body.contains("class=\"table-wrap\""),
            "{}",
            a.body
        );
        assert!(a.body.contains("class=\"btn primary\">Add model"));
        assert!(a.body.contains("class=\"input\""));
    });
}

#[test]
fn e2e_0118_041_042_053_converters_in_cards_empty_state_and_rejected_form() {
    run(async {
        let k = Kit::new(ADMIN, false).await;
        let empty = k.get("/app/converters", Some(ADMIN), false).await;
        assert!(
            empty.body.contains("class=\"empty\"") && empty.body.contains("No converter"),
            "{}",
            empty.body
        );
        assert!(empty.body.contains("class=\"card\""));
        let bad = k
            .post("/app/converters/converter/create", ADMIN, false, "name=&form=program", true)
            .await;
        assert!(bad.status.is_client_error(), "{}", bad.status);
        assert!(
            bad.body.contains("class=\"alert danger\"") && bad.body.contains("class=\"shell\""),
            "{}",
            bad.body
        );
        let ok = k
            .post(
                "/app/converters/converter/create",
                ADMIN,
                false,
                "name=c1&form=program&command=cat%20%7Bdocument%7D",
                false,
            )
            .await;
        assert_eq!(ok.status, StatusCode::SEE_OTHER, "{}", ok.body);
        let a = k.get("/app/converters", Some(ADMIN), false).await;
        assert!(
            a.body.contains("data-converter=") && a.body.contains("data-open=\"m-converter-"),
            "{}",
            a.body
        );
    });
}

#[test]
fn e2e_0118_043_044_054_settings_vtabs_and_rejected_change() {
    run(async {
        let k = Kit::new(ADMIN, false).await;
        let a = k.get("/app/settings", Some(ADMIN), false).await;
        assert!(
            a.body.contains("class=\"vtabs\"") && a.body.contains("data-tabs=\"st\""),
            "{}",
            a.body
        );
        let tabs = a.body.matches("<button type=\"button\" class=\"tab").count();
        let panels = a.body.matches("<section class=\"panel").count();
        assert!(tabs >= 14 && tabs == panels, "{tabs} {panels}");
        assert_eq!(a.body.matches("tab active").count(), 1);
        let bad =
            k.post("/app/settings/set", ADMIN, false, "section=nope&value=%7B%7D", false).await;
        assert!(bad.status.is_client_error());
        assert!(bad.body.contains("class=\"alert danger\""), "{}", bad.body);
    });
}

#[test]
fn e2e_0118_045_046_055_file_metadata_picker_card_and_alert() {
    run(async {
        let k = Kit::new(ADMIN, false).await;
        k.project("p1", ADMIN, &[]).await;
        let a = k.get("/app/file-metadata", Some(ADMIN), false).await;
        assert!(
            a.body.contains("class=\"card\"")
                && a.body.contains("<select class=\"select\" name=\"mount_id\">"),
            "{}",
            a.body
        );
        let w =
            "{\"path\":\"/a.txt\",\"content\":\"x\",\"overwrite\":true,\"create_parents\":true}";
        assert_eq!(k.api("/api/fs/p1/write", ADMIN, w).await.status, StatusCode::OK);
        let page = k.get_cookie("/app/file-metadata?mount_id=p1&path=%2Fa.txt", ADMIN).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        let form = format!(
            "csrf_token={}&mount_id=p1&path=%2Fa.txt&kind=search_index",
            csrf_of(&page.body)
        );
        let r = k.post("/app/file-metadata/recalculate", ADMIN, true, &form, false).await;
        // Search is off in this server: the refusal is the shell with the alert, status kept.
        assert_eq!(r.status, StatusCode::NOT_IMPLEMENTED, "{}", r.body);
        assert!(
            r.body.contains("class=\"alert danger\"") && r.body.contains("search is not enabled"),
            "{}",
            r.body
        );
        assert!(
            page.body.contains("data-section=\"text\"") && page.body.contains("class=\"card\"")
        );
        let nope =
            k.get("/app/file-metadata?mount_id=nope&path=%2Fa.txt", Some(ADMIN), false).await;
        assert!(nope.status.is_client_error());
        assert!(
            nope.body.contains("class=\"alert danger\"") && nope.body.contains("class=\"shell\""),
            "{}",
            nope.body
        );
    });
}

#[test]
fn e2e_0118_061_074_markup_in_a_model_name_is_escaped_in_the_page_and_the_modal() {
    run(async {
        let k = Kit::new(ADMIN, false).await;
        let pid = with_provider(&k, "Local").await;
        with_model(&k, &pid, "%3Cb%3Ex%3C%2Fb%3E").await;
        let a = k.get("/app/models", Some(ADMIN), false).await;
        assert!(!a.body.contains("<b>x</b>"), "{}", a.body);
        assert!(a.body.contains("The model &lt;b&gt;x&lt;/b&gt; is deleted."));
    });
}

#[test]
fn e2e_0118_068_to_075_the_confirmation_modal() {
    run(async {
        let k = Kit::new(ADMIN, false).await;
        let pid = with_provider(&k, "Local").await;
        with_model(&k, &pid, "m1").await;
        with_model(&k, &pid, "m2").await;
        let page = k.get_cookie("/app/models", ADMIN).await;
        let b = &page.body;
        // 068: trigger, overlay and modal.
        assert!(
            b.contains("data-open=\"m-model-")
                && b.contains("<div class=\"overlay\" id=\"m-model-")
        );
        assert!(b.contains(
            "<div class=\"modal\"><form method=\"post\" action=\"/app/models/model/delete\">"
        ));
        // 069 and 075: cancel and trigger never submit.
        assert!(b.contains("<button type=\"button\" class=\"btn\" data-close>Cancel</button>"));
        assert!(!b.contains("<button class=\"btn sm danger\" data-open"));
        assert!(b.contains("<button type=\"button\" class=\"btn sm danger\" data-open="));
        // 070: every form of the page carries the one token.
        let token = csrf_of(b);
        let all: Vec<&str> = b
            .split("name=\"csrf_token\" value=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap())
            .collect();
        assert!(all.len() > 4 && all.iter().all(|t| *t == token));
        // 073: ids distinct.
        let mut ids: Vec<&str> = b
            .split("<div class=\"overlay\" id=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap())
            .collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(n, ids.len());
        // 071: the form posts as it did without script; the nojs sheet is linked and served.
        assert!(
            b.contains("<noscript><link rel=\"stylesheet\" href=\"/app/assets/css/nojs.css\">")
        );
        let css = k.get("/app/assets/css/nojs.css", None, false).await;
        assert!(
            css.body.contains("[data-open],[data-close]{display:none}")
                && css.body.contains(".panel{display:block}")
        );
        let mid = b.split("data-model=\"").nth(1).unwrap().split('"').next().unwrap();
        let good = k
            .post(
                "/app/models/model/delete",
                ADMIN,
                true,
                &format!("csrf_token={token}&model_id={mid}"),
                false,
            )
            .await;
        assert_eq!(good.status, StatusCode::SEE_OTHER);
        // 072: the same token again is refused with JSON.
        let again = k
            .post(
                "/app/models/model/delete",
                ADMIN,
                true,
                &format!("csrf_token={token}&model_id={mid}"),
                true,
            )
            .await;
        assert_eq!(again.status, StatusCode::FORBIDDEN);
        assert!(
            again.body.contains("missing or invalid csrf_token") && !again.body.contains("<html")
        );
    });
}

#[test]
fn e2e_0118_027b_no_dash_as_punctuation_in_the_shell_texts() {
    run(async {
        let k = Kit::new(ADMIN, true).await;
        let a = k.get("/app/trash", Some(ADMIN), false).await;
        let text: String = a
            .body
            .split('<')
            .filter_map(|t| t.split_once('>').map(|(_, x)| x.to_string()))
            .collect();
        assert!(
            !text.contains('\u{2014}') && !text.contains('\u{2013}') && !text.contains(" - "),
            "{text}"
        );
    });
}
