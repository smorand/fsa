# US-0249: the site is read as the web standard reads it, and the other web tools are left alone

> Spec: SPEC-0097 (sdlc/specs/SPEC-0097-web-access-lists/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0248
> min_tier: 2
> Files touched: 3
> Closes spec: no

## Objective
Entry and request are reduced to the same form before comparing: letter case, one trailing dot, accented names to ASCII (`straße.example` stays distinct from `strasse.example`), port and credentials dropped from the refusal, an IPv4 mapped entry covering the plain address, `::/0` covering no plain IPv4 address, a range whose address has bits below its prefix. Web search, news and suggestions keep working whatever the lists hold.

## Requirements covered
- SPEC-0097/FR-NEW-005: The web access context SHALL accept as a list entry exactly one of: a host name (`docs.example`), a name with a leading wildcard (`*.corp.example`), an IPv4 or IPv6 address (`10.0.0.5`, `fd12::1`), or an address range written with a slash (`169.254.0.0/16`, `fd00::/8`).
- SPEC-0097/FR-NEW-006: WHEN a request names a site THE web access context SHALL decide the black list on the site as typed and on every address the site resolves to, and the white list on the site as typed or on all of its resolved addresses.
- SPEC-0097/FR-NEW-008: WHEN a request is refused by the lists THE web access context SHALL answer `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`, SHALL send nothing to that site, and SHALL NOT show any resolved address.
- SPEC-0097/FR-NEW-010: IF a list contains an entry that is not one of the forms of FR-NEW-005 THEN THE server SHALL refuse to start and SHALL say which list and which entry are invalid.
- SPEC-0097/FR-NEW-011: The web access context SHALL NOT apply the lists to web search, web news and web suggestions, nor to any tool other than `web.fetch` and `web.download`.

## Independent Test
Run `cargo test -p mcp-fs-core --lib -- web_access_e2e`: the 14 tests of this story and the 14 of US-0248 are green. A fetch of `http://u:p@Blocked.example:8080/` with black list `blocked.example` answers `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists` with no `u:p` and no `8080` in the answer, and the stand in counts 0 requests.

## Design excerpt
### Decisions
- DEC-004 (SPEC-0097): Entry parsing is `parse_entry(&str) -> Result<Entry, ()>` with `Entry = Name | Below | Addr | Range`. A name is checked against the stricter entry rules label by label (letters, digits, hyphen, underscore, 63 and 253 limits, no empty label, `*` only as the whole first label, numeric last label or `0x` prefix must be a four part IPv4 address, `xn--` must decode), then converted with `url::Host::parse`. A range is `address/length` with a length of one to three digits without sign or leading zero. IPv4 mapped IPv6 entries become IPv4 entries; an IPv6 range of 96 to 128 bits whose first 96 bits are `::ffff:0:0` becomes the IPv4 range with length minus 96; other IPv6 ranges only cover native IPv6 addresses.
- DEC-012 (SPEC-0097): The `<site>` of a message is read from `url::Url::host_str()` of the address as parsed (lower case, ASCII form), with one trailing dot removed and the brackets of an IPv6 address removed. `Ipv6Addr::to_string` is never used for it: Rust prints an IPv4 mapped address as `::ffff:127.0.0.1` while the spec wants `::ffff:7f00:1`. The message is `ToolError::forbidden("host '<site>' is not allowed by the web access lists")`, so it renders `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. The resolved addresses never enter a message.
- DEC-013 (SPEC-0097): Matching rules in code: a request site is a `Domain(ascii)`, an `Ipv4` or an `Ipv6`. An IPv6 address that is IPv4 mapped is judged as its IPv4 address, for requests and for resolved addresses. Black list: refuse if the typed site matches an entry or any resolved address does. White list: serve if the typed site matches a name entry, or the site resolved to at least one address and every address matches an entry (an empty answer is never white listed). Both: refuse on black first, then apply the white rule. `Below` matches a domain that ends with `.` plus the entry and is longer; `Name` is equality.
- DEC-014 (SPEC-0097): The lists apply to `web.fetch` and `web.download` only: `web.search`, `web.news`, `web.suggestions` keep their direct `build_client(10)` calls (`web.rs:331`, `:408`, `:615`), the git tools and `context7.*` are not touched.
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
| `crates/core/src/tools/web_access.rs` | modify (trailing dot, accents, port and credentials, mapped entries and ranges, IPv6 range rules) |
| `crates/core/src/tools/web_access_e2e.rs` | modify |
| `.agent_docs/config.md` | modify (normalization rules) |

### Existing patterns
Stand in request counter: `Record.requests` at `crates/core/src/tools/web_testkit.rs:30-35`; search tools stand in via `WebConfig.test_base` (`crates/core/src/tools/web.rs:306-312`).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

Cast used by every test of this story (verbatim from the spec):

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

Test level and location: functional e2e in `crates/core/src/tools/web_access_e2e.rs`. Function names `e2e_0097_<NNN>_<slug>` / `e2e_0097_sec_<NNN>_<slug>`. Cleanup: each test builds its own temporary project and stand ins; nothing to tear down.

### SPEC-0097/E2E-017: a refused request never reaches the site
- **Category:** side effect. **Scenario:** SC-002. **Requirements:** FR-NEW-008
- **Given** black list `blocked.example` and a stand in for it. **When** Alice fetches it. **Then** she gets the refusal. **And** the stand in counts 0 requests.

### SPEC-0097/E2E-018: web search unaffected
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-011
- **Given** the search site of the product is `search.example` (a stand in answering `Result A`) and black list `search.example`. **When** Alice asks `web.search` for `weather`. **Then** she receives `Result A`.

### SPEC-0097/E2E-028: web news unaffected
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-011
- **Given** the search site of the product is `search.example` (a stand in answering `Headline B`) and white list `docs.example`. **When** Alice asks `web.news` for `economy`. **Then** she receives `Headline B`.

### SPEC-0097/E2E-034: web suggestions unaffected
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-011
- **Given** the search site of the product is `search.example` (a stand in answering `weather today`), white list `docs.example` and black list `secret.docs.example`. **When** Alice asks `web.suggestions` for `weat`. **Then** she receives `weather today`.

### SPEC-0097/E2E-046: upper case wildcard and IPv6 accepted
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["*.Corp.EXAMPLE", "FD12::1"]`. **When** the server starts and Alice fetches `http://wiki.corp.example/`. **Then** the server starts and Alice gets the refusal naming `wiki.corp.example`.

### SPEC-0097/E2E-053: trailing dot does not bypass the list
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `blocked.example`. **When** Alice fetches `http://blocked.example./`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-057: a name written with accents
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `bücher.example`. **When** Alice fetches `http://xn--bcher-kva.example/`. **Then** she gets `ERR_FORBIDDEN: host 'xn--bcher-kva.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-061: a range written with bits below its prefix
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `10.0.0.5/24` and a site at `10.0.0.200` answering `Edge`. **When** Alice fetches `http://10.0.0.200/`. **Then** she gets `ERR_FORBIDDEN: host '10.0.0.200' is not allowed by the web access lists`.

### SPEC-0097/E2E-062: an IPv4 mapped entry covers the plain address
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-005, FR-NEW-006
- **Given** black list `::ffff:127.0.0.1` and a stand in at `127.0.0.1`. **When** Alice fetches `http://127.0.0.1/`. **Then** she gets `ERR_FORBIDDEN: host '127.0.0.1' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

### SPEC-0097/E2E-065: port and credentials are not in the refusal
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-008
- **Given** black list `blocked.example`. **When** Alice fetches `http://u:p@Blocked.example:8080/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`, with no `u:p` and no `8080`.

### SPEC-0097/E2E-073: a mapped range and a range of length zero
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-005, FR-NEW-006
- **Given** a stand in at `10.1.2.3` answering `Ten`, and black list `["::ffff:10.0.0.0/104"]`, then separately `["0.0.0.0/0"]`. **When** Alice fetches `http://10.1.2.3/` in each run. **Then** both times she gets `ERR_FORBIDDEN: host '10.1.2.3' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

### SPEC-0097/E2E-075: an IPv6 range of length zero does not cover plain IPv4
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-005, FR-NEW-006
- **Given** black list `["::/0"]`, a stand in at `127.0.0.1` answering `Local` and a stand in at `fd12::1` answering `V6`. **When** Alice fetches `http://127.0.0.1/`, then `http://[fd12::1]/`. **Then** the first returns `Local` and the second gives `ERR_FORBIDDEN: host 'fd12::1' is not allowed by the web access lists`.

### SPEC-0097/E2E-082: hyphens at the edge of a label, and a valid punycode label
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["-a.example"]`, then `["a-.example"]`, then `["xn--bcher-kva.example"]`. **When** the server starts with each. **Then** it starts each time. **And** with black list `["-a.example"]`, Alice fetching `http://-a.example/` gets `ERR_FORBIDDEN: host '-a.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-084: a name with a sharp s stays distinct
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `straße.example` and a site `strasse.example` answering `Plain`. **When** Alice fetches `http://strasse.example/`, then `http://straße.example/`. **Then** the first returns `Plain` and the second gives `ERR_FORBIDDEN: host 'xn--strae-oqa.example' is not allowed by the web access lists`.

## Constraints
- Files not to touch: `crates/core/src/tools/web.rs` except if a test needs the seam (it should not), `spec.md`.
- Dependencies not to add: any.
- The search, news and suggestions tools are NOT to be wrapped (DEC-014): the three tests assert they still answer.
- Scope boundary: resolved addresses (US-0252), white list (US-0250).
- Use `Url::host_str()` for the site text (DEC-012), not `Ipv6Addr::to_string`.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs`, the two configuration snapshots (`config.rs:1710-1711`) and `tool-contract-golden.json` pass unmodified; both lists empty behaves exactly as before.
- [ ] Docs updated: `.agent_docs/config.md` (web section).
- [ ] Self review (full): every requirement above has its tests green, no `unwrap` or `expect` on a fallible path, no dash used as punctuation in code or comments, comments explain why.
- [ ] One commit `US-0249: the site is read as the web standard reads it, and the other web tools are left alone` on the work branch `feat/0156-web-access-lists`, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0097` and the requirement ids.
