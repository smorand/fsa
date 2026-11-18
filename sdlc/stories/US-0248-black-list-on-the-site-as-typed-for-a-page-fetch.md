# US-0248: a black list refuses the site as typed for `web.fetch` returning text

> Spec: SPEC-0097 (sdlc/specs/SPEC-0097-web-access-lists/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0247
> min_tier: 2
> Files touched: 5
> Closes spec: no

## Objective
With a black list set, `web.fetch` (text mode) refuses a site whose name, wildcard name, address or range matches an entry, with the exact refusal, and never sends the request; with both lists empty nothing changes. The decision is made on the site as the request types it (no name lookup yet).

## Requirements covered
- SPEC-0097/FR-NEW-001: WHILE both lists are empty THE web access context SHALL let `web.fetch` and `web.download` reach any site, exactly as before this change.
- SPEC-0097/FR-NEW-002: WHEN the black list has at least one entry and the white list is empty THE web access context SHALL refuse a request that matches the black list and SHALL serve every other request.
- SPEC-0097/FR-NEW-005: The web access context SHALL accept as a list entry exactly one of: a host name (`docs.example`), a name with a leading wildcard (`*.corp.example`), an IPv4 or IPv6 address (`10.0.0.5`, `fd12::1`), or an address range written with a slash (`169.254.0.0/16`, `fd00::/8`).
- SPEC-0097/FR-NEW-008: WHEN a request is refused by the lists THE web access context SHALL answer `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`, SHALL send nothing to that site, and SHALL NOT show any resolved address.

## Independent Test
Run `cargo test -p mcp-fs-core --lib -- web_access_e2e web_bound_e2e tool_surface_e2e`: the 14 tests of this story are green and the 67 existing web tests pass unmodified. A fetch of `http://blocked.example/` with black list `blocked.example` answers `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists` through the MCP tool call, not a mock.

## Design excerpt
### Decisions
- DEC-002 (SPEC-0097): One new module `crates/core/src/tools/web_access.rs` owns entry parsing, site reading, matching, admission (decision plus name lookup) and the redirect following loop. `tools/web.rs` only calls it.
- DEC-006 (SPEC-0097): Fail closed at registration: `web::register` builds the lists with `AccessLists::from_config`; if it fails (only reachable by a configuration that skipped `validate`, as some tests do), every call of `web.fetch` and `web.download` returns that error instead of reaching the network. No `expect` or `unwrap`.
- DEC-007 (SPEC-0097): Both lists empty takes today's exact path: the default `build_client` client, default redirect policy (up to 10 hops), no lookup, no pinning, no new header. The new code runs only when at least one list is non empty.
- DEC-008 (SPEC-0097): `web.download`, `web.fetch` with a destination and `web.fetch` returning text each call `AccessLists::admit(url, deadline)` right after their argument, membership and path checks and before `write_limit`, and pass the returned `Admission` (site, pinned addresses) to the sender. A refusal therefore precedes the storage allowance refusal (E2E-051), a non member never reaches it (E2E-049), and the single name lookup of a first hop is shared by the check and the connection.
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
| `crates/core/src/tools/web_access.rs` | modify (site reading, `Site`, black list decision, `admit` for a typed site, `TestNet` site table, refusal) |
| `crates/core/src/tools/web.rs` | modify (`register` builds the lists; the text mode of `web.fetch` calls `admit`) |
| `crates/core/src/config.rs` | modify (`#[cfg(test)] test_net` field, Debug untouched) |
| `crates/core/src/tools/web_access_e2e.rs` | create (`served_with_lists`, stand in helpers, tests) |
| `.agent_docs/config.md` | modify (decision rule, black list) |

### Existing patterns
MCP tool call through the served app and a raw TCP stand in: `crates/core/src/tools/web_bound_e2e.rs:57-100` (`served`, `stand_in`, `call_tool`); the helper `served_with_base` at `crates/core/src/tools/web_testkit.rs:298-318` is the model for `served_with_lists`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

Cast used by every test of this story (verbatim from the spec):

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

Test level and location: functional e2e in `crates/core/src/tools/web_access_e2e.rs`. Function names `e2e_0097_<NNN>_<slug>` / `e2e_0097_sec_<NNN>_<slug>`. Cleanup: each test builds its own temporary project and stand ins; nothing to tear down.

### SPEC-0097/E2E-001: fetch with empty lists
- **Category:** happy. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty. **When** Alice asks `web.fetch` for `http://news.example/`. **Then** she receives `Daily headlines`.

### SPEC-0097/E2E-003: own machine reachable with empty lists
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty and a stand in on `127.0.0.1` answering `Local`. **When** Alice fetches it. **Then** she receives `Local`.

### SPEC-0097/E2E-004: unreachable site, empty lists
- **Category:** failure. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty and `down.example` that refuses connections. **When** Alice fetches `http://down.example/`. **Then** she gets the same failure message as before this change (`fetch request failed`, never the lists refusal).

### SPEC-0097/E2E-005: wrong start of address, empty lists
- **Category:** failure. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty. **When** Alice fetches `ftp://news.example/`. **Then** she gets `url must start with http:// or https://`.

### SPEC-0097/E2E-007: unlisted site served
- **Category:** happy. **Scenario:** SC-002. **Requirements:** FR-NEW-002
- **Given** black list `blocked.example`. **When** Alice fetches `http://news.example/`. **Then** she receives `Daily headlines`.

### SPEC-0097/E2E-008: listed name refused
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-008
- **Given** black list `blocked.example`. **When** Alice fetches `http://blocked.example/`. **Then** she gets exactly `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-009: wildcard refuses a subdomain
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-005
- **Given** black list `*.corp.example`. **When** Alice fetches `http://wiki.corp.example/` and then `http://a.b.corp.example/`. **Then** both get the refusal naming the site typed.

### SPEC-0097/E2E-010: exact address refused
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-005
- **Given** black list `10.0.0.5`. **When** Alice fetches `http://10.0.0.5/`. **Then** she gets `ERR_FORBIDDEN: host '10.0.0.5' is not allowed by the web access lists`.

### SPEC-0097/E2E-012: IPv6 range refused
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-005
- **Given** black list `fd00::/8`. **When** Alice fetches `http://[fd12::1]/`. **Then** she gets `ERR_FORBIDDEN: host 'fd12::1' is not allowed by the web access lists`.

### SPEC-0097/E2E-013: wildcard does not match the bare name
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `*.corp.example` and a site `corp.example` answering `Corp home`. **When** Alice fetches `http://corp.example/`. **Then** she receives `Corp home`.

### SPEC-0097/E2E-014: letter case never matters
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `BLOCKED.Example`. **When** Alice fetches `http://Blocked.EXAMPLE/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-015: neighbour address outside an entry
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `10.0.0.5` and a site at `10.0.0.6` answering `Six`. **When** Alice fetches `http://10.0.0.6/`. **Then** she receives `Six`.

### SPEC-0097/E2E-016: range boundaries
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `10.0.0.0/24` and a site at `10.0.1.0` answering `Next`. **When** Alice fetches `http://10.0.0.0/`, then `http://10.0.0.255/`, then `http://10.0.1.0/`. **Then** the first two are refused and the third returns `Next`.

### SPEC-0097/E2E-071: a redirect without a destination, empty lists
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty and `news.example` answering status 302 with no destination and the body `Moved`. **When** Alice fetches `http://news.example/`. **Then** she receives `Moved`, not `ERR_INTERNAL_ERROR` and not the lists refusal.

## Constraints
- Files not to touch: `spec.md`, `tool-contract-golden.json`, `TOOL_CONTRACT.txt` (no schema change).
- Dependencies not to add: any.
- Both lists empty must take today's path untouched (DEC-007).
- Scope boundary: no name lookup (US-0252), no white list (US-0250), no download or save mode (US-0251), no redirects (US-0254).
- Interim rule of this story: a host name that matches no black entry is sent as today, with the test seam rewriting it to the stand in after the decision.
- Never put a resolved address in a message (DEC-012).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs`, the two configuration snapshots (`config.rs:1710-1711`) and `tool-contract-golden.json` pass unmodified; both lists empty behaves exactly as before.
- [ ] Docs updated: `.agent_docs/config.md` (web section).
- [ ] Self review (full): every requirement above has its tests green, no `unwrap` or `expect` on a fallible path, no dash used as punctuation in code or comments, comments explain why.
- [ ] One commit `US-0248: a black list refuses the site as typed for `web.fetch` returning text` on the work branch `feat/0156-web-access-lists`, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0097` and the requirement ids.
