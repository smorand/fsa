# US-0246: accept and refuse web access list entries at boot

> Spec: SPEC-0097 (sdlc/specs/SPEC-0097-web-access-lists/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> min_tier: 2
> Files touched: 5
> Closes spec: no

## Objective
An operator can set two lists of sites, `web.black_list_url` and `web.white_list_url`, and the server refuses to start, whether or not the web tools are on, when an entry is not a host name, a `*.host` name, an IP address or an address range. Nothing is enforced on requests yet: this story delivers the settings and the boot refusal.

## Requirements covered
- SPEC-0097/FR-NEW-005: The web access context SHALL accept as a list entry exactly one of: a host name (`docs.example`), a name with a leading wildcard (`*.corp.example`), an IPv4 or IPv6 address (`10.0.0.5`, `fd12::1`), or an address range written with a slash (`169.254.0.0/16`, `fd00::/8`).
- SPEC-0097/FR-NEW-010: IF a list contains an entry that is not one of the forms of FR-NEW-005 THEN THE server SHALL refuse to start and SHALL say which list and which entry are invalid.
- SPEC-0097/FR-NEW-012: The server program SHALL check every entry of both lists at the start of every command that reads the configuration (`serve`, `purge`, `migrate`), whether or not the web tools are on, and SHALL refuse with the message of FR-NEW-010 and exit with a non zero status before doing any work on an invalid entry. For `migrate` the source configuration is checked first, then the destination, and the first invalid entry found is the one named. The commands `keys`, `token` and `version` do not read the configuration and are unchanged.

## Independent Test
Run `cargo test -p mcp-fs-core --lib -- web_access_boot_e2e` (through `make test`): every listed test is green; then `cargo test -p mcp-fs-core --lib -- config::` still passes unmodified (the two frozen configuration snapshots are unchanged).

## Design excerpt
### Decisions
- DEC-001 (SPEC-0097): No new dependency. Entries and address ranges are parsed with `std::net`; hosts are read and converted to their ASCII form with the `url` crate (WHATWG host parsing, UTS 46 non transitional), which is already a direct dependency.
- DEC-003 (SPEC-0097): Two settings in the existing web section: `web.black_list_url` and `web.white_list_url`, each `Vec<String>` with `#[serde(default)]`, empty by default. The hand written `Debug` of `WebConfig` leaves them out, as it leaves out `test_base`, so the two frozen configuration snapshots stay byte identical and no existing test changes.
- DEC-004 (SPEC-0097): Entry parsing is `parse_entry(&str) -> Result<Entry, ()>` with `Entry = Name | Below | Addr | Range`. A name is checked against the stricter entry rules label by label (letters, digits, hyphen, underscore, 63 and 253 limits, no empty label, `*` only as the whole first label, numeric last label or `0x` prefix must be a four part IPv4 address, `xn--` must decode), then converted with `url::Host::parse`. A range is `address/length` with a length of one to three digits without sign or leading zero. IPv4 mapped IPv6 entries become IPv4 entries; an IPv6 range of 96 to 128 bits whose first 96 bits are `::ffff:0:0` becomes the IPv4 range with length minus 96; other IPv6 ranges only cover native IPv6 addresses.
- DEC-005 (SPEC-0097): Boot validation: `WebConfig::validate_lists` (calls `web_access::AccessLists::from_config`) is called from `ServerConfig::validate` before the other checks of the web section, whether or not `web.enabled` is true. Because `ServerConfig::load` runs `validate` (`config.rs:985`), `serve`, `purge` and `migrate` (source then destination) refuse an invalid entry with `ToolError::invalid_argument("invalid entry '<entry>' in the web <black|white> list: expected a host name, a *.host name, an IP address or an address range")`, rendered by `cli::run` as `error: ERR_INVALID_ARGUMENT: ...` and a failure exit code. `keys`, `token` and `version` do not load a configuration and are unchanged. The first invalid entry is named, black list before white list, in list order.
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
| `crates/core/src/config.rs` | modify (two fields, Debug untouched, `validate` hook) |
| `crates/core/src/tools/web_access.rs` | create (Entry, parse_entry for names, wildcard names, addresses, ranges; AccessLists::from_config) |
| `crates/core/src/tools/mod.rs` | modify (two `mod` lines) |
| `crates/core/src/tools/web_access_boot_e2e.rs` | create |
| `.agent_docs/config.md` | modify (settings and refusal text) |

### Existing patterns
Validation functions returning one `ToolError::invalid_argument`: `crates/core/src/config.rs:1136-1139` (`validate_doc_service`, but note it returns early; this check must not). Configuration tests: `crates/core/src/config.rs:1700-1712` (snapshots, do not modify).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

Cast used by every test of this story (verbatim from the spec):

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

Test level and location: integration in `crates/core/src/tools/web_access_boot_e2e.rs`. Function names `e2e_0097_<NNN>_<slug>` / `e2e_0097_sec_<NNN>_<slug>`. Cleanup: each test builds its own temporary project and stand ins; nothing to tear down.

### SPEC-0097/E2E-041: valid lists start the server
- **Category:** happy. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["10.0.0.0/8", "*.corp.example", "fd00::/8", "blocked.example"]` and white list `["docs.example"]`. **When** the operator starts the server. **Then** it starts.

### SPEC-0097/E2E-042: an address with a scheme and path
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["https://blocked.example/x"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry 'https://blocked.example/x' in the web black list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

### SPEC-0097/E2E-043: impossible range length
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** white list `["10.0.0.0/33"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '10.0.0.0/33' in the web white list: expected a host name, a *.host name, an IP address or an address range`.

### SPEC-0097/E2E-044: wildcard alone
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["*"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '*' in the web black list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

### SPEC-0097/E2E-045: an entry with a port
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["example.com:8080"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry 'example.com:8080' in the web black list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

### SPEC-0097/E2E-047: a refused start leaves nothing running
- **Category:** side effect. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["*"]`. **When** the server starts and refuses. **Then** nothing listens on the server's port.

### SPEC-0097/E2E-056: malformed name entry
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["a..b"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry 'a..b' in the web black list: expected a host name, a *.host name, an IP address or an address range`.

### SPEC-0097/E2E-063: the web switch off does not skip the check
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** the web tools off and black list `["*"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '*' in the web black list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

### SPEC-0097/E2E-066: valid lists with the web tools off
- **Category:** happy. **Scenario:** SC-007. **Requirements:** FR-NEW-012
- **Given** the web tools off and black list `["10.0.0.0/8"]`. **When** the server starts. **Then** it starts.

### SPEC-0097/E2E-067: bad white list entry with the web tools off
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** the web tools off and white list `["10.0.0.0/33"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '10.0.0.0/33' in the web white list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

## Constraints
- Files not to touch: `crates/core/src/tools/web.rs` (no enforcement in this story), `spec.md`.
- Dependencies not to add: any (DEC-001).
- Do not add the two fields to the hand written `Debug` of `WebConfig` (DEC-003).
- Scope boundary: matching, requests and the other grammar details (hyphens, mapped ranges, digits, trailing dot, accents) belong to later stories; here an entry is valid when it is a plain name, `*.name`, a dotted four part IPv4 address, an IPv6 address, or `address/length`.
- Interim rule of this story, replaced by US-0247: any `*`, port, scheme, path, empty entry or malformed range is invalid.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs`, the two configuration snapshots (`config.rs:1710-1711`) and `tool-contract-golden.json` pass unmodified; both lists empty behaves exactly as before.
- [ ] Docs updated: `.agent_docs/config.md` (web section).
- [ ] Self review (full): every requirement above has its tests green, no `unwrap` or `expect` on a fallible path, no dash used as punctuation in code or comments, comments explain why.
- [ ] One commit `US-0246: accept and refuse web access list entries at boot` on the work branch `feat/0156-web-access-lists`, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0097` and the requirement ids.
