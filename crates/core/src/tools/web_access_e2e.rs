//! SPEC-0097: the web access lists, driven through the MCP route of the served app
//! (DEC-016) against raw TCP stand in sites. The `TestNet` seam rewrites the address of a
//! request to the loopback stand in AFTER the lists decided, so the decision is read on the
//! address as typed while the bytes reach the stand in (DEC-015).

use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::Router;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::state::AppState;
use crate::tools::tool_surface_e2e::{app_with_atlas, call_tool, setup, success_payload};
use crate::tools::web_access::TestNet;
use crate::tools::web_testkit::{Record, Script, stand_in};

/// The served app with the web family on, the two lists set and the test seam installed.
pub(crate) async fn served_with_lists(
    black: &[&str],
    white: &[&str],
    net: Option<TestNet>,
) -> (Router, String, Arc<AppState>, TempDir) {
    served_with_lists_and_base(black, white, net, None).await
}

/// [`served_with_lists`] with the search, news and suggestion hosts pointed at `test_base`.
async fn served_with_lists_and_base(
    black: &[&str],
    white: &[&str],
    net: Option<TestNet>,
    test_base: Option<String>,
) -> (Router, String, Arc<AppState>, TempDir) {
    let d = tempfile::tempdir().unwrap();
    let (mut c, token) = setup(d.path());
    c.web.enabled = true;
    c.web.black_list_url = black.iter().map(ToString::to_string).collect();
    c.web.white_list_url = white.iter().map(ToString::to_string).collect();
    c.web.test_net = net.map(Arc::new);
    c.web.test_base = test_base;
    let (app, state) = app_with_atlas(c).await;
    (app, token, state, d)
}

fn page(body: &str) -> Script {
    Script::Fixed {
        body: body.as_bytes().to_vec(),
        content_type: Some("text/plain".into()),
        chunked: false,
        encoding: None,
    }
}

/// A site answering `body` on every path, and the record of what it received.
async fn site(body: &str) -> (u16, Arc<Record>) {
    stand_in(vec![("/", page(body))]).await
}

async fn fetch(app: &Router, token: &str, url: &str) -> Value {
    call_tool(app, token, "web.fetch", json!({"url": url})).await
}

/// The error of a refused call, without the framing the MCP route puts before every tool error.
fn refusal_text(v: &Value) -> String {
    assert!(v["error"].is_null(), "the tool is served, so it is dispatched: {v}");
    assert_eq!(v["result"]["isError"], Value::Bool(true), "the call must be refused: {v}");
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or_else(|| panic!("{v}"));
    let rest =
        text.strip_prefix("An error occurred invoking '").unwrap_or_else(|| panic!("{text}"));
    let (_tool, error) = rest.split_once("': ").unwrap_or_else(|| panic!("{text}"));
    error.to_string()
}

fn refused(site: &str) -> String {
    format!("ERR_FORBIDDEN: host '{site}' is not allowed by the web access lists")
}

fn served_text(v: &Value) -> String {
    match success_payload(v) {
        Value::String(s) => s,
        other => panic!("{other}"),
    }
}

/// A site answering the raw bytes `response` to every request.
async fn raw_site(response: &'static [u8]) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = l.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf).await;
                let _ = s.write_all(response).await;
            });
        }
    });
    port
}

fn requests(rec: &Record) -> usize {
    rec.requests.load(Ordering::SeqCst)
}

// ── SPEC-0097/E2E-001 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_001_fetch_with_empty_lists() {
    let (port, _rec) = site("Daily headlines").await;
    let net = TestNet::default().with_site("news.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(served_text(&v), "Daily headlines");
}

// ── SPEC-0097/E2E-003 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_003_own_machine_reachable_with_empty_lists() {
    let (port, _rec) = site("Local").await;
    let (app, token, _s, _d) = served_with_lists(&[], &[], None).await;

    let v = fetch(&app, &token, &format!("http://127.0.0.1:{port}/")).await;

    assert_eq!(served_text(&v), "Local");
}

// ── SPEC-0097/E2E-004 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_004_unreachable_site_empty_lists() {
    // A port nothing listens on: the connection is refused.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let net = TestNet::default().with_site("down.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://down.example/").await;

    let text = refusal_text(&v);
    assert!(text.contains("fetch request failed"), "{text}");
    assert!(!text.contains("web access lists"), "{text}");
}

// ── SPEC-0097/E2E-005 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_005_wrong_start_of_address_empty_lists() {
    let (app, token, _s, _d) = served_with_lists(&[], &[], None).await;

    let v = fetch(&app, &token, "ftp://news.example/").await;

    assert!(refusal_text(&v).contains("url must start with http:// or https://"));
}

// ── SPEC-0097/E2E-007 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_007_unlisted_site_served() {
    let (port, _rec) = site("Daily headlines").await;
    let net = cast().with_site("news.example", port);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(served_text(&v), "Daily headlines");
}

// ── SPEC-0097/E2E-008 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_008_listed_name_refused() {
    let (port, rec) = site("Secret").await;
    let net = TestNet::default().with_site("blocked.example", port);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://blocked.example/").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
    assert_eq!(requests(&rec), 0, "nothing is sent to a refused site");
}

// ── SPEC-0097/E2E-009 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_009_wildcard_refuses_a_subdomain() {
    let (port, rec) = site("Secret").await;
    let net =
        TestNet::default().with_site("wiki.corp.example", port).with_site("a.b.corp.example", port);
    let (app, token, _s, _d) = served_with_lists(&["*.corp.example"], &[], Some(net)).await;

    let first = fetch(&app, &token, "http://wiki.corp.example/").await;
    let second = fetch(&app, &token, "http://a.b.corp.example/").await;

    assert_eq!(refusal_text(&first), refused("wiki.corp.example"));
    assert_eq!(refusal_text(&second), refused("a.b.corp.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-010 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_010_exact_address_refused() {
    let (port, rec) = site("Secret").await;
    let net = TestNet::default().with_site("10.0.0.5", port);
    let (app, token, _s, _d) = served_with_lists(&["10.0.0.5"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://10.0.0.5/").await;

    assert_eq!(refusal_text(&v), refused("10.0.0.5"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-012 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_012_ipv6_range_refused() {
    let (port, rec) = site("Secret").await;
    let net = TestNet::default().with_site("fd12::1", port);
    let (app, token, _s, _d) = served_with_lists(&["fd00::/8"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://[fd12::1]/").await;

    assert_eq!(refusal_text(&v), refused("fd12::1"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-013 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_013_wildcard_does_not_match_the_bare_name() {
    let (port, _rec) = site("Corp home").await;
    let net = cast().with_site("corp.example", port);
    let (app, token, _s, _d) = served_with_lists(&["*.corp.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://corp.example/").await;

    assert_eq!(served_text(&v), "Corp home");
}

// ── SPEC-0097/E2E-014 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_014_letter_case_never_matters() {
    let (port, rec) = site("Secret").await;
    let net = TestNet::default().with_site("blocked.example", port);
    let (app, token, _s, _d) = served_with_lists(&["BLOCKED.Example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://Blocked.EXAMPLE/").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-015 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_015_neighbour_address_outside_an_entry() {
    let (port, _rec) = site("Six").await;
    let net = TestNet::default().with_site("10.0.0.6", port);
    let (app, token, _s, _d) = served_with_lists(&["10.0.0.5"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://10.0.0.6/").await;

    assert_eq!(served_text(&v), "Six");
}

// ── SPEC-0097/E2E-016 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_016_range_boundaries() {
    let (port, _rec) = site("Next").await;
    let (port_in, rec_in) = site("Inside").await;
    let net = TestNet::default()
        .with_site("10.0.0.0", port_in)
        .with_site("10.0.0.255", port_in)
        .with_site("10.0.1.0", port);
    let (app, token, _s, _d) = served_with_lists(&["10.0.0.0/24"], &[], Some(net)).await;

    let low = fetch(&app, &token, "http://10.0.0.0/").await;
    let high = fetch(&app, &token, "http://10.0.0.255/").await;
    let next = fetch(&app, &token, "http://10.0.1.0/").await;

    assert_eq!(refusal_text(&low), refused("10.0.0.0"));
    assert_eq!(refusal_text(&high), refused("10.0.0.255"));
    assert_eq!(served_text(&next), "Next");
    assert_eq!(requests(&rec_in), 0);
}

// ── SPEC-0097/E2E-071 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_071_a_redirect_without_a_destination_empty_lists() {
    let port =
        raw_site(b"HTTP/1.1 302 Found\r\nContent-Length: 5\r\nConnection: close\r\n\r\nMoved")
            .await;
    let net = TestNet::default().with_site("news.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(served_text(&v), "Moved");
}

// ── SPEC-0097/E2E-017 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_017_a_refused_request_never_reaches_the_site() {
    let (port, rec) = site("Secret").await;
    let net = TestNet::default().with_site("blocked.example", port);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://blocked.example/").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-018, E2E-028, E2E-034: the search tools are left alone ────

/// The search, news and suggestion hosts of the product, as one stand in answering `body` on
/// the three routes, with the lists set.
async fn search_app(black: &[&str], white: &[&str], body: &str) -> (Router, String, TempDir) {
    let routes = vec![
        ("/html/", page(&format!("<a class=\"result__a\" href=\"https://x.test/\">{body}</a>"))),
        (
            "/news.js",
            page(&format!(
                "{{\"results\":[{{\"title\":\"{body}\",\"url\":\"https://x.test/\"}}]}}"
            )),
        ),
        ("/ac/", page(&format!("[\"weat\",[\"{body}\"]]"))),
    ];
    let (port, _rec) = stand_in(routes).await;
    let base = Some(format!("http://127.0.0.1:{port}"));
    let (app, token, _s, d) = served_with_lists_and_base(black, white, None, base).await;
    (app, token, d)
}

#[tokio::test]
async fn e2e_0097_018_web_search_unaffected() {
    let (app, token, _d) = search_app(&["search.example"], &[], "Result A").await;

    let v = call_tool(&app, &token, "web.search", json!({"query": "weather"})).await;

    assert!(success_payload(&v).to_string().contains("Result A"), "{v}");
}

#[tokio::test]
async fn e2e_0097_028_web_news_unaffected() {
    let (app, token, _d) = search_app(&[], &["docs.example"], "Headline B").await;

    let v = call_tool(&app, &token, "web.news", json!({"query": "economy"})).await;

    assert!(success_payload(&v).to_string().contains("Headline B"), "{v}");
}

#[tokio::test]
async fn e2e_0097_034_web_suggestions_unaffected() {
    let (app, token, _d) =
        search_app(&["secret.docs.example"], &["docs.example"], "weather today").await;

    let v = call_tool(&app, &token, "web.suggestions", json!({"query": "weat"})).await;

    assert!(success_payload(&v).to_string().contains("weather today"), "{v}");
}

// ── SPEC-0097/E2E-046 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_046_upper_case_wildcard_and_ipv6_accepted() {
    let list = ["*.Corp.EXAMPLE", "FD12::1"];
    start_with_black(&list);
    let (app, token, _s, _d) = served_with_lists(&list, &[], None).await;

    let v = fetch(&app, &token, "http://wiki.corp.example/").await;

    assert_eq!(refusal_text(&v), refused("wiki.corp.example"));
}

/// The configuration is read the way the server reads it at boot, and must be accepted.
fn start_with_black(list: &[&str]) {
    let yaml = format!(
        "web:\n  enabled: true\n  black_list_url: {}\n",
        serde_json::to_string(list).unwrap()
    );
    crate::config::ServerConfig::from_yaml(&yaml).expect("the server must start");
}

// ── SPEC-0097/E2E-053 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_053_trailing_dot_does_not_bypass_the_list() {
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], None).await;

    let v = fetch(&app, &token, "http://blocked.example./").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
}

// ── SPEC-0097/E2E-057 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_057_a_name_written_with_accents() {
    let (app, token, _s, _d) = served_with_lists(&["bücher.example"], &[], None).await;

    let v = fetch(&app, &token, "http://xn--bcher-kva.example/").await;

    assert_eq!(refusal_text(&v), refused("xn--bcher-kva.example"));
}

// ── SPEC-0097/E2E-061 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_061_a_range_written_with_bits_below_its_prefix() {
    let (port, rec) = site("Edge").await;
    let net = TestNet::default().with_site("10.0.0.200", port);
    let (app, token, _s, _d) = served_with_lists(&["10.0.0.5/24"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://10.0.0.200/").await;

    assert_eq!(refusal_text(&v), refused("10.0.0.200"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-062 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_062_an_ipv4_mapped_entry_covers_the_plain_address() {
    let (port, rec) = site("Local").await;
    let net = TestNet::default().with_site("127.0.0.1", port);
    let (app, token, _s, _d) = served_with_lists(&["::ffff:127.0.0.1"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://127.0.0.1/").await;

    assert_eq!(refusal_text(&v), refused("127.0.0.1"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-065 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_065_port_and_credentials_are_not_in_the_refusal() {
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], None).await;

    let v = fetch(&app, &token, "http://u:p@Blocked.example:8080/").await;

    let text = refusal_text(&v);
    assert_eq!(text, refused("blocked.example"));
    assert!(!text.contains("u:p") && !text.contains("8080"), "{text}");
}

// ── SPEC-0097/E2E-073 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_073_a_mapped_range_and_a_range_of_length_zero() {
    for entry in ["::ffff:10.0.0.0/104", "0.0.0.0/0"] {
        let (port, rec) = site("Ten").await;
        let net = TestNet::default().with_site("10.1.2.3", port);
        let (app, token, _s, _d) = served_with_lists(&[entry], &[], Some(net)).await;

        let v = fetch(&app, &token, "http://10.1.2.3/").await;

        assert_eq!(refusal_text(&v), refused("10.1.2.3"), "{entry}");
        assert_eq!(requests(&rec), 0, "{entry}");
    }
}

// ── SPEC-0097/E2E-075 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_075_an_ipv6_range_of_length_zero_does_not_cover_plain_ipv4() {
    let (port4, _r4) = site("Local").await;
    let (port6, _r6) = site("V6").await;
    let net = TestNet::default().with_site("127.0.0.1", port4).with_site("fd12::1", port6);
    let (app, token, _s, _d) = served_with_lists(&["::/0"], &[], Some(net)).await;

    let plain = fetch(&app, &token, "http://127.0.0.1/").await;
    let v6 = fetch(&app, &token, "http://[fd12::1]/").await;

    assert_eq!(served_text(&plain), "Local");
    assert_eq!(refusal_text(&v6), refused("fd12::1"));
}

// ── SPEC-0097/E2E-082 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_082_hyphens_at_the_edge_of_a_label_and_a_valid_punycode_label() {
    for entry in ["-a.example", "a-.example", "xn--bcher-kva.example"] {
        start_with_black(&[entry]);
    }
    let (app, token, _s, _d) = served_with_lists(&["-a.example"], &[], None).await;

    let v = fetch(&app, &token, "http://-a.example/").await;

    assert_eq!(refusal_text(&v), refused("-a.example"));
}

// ── SPEC-0097/E2E-084 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_084_a_name_with_a_sharp_s_stays_distinct() {
    let (port, _rec) = site("Plain").await;
    let net = cast().with_site("strasse.example", port);
    let (app, token, _s, _d) = served_with_lists(&["straße.example"], &[], Some(net)).await;

    let plain = fetch(&app, &token, "http://strasse.example/").await;
    let sharp = fetch(&app, &token, "http://straße.example/").await;

    assert_eq!(served_text(&plain), "Plain");
    assert_eq!(refusal_text(&sharp), refused("xn--strae-oqa.example"));
}

// ── SPEC-0097/E2E-019 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_019_white_listed_name_served() {
    let (port, _rec) = site("Manual v1").await;
    let net = cast().with_site("docs.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &["docs.example"], Some(net)).await;

    let v = fetch(&app, &token, "http://docs.example/").await;

    assert_eq!(served_text(&v), "Manual v1");
}

// ── SPEC-0097/E2E-020 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_020_white_listed_range_served() {
    let (port, _rec) = site("Seven").await;
    let net = TestNet::default().with_site("10.0.0.7", port);
    let (app, token, _s, _d) = served_with_lists(&[], &["10.0.0.0/24"], Some(net)).await;

    let v = fetch(&app, &token, "http://10.0.0.7/").await;

    assert_eq!(served_text(&v), "Seven");
}

// ── SPEC-0097/E2E-021 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_021_other_site_refused() {
    let (port, rec) = site("Daily headlines").await;
    let net = TestNet::default().with_site("news.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &["docs.example"], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(refusal_text(&v), refused("news.example"));
    assert_eq!(requests(&rec), 0, "nothing is sent to a refused site");
}

// ── SPEC-0097/E2E-022 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_022_metadata_address_not_white_listed() {
    let (port, rec) = site("Keys").await;
    let net = TestNet::default().with_site("169.254.169.254", port);
    let (app, token, _s, _d) = served_with_lists(&[], &["docs.example"], Some(net)).await;

    let v = fetch(&app, &token, "http://169.254.169.254/latest/meta-data/").await;

    assert_eq!(refusal_text(&v), refused("169.254.169.254"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-024 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_024_white_wildcard_excludes_the_bare_name() {
    let (port, rec) = site("Sub").await;
    let net = cast().with_site("a.docs.example", port).with_site("docs.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &["*.docs.example"], Some(net)).await;

    let first = fetch(&app, &token, "http://a.docs.example/").await;
    let second = fetch(&app, &token, "http://docs.example/").await;

    assert_eq!(served_text(&first), "Sub");
    assert_eq!(refusal_text(&second), refused("docs.example"));
    assert_eq!(requests(&rec), 1, "only the white listed site was reached");
}

// ── SPEC-0097/E2E-029 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_029_white_minus_black_served() {
    let (port, _rec) = site("Public manual").await;
    let net = cast().with_site("www.docs.example", port);
    let (app, token, _s, _d) =
        served_with_lists(&["secret.docs.example"], &["*.docs.example"], Some(net)).await;

    let v = fetch(&app, &token, "http://www.docs.example/").await;

    assert_eq!(served_text(&v), "Public manual");
}

// ── SPEC-0097/E2E-030 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_030_a_site_in_both_lists_is_refused() {
    let (port, rec) = site("Secret").await;
    let net = TestNet::default().with_site("secret.docs.example", port);
    let (app, token, _s, _d) =
        served_with_lists(&["secret.docs.example"], &["*.docs.example"], Some(net)).await;

    let v = fetch(&app, &token, "http://secret.docs.example/").await;

    assert_eq!(refusal_text(&v), refused("secret.docs.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-031 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_031_outside_the_white_list_refused() {
    let (port, rec) = site("Daily headlines").await;
    let net = TestNet::default().with_site("news.example", port);
    let (app, token, _s, _d) =
        served_with_lists(&["secret.docs.example"], &["*.docs.example"], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(refusal_text(&v), refused("news.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-032 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_032_white_range_minus_one_black_address() {
    let (port, rec) = site("Six").await;
    let net = TestNet::default().with_site("10.0.0.5", port).with_site("10.0.0.6", port);
    let (app, token, _s, _d) = served_with_lists(&["10.0.0.5"], &["10.0.0.0/24"], Some(net)).await;

    let first = fetch(&app, &token, "http://10.0.0.5/").await;
    let second = fetch(&app, &token, "http://10.0.0.6/").await;

    assert_eq!(refusal_text(&first), refused("10.0.0.5"));
    assert_eq!(served_text(&second), "Six");
    assert_eq!(requests(&rec), 1);
}

// ── US-0251: downloads and saved pages, in the specified order ─────────────

const SAVE_PROJECT: &str = "atlas";

/// A token for a person `setup` did not mint one for, signed with the fixture's key.
fn token_for(dir: &std::path::Path, person: &str) -> String {
    let key = dir.join("keys").join(crate::keys::PRIVATE_KEY_FILE);
    crate::keys::mint_token_from_file(
        &key,
        person,
        crate::keys::DEFAULT_ISSUER,
        crate::keys::DEFAULT_CLAIM,
        3600,
    )
    .unwrap()
}

async fn download(app: &Router, token: &str, url: &str, path: &str) -> Value {
    let args = json!({"url": url, "mount_id": SAVE_PROJECT, "path": path});
    call_tool(app, token, "web.download", args).await
}

async fn fetch_into(app: &Router, token: &str, url: &str, path: &str) -> Value {
    let args = json!({"url": url, "mount_id": SAVE_PROJECT, "save_path": path});
    call_tool(app, token, "web.fetch", args).await
}

async fn stored(state: &AppState, path: &str) -> Option<Vec<u8>> {
    let client = state.stores.client(SAVE_PROJECT).await.unwrap();
    if client.exists(path).await.unwrap() {
        Some(client.read_bytes(path).await.unwrap())
    } else {
        None
    }
}

async fn used(state: &AppState) -> i64 {
    let client = state.stores.client(SAVE_PROJECT).await.unwrap();
    crate::quota::used_bytes(&client).await.unwrap()
}

fn assert_saved(v: &Value) {
    assert!(v["error"].is_null(), "{v}");
    assert_ne!(v["result"]["isError"], Value::Bool(true), "the call must succeed: {v}");
}

// ── SPEC-0097/E2E-002 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_002_download_with_empty_lists() {
    let (port, _rec) = site("Manual v1").await;
    let net = TestNet::default().with_site("docs.example", port);
    let (app, token, state, _d) = served_with_lists(&[], &[], Some(net)).await;

    let v = download(&app, &token, "http://docs.example/manual.pdf", "/manual.pdf").await;

    assert_saved(&v);
    assert_eq!(stored(&state, "/manual.pdf").await.as_deref(), Some(&b"Manual v1"[..]));
}

// ── SPEC-0097/E2E-006 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_006_download_counts_storage_empty_lists() {
    let (port, _rec) = site("Manual v1").await;
    let net = TestNet::default().with_site("docs.example", port);
    let (app, token, state, _d) = served_with_lists(&[], &[], Some(net)).await;
    let client = state.stores.client(SAVE_PROJECT).await.unwrap();
    client.write_bytes_atomic("/docs/filler.bin", &[b'f'; 100]).await.unwrap();
    assert_eq!(used(&state).await, 100);

    let v = download(&app, &token, "http://docs.example/manual.pdf", "/manual.pdf").await;

    assert_saved(&v);
    assert_eq!(used(&state).await, 109);
}

// ── SPEC-0097/E2E-027 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_027_allowed_fetch_with_a_destination_saves() {
    let (port, _rec) = site("Manual v1").await;
    let net = cast().with_site("docs.example", port);
    let (app, token, state, _d) = served_with_lists(&[], &["docs.example"], Some(net)).await;

    let v = fetch_into(&app, &token, "http://docs.example/", "/page.html").await;

    assert_saved(&v);
    assert_eq!(stored(&state, "/page.html").await.as_deref(), Some(&b"Manual v1"[..]));
}

// ── SPEC-0097/E2E-033 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_033_allowed_download_under_both_lists_saves() {
    let (port, _rec) = site("Public manual").await;
    let net = cast().with_site("www.docs.example", port);
    let (app, token, state, _d) =
        served_with_lists(&["secret.docs.example"], &["*.docs.example"], Some(net)).await;

    let v = download(&app, &token, "http://www.docs.example/m.pdf", "/m.pdf").await;

    assert_saved(&v);
    assert_eq!(stored(&state, "/m.pdf").await.as_deref(), Some(&b"Public manual"[..]));
}

// ── SPEC-0097/E2E-048 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_048_allowed_download_under_a_black_list() {
    let (port, _rec) = site("Manual v1").await;
    let net = cast().with_site("docs.example", port);
    let (app, token, state, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = download(&app, &token, "http://docs.example/m.pdf", "/m.pdf").await;

    assert_saved(&v);
    assert_eq!(stored(&state, "/m.pdf").await.as_deref(), Some(&b"Manual v1"[..]));
}

// ── SPEC-0097/E2E-049 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_049_a_non_member_sees_the_membership_refusal() {
    let (port, rec) = site("Blocked").await;
    let net = TestNet::default().with_site("blocked.example", port);
    let (app, _token, _s, d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;
    let eve = token_for(d.path(), "eve@test.com");

    let v = download(&app, &eve, "http://blocked.example/x", "/x").await;

    let text = refusal_text(&v);
    assert!(text.contains("ERR_FORBIDDEN"), "{text}");
    assert!(text.contains("'eve@test.com' is not a member of 'atlas'"), "{text}");
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-050 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_050_an_existing_file_is_untouched_by_a_refused_save() {
    let (port, rec) = site("New").await;
    let net = TestNet::default().with_site("blocked.example", port);
    let (app, token, state, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;
    let client = state.stores.client(SAVE_PROJECT).await.unwrap();
    client.write_bytes_atomic("/page.html", b"Old").await.unwrap();

    let v = fetch_into(&app, &token, "http://blocked.example/", "/page.html").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
    assert_eq!(stored(&state, "/page.html").await.as_deref(), Some(&b"Old"[..]));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-051 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_051_storage_unchanged_and_the_lists_refusal_comes_first() {
    let (port, rec) = site("Manual v1").await;
    let net = TestNet::default().with_site("blocked.example", port);
    let (app, token, state, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;
    let client = state.stores.client(SAVE_PROJECT).await.unwrap();
    client.write_bytes_atomic("/docs/filler.bin", &[b'f'; 100]).await.unwrap();
    state.admin.set_quota(SAVE_PROJECT, Some(100)).await.unwrap();

    let v = download(&app, &token, "http://blocked.example/m.pdf", "/m.pdf").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
    assert_eq!(used(&state).await, 100);
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-076 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_076_refusal_whatever_the_scheme_in_save_mode() {
    let (port, rec) = site("Daily headlines").await;
    let net = TestNet::default().with_site("blocked.example", port).with_site("news.example", port);
    let (app, token, state, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let first = fetch_into(&app, &token, "ftp://blocked.example/", "/f.txt").await;
    let second = fetch_into(&app, &token, "ftp://news.example/", "/g.txt").await;

    assert_eq!(refusal_text(&first), refused("blocked.example"));
    assert!(
        refusal_text(&second).starts_with("ERR_INTERNAL_ERROR: fetch request failed"),
        "{second}"
    );
    assert_eq!(stored(&state, "/f.txt").await, None);
    assert_eq!(stored(&state, "/g.txt").await, None);
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-SEC-001, 006, 007 ─────────────────────────────────────────

const METADATA: &str = "http://169.254.169.254/latest/meta-data/iam/security-credentials/";

async fn metadata_setup() -> (Router, String, Arc<AppState>, TempDir, Arc<Record>) {
    let (port, rec) = site("AccessKeyId=AKIAEXAMPLE").await;
    let net = TestNet::default().with_site("169.254.169.254", port);
    let (app, token, state, d) = served_with_lists(&["169.254.0.0/16"], &[], Some(net)).await;
    (app, token, state, d, rec)
}

#[tokio::test]
async fn e2e_0097_sec_001_cloud_metadata_credentials() {
    let (app, token, _s, _d, rec) = metadata_setup().await;

    let v = fetch(&app, &token, METADATA).await;

    let text = refusal_text(&v);
    assert_eq!(text, refused("169.254.169.254"));
    assert!(!text.contains("AKIAEXAMPLE"));
    assert_eq!(requests(&rec), 0);
}

#[tokio::test]
async fn e2e_0097_sec_006_download_of_the_metadata_address() {
    let (app, token, state, _d, rec) = metadata_setup().await;

    let v = download(&app, &token, METADATA, "/creds.txt").await;

    assert_eq!(refusal_text(&v), refused("169.254.169.254"));
    assert_eq!(stored(&state, "/creds.txt").await, None);
    assert_eq!(requests(&rec), 0);
}

#[tokio::test]
async fn e2e_0097_sec_007_fetch_with_a_destination_of_the_metadata_address() {
    let (app, token, state, _d, rec) = metadata_setup().await;

    let v = fetch_into(&app, &token, METADATA, "/creds.txt").await;

    assert_eq!(refusal_text(&v), refused("169.254.169.254"));
    assert_eq!(stored(&state, "/creds.txt").await, None);
    assert_eq!(requests(&rec), 0);
}

// ── US-0252: the addresses a name resolves to ──────────────────────────────

use crate::tools::web_access::NameAnswer;

fn addrs(list: &[&str]) -> Vec<NameAnswer> {
    vec![NameAnswer::Addrs(list.iter().map(|a| a.parse().unwrap()).collect())]
}

/// The names of the cast, each with the address the spec gives it, plus the public names the
/// earlier stories fetch: since US-0252 a name is looked up whenever a list is set.
fn cast() -> TestNet {
    TestNet::default()
        .with_name("corp.example", addrs(&["93.184.216.40"]))
        .with_name("a.docs.example", addrs(&["93.184.216.41"]))
        .with_name("www.docs.example", addrs(&["93.184.216.42"]))
        .with_name("strasse.example", addrs(&["93.184.216.43"]))
        .with_name("news.example", addrs(&["93.184.216.34"]))
        .with_name("docs.example", addrs(&["93.184.216.35"]))
        .with_name("friendly.example", addrs(&["10.1.2.3"]))
        .with_name("mixed.example", addrs(&["93.184.216.36", "10.1.2.4"]))
        .with_name("localhost", addrs(&["127.0.0.1"]))
}

// ── SPEC-0097/E2E-011 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_011_a_name_for_the_own_machine_is_refused() {
    let (port, rec) = site("Local").await;
    let net = cast().with_site("localhost", port);
    let (app, token, _s, _d) = served_with_lists(&["127.0.0.0/8"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://localhost/").await;

    let text = refusal_text(&v);
    assert_eq!(text, refused("localhost"));
    assert!(!text.contains("127.0.0.1"), "{text}");
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-023 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_023_unresolvable_name_is_not_white_listed_by_address() {
    let (port, rec) = site("Ghost").await;
    let net = cast().with_site("ghost.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &["10.0.0.0/24"], Some(net)).await;

    let v = fetch(&app, &token, "http://ghost.example/").await;

    assert_eq!(refusal_text(&v), refused("ghost.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-025 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_025_a_name_resolving_inside_a_white_listed_range() {
    let (port, _rec) = site("Internal payroll").await;
    let net = cast().with_site("friendly.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &["10.1.2.0/24"], Some(net)).await;

    let v = fetch(&app, &token, "http://friendly.example/").await;

    assert_eq!(served_text(&v), "Internal payroll");
}

// ── SPEC-0097/E2E-026 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_026_a_name_resolving_partly_outside_the_white_list() {
    let (port, rec) = site("Mixed").await;
    let net = cast().with_site("mixed.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &["10.1.2.0/24"], Some(net)).await;

    let v = fetch(&app, &token, "http://mixed.example/").await;

    assert_eq!(refusal_text(&v), refused("mixed.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-039 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_039_a_name_pointing_to_a_public_address() {
    let (port, _rec) = site("Daily headlines").await;
    let net = cast().with_site("news.example", port);
    let (app, token, _s, _d) = served_with_lists(&["10.0.0.0/8"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(served_text(&v), "Daily headlines");
}

// ── SPEC-0097/E2E-040 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_040_two_addresses_one_black_listed() {
    let (port, rec) = site("Mixed").await;
    let net = cast().with_site("mixed.example", port);
    let (app, token, _s, _d) = served_with_lists(&["10.0.0.0/8"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://mixed.example/").await;

    assert_eq!(refusal_text(&v), refused("mixed.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-052 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_052_refused_name_leaves_no_file() {
    let (port, rec) = site("Internal payroll").await;
    let net = cast().with_site("friendly.example", port);
    let (app, token, state, _d) = served_with_lists(&["10.0.0.0/8"], &[], Some(net)).await;

    let v = fetch_into(&app, &token, "http://friendly.example/", "/p.html").await;

    assert_eq!(refusal_text(&v), refused("friendly.example"));
    assert_eq!(stored(&state, "/p.html").await, None);
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-054 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_054_unresolvable_name_under_a_black_list() {
    let (port, rec) = site("Ghost").await;
    let net = cast().with_site("ghost.example", port);
    let (app, token, _s, _d) = served_with_lists(&["10.0.0.0/8"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://ghost.example/").await;

    let text = refusal_text(&v);
    assert_eq!(
        text,
        "ERR_INTERNAL_ERROR: fetch request failed: no address for host 'ghost.example'",
    );
    assert_ne!(text, refused("ghost.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-055 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_055_unresolvable_name_download() {
    let (port, rec) = site("Ghost").await;
    let net = cast().with_site("ghost.example", port);
    let (app, token, state, _d) = served_with_lists(&["10.0.0.0/8"], &[], Some(net)).await;

    let v = download(&app, &token, "http://ghost.example/g.pdf", "/g.pdf").await;

    let text = refusal_text(&v);
    assert!(text.starts_with("ERR_INTERNAL_ERROR: fetch request failed"), "{text}");
    assert_eq!(stored(&state, "/g.pdf").await, None);
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-087 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_087_a_lookup_that_times_out() {
    let (port, rec) = site("Slow").await;
    let net =
        cast().with_name("slow.example", vec![NameAnswer::Hang]).with_site("slow.example", port);
    let (app, token, _s, _d) = served_with_lists(&[], &["docs.example"], Some(net)).await;
    let started = std::time::Instant::now();

    let args = json!({"url": "http://slow.example/", "timeout_secs": 1});
    let v = call_tool(&app, &token, "web.fetch", args).await;

    assert_eq!(refusal_text(&v), "ERR_INTERNAL_ERROR: request timed out");
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-SEC-003, 008 ──────────────────────────────────────────────

async fn admin_panel_refused(name: &str, addr: &str) {
    let (port, rec) = site("ADMIN PANEL").await;
    let net = cast().with_name(name, addrs(&[addr])).with_site(name, port);
    let (app, token, _s, _d) = served_with_lists(&["127.0.0.0/8"], &[], Some(net)).await;

    let v = fetch(&app, &token, &format!("http://{name}/admin")).await;

    let text = refusal_text(&v);
    assert_eq!(text, refused(name));
    assert!(!text.contains("127.0.0.1"), "{text}");
    assert_eq!(requests(&rec), 0);
}

#[tokio::test]
async fn e2e_0097_sec_003_a_name_that_points_to_the_own_machine() {
    admin_panel_refused("evil.example", "127.0.0.1").await;
}

#[tokio::test]
async fn e2e_0097_sec_008_a_name_resolving_to_an_ipv4_mapped_ipv6_address() {
    admin_panel_refused("evil6.example", "::ffff:127.0.0.1").await;
}

// ── SPEC-0097/E2E-SEC-009 ───────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_sec_009_a_name_whose_second_lookup_differs() {
    // The site table is keyed on addresses only: the bytes go where the address the request
    // was sent to points, so a sender that looked the name up again would reach the loopback.
    let (public, _p) = site("Example").await;
    let (loopback, rec) = site("ADMIN PANEL").await;
    let answers = vec![
        NameAnswer::Addrs(vec!["93.184.216.34".parse().unwrap()]),
        NameAnswer::Addrs(vec!["127.0.0.1".parse().unwrap()]),
    ];
    let net = Arc::new(
        cast()
            .with_name("rebind.example", answers)
            .with_site("93.184.216.34", public)
            .with_site("127.0.0.1", loopback),
    );
    let d = tempfile::tempdir().unwrap();
    let (mut c, token) = setup(d.path());
    c.web.enabled = true;
    c.web.black_list_url = vec!["127.0.0.0/8".to_string()];
    c.web.test_net = Some(net.clone());
    let (app, _state) = app_with_atlas(c).await;

    let v = fetch(&app, &token, "http://rebind.example/admin").await;

    assert!(!v.to_string().contains("ADMIN PANEL"), "{v}");
    assert_eq!(requests(&rec), 0);
    assert_eq!(net.lookups(), 1);
}

// ── US-0252: the connection goes to the checked address (DEC-009) ──────────

#[tokio::test]
async fn e2e_0097_sec_009_pinned_connection_reaches_the_checked_address() {
    // No site entry: the request URL is not rewritten, so only the pin of the production sender
    // can make a name the real resolver does not know reach the stand in.
    let (port, rec) = site("Pinned").await;
    let net = cast().with_name("pinned.example", addrs(&["127.0.0.1"]));
    let (app, token, _s, _d) = served_with_lists(&["10.0.0.0/8"], &[], Some(net)).await;

    let v = fetch(&app, &token, &format!("http://pinned.example:{port}/")).await;

    assert_eq!(served_text(&v), "Pinned");
    assert_eq!(requests(&rec), 1);
}

// ── US-0253: unreadable addresses, numeric forms, the proxy limit ──────────

/// A site answering `response` to every request, and how many requests it received.
async fn counting_site(response: &'static [u8]) -> (u16, Arc<std::sync::atomic::AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = count.clone();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = l.accept().await {
            seen.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf).await;
                let _ = s.write_all(response).await;
            });
        }
    });
    (port, count)
}

fn fetch_failed(v: &Value) {
    let text = refusal_text(v);
    assert!(text.starts_with("ERR_INTERNAL_ERROR: fetch request failed"), "{text}");
    assert!(!text.contains("web access lists"), "{text}");
}

// ── SPEC-0097/E2E-058 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_058_an_address_with_a_zone_is_never_sent() {
    let (port, rec) = site("Link local").await;
    let net = cast().with_site("fe80::1", port);
    let (app, token, _s, _d) = served_with_lists(&["fe80::/10"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://[fe80::1%25eth0]/").await;

    fetch_failed(&v);
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-068, 069, 070 ─────────────────────────────────────────────

const PROXY_ANSWER: &[u8] =
    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 15\r\nConnection: close\r\n\r\nDaily headlines";

async fn behind_proxy(
    black: &[&str],
) -> (Router, String, TempDir, Arc<std::sync::atomic::AtomicUsize>) {
    let (port, count) = counting_site(PROXY_ANSWER).await;
    let net = cast().with_proxy(port);
    let (app, token, _s, d) = served_with_lists(black, &[], Some(net)).await;
    (app, token, d, count)
}

#[tokio::test]
async fn e2e_0097_068_blocked_name_with_a_proxy_in_the_environment() {
    let (app, token, _d, proxied) = behind_proxy(&["blocked.example"]).await;

    let v = fetch(&app, &token, "http://blocked.example/").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
    assert_eq!(proxied.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn e2e_0097_069_allowed_site_with_a_proxy_in_the_environment() {
    let (app, token, _d, proxied) = behind_proxy(&["blocked.example"]).await;

    let args = json!({"url": "http://news.example/", "timeout_secs": 3});
    let v = call_tool(&app, &token, "web.fetch", args).await;

    assert_eq!(served_text(&v), "Daily headlines");
    assert_eq!(proxied.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn e2e_0097_070_name_resolved_to_a_black_listed_address_with_a_proxy() {
    let (app, token, _d, proxied) = behind_proxy(&["10.0.0.0/8"]).await;

    let v = fetch(&app, &token, "http://friendly.example/").await;

    assert_eq!(refusal_text(&v), refused("friendly.example"));
    assert_eq!(proxied.load(Ordering::SeqCst), 0);
}

// ── SPEC-0097/E2E-080 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_080_an_address_with_no_host_is_never_sent() {
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(cast())).await;

    let v = fetch(&app, &token, "http://").await;

    fetch_failed(&v);
}

// ── SPEC-0097/E2E-081 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_081_an_address_with_no_scheme_save_mode() {
    let (port, rec) = site("Manual v1").await;
    let net = cast().with_site("docs.example", port);
    let (app, token, state, _d) = served_with_lists(&[], &["docs.example"], Some(net)).await;

    let v = fetch_into(&app, &token, "docs.example/x", "/p.html").await;

    fetch_failed(&v);
    assert_eq!(stored(&state, "/p.html").await, None);
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-083, E2E-SEC-004, E2E-SEC-005 ─────────────────────────────

/// Each written form of the own machine is refused under the site the standard reads it as.
async fn own_machine_refused(forms: &[(&str, &str)]) {
    let (port, rec) = site("ADMIN PANEL").await;
    let net = cast().with_site("127.0.0.1", port).with_site("::ffff:7f00:1", port);
    let (app, token, _s, _d) = served_with_lists(&["127.0.0.0/8"], &[], Some(net)).await;

    for (url, shown) in forms {
        let v = fetch(&app, &token, url).await;
        assert_eq!(refusal_text(&v), refused(shown), "{url}");
    }
    assert_eq!(requests(&rec), 0);
}

#[tokio::test]
async fn e2e_0097_083_other_written_forms_of_the_own_machine() {
    own_machine_refused(&[
        ("http://127.1/", "127.0.0.1"),
        ("http://0177.0.0.1/", "127.0.0.1"),
        ("http://0x7f.1/", "127.0.0.1"),
    ])
    .await;
}

#[tokio::test]
async fn e2e_0097_sec_004_decimal_form_of_the_own_machine() {
    own_machine_refused(&[("http://2130706433/", "127.0.0.1")]).await;
}

#[tokio::test]
async fn e2e_0097_sec_005_ipv4_mapped_ipv6_form() {
    own_machine_refused(&[("http://[::ffff:127.0.0.1]/", "::ffff:7f00:1")]).await;
}

// ── US-0254: redirects, hop by hop ─────────────────────────────────────────

/// A site answering a 302 towards `location`, and how many requests it received.
async fn redirecting(location: &str) -> (u16, Arc<std::sync::atomic::AtomicUsize>) {
    let answer = format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    counting_site(Box::leak(answer.into_bytes().into_boxed_slice())).await
}

fn hits(count: &std::sync::atomic::AtomicUsize) -> usize {
    count.load(Ordering::SeqCst)
}

// ── SPEC-0097/E2E-035 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_035_redirect_to_an_allowed_site() {
    let (news, _n) = redirecting("http://docs.example/").await;
    let (docs, _rec) = site("Manual v1").await;
    let net = cast().with_site("news.example", news).with_site("docs.example", docs);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(served_text(&v), "Manual v1");
}

// ── SPEC-0097/E2E-036 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_036_redirect_to_a_refused_site() {
    let (news, _n) = redirecting("http://docs.example/").await;
    let (docs, rec) = site("Manual v1").await;
    let net = cast().with_site("news.example", news).with_site("docs.example", docs);
    let (app, token, _s, _d) = served_with_lists(&[], &["news.example"], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(refusal_text(&v), refused("docs.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-037 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_037_second_hop_of_a_chain_refused() {
    let (news, _n) = redirecting("http://docs.example/").await;
    let (docs, _d2) = redirecting("http://blocked.example/").await;
    let (blocked, rec) = site("Blocked").await;
    let net = cast()
        .with_site("news.example", news)
        .with_site("docs.example", docs)
        .with_site("blocked.example", blocked);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-038 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_038_download_refused_at_a_hop_saves_nothing() {
    let (news, _n) = redirecting("http://docs.example/f.pdf").await;
    let (docs, rec) = site("Manual v1").await;
    let net = cast().with_site("news.example", news).with_site("docs.example", docs);
    let (app, token, state, _d) = served_with_lists(&[], &["news.example"], Some(net)).await;

    let v = download(&app, &token, "http://news.example/f.pdf", "/f.pdf").await;

    assert_eq!(refusal_text(&v), refused("docs.example"));
    assert_eq!(stored(&state, "/f.pdf").await, None);
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-064 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_064_a_redirect_loop_keeps_todays_failure() {
    let (news, count) = redirecting("http://news.example/").await;
    let net = cast().with_site("news.example", news);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    fetch_failed(&v);
    // The first request and the 10 followed hops, as the default policy of the client sends.
    assert_eq!(hits(&count), 11);
}

// ── SPEC-0097/E2E-074 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_074_a_redirect_without_a_destination_black_list_set() {
    let port = raw_site(
        b"HTTP/1.1 302 Found\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nConnection: close\r\n\r\nMoved",
    )
    .await;
    let net = cast().with_site("news.example", port);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(served_text(&v), "Moved");
}

// ── SPEC-0097/E2E-077, 078 ──────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_077_a_hop_to_a_refused_site_on_a_non_web_scheme() {
    let (news, _n) = redirecting("ftp://blocked.example/").await;
    let net = cast().with_site("news.example", news);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
}

#[tokio::test]
async fn e2e_0097_078_a_hop_to_an_allowed_site_on_a_non_web_scheme() {
    let (news, _n) = redirecting("ftp://docs.example/").await;
    let net = cast().with_site("news.example", news);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    fetch_failed(&v);
}

// ── SPEC-0097/E2E-079 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_079_the_refused_site_is_the_eleventh_hop() {
    // hop0 redirects to hop1 ... hop9 to hop10: 10 followed hops, then hop10 leads to blocked.
    let mut net = cast();
    for i in 0..=10 {
        let next = if i == 10 {
            "http://blocked.example/".to_string()
        } else {
            format!("http://hop{}.example/", i + 1)
        };
        let (port, _c) = redirecting(&next).await;
        let name = format!("hop{i}.example");
        net = net.with_name(&name, addrs(&["93.184.216.50"])).with_site(&name, port);
    }
    let (blocked, rec) = site("Blocked").await;
    let net = net.with_site("blocked.example", blocked);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://hop0.example/").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-088 ───────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_088_a_protocol_relative_redirect_to_a_listed_site() {
    let (news, _n) = redirecting("//blocked.example/x").await;
    let (blocked, rec) = site("Blocked").await;
    let net = cast().with_site("news.example", news).with_site("blocked.example", blocked);
    let (app, token, _s, _d) = served_with_lists(&["blocked.example"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    assert_eq!(refusal_text(&v), refused("blocked.example"));
    assert_eq!(requests(&rec), 0);
}

// ── SPEC-0097/E2E-SEC-002 ───────────────────────────────────────────────────

#[tokio::test]
async fn e2e_0097_sec_002_redirect_from_a_public_page_to_the_metadata_address() {
    let (news, _n) = redirecting("http://169.254.169.254/latest/meta-data/").await;
    let (meta, rec) = site("AccessKeyId=AKIAEXAMPLE").await;
    let net = cast().with_site("news.example", news).with_site("169.254.169.254", meta);
    let (app, token, _s, _d) = served_with_lists(&["169.254.0.0/16"], &[], Some(net)).await;

    let v = fetch(&app, &token, "http://news.example/").await;

    let text = refusal_text(&v);
    assert_eq!(text, refused("169.254.169.254"));
    assert!(!text.contains("AKIAEXAMPLE"));
    assert_eq!(requests(&rec), 0);
}
