# US-0251: downloads and saved pages are refused cleanly, in the specified order

> Spec: SPEC-0097 (sdlc/specs/SPEC-0097-web-access-lists/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0250
> min_tier: 2
> Files touched: 3
> Closes spec: no

## Objective
`web.download` and `web.fetch` with a destination obey the lists exactly like the text mode, in the specified order: argument, membership and path, lists, storage allowance, request. A refused save creates no file, changes none and leaves the storage use as it was; a refusal applies whatever the scheme of the address in the save mode.

## Requirements covered
- SPEC-0097/FR-NEW-001: WHILE both lists are empty THE web access context SHALL let `web.fetch` and `web.download` reach any site, exactly as before this change.
- SPEC-0097/FR-NEW-002: WHEN the black list has at least one entry and the white list is empty THE web access context SHALL refuse a request that matches the black list and SHALL serve every other request.
- SPEC-0097/FR-NEW-003: WHEN the white list has at least one entry and the black list is empty THE web access context SHALL serve a request that matches the white list and SHALL refuse every other request.
- SPEC-0097/FR-NEW-004: WHEN both lists have at least one entry THE web access context SHALL refuse a request that matches the black list, and among the others SHALL serve only a request that matches the white list.
- SPEC-0097/FR-NEW-008: WHEN a request is refused by the lists THE web access context SHALL answer `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`, SHALL send nothing to that site, and SHALL NOT show any resolved address.
- SPEC-0097/FR-NEW-009: WHEN a request that saves into a project is refused by the lists THE web access context SHALL NOT create or change any file, and the project's storage use SHALL be unchanged.

## Independent Test
Run `cargo test -p mcp-fs-core --lib -- web_access_e2e`: the 12 tests of this story are green. `web.download` of `http://169.254.169.254/latest/meta-data/iam/security-credentials/` into `/creds.txt` with black list `169.254.0.0/16` answers the refusal, `/creds.txt` does not exist and the stand in counts 0 requests.

## Design excerpt
### Decisions
- DEC-007 (SPEC-0097): Both lists empty takes today's exact path: the default `build_client` client, default redirect policy (up to 10 hops), no lookup, no pinning, no new header. The new code runs only when at least one list is non empty.
- DEC-008 (SPEC-0097): `web.download`, `web.fetch` with a destination and `web.fetch` returning text each call `AccessLists::admit(url, deadline)` right after their argument, membership and path checks and before `write_limit`, and pass the returned `Admission` (site, pinned addresses) to the sender. A refusal therefore precedes the storage allowance refusal (E2E-051), a non member never reaches it (E2E-049), and the single name lookup of a first hop is shared by the check and the connection.
- DEC-012 (SPEC-0097): The `<site>` of a message is read from `url::Url::host_str()` of the address as parsed (lower case, ASCII form), with one trailing dot removed and the brackets of an IPv6 address removed. `Ipv6Addr::to_string` is never used for it: Rust prints an IPv4 mapped address as `::ffff:127.0.0.1` while the spec wants `::ffff:7f00:1`. The message is `ToolError::forbidden("host '<site>' is not allowed by the web access lists")`, so it renders `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. The resolved addresses never enter a message.
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
| `crates/core/src/tools/web.rs` | modify (`web.download` and the save mode of `web.fetch` call `admit` after membership and path, before `write_limit`) |
| `crates/core/src/tools/web_access_e2e.rs` | modify |
| `.agent_docs/config.md` | modify (order of refusals) |

### Existing patterns
Save and quota assertions: `crates/core/src/tools/web_bound_e2e.rs:700-830` (`web.download` and save mode tests against `served`).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

Cast used by every test of this story (verbatim from the spec):

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

Test level and location: exploit in `crates/core/src/tools/web_access_e2e.rs`; functional e2e in `crates/core/src/tools/web_access_e2e.rs`. Function names `e2e_0097_<NNN>_<slug>` / `e2e_0097_sec_<NNN>_<slug>`. Cleanup: each test builds its own temporary project and stand ins; nothing to tear down.

Exploit tests (verbatim preface from the spec): Each test carries the literal payload. Each asserts the refusal AND, through a second channel, that the stand in site received 0 requests. Red before the fix because: with both lists empty at that time, the call returns the stand in's body instead of a refusal (`expected the refusal, got the page text`).

### SPEC-0097/E2E-002: download with empty lists
- **Category:** happy. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty. **When** Alice asks `web.download` for `http://docs.example/manual.pdf` into `/manual.pdf` of `proj`. **Then** the call succeeds. **And** `/manual.pdf` holds `Manual v1`.

### SPEC-0097/E2E-006: download counts storage, empty lists
- **Category:** side effect. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty and `proj` using 100 bytes. **When** Alice downloads `http://docs.example/manual.pdf` (page `Manual v1`, 9 bytes) into `/manual.pdf`. **Then** `proj` uses 109 bytes.

### SPEC-0097/E2E-027: allowed fetch with a destination saves
- **Category:** side effect. **Scenario:** SC-003. **Requirements:** FR-NEW-003
- **Given** white list `docs.example`. **When** Alice fetches `http://docs.example/` with destination `/page.html` in `proj`. **Then** `/page.html` holds `Manual v1`.

### SPEC-0097/E2E-033: allowed download under both lists saves
- **Category:** side effect. **Scenario:** SC-004. **Requirements:** FR-NEW-004
- **Given** white list `*.docs.example`, black list `secret.docs.example`. **When** Alice downloads `http://www.docs.example/m.pdf` (`Public manual`) into `/m.pdf`. **Then** `/m.pdf` holds `Public manual`.

### SPEC-0097/E2E-048: allowed download under a black list
- **Category:** happy. **Scenario:** SC-008. **Requirements:** FR-NEW-009
- **Given** black list `blocked.example`. **When** Alice downloads `http://docs.example/m.pdf` into `/m.pdf`. **Then** `/m.pdf` holds `Manual v1`.

### SPEC-0097/E2E-049: a non member sees the membership refusal
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-008
- **Given** black list `blocked.example`. **When** Bob asks `web.download` for `http://blocked.example/x` into `proj`. **Then** he gets `ERR_FORBIDDEN: 'bob@example.com' is not a member of 'proj'`, not the lists refusal.

### SPEC-0097/E2E-050: an existing file is untouched by a refused save
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-009
- **Given** black list `blocked.example` and `/page.html` holding `Old`. **When** Alice fetches `http://blocked.example/` with destination `/page.html`. **Then** she gets the refusal. **And** `/page.html` still holds `Old`.

### SPEC-0097/E2E-051: storage use unchanged, and the lists refusal comes first
- **Category:** side effect. **Scenario:** SC-008. **Requirements:** FR-NEW-009, FR-NEW-008
- **Given** black list `blocked.example`, `proj` using 100 bytes and its storage allowance fully used. **When** Alice downloads `http://blocked.example/m.pdf` into `/m.pdf`. **Then** she gets the lists refusal, not a storage refusal. **And** `proj` still uses 100 bytes.

### SPEC-0097/E2E-076: refusal whatever the scheme, in save mode
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-008
- **Given** black list `blocked.example`. **When** Alice fetches `ftp://blocked.example/` with destination `/f.txt`, and then `ftp://news.example/` with destination `/g.txt`. **Then** the first gives `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`; the second gives an answer starting with `ERR_INTERNAL_ERROR: fetch request failed`. **And** neither `/f.txt` nor `/g.txt` exists.

### SPEC-0097/E2E-SEC-001: cloud metadata credentials
- **Covers:** class sweep occurrence #1 (Section 14.2). **Requirements:** FR-NEW-002, FR-NEW-008
- **Given** black list `169.254.0.0/16` and a stand in at `169.254.169.254` answering `AccessKeyId=AKIAEXAMPLE`. **When** Alice fetches `http://169.254.169.254/latest/meta-data/iam/security-credentials/`. **Then** she gets `ERR_FORBIDDEN: host '169.254.169.254' is not allowed by the web access lists`, and her answer does not contain `AKIAEXAMPLE`. **And** the stand in counts 0 requests.

### SPEC-0097/E2E-SEC-006: download of the metadata address
- **Covers:** occurrence #1 reached through `web.download`. **Requirements:** FR-NEW-002, FR-NEW-009
- **Given** black list `169.254.0.0/16` and the same stand in. **When** Alice asks `web.download` for `http://169.254.169.254/latest/meta-data/iam/security-credentials/` into `/creds.txt`. **Then** she gets the refusal. **And** `/creds.txt` does not exist and the stand in counts 0 requests.

### SPEC-0097/E2E-SEC-007: fetch with a destination of the metadata address
- **Covers:** occurrence #1 reached through the save mode of `web.fetch`. **Requirements:** FR-NEW-002, FR-NEW-009
- **Given** the same setup. **When** Alice fetches that address with destination `/creds.txt`. **Then** she gets the refusal. **And** `/creds.txt` does not exist and the stand in counts 0 requests.

## Constraints
- Files not to touch: `spec.md`, `crates/core/src/tools/fs_ops` (the write path is unchanged).
- Dependencies not to add: any.
- `admit` must run after `volume()` and `norm()` and before `write_limit` in all three paths (DEC-008).
- In the save mode of `web.fetch` there is no `http://` prefix check today; do not add one (FR-NEW-008, E2E-076).
- Scope boundary: name lookups and redirects.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs`, the two configuration snapshots (`config.rs:1710-1711`) and `tool-contract-golden.json` pass unmodified; both lists empty behaves exactly as before.
- [ ] Docs updated: `.agent_docs/config.md` (web section).
- [ ] Self review (full): every requirement above has its tests green, no `unwrap` or `expect` on a fallible path, no dash used as punctuation in code or comments, comments explain why.
- [ ] One commit `US-0251: downloads and saved pages are refused cleanly, in the specified order` on the work branch `feat/0156-web-access-lists`, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0097` and the requirement ids.
