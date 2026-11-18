# US-0247: the full entry grammar, and every command that reads the configuration

> Spec: SPEC-0097 (sdlc/specs/SPEC-0097-web-access-lists/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0246
> min_tier: 2
> Files touched: 5
> Closes spec: no

## Objective
The entry grammar of the spec is complete: digit only and `0x` names, range lengths, bracketed and zoned IPv6, `xn--` labels, label and name length limits, leading spaces, a single trailing dot. `purge` and `migrate` refuse an invalid entry like `serve` does and `purge` with valid lists still runs.

## Requirements covered
- SPEC-0097/FR-NEW-005: The web access context SHALL accept as a list entry exactly one of: a host name (`docs.example`), a name with a leading wildcard (`*.corp.example`), an IPv4 or IPv6 address (`10.0.0.5`, `fd12::1`), or an address range written with a slash (`169.254.0.0/16`, `fd00::/8`).
- SPEC-0097/FR-NEW-010: IF a list contains an entry that is not one of the forms of FR-NEW-005 THEN THE server SHALL refuse to start and SHALL say which list and which entry are invalid.
- SPEC-0097/FR-NEW-012: The server program SHALL check every entry of both lists at the start of every command that reads the configuration (`serve`, `purge`, `migrate`), whether or not the web tools are on, and SHALL refuse with the message of FR-NEW-010 and exit with a non zero status before doing any work on an invalid entry. For `migrate` the source configuration is checked first, then the destination, and the first invalid entry found is the one named. The commands `keys`, `token` and `version` do not read the configuration and are unchanged.

## Independent Test
Run `cargo test -p mcp-fs-core --lib -- web_access_boot_e2e`; the eight tests of this story plus the ten of US-0246 are green. `cli::dispatch` of `purge` with an invalid black list returns the error `ERR_INVALID_ARGUMENT: invalid entry '*' in the web black list: ...` and no purge step ran.

## Design excerpt
### Decisions
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
| `crates/core/src/tools/web_access.rs` | modify (complete grammar of FR-NEW-005 for entries) |
| `crates/core/src/tools/web_access_boot_e2e.rs` | modify |
| `.agent_docs/config.md` | modify (entry grammar table) |
| `config/local.yaml` | modify (commented settings) |
| `config/local.yaml.template` | modify (commented settings) |

### Existing patterns
`cli::dispatch(Cli::parse_from(["mcp-fs","purge","--config",path]))` is used by the existing purge tests in `crates/core/src/cli.rs:700-720`; `migrate` loads two configs at `cli.rs:173-175`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

Cast used by every test of this story (verbatim from the spec):

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

Test level and location: integration in `crates/core/src/tools/web_access_boot_e2e.rs`. Function names `e2e_0097_<NNN>_<slug>` / `e2e_0097_sec_<NNN>_<slug>`. Cleanup: each test builds its own temporary project and stand ins; nothing to tear down.

### SPEC-0097/E2E-059: numeric entry that is not a four part address
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** white list `["2130706433"]`, and then `["0x7f000001"]`, and then `["10.1"]`. **When** the server starts with each. **Then** each time it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web white list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

### SPEC-0097/E2E-060: more malformed entries
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["010.0.0.1"]`, then `[" blocked.example"]` (leading space), then `["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.example"]` (the label is 64 times `a`), then `["xn--a"]`. **When** the server starts with each. **Then** each time it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web black list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

### SPEC-0097/E2E-072: malformed range lengths and bracketed or zoned IPv6 entries
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["10.0.0.0/08"]`, then `["10.0.0.0/+8"]`, then `["[fd00::1]"]`, then `["fe80::1%eth0"]`. **When** the server starts with each. **Then** each time it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web black list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

### SPEC-0097/E2E-085: an empty punycode label
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["xn--"]`, then `["XN--"]`. **When** the server starts with each. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web black list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

### SPEC-0097/E2E-086: more than one trailing dot
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["blocked.example.."]`, then `["."]`. **When** the server starts with each. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web black list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

### SPEC-0097/E2E-089: the purge command refuses a bad entry
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-012
- **Given** black list `["*"]` and a project with expired files. **When** the operator runs `mcp-fs purge`. **Then** it prints `error: ERR_INVALID_ARGUMENT: invalid entry '*' in the web black list: expected a host name, a *.host name, an IP address or an address range`, exits non zero, and no file is purged.

### SPEC-0097/E2E-090: the migrate command refuses a bad entry
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-012
- **Given** a source configuration with white list `["10.0.0.0/33"]`. **When** the operator runs `mcp-fs migrate` with it. **Then** it prints `error: ERR_INVALID_ARGUMENT: invalid entry '10.0.0.0/33' in the web white list: expected a host name, a *.host name, an IP address or an address range`, exits non zero, and nothing is copied.

### SPEC-0097/E2E-091: the purge command accepts valid lists
- **Category:** happy. **Scenario:** SC-007. **Requirements:** FR-NEW-012
- **Given** black list `["10.0.0.0/8"]`. **When** the operator runs `mcp-fs purge`. **Then** it runs and exits zero.

## Constraints
- Files not to touch: `crates/core/src/tools/web.rs`, `crates/core/src/cli.rs` (validation already runs in `ServerConfig::load`).
- Dependencies not to add: any.
- Scope boundary: matching a request against entries is US-0248 and later.
- For `migrate`, the source configuration is loaded and validated before the destination: the first invalid entry found is the one named.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs`, the two configuration snapshots (`config.rs:1710-1711`) and `tool-contract-golden.json` pass unmodified; both lists empty behaves exactly as before.
- [ ] Docs updated: `.agent_docs/config.md` (web section), `config/local.yaml`, `config/local.yaml.template`.
- [ ] Self review (full): every requirement above has its tests green, no `unwrap` or `expect` on a fallible path, no dash used as punctuation in code or comments, comments explain why.
- [ ] One commit `US-0247: the full entry grammar, and every command that reads the configuration` on the work branch `feat/0156-web-access-lists`, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0097` and the requirement ids.
