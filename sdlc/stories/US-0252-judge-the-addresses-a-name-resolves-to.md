# US-0252: judge the addresses a site name resolves to, and connect to the checked address

> Spec: SPEC-0097 (sdlc/specs/SPEC-0097-web-access-lists/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0251
> min_tier: 1
> Files touched: 4
> Closes spec: no

## Objective
A site name is looked up once; the black list refuses it when any resolved address matches, the white list serves it when every resolved address matches, a lookup that ends in an error counts as no address and is never sent, a lookup that overruns the request timeout fails with `request timed out`, and the request is sent to the very address that was checked, so a name that answers differently the second time cannot reach a refused address.

## Requirements covered
- SPEC-0097/FR-NEW-002: WHEN the black list has at least one entry and the white list is empty THE web access context SHALL refuse a request that matches the black list and SHALL serve every other request.
- SPEC-0097/FR-NEW-003: WHEN the white list has at least one entry and the black list is empty THE web access context SHALL serve a request that matches the white list and SHALL refuse every other request.
- SPEC-0097/FR-NEW-006: WHEN a request names a site THE web access context SHALL decide the black list on the site as typed and on every address the site resolves to, and the white list on the site as typed or on all of its resolved addresses.
- SPEC-0097/FR-NEW-009: WHEN a request that saves into a project is refused by the lists THE web access context SHALL NOT create or change any file, and the project's storage use SHALL be unchanged.

## Independent Test
Run `cargo test -p mcp-fs-core --lib -- web_access_e2e`: the 13 tests of this story are green. With black list `127.0.0.0/8`, a name that answers `93.184.216.34` first and `127.0.0.1` afterwards is fetched without the loopback stand in counting a request (E2E-SEC-009), and the name table's lookup counter reads 1 for that call.

## Design excerpt
### Decisions
- DEC-009 (SPEC-0097): The connection goes to the checked address: each hop is sent by a fresh client built with `ClientBuilder::resolve_to_addrs(host, &checked_addresses)` for a host name; an IP literal needs no pin. The client still sets the request timeout from the time left of the call, the user agent `mcp-fs-web/1.0`, and keeps the environment proxy (`auto_sys_proxy`, no `no_proxy()`), the limit the owner accepted (option A, BL-0157 owns the proxy design).
- DEC-011 (SPEC-0097): Name lookup uses `tokio::net::lookup_host((host, 0))` bounded by the call's deadline with `tokio::time::timeout_at`. Any error, an empty answer, or no answer counts as "resolves to no address"; a lookup that overruns the deadline fails the call with `ERR_INTERNAL_ERROR: request timed out`. A host with no address is never sent: it fails with `ERR_INTERNAL_ERROR: fetch request failed: no address for host '<site>'` unless the lists refuse it first. The deadline is `timeout_secs` for the whole call, shared by the lookups, the hops and the body read.
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
| `crates/core/src/tools/web_access.rs` | modify (name lookup bounded by the deadline, address judgement, `Admission` with pinned addresses, per hop client with `resolve_to_addrs`, test name table) |
| `crates/core/src/tools/web.rs` | modify (`fetch_raw` takes the `Admission` and the deadline) |
| `crates/core/src/tools/web_access_e2e.rs` | modify |
| `.agent_docs/config.md` | modify (address matching, lookup errors and timeout) |

### Existing patterns
Bounded wait: `tokio::time::timeout_at` on a `tokio::time::Instant` deadline; the per hop client builder is `build_client` (`crates/core/src/tools/web.rs:260-266`) extended with `resolve_to_addrs`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

Cast used by every test of this story (verbatim from the spec):

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

Test level and location: exploit in `crates/core/src/tools/web_access_e2e.rs`; functional e2e in `crates/core/src/tools/web_access_e2e.rs`. Function names `e2e_0097_<NNN>_<slug>` / `e2e_0097_sec_<NNN>_<slug>`. Cleanup: each test builds its own temporary project and stand ins; nothing to tear down.

Exploit tests (verbatim preface from the spec): Each test carries the literal payload. Each asserts the refusal AND, through a second channel, that the stand in site received 0 requests. Red before the fix because: with both lists empty at that time, the call returns the stand in's body instead of a refusal (`expected the refusal, got the page text`).

### SPEC-0097/E2E-011: a name for the own machine is refused
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-006
- **Given** black list `127.0.0.0/8`. **When** Alice fetches `http://localhost/`. **Then** she gets `ERR_FORBIDDEN: host 'localhost' is not allowed by the web access lists`, and the message contains no `127.0.0.1`.

### SPEC-0097/E2E-023: unresolvable name is not white listed by address
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-006
- **Given** white list `10.0.0.0/24` and a name `ghost.example` that resolves to no address. **When** Alice fetches `http://ghost.example/`. **Then** she gets `ERR_FORBIDDEN: host 'ghost.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-025: a name resolving inside a white listed range
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** white list `10.1.2.0/24`. **When** Alice fetches `http://friendly.example/`. **Then** she receives `Internal payroll`.

### SPEC-0097/E2E-026: a name resolving partly outside the white list
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** white list `10.1.2.0/24`. **When** Alice fetches `http://mixed.example/`. **Then** she gets `ERR_FORBIDDEN: host 'mixed.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-039: a name pointing to a public address
- **Category:** happy. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `10.0.0.0/8`. **When** Alice fetches `http://news.example/` (resolves to `93.184.216.34`). **Then** she receives `Daily headlines`.

### SPEC-0097/E2E-040: two addresses, one black listed
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `10.0.0.0/8`. **When** Alice fetches `http://mixed.example/`. **Then** she gets `ERR_FORBIDDEN: host 'mixed.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-052: refused name leaves no file
- **Category:** side effect. **Scenario:** SC-006. **Requirements:** FR-NEW-006, FR-NEW-009
- **Given** black list `10.0.0.0/8`. **When** Alice fetches `http://friendly.example/` with destination `/p.html`. **Then** she gets the refusal naming `friendly.example`. **And** `/p.html` does not exist.

### SPEC-0097/E2E-054: unresolvable name under a black list
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-006
- **Given** black list `10.0.0.0/8` and a name `ghost.example` that resolves to no address. **When** Alice fetches `http://ghost.example/`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed` and is not the lists refusal.

### SPEC-0097/E2E-055: unresolvable name, download
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-006, FR-NEW-009
- **Given** black list `10.0.0.0/8` and `ghost.example` resolving to no address. **When** Alice downloads `http://ghost.example/g.pdf` into `/g.pdf`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed`. **And** `/g.pdf` does not exist.

### SPEC-0097/E2E-087: a lookup that times out
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** white list `docs.example` and a name `slow.example` whose lookup does not answer within the request timeout. **When** Alice fetches `http://slow.example/` with `timeout_secs` 1. **Then** she gets `ERR_INTERNAL_ERROR: request timed out`.

### SPEC-0097/E2E-SEC-003: a name that points to the own machine
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8`; `evil.example` resolves to `127.0.0.1` where a stand in answers `ADMIN PANEL`. **When** Alice fetches `http://evil.example/admin`. **Then** she gets the refusal naming `evil.example` without `127.0.0.1`. **And** the stand in counts 0 requests.

### SPEC-0097/E2E-SEC-008: a name resolving to an IPv4 mapped IPv6 address
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8`; `evil6.example` resolves to `::ffff:127.0.0.1` where a stand in answers `ADMIN PANEL`. **When** Alice fetches `http://evil6.example/admin`. **Then** she gets `ERR_FORBIDDEN: host 'evil6.example' is not allowed by the web access lists`, without `127.0.0.1`. **And** the stand in counts 0 requests.

### SPEC-0097/E2E-SEC-009: a name whose second lookup differs
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8`; `rebind.example` answers `93.184.216.34` on its first lookup and `127.0.0.1` on every later one; a stand in at `127.0.0.1` answers `ADMIN PANEL`. **When** Alice fetches `http://rebind.example/admin`. **Then** her answer does not contain `ADMIN PANEL`. **And** the stand in at `127.0.0.1` counts 0 requests.

## Constraints
- Files not to touch: `spec.md`, `Cargo.toml` (`tokio::net` is already enabled and used at `app.rs:174`).
- Dependencies not to add: any.
- Do NOT call `no_proxy()` (owner option A); document the proxy limit in `.agent_docs/config.md`.
- The call has ONE deadline: lookups, hops and body read share the `timeout_secs` of the call.
- A host with no address is not sent, and the failure is `ERR_INTERNAL_ERROR: fetch request failed: no address for host '<site>'` unless the lists refuse it first.
- Both lists empty keeps today's path (DEC-007): no lookup.
- Scope boundary: unreadable addresses, numeric forms, the proxy tests and redirects.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs`, the two configuration snapshots (`config.rs:1710-1711`) and `tool-contract-golden.json` pass unmodified; both lists empty behaves exactly as before.
- [ ] Docs updated: `.agent_docs/config.md` (web section).
- [ ] Self review (full): every requirement above has its tests green, no `unwrap` or `expect` on a fallible path, no dash used as punctuation in code or comments, comments explain why.
- [ ] One commit `US-0252: judge the addresses a site name resolves to, and connect to the checked address` on the work branch `feat/0156-web-access-lists`, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0097` and the requirement ids.
