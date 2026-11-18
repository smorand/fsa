# US-0253: unreadable addresses are never sent, numeric forms are read as the plain address, the proxy limit is tested

> Spec: SPEC-0097 (sdlc/specs/SPEC-0097-web-access-lists/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0252
> min_tier: 2
> Files touched: 4
> Closes spec: no

## Objective
An address that has no host, an accented or unconvertible host, an IPv6 zone, or no scheme at all is never sent while a list is set and fails with today's `fetch request failed` text; `127.1`, `0177.0.0.1`, `0x7f.1`, `2130706433` and `[::ffff:127.0.0.1]` are judged as the plain address `127.0.0.1` and refused with that address as the site; with a proxy in the environment the lists still decide on the typed site and the addresses the server resolves.

## Requirements covered
- SPEC-0097/FR-NEW-006: WHEN a request names a site THE web access context SHALL decide the black list on the site as typed and on every address the site resolves to, and the white list on the site as typed or on all of its resolved addresses.

## Independent Test
Run `cargo test -p mcp-fs-core --lib -- web_access_e2e`: the nine tests of this story are green. With black list `127.0.0.0/8`, `http://2130706433/` answers `ERR_FORBIDDEN: host '127.0.0.1' is not allowed by the web access lists` and the loopback stand in counts 0 requests.

## Design excerpt
### Decisions
- DEC-001 (SPEC-0097): No new dependency. Entries and address ranges are parsed with `std::net`; hosts are read and converted to their ASCII form with the `url` crate (WHATWG host parsing, UTS 46 non transitional), which is already a direct dependency.
- DEC-009 (SPEC-0097): The connection goes to the checked address: each hop is sent by a fresh client built with `ClientBuilder::resolve_to_addrs(host, &checked_addresses)` for a host name; an IP literal needs no pin. The client still sets the request timeout from the time left of the call, the user agent `mcp-fs-web/1.0`, and keeps the environment proxy (`auto_sys_proxy`, no `no_proxy()`), the limit the owner accepted (option A, BL-0157 owns the proxy design).
- DEC-012 (SPEC-0097): The `<site>` of a message is read from `url::Url::host_str()` of the address as parsed (lower case, ASCII form), with one trailing dot removed and the brackets of an IPv6 address removed. `Ipv6Addr::to_string` is never used for it: Rust prints an IPv4 mapped address as `::ffff:127.0.0.1` while the spec wants `::ffff:7f00:1`. The message is `ToolError::forbidden("host '<site>' is not allowed by the web access lists")`, so it renders `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. The resolved addresses never enter a message.
- DEC-013 (SPEC-0097): Matching rules in code: a request site is a `Domain(ascii)`, an `Ipv4` or an `Ipv6`. An IPv6 address that is IPv4 mapped is judged as its IPv4 address, for requests and for resolved addresses. Black list: refuse if the typed site matches an entry or any resolved address does. White list: serve if the typed site matches a name entry, or the site resolved to at least one address and every address matches an entry (an empty answer is never white listed). Both: refuse on black first, then apply the white rule. `Below` matches a domain that ends with `.` plus the entry and is longer; `Name` is equality.
- DEC-015 (SPEC-0097): Test seam, `#[cfg(test)]` only, in the same style as `test_base` (`config.rs:628-631`): a field `WebConfig.test_net: Option<Arc<TestNet>>` with `#[serde(skip)]`, left out of `Debug`. `TestNet` holds (a) a name table answering `Addrs(Vec<IpAddr>)`, `NoAddress` or `Hang` per name, one answer per successive lookup with the last one repeating, and a lookup counter; (b) a site table mapping a host or literal address to the port of a loopback stand in, applied at send time AFTER the decision, rewriting the request address to `127.0.0.1:<port>` (so the lists are exercised on the real typed address while the bytes reach the stand in); (c) an optional proxy address handed to `ClientBuilder::proxy`. A name missing from the table answers `NoAddress`: no test ever reaches the real resolver.
- DEC-016 (SPEC-0097): Tests live in two new files registered in `tools/mod.rs` next to `web_bound_e2e`: `tools/web_access_boot_e2e.rs` (configuration and command level, no HTTP) and `tools/web_access_e2e.rs` (MCP tool calls through the served app, the seam and a raw TCP stand in). Function names are `e2e_0097_<NNN>_<slug>` and `e2e_0097_sec_<NNN>_<slug>`, as `e2e_0093_*` does. The helper `served_with_lists(black, white, net)` copies `served_with_base` (`web_testkit.rs:298-318`) and sets the lists and `test_net` on the configuration. Boot tests call `ServerConfig::validate` or `cli::dispatch(Cli::parse_from([...]))` (`cli.rs:139-168`), never a child process.
- DEC-017 (SPEC-0097): Documentation: each story updates `.agent_docs/config.md` (section "web and context7", the web access lists) for the part it delivers; `config/local.yaml` and `config/local.yaml.template` get the two commented settings; `README.md` and `AGENTS.md` get one short paragraph each. The tool contract (`TOOL_CONTRACT.txt`, `tool-contract-golden.json`) is not touched: no tool schema or description changes.

### Data model
No persisted data. Configuration only: `web.black_list_url` and `web.white_list_url`, each a list of strings, default empty, `#[serde(default)]` (DEC-003).

### Contracts
- Refusal: `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`.
- Boot refusal: `error: ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web <black|white> list: expected a host name, a *.host name, an IP address or an address range`.
- Entry forms: spec FR-NEW-005 (copied in the requirements above and in DEC-004).
- Crate internal: `AccessLists::from_config(&WebConfig) -> Result<AccessLists>`, `AccessLists::admit(&self, url: &str, deadline: tokio::time::Instant) -> Result<Admission>` (US-0248 onward), `fetch_raw(..., access, admission, deadline)` (US-0252 onward).

### Paths
| Path | Action |
|---|---|
| `crates/core/src/tools/web_access.rs` | modify (unreadable address failure, proxy seam in the per hop client) |
| `crates/core/src/tools/web_access_e2e.rs` | modify |
| `.agent_docs/config.md` | modify (proxy limit) |
| `AGENTS.md` | modify (one paragraph: the access lists) |

### Existing patterns
Proxy stand in: a second raw TCP stand in plus `ClientBuilder::proxy(reqwest::Proxy::http(url))` applied only when `test_net.proxy` is set (DEC-015).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

Cast used by every test of this story (verbatim from the spec):

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

Test level and location: exploit in `crates/core/src/tools/web_access_e2e.rs`; functional e2e in `crates/core/src/tools/web_access_e2e.rs`. Function names `e2e_0097_<NNN>_<slug>` / `e2e_0097_sec_<NNN>_<slug>`. Cleanup: each test builds its own temporary project and stand ins; nothing to tear down.

Exploit tests (verbatim preface from the spec): Each test carries the literal payload. Each asserts the refusal AND, through a second channel, that the stand in site received 0 requests. Red before the fix because: with both lists empty at that time, the call returns the stand in's body instead of a refusal (`expected the refusal, got the page text`).

### SPEC-0097/E2E-058: an address with a zone is never sent
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `fe80::/10` and a stand in at `fe80::1`. **When** Alice fetches `http://[fe80::1%25eth0]/`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed`. **And** the stand in counts 0 requests.

### SPEC-0097/E2E-068: blocked name with a proxy in the environment
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** the server started with a web proxy set in its environment (a stand in proxy), black list `blocked.example`. **When** Alice fetches `http://blocked.example/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`. **And** the stand in proxy counts 0 requests.

### SPEC-0097/E2E-069: allowed site with a proxy in the environment
- **Category:** happy. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** the same proxy and black list `blocked.example`. **When** Alice fetches `http://news.example/`. **Then** she receives the proxy's answer `Daily headlines`. **And** the stand in proxy counts 1 request (the proxy makes the connection).

### SPEC-0097/E2E-070: name resolved by the server to a black listed address, with a proxy
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** the same proxy, black list `10.0.0.0/8` and `friendly.example` resolving (on the server) to `10.1.2.3`. **When** Alice fetches `http://friendly.example/`. **Then** she gets `ERR_FORBIDDEN: host 'friendly.example' is not allowed by the web access lists`. **And** the stand in proxy counts 0 requests.

### SPEC-0097/E2E-080: an address with no host is never sent
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `blocked.example`. **When** Alice fetches `http://`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed` and is not the lists refusal.

### SPEC-0097/E2E-081: an address with no scheme, save mode
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-006
- **Given** white list `docs.example`. **When** Alice fetches `docs.example/x` with destination `/p.html`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed`. **And** `/p.html` does not exist.

### SPEC-0097/E2E-083: other written forms of the own machine
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8` and a stand in at `127.0.0.1`. **When** Alice fetches `http://127.1/`, then `http://0177.0.0.1/`, then `http://0x7f.1/`. **Then** each time she gets `ERR_FORBIDDEN: host '127.0.0.1' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

### SPEC-0097/E2E-SEC-004: decimal form of the own machine
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8` and a stand in at `127.0.0.1`. **When** Alice fetches `http://2130706433/`. **Then** she gets `ERR_FORBIDDEN: host '127.0.0.1' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

### SPEC-0097/E2E-SEC-005: IPv4 mapped IPv6 form
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8` and a stand in at `127.0.0.1`. **When** Alice fetches `http://[::ffff:127.0.0.1]/`. **Then** she gets `ERR_FORBIDDEN: host '::ffff:7f00:1' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

## Constraints
- Files not to touch: `spec.md`, `crates/core/src/cli.rs`.
- Dependencies not to add: any.
- Reading of the site is delegated to `url::Url` (WHATWG); do not hand roll IPv4 number parsing (DEC-001).
- Scope boundary: redirects.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs`, the two configuration snapshots (`config.rs:1710-1711`) and `tool-contract-golden.json` pass unmodified; both lists empty behaves exactly as before.
- [ ] Docs updated: `.agent_docs/config.md` (web section), `AGENTS.md`.
- [ ] Self review (full): every requirement above has its tests green, no `unwrap` or `expect` on a fallible path, no dash used as punctuation in code or comments, comments explain why.
- [ ] One commit `US-0253: unreadable addresses are never sent, numeric forms are read as the plain address, the proxy limit is tested` on the work branch `feat/0156-web-access-lists`, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0097` and the requirement ids.
