# US-0250: a white list, and a white list minus a black list

> Spec: SPEC-0097 (sdlc/specs/SPEC-0097-web-access-lists/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0249
> min_tier: 2
> Files touched: 3
> Closes spec: no

## Objective
With a white list set, only a site whose name or address matches an entry is served; with both lists, a site must match the white list and not the black list, and a site named in both is refused. The decision table of the four combinations is documented in one table.

## Requirements covered
- SPEC-0097/FR-NEW-003: WHEN the white list has at least one entry and the black list is empty THE web access context SHALL serve a request that matches the white list and SHALL refuse every other request.
- SPEC-0097/FR-NEW-004: WHEN both lists have at least one entry THE web access context SHALL refuse a request that matches the black list, and among the others SHALL serve only a request that matches the white list.
- SPEC-0097/FR-NEW-005: The web access context SHALL accept as a list entry exactly one of: a host name (`docs.example`), a name with a leading wildcard (`*.corp.example`), an IPv4 or IPv6 address (`10.0.0.5`, `fd12::1`), or an address range written with a slash (`169.254.0.0/16`, `fd00::/8`).
- SPEC-0097/FR-NEW-008: WHEN a request is refused by the lists THE web access context SHALL answer `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`, SHALL send nothing to that site, and SHALL NOT show any resolved address.

## Independent Test
Run `cargo test -p mcp-fs-core --lib -- web_access_e2e`: the nine tests of this story and the earlier ones are green. With white list `docs.example`, a fetch of `http://news.example/` answers `ERR_FORBIDDEN: host 'news.example' is not allowed by the web access lists` and a fetch of `http://docs.example/` answers `Manual v1`.

## Design excerpt
### Decisions
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
| `crates/core/src/tools/web_access.rs` | modify (white list decision, the four combinations) |
| `crates/core/src/tools/web_access_e2e.rs` | modify |
| `.agent_docs/config.md` | modify (the table of the four combinations) |

### Existing patterns
The black list decision of US-0248 in `crates/core/src/tools/web_access.rs` (`admit`); extend it, do not duplicate it.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

Cast used by every test of this story (verbatim from the spec):

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

Test level and location: functional e2e in `crates/core/src/tools/web_access_e2e.rs`. Function names `e2e_0097_<NNN>_<slug>` / `e2e_0097_sec_<NNN>_<slug>`. Cleanup: each test builds its own temporary project and stand ins; nothing to tear down.

### SPEC-0097/E2E-019: white listed name served
- **Category:** happy. **Scenario:** SC-003. **Requirements:** FR-NEW-003
- **Given** white list `docs.example`. **When** Alice fetches `http://docs.example/`. **Then** she receives `Manual v1`.

### SPEC-0097/E2E-020: white listed range served
- **Category:** happy. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-005
- **Given** white list `10.0.0.0/24` and a site at `10.0.0.7` answering `Seven`. **When** Alice fetches `http://10.0.0.7/`. **Then** she receives `Seven`.

### SPEC-0097/E2E-021: other site refused
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-008
- **Given** white list `docs.example`. **When** Alice fetches `http://news.example/`. **Then** she gets `ERR_FORBIDDEN: host 'news.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-022: metadata address not white listed
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-003
- **Given** white list `docs.example`. **When** Alice fetches `http://169.254.169.254/latest/meta-data/`. **Then** she gets `ERR_FORBIDDEN: host '169.254.169.254' is not allowed by the web access lists`.

### SPEC-0097/E2E-024: wildcard in the white list excludes the bare name
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-005
- **Given** white list `*.docs.example` and a site `a.docs.example` answering `Sub`. **When** Alice fetches `http://a.docs.example/` then `http://docs.example/`. **Then** the first returns `Sub` and the second is refused.

### SPEC-0097/E2E-029: white minus black, served
- **Category:** happy. **Scenario:** SC-004. **Requirements:** FR-NEW-004
- **Given** white list `*.docs.example`, black list `secret.docs.example`. **When** Alice fetches `http://www.docs.example/` (page `Public manual`). **Then** she receives `Public manual`.

### SPEC-0097/E2E-030: a site in both lists is refused
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-004
- **Given** white list `*.docs.example`, black list `secret.docs.example`. **When** Alice fetches `http://secret.docs.example/`. **Then** she gets `ERR_FORBIDDEN: host 'secret.docs.example' is not allowed by the web access lists`.

### SPEC-0097/E2E-031: outside the white list, refused
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-004
- **Given** white list `*.docs.example`, black list `secret.docs.example`. **When** Alice fetches `http://news.example/`. **Then** she gets the refusal naming `news.example`.

### SPEC-0097/E2E-032: white range minus one black address
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-004, FR-NEW-005
- **Given** white list `10.0.0.0/24`, black list `10.0.0.5`. **When** Alice fetches `http://10.0.0.5/` and then `http://10.0.0.6/` (page `Six`). **Then** the first is refused and the second returns `Six`.

## Constraints
- Files not to touch: `crates/core/src/tools/web.rs`, `spec.md`.
- Dependencies not to add: any.
- Interim rule of this story, refined by US-0252: a host name that does not match a white name entry is refused without looking it up.
- Black is decided first, then the white rule (DEC-013).
- Scope boundary: resolved addresses, download and save mode, redirects.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs`, the two configuration snapshots (`config.rs:1710-1711`) and `tool-contract-golden.json` pass unmodified; both lists empty behaves exactly as before.
- [ ] Docs updated: `.agent_docs/config.md` (web section).
- [ ] Self review (full): every requirement above has its tests green, no `unwrap` or `expect` on a fallible path, no dash used as punctuation in code or comments, comments explain why.
- [ ] One commit `US-0250: a white list, and a white list minus a black list` on the work branch `feat/0156-web-access-lists`, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0097` and the requirement ids.
