# US-0254: redirects are checked hop by hop, and the spec is closed

> Spec: SPEC-0097 (sdlc/specs/SPEC-0097-web-access-lists/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0253
> min_tier: 1
> Files touched: 5
> Closes spec: yes

## Objective
With a list set, every redirect hop is decided like a first request before it is followed, on any scheme and even beyond the hop limit; a relative or protocol relative destination is read against the hop that sent it; a redirect without a usable destination is returned as the answer; the failures of today keep their text. This story closes SPEC-0097: the whole acceptance set of the spec is green.

## Requirements covered
- SPEC-0097/FR-NEW-007: WHEN a site answers with a redirect THE web access context SHALL apply the same decision to the redirected site before following it, on every hop of a chain.
- SPEC-0097/FR-NEW-009: WHEN a request that saves into a project is refused by the lists THE web access context SHALL NOT create or change any file, and the project's storage use SHALL be unchanged.

## Independent Test
Run `cargo test -p mcp-fs-core --lib -- web_access_e2e web_access_boot_e2e web_bound_e2e tool_surface_e2e` then `make check`: all 100 tests of SPEC-0097 and the 67 existing web tests pass, none modified. A public page redirecting to `http://169.254.169.254/latest/meta-data/` with black list `169.254.0.0/16` answers the refusal naming `169.254.169.254` and the metadata stand in counts 0 requests.

## Design excerpt
### Decisions
- DEC-007 (SPEC-0097): Both lists empty takes today's exact path: the default `build_client` client, default redirect policy (up to 10 hops), no lookup, no pinning, no new header. The new code runs only when at least one list is non empty.
- DEC-009 (SPEC-0097): The connection goes to the checked address: each hop is sent by a fresh client built with `ClientBuilder::resolve_to_addrs(host, &checked_addresses)` for a host name; an IP literal needs no pin. The client still sets the request timeout from the time left of the call, the user agent `mcp-fs-web/1.0`, and keeps the environment proxy (`auto_sys_proxy`, no `no_proxy()`), the limit the owner accepted (option A, BL-0157 owns the proxy design).
- DEC-010 (SPEC-0097): With a list non empty redirects are followed by hand: a client with `redirect::Policy::none()`, a loop of at most 10 followed hops on statuses 301, 302, 303, 307 and 308, the next address is `Url::join(location)` (so a relative or protocol relative destination resolves against the hop that sent it), and `admit` runs on the next address before it is sent and before the "too many redirects" check. A redirect answer with no usable `Location` is returned as the answer. Failures keep today's prefix `ERR_INTERNAL_ERROR: fetch request failed`.
- DEC-011 (SPEC-0097): Name lookup uses `tokio::net::lookup_host((host, 0))` bounded by the call's deadline with `tokio::time::timeout_at`. Any error, an empty answer, or no answer counts as "resolves to no address"; a lookup that overruns the deadline fails the call with `ERR_INTERNAL_ERROR: request timed out`. A host with no address is never sent: it fails with `ERR_INTERNAL_ERROR: fetch request failed: no address for host '<site>'` unless the lists refuse it first. The deadline is `timeout_secs` for the whole call, shared by the lookups, the hops and the body read.
- DEC-012 (SPEC-0097): The `<site>` of a message is read from `url::Url::host_str()` of the address as parsed (lower case, ASCII form), with one trailing dot removed and the brackets of an IPv6 address removed. `Ipv6Addr::to_string` is never used for it: Rust prints an IPv4 mapped address as `::ffff:127.0.0.1` while the spec wants `::ffff:7f00:1`. The message is `ToolError::forbidden("host '<site>' is not allowed by the web access lists")`, so it renders `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. The resolved addresses never enter a message.
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
| `crates/core/src/tools/web_access.rs` | modify (the hop loop, `Url::join`, decision before the hop limit) |
| `crates/core/src/tools/web.rs` | modify (`fetch_raw` uses the hop loop when a list is non empty) |
| `crates/core/src/tools/web_access_e2e.rs` | modify |
| `.agent_docs/config.md` | modify (redirects) |
| `README.md` | modify (one paragraph: the access lists) |

### Existing patterns
Manual redirect loop: `crates/core/src/git/provider/mod.rs:500-548` (follows `Location` with `Url::join`, bounded by `MAX_REDIRECTS`).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

Cast used by every test of this story (verbatim from the spec):

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

Test level and location: exploit in `crates/core/src/tools/web_access_e2e.rs`; functional e2e in `crates/core/src/tools/web_access_e2e.rs`. Function names `e2e_0097_<NNN>_<slug>` / `e2e_0097_sec_<NNN>_<slug>`. Cleanup: each test builds its own temporary project and stand ins; nothing to tear down.

Exploit tests (verbatim preface from the spec): Each test carries the literal payload. Each asserts the refusal AND, through a second channel, that the stand in site received 0 requests. Red before the fix because: with both lists empty at that time, the call returns the stand in's body instead of a refusal (`expected the refusal, got the page text`).

### SPEC-0097/E2E-035: redirect to an allowed site
- **Category:** happy. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example`; `news.example` redirects to `docs.example`. **When** Alice fetches `http://news.example/`. **Then** she receives `Manual v1`.

### SPEC-0097/E2E-036: redirect to a refused site
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** white list `news.example`; `news.example` redirects to `http://docs.example/`. **When** Alice fetches `http://news.example/`. **Then** she gets `ERR_FORBIDDEN: host 'docs.example' is not allowed by the web access lists`. **And** a stand in for `docs.example` counts 0 requests.

### SPEC-0097/E2E-037: second hop of a chain refused
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example`; `news.example` redirects to `docs.example`, which redirects to `http://blocked.example/`. **When** Alice fetches `http://news.example/`. **Then** she gets the refusal naming `blocked.example`.

### SPEC-0097/E2E-038: download refused at a hop saves nothing
- **Category:** side effect. **Scenario:** SC-005. **Requirements:** FR-NEW-007, FR-NEW-009
- **Given** white list `news.example`; `news.example` redirects to `docs.example`. **When** Alice downloads `http://news.example/f.pdf` into `/f.pdf`. **Then** she gets the refusal naming `docs.example`. **And** `/f.pdf` does not exist.

### SPEC-0097/E2E-064: a redirect loop keeps today's failure
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` redirecting to itself without end. **When** Alice fetches `http://news.example/`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed` and is not the lists refusal.

### SPEC-0097/E2E-074: a redirect without a destination, black list set
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` answering status 302 with no destination and the body `Moved`. **When** Alice fetches `http://news.example/`. **Then** she receives `Moved`, not `ERR_INTERNAL_ERROR` and not the lists refusal.

### SPEC-0097/E2E-077: a hop to a refused site on a non web scheme
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` redirecting to `ftp://blocked.example/`. **When** Alice fetches `http://news.example/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-078: a hop to an allowed site on a non web scheme
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` redirecting to `ftp://docs.example/`. **When** Alice fetches `http://news.example/`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed`.

### SPEC-0097/E2E-079: the refused site is the eleventh hop
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and a chain of 10 allowed hops whose next hop leads to `http://blocked.example/`. **When** Alice fetches the first. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`, not the too many hops failure.

### SPEC-0097/E2E-088: a protocol relative redirect to a listed site
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` redirecting to `//blocked.example/x`. **When** Alice fetches `http://news.example/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`. **And** a stand in for `blocked.example` counts 0 requests.

### SPEC-0097/E2E-SEC-002: redirect from a public page to the metadata address
- **Covers:** occurrence #1. **Requirements:** FR-NEW-007
- **Given** black list `169.254.0.0/16`; `news.example` answers with a redirect to `http://169.254.169.254/latest/meta-data/`. **When** Alice fetches `http://news.example/`. **Then** she gets the refusal naming `169.254.169.254`. **And** the stand in at that address counts 0 requests.

## Constraints
- Files not to touch: `spec.md`, `tool-contract-golden.json`.
- Dependencies not to add: any.
- The lists decision on a hop comes BEFORE the hop limit and before the scheme error (DEC-010).
- The default path (both lists empty) keeps reqwest's default redirect policy (DEC-007).
- Statuses followed: 301, 302, 303, 307, 308; maximum 10 followed hops.
- Closing story: the coverage audit of the spec (every FR-NEW cited by a story) must be re checked before the commit.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs`, the two configuration snapshots (`config.rs:1710-1711`) and `tool-contract-golden.json` pass unmodified; both lists empty behaves exactly as before.
- [ ] Docs updated: `.agent_docs/config.md` (web section), `README.md`.
- [ ] Self review (full): every requirement above has its tests green, no `unwrap` or `expect` on a fallible path, no dash used as punctuation in code or comments, comments explain why.
- [ ] Closes spec: the spec's full acceptance set green (every E2E- of SPEC-0097, 100 tests), and every requirement FR-NEW-001 to FR-NEW-012 cited by at least one story with status done or landed.
- [ ] One commit `US-0254: redirects are checked hop by hop, and the spec is closed` on the work branch `feat/0156-web-access-lists`, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0097` and the requirement ids.
