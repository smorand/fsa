# SPEC-0097: Black list and white list for the addresses web.fetch and web.download can reach, design

> Spec: sdlc/specs/SPEC-0097-web-access-lists/spec.md
> Nature: FEAT
> Depth: M
> Security: internal finding
> Designed on: 2026-11-16
> Target tier: 2 (default: no `--tier`, no `Tier:` in `sdlc/constitution.md`, no `.spec.json`)

## 1. Stack and skills

Existing project, Rust workspace. The stack is what the manifests say; nothing is added (DEC-001).

| Element | Choice | Version | DEC |
|---|---|---|---|
| Language | Rust, edition 2024 | per `Cargo.toml` | none |
| HTTP client | `reqwest` (rustls, json, multipart) | 0.12.28 (`Cargo.lock`; context7 `/seanmonstar/reqwest` lists v0.12.28; `resolve_to_addrs`, `redirect::Policy::none`, `proxy` verified there) | DEC-009, DEC-010 |
| URL and host reading | `url` | 2.5.8 (`Cargo.lock`, `Cargo.toml:71`) | DEC-001 |
| Async runtime | `tokio` (`net` already used by `crates/core/src/app.rs:174`) | 1.54.1 (`Cargo.lock`) | DEC-011 |
| Tests | `cargo test` through `make test`, raw TCP stand ins | existing | DEC-016 |

Skills loaded: `rust` (via `load_dynamic_context`). Effect: library crate `crates/core` holds all logic; `thiserror` style typed errors are already `ToolError`; no `unwrap`/`expect` on fallible paths (DEC-006); `make check` is the gate; functional e2e tests for every surface. Where the skill's template differs from this repository (its own `Makefile`, `./test.sh`), the repository wins (AGENTS.md).

## 2. Approaches

### Chosen: a guard module with a decision, one lookup, a pinned connection and a manual redirect loop
`tools/web_access.rs` parses entries once at registration, decides a request on the site as typed, looks a name up once with a deadline, judges the addresses, pins the connection to the checked addresses with `resolve_to_addrs`, and follows redirects by hand with the same decision on every hop (DEC-002, DEC-008 to DEC-011). Pros: the address checked is the address used (closes the rebinding gap, E2E-SEC-009); one decision function; the default path stays byte for byte today's (DEC-007). Cons: a second redirect loop exists beside reqwest's (its limits and failure text are reproduced and tested). Affected scope: `config.rs`, `tools/web.rs`, the new module, tests, docs. Best suited when: the spec requires the connection to go where the check looked.

### Rejected: custom `redirect::Policy` plus a filtering `dns_resolver`
A policy callback is synchronous and cannot look a name up; a resolver sees only host names (an IP literal such as `169.254.169.254` never goes through it) and is called again for every connection; its refusals surface only through the error source chain. Rejected in DEC-009 and DEC-010.

### Rejected: check first, then let the default client connect
The client would look the name up a second time, so a name answering a public address then `127.0.0.1` reaches the stand in (E2E-SEC-009). Rejected in DEC-009 and DEC-011.

### Rejected: a third party SSRF or CIDR crate
A new dependency for two integer masks and a decision table (ASK ONLY FOR category 2, not needed). Rejected in DEC-001.

## 3. Architecture

```
web.fetch / web.download handler (tools/web.rs)
  1 argument checks, volume() membership, norm() path        (unchanged)
  2 AccessLists::admit(url, deadline)  ---------------------> tools/web_access.rs
        read_site(url)           url::Url, host_str, ASCII, one trailing dot dropped
        typed decision           black name match -> refuse ; IP literal -> judge
        name lookup (once)       tokio::net::lookup_host bounded by the deadline
        judge addresses          black: any ; white: all ; mapped IPv6 -> IPv4
        -> Admission (site, pinned addresses)
  3 write_limit(...)             (unchanged, after the refusal)
  4 fetch_raw(url, ..., admission)
        lists empty  : default client, default redirects          (today)
        lists set    : per hop client (resolve_to_addrs), Policy::none(),
                       Url::join(location), admit(next) BEFORE the hop limit
  5 read_answer / write_bytes    (unchanged)
```

One bounded context from the spec is touched: Web access (`web_access.rs`, `web.rs`, `config.rs`). Project files (`fs_ops::write_bytes`, `write_limit`) is only called, never changed. `web.search`, `web.news`, `web.suggestions`, `git.*`, `context7.*` are untouched (DEC-014). The REST plane does not serve these tools (they exist in `register()` and `catalog()` only, `mcp/server.rs:70-75`, `api/openapi.rs:145`), so there is no second implementation to keep in line.

## 4. Data model

No persisted data. Configuration only, in the existing `web` section:

| Key | Type | Default | Meaning |
|---|---|---|---|
| `web.black_list_url` | list of strings | `[]` | sites the tools must not reach |
| `web.white_list_url` | list of strings | `[]` | when non empty, the only sites the tools can reach |

Entry forms and grammar: spec FR-NEW-005, parsed by DEC-004. Both fields use `#[serde(default)]`, so an existing configuration file loads unchanged. They are left out of the hand written `Debug` of `WebConfig` (DEC-003).

## 5. Contracts and interfaces

Messages (verbatim from the spec):
- refusal: `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`;
- boot: `error: ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web <black|white> list: expected a host name, a *.host name, an IP address or an address range`;
- unchanged texts reused: `url must start with http:// or https://`, `ERR_INTERNAL_ERROR: fetch request failed...`, `ERR_INTERNAL_ERROR: request timed out`.

Rust surface (crate internal, `pub(crate)`):
```rust
// tools/web_access.rs
pub(crate) struct AccessLists { /* black: Vec<Entry>, white: Vec<Entry> */ }
impl AccessLists {
    pub(crate) fn from_config(cfg: &WebConfig) -> Result<Self>;        // boot + register
    pub(crate) fn is_empty(&self) -> bool;                              // both lists empty
    pub(crate) async fn admit(&self, url: &str, deadline: tokio::time::Instant) -> Result<Admission>;
}
pub(crate) struct Admission { /* site text, pinned addresses */ }
// tools/web.rs
async fn fetch_raw(url: &str, timeout_secs: u64, cap: u64, over: impl Fn(Option<u64>) -> ToolError,
                   access: &AccessLists, admission: Option<Admission>, deadline: Instant) -> Result<(Vec<u8>, String)>;
```
No tool schema, description or REST route changes: `tool-contract-golden.json` stays identical (DEC-017).

## 6. File structure impact

| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/core/src/tools/web_access.rs` | create | FR-NEW-002 to FR-NEW-010 | US-0246 and every later story |
| `crates/core/src/config.rs` | modify (two fields, `validate` hook, `#[cfg(test)] test_net`) | FR-NEW-010, FR-NEW-012 | US-0246, US-0248 |
| `crates/core/src/tools/web.rs` | modify (`register`, three handlers, `fetch_raw`) | FR-NEW-002 to FR-NEW-009 | US-0248, US-0251, US-0252, US-0254 |
| `crates/core/src/tools/mod.rs` | modify (`mod web_access`, two test modules) | DEC-002, DEC-016 | US-0246 |
| `crates/core/src/tools/web_access_boot_e2e.rs` | create | FR-NEW-005, FR-NEW-010, FR-NEW-012 | US-0246, US-0247 |
| `crates/core/src/tools/web_access_e2e.rs` | create | FR-NEW-001 to FR-NEW-011 | US-0248 and later |
| `.agent_docs/config.md` | modify (web section) | FR-NEW-001 (spec 7.4) | every story |
| `config/local.yaml`, `config/local.yaml.template` | modify (commented settings) | spec 7.4 | US-0247 |
| `AGENTS.md`, `README.md` | modify (one paragraph each) | repository rule | US-0253, US-0254 |

Affected existing tests: none modified. The 51 tests of `web_bound_e2e.rs`, the 16 of `tool_surface_e2e.rs` and the two configuration snapshots (`config.rs:1710-1711`) pass unmodified (DEC-003, DEC-007). `tool-contract-golden.json` unchanged.

## 7. Compatibility and risks

- Both lists empty: behavior byte for byte today's (DEC-007). Existing configuration files load unchanged.
- A new boot refusal exists for a malformed entry only; no existing configuration has the keys.
- The proxy limit (owner option A, spec FR-NEW-006): with a proxy in the server's environment the proxy opens the connection; the lists still decide on the typed site and on what the server resolves. BL-0157 owns the proxy design and carries a note.
- A second redirect loop duplicates reqwest's limits (10 hops, five statuses). Risk: drift from reqwest on an edge case; mitigated by tests E2E-064, 074, 077 to 079, 088, and by taking the default path when no list is set.
- Residual and out of scope (spec 14.2): BL-0159 (pandoc image fetch), git declared hosts resolving to private addresses, the operator's converter command.
- Ordering: US-0246 and US-0247 validate entries before any enforcement exists, so an operator sees refusals of malformed entries before lists act. No story lands a half feature that breaks main: the settings are inert until US-0248.
- Rollback: remove the two keys from the configuration; no data to migrate.

## 8. Security design

Threat surface: untrusted source = a project member's `url` argument of `web.fetch` and `web.download`, and the `Location` headers of the sites those requests reach; dangerous sink = the outbound connection the server opens (CWE-918). Trust boundary = the tool call.

Class sweep occurrence kept in this lot (spec 14.2 row 1): `crates/core/src/tools/web.rs:449-466` (`fetch_raw`), reached from `web.rs:120` (save mode), `:164` (download), `:602` (text mode).
- **Fix site and mechanism:** the decision and the pinned connection in `tools/web_access.rs`, called from the three handlers (DEC-008) and from the hop loop inside `fetch_raw` (DEC-010). Mechanism: operator allow and deny lists on the typed site and on every resolved address, single lookup with `resolve_to_addrs` (DEC-009), decision on every redirect hop. Chosen by DEC-002, DEC-008, DEC-009, DEC-010, DEC-013.
- **Exploit tests (E2E-SEC-001 to E2E-SEC-009):** each runs through the MCP tool call of the served app (`call_tool`), carries the literal payload of the spec, asserts the exact refusal, and reads the absence of effect through a second channel: the raw TCP stand in's request counter (`Record.requests`), the absence of the saved file in the project, and for the rebinding case the lookup counter of the test name table. Each is red before the fix because the stand in answers and its body comes back or is saved (the spec states the failing assertion in 9.3).
- **Deferred occurrences (spec 14.2):** row 2 BL-0159 (pandoc images), rows 3, 11, 12 reported residuals, rows 4 to 10 and 13 safe with cited controls.
- **Secrets and logs:** no credential is handled. Messages never contain a resolved address (DEC-012). Entries are not secrets and appear in boot errors.
- **Least privilege:** the guard only reads configuration and performs one lookup; it opens no new listener and writes nothing.

## 9. Test strategy

Every acceptance test is a real tool call or a real boot path, never a mock of the guard. Levels: integration (boot: configuration and `cli::dispatch`), functional e2e (tool call through the served app against a raw TCP stand in), exploit (the same driver, with the second channel). Test data: generated in code (name table, sites table, stand ins), nothing user provided.

| Test | Level | Driver | Location | Story |
|---|---|---|---|---|
| E2E-001 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-002 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-003 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-004 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-005 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-006 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-007 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-008 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-009 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-010 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-011 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-012 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-013 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-014 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-015 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-016 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-017 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-018 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-019 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0250 |
| E2E-020 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0250 |
| E2E-021 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0250 |
| E2E-022 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0250 |
| E2E-023 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-024 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0250 |
| E2E-025 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-026 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-027 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-028 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-029 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0250 |
| E2E-030 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0250 |
| E2E-031 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0250 |
| E2E-032 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0250 |
| E2E-033 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-034 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-035 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-036 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-037 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-038 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-039 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-040 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-041 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-042 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-043 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-044 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-045 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-046 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-047 | integration | `cli::dispatch(Cli::parse_from([...]))` with a configuration file | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-048 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-049 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-050 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-051 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-052 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-053 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-054 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-055 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-056 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-057 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-058 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0253 |
| E2E-059 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0247 |
| E2E-060 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0247 |
| E2E-061 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-062 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-063 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-064 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-065 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-066 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-067 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0246 |
| E2E-068 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0253 |
| E2E-069 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0253 |
| E2E-070 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0253 |
| E2E-071 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0248 |
| E2E-072 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0247 |
| E2E-073 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-074 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-075 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-076 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-077 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-078 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-079 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-080 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0253 |
| E2E-081 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0253 |
| E2E-082 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-083 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0253 |
| E2E-084 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0249 |
| E2E-085 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0247 |
| E2E-086 | integration | `ServerConfig::validate` on a built configuration | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0247 |
| E2E-087 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-088 | functional e2e | MCP tool call through the served app against a raw TCP stand in | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-089 | integration | `cli::dispatch(Cli::parse_from([...]))` with a configuration file | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0247 |
| E2E-090 | integration | `cli::dispatch(Cli::parse_from([...]))` with a configuration file | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0247 |
| E2E-091 | integration | `cli::dispatch(Cli::parse_from([...]))` with a configuration file | `crates/core/src/tools/web_access_boot_e2e.rs` | US-0247 |
| E2E-SEC-001 | exploit | MCP tool call through the served app, stand in site counted | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-SEC-002 | exploit | MCP tool call through the served app, stand in site counted | `crates/core/src/tools/web_access_e2e.rs` | US-0254 |
| E2E-SEC-003 | exploit | MCP tool call through the served app, stand in site counted | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-SEC-004 | exploit | MCP tool call through the served app, stand in site counted | `crates/core/src/tools/web_access_e2e.rs` | US-0253 |
| E2E-SEC-005 | exploit | MCP tool call through the served app, stand in site counted | `crates/core/src/tools/web_access_e2e.rs` | US-0253 |
| E2E-SEC-006 | exploit | MCP tool call through the served app, stand in site counted | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-SEC-007 | exploit | MCP tool call through the served app, stand in site counted | `crates/core/src/tools/web_access_e2e.rs` | US-0251 |
| E2E-SEC-008 | exploit | MCP tool call through the served app, stand in site counted | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |
| E2E-SEC-009 | exploit | MCP tool call through the served app, stand in site counted | `crates/core/src/tools/web_access_e2e.rs` | US-0252 |

## 10. Decisions log

- **DEC-001:** No new dependency. Entries and address ranges are parsed with `std::net`; hosts are read and converted to their ASCII form with the `url` crate (WHATWG host parsing, UTS 46 non transitional), which is already a direct dependency.. **Obliged by:** SPEC-0097/FR-NEW-005, FR-NEW-006. **Rationale:** The spec fixes the reading rules by naming the WHATWG URL and UTS 46 standards; `url` implements both, a second parser could disagree with the one `reqwest` uses to send. **Alternatives considered:** `ipnet` for ranges (new dependency, a range is two integer masks); `idna` directly (the `url` crate already wraps it and applies the URL flavour of the rules); `addr` or `publicsuffix` (not needed). **Source:** design. **Evidence:** existing code `Cargo.toml:71` (`url = "2.5"`), `Cargo.lock` url 2.5.8, reqwest 0.12.28; context7 /seanmonstar/reqwest versions include v0.12.28
- **DEC-002:** One new module `crates/core/src/tools/web_access.rs` owns entry parsing, site reading, matching, admission (decision plus name lookup) and the redirect following loop. `tools/web.rs` only calls it.. **Obliged by:** SPEC-0097/FR-NEW-002 to FR-NEW-009. **Rationale:** Keeps the 700 line `web.rs` readable and makes the decision testable without HTTP; the MCP and REST planes both reach it through the existing tool functions, never a second copy (AGENTS.md conventions). **Alternatives considered:** growing `web.rs` in place (mixes tool schemas with address policy). **Source:** design. **Evidence:** design; `crates/core/src/tools/web.rs:449-466` (`fetch_raw`, the single choke point)
- **DEC-003:** Two settings in the existing web section: `web.black_list_url` and `web.white_list_url`, each `Vec<String>` with `#[serde(default)]`, empty by default. The hand written `Debug` of `WebConfig` leaves them out, as it leaves out `test_base`, so the two frozen configuration snapshots stay byte identical and no existing test changes.. **Obliged by:** SPEC-0097/FR-NEW-001, FR-NEW-005 (names fixed by the owner, spec 14.3). **Rationale:** Both frozen snapshots print `WebConfig { enabled, max_results, request_timeout_secs, safe_search }`; adding fields to the derived output would modify two existing tests. **Alternatives considered:** adding the fields to `Debug` and updating both snapshots (modifies existing tests without a requirement forcing it). **Source:** design. **Evidence:** spec constraint (14.3) and existing code `crates/core/src/config.rs:623-658`, `:1710-1711`
- **DEC-004:** Entry parsing is `parse_entry(&str) -> Result<Entry, ()>` with `Entry = Name | Below | Addr | Range`. A name is checked against the stricter entry rules label by label (letters, digits, hyphen, underscore, 63 and 253 limits, no empty label, `*` only as the whole first label, numeric last label or `0x` prefix must be a four part IPv4 address, `xn--` must decode), then converted with `url::Host::parse`. A range is `address/length` with a length of one to three digits without sign or leading zero. IPv4 mapped IPv6 entries become IPv4 entries; an IPv6 range of 96 to 128 bits whose first 96 bits are `::ffff:0:0` becomes the IPv4 range with length minus 96; other IPv6 ranges only cover native IPv6 addresses.. **Obliged by:** SPEC-0097/FR-NEW-005. **Rationale:** The grammar of FR-NEW-005 is closed and spelled; one function returning a typed value keeps matching trivial and makes every invalid shape a test row. **Alternatives considered:** regex per shape (cannot express the decode rule of `xn--` or the range length checks). **Source:** design. **Evidence:** spec FR-NEW-005 and 9.2 E2E-041 to E2E-086; `crates/core/src/git/remote.rs:246-270` (`validate_remote_url`, precedent for a validation function returning one message)
- **DEC-005:** Boot validation: `WebConfig::validate_lists` (calls `web_access::AccessLists::from_config`) is called from `ServerConfig::validate` before the other checks of the web section, whether or not `web.enabled` is true. Because `ServerConfig::load` runs `validate` (`config.rs:985`), `serve`, `purge` and `migrate` (source then destination) refuse an invalid entry with `ToolError::invalid_argument("invalid entry '<entry>' in the web <black|white> list: expected a host name, a *.host name, an IP address or an address range")`, rendered by `cli::run` as `error: ERR_INVALID_ARGUMENT: ...` and a failure exit code. `keys`, `token` and `version` do not load a configuration and are unchanged. The first invalid entry is named, black list before white list, in list order.. **Obliged by:** SPEC-0097/FR-NEW-010, FR-NEW-012. **Rationale:** One call site covers every command that reads the configuration; `validate_doc_service` returns early when its switch is off (`config.rs:1137-1139`), the web check deliberately does not. **Alternatives considered:** checking only in `serve` (purge and migrate would accept a bad entry); checking in `register()` only (skipped when the web family is off). **Source:** design. **Evidence:** existing code `crates/core/src/config.rs:985`, `:990-1007`, `:1136-1139`; `crates/core/src/cli.rs:133-135`, `:169-175`, `:194`, `:416-419`
- **DEC-006:** Fail closed at registration: `web::register` builds the lists with `AccessLists::from_config`; if it fails (only reachable by a configuration that skipped `validate`, as some tests do), every call of `web.fetch` and `web.download` returns that error instead of reaching the network. No `expect` or `unwrap`.. **Obliged by:** SPEC-0097/FR-NEW-010. **Rationale:** `register()` returns `()` and `app::build_with_state` does not call `validate`, so the invariant is not provable at the call site; failing closed keeps the guarantee without a panic. **Alternatives considered:** `expect("validated at boot")` (panics on a programmatic configuration). **Source:** design. **Evidence:** existing code `crates/core/src/mcp/server.rs:70-75`, `crates/core/src/app.rs` (no `validate()` call)
- **DEC-007:** Both lists empty takes today's exact path: the default `build_client` client, default redirect policy (up to 10 hops), no lookup, no pinning, no new header. The new code runs only when at least one list is non empty.. **Obliged by:** SPEC-0097/FR-NEW-001, FR-NEW-009 (unchanged behavior). **Rationale:** Guarantees "exactly as before" by construction and keeps the 67 existing web tests as the proof (51 in `web_bound_e2e.rs`, 16 in `tool_surface_e2e.rs`). **Alternatives considered:** one code path for both cases (every difference between the default policy and the manual loop becomes a regression risk for operators who set no list). **Source:** design. **Evidence:** existing code `crates/core/src/tools/web.rs:260-266`, `:449-466`
- **DEC-008:** `web.download`, `web.fetch` with a destination and `web.fetch` returning text each call `AccessLists::admit(url, deadline)` right after their argument, membership and path checks and before `write_limit`, and pass the returned `Admission` (site, pinned addresses) to the sender. A refusal therefore precedes the storage allowance refusal (E2E-051), a non member never reaches it (E2E-049), and the single name lookup of a first hop is shared by the check and the connection.. **Obliged by:** SPEC-0097/FR-NEW-008, FR-NEW-006. **Rationale:** The spec orders the refusals; `write_limit` needs only project state so it can follow, while the lookup cannot be repeated without reopening a time of check gap. **Alternatives considered:** checking inside `fetch_raw` (after `write_limit`, wrong order). **Source:** design. **Evidence:** existing code `crates/core/src/tools/web.rs:105-133` (save mode, no address check), `:150-170` (download), `:596-609` (text mode), `:496` (`write_limit`)
- **DEC-009:** The connection goes to the checked address: each hop is sent by a fresh client built with `ClientBuilder::resolve_to_addrs(host, &checked_addresses)` for a host name; an IP literal needs no pin. The client still sets the request timeout from the time left of the call, the user agent `mcp-fs-web/1.0`, and keeps the environment proxy (`auto_sys_proxy`, no `no_proxy()`), the limit the owner accepted (option A, BL-0157 owns the proxy design).. **Obliged by:** SPEC-0097/FR-NEW-006. **Rationale:** `resolve_to_addrs` overrides DNS for the host at connection time only, so one lookup serves the check and the connection (reqwest 0.12.28, context7 /seanmonstar/reqwest, `src/async_impl/client.rs` `resolve_to_addrs`). **Alternatives considered:** `dns_resolver` filter (cannot see IP literals, error mapping through the source chain, called again on every connection); pre check then plain client (rebinding gap, E2E-SEC-009). **Source:** design. **Evidence:** existing code `crates/core/src/tools/web.rs:260-266`; reqwest-0.12.28 `src/async_impl/client.rs:2278` (`resolve_to_addrs`), `:309` and `:419` (`auto_sys_proxy`)
- **DEC-010:** With a list non empty redirects are followed by hand: a client with `redirect::Policy::none()`, a loop of at most 10 followed hops on statuses 301, 302, 303, 307 and 308, the next address is `Url::join(location)` (so a relative or protocol relative destination resolves against the hop that sent it), and `admit` runs on the next address before it is sent and before the "too many redirects" check. A redirect answer with no usable `Location` is returned as the answer. Failures keep today's prefix `ERR_INTERNAL_ERROR: fetch request failed`.. **Obliged by:** SPEC-0097/FR-NEW-007. **Rationale:** A synchronous redirect policy callback cannot look names up; the git provider client already follows redirects by hand with the same shape. **Alternatives considered:** `redirect::Policy::custom` (cannot resolve names, so a hop could reach a refused address through a name). **Source:** design. **Evidence:** existing code `crates/core/src/git/provider/mod.rs:391` (`Policy::none()`), `:519-547` (manual loop); reqwest default `Policy::limited(10)`
- **DEC-011:** Name lookup uses `tokio::net::lookup_host((host, 0))` bounded by the call's deadline with `tokio::time::timeout_at`. Any error, an empty answer, or no answer counts as "resolves to no address"; a lookup that overruns the deadline fails the call with `ERR_INTERNAL_ERROR: request timed out`. A host with no address is never sent: it fails with `ERR_INTERNAL_ERROR: fetch request failed: no address for host '<site>'` unless the lists refuse it first. The deadline is `timeout_secs` for the whole call, shared by the lookups, the hops and the body read.. **Obliged by:** SPEC-0097/FR-NEW-006. **Rationale:** FR-NEW-006 treats every lookup error alike and bounds the lookup by the request timeout; refusing to send an unresolved host keeps "address checked is address used". **Alternatives considered:** letting the client look the name up again when the first lookup failed (reopens the rebinding gap). **Source:** design. **Evidence:** existing code `crates/core/src/app.rs:174` (`tokio::net` already used in production, feature enabled)
- **DEC-012:** The `<site>` of a message is read from `url::Url::host_str()` of the address as parsed (lower case, ASCII form), with one trailing dot removed and the brackets of an IPv6 address removed. `Ipv6Addr::to_string` is never used for it: Rust prints an IPv4 mapped address as `::ffff:127.0.0.1` while the spec wants `::ffff:7f00:1`. The message is `ToolError::forbidden("host '<site>' is not allowed by the web access lists")`, so it renders `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. The resolved addresses never enter a message.. **Obliged by:** SPEC-0097/FR-NEW-008. **Rationale:** The spec spells the normalized forms (`127.0.0.1` for `2130706433`, `::ffff:7f00:1` for the mapped form, `xn--strae-oqa.example`). **Alternatives considered:** formatting the parsed `Host` variant by hand (disagrees on the mapped form). **Source:** design. **Evidence:** spec FR-NEW-008; existing code `crates/core/src/tools/web.rs:547-553` (`host_of` already parses with `reqwest::Url`), `crates/core/src/errors.rs:59-60` (`forbidden`)
- **DEC-013:** Matching rules in code: a request site is a `Domain(ascii)`, an `Ipv4` or an `Ipv6`. An IPv6 address that is IPv4 mapped is judged as its IPv4 address, for requests and for resolved addresses. Black list: refuse if the typed site matches an entry or any resolved address does. White list: serve if the typed site matches a name entry, or the site resolved to at least one address and every address matches an entry (an empty answer is never white listed). Both: refuse on black first, then apply the white rule. `Below` matches a domain that ends with `.` plus the entry and is longer; `Name` is equality.. **Obliged by:** SPEC-0097/FR-NEW-002, FR-NEW-003, FR-NEW-004, FR-NEW-006. **Rationale:** Direct transcription of the spec's decision table and its business rules. **Alternatives considered:** a combined score or a single list (the four cases are not one rule: the white minus black case needs black first). **Source:** design. **Evidence:** spec FR-NEW-002 to FR-NEW-006
- **DEC-014:** The lists apply to `web.fetch` and `web.download` only: `web.search`, `web.news`, `web.suggestions` keep their direct `build_client(10)` calls (`web.rs:331`, `:408`, `:615`), the git tools and `context7.*` are not touched.. **Obliged by:** SPEC-0097/FR-NEW-011. **Rationale:** The three search tools contact fixed hosts chosen by the product (spec 14.2 row 8). **Alternatives considered:** wrapping every `build_client` (would filter the product's own fixed hosts). **Source:** design. **Evidence:** existing code `crates/core/src/tools/web.rs:331`, `:408`, `:615`
- **DEC-015:** Test seam, `#[cfg(test)]` only, in the same style as `test_base` (`config.rs:628-631`): a field `WebConfig.test_net: Option<Arc<TestNet>>` with `#[serde(skip)]`, left out of `Debug`. `TestNet` holds (a) a name table answering `Addrs(Vec<IpAddr>)`, `NoAddress` or `Hang` per name, one answer per successive lookup with the last one repeating, and a lookup counter; (b) a site table mapping a host or literal address to the port of a loopback stand in, applied at send time AFTER the decision, rewriting the request address to `127.0.0.1:<port>` (so the lists are exercised on the real typed address while the bytes reach the stand in); (c) an optional proxy address handed to `ClientBuilder::proxy`. A name missing from the table answers `NoAddress`: no test ever reaches the real resolver.. **Obliged by:** SPEC-0097/FR-NEW-006 (E2E-SEC-009 needs a second lookup that differs), FR-NEW-007, FR-NEW-009 (spec 14.3: connect seam, name table, proxy seam). **Rationale:** The stand ins bind only the loopback address (`tools/web_testkit.rs:79-81`), so literal addresses such as `169.254.169.254` and names such as `news.example` cannot be reached directly. **Alternatives considered:** binding real non loopback addresses (not portable, needs privileges); a global resolver override (leaks across parallel tests). **Source:** design. **Evidence:** spec 14.3; existing code `crates/core/src/config.rs:628-631`, `:636-645`, `crates/core/src/tools/web.rs:306-312` (`test_base` precedent)
- **DEC-016:** Tests live in two new files registered in `tools/mod.rs` next to `web_bound_e2e`: `tools/web_access_boot_e2e.rs` (configuration and command level, no HTTP) and `tools/web_access_e2e.rs` (MCP tool calls through the served app, the seam and a raw TCP stand in). Function names are `e2e_0097_<NNN>_<slug>` and `e2e_0097_sec_<NNN>_<slug>`, as `e2e_0093_*` does. The helper `served_with_lists(black, white, net)` copies `served_with_base` (`web_testkit.rs:298-318`) and sets the lists and `test_net` on the configuration. Boot tests call `ServerConfig::validate` or `cli::dispatch(Cli::parse_from([...]))` (`cli.rs:139-168`), never a child process.. **Obliged by:** SPEC-0097/all FR (test strategy). **Rationale:** Matches the existing layout for the web family; `dispatch` is the seam the CLI already offers for tests. **Alternatives considered:** a child process per boot test (slow, the binary crate lives in `crates/mcp-fs`). **Source:** design. **Evidence:** existing code `crates/core/src/tools/web_testkit.rs:80-95`, `:290-318`; `crates/core/src/tools/web_bound_e2e.rs:1-60`; `crates/core/src/cli.rs:139-168`
- **DEC-017:** Documentation: each story updates `.agent_docs/config.md` (section "web and context7", the web access lists) for the part it delivers; `config/local.yaml` and `config/local.yaml.template` get the two commented settings; `README.md` and `AGENTS.md` get one short paragraph each. The tool contract (`TOOL_CONTRACT.txt`, `tool-contract-golden.json`) is not touched: no tool schema or description changes.. **Obliged by:** SPEC-0097/FR-NEW-001 (7.4: the documentation table and the recommended `169.254.0.0/16` entry). **Rationale:** Spec section 7.4 asks for one table of the four combinations and the metadata entry. **Alternatives considered:** documenting in one final story (the spec converge story already carries the code of the last slice). **Source:** design. **Evidence:** spec 7.4; existing code `.agent_docs/config.md:285-306`, `config/local.yaml:66-68`

## 11. Stories

| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0246 | accept and refuse web access list entries at boot | P1 | FR-NEW-005, FR-NEW-010, FR-NEW-012 | main | 2 | no |
| US-0247 | the full entry grammar, and every command that reads the configuration | P1 | FR-NEW-005, FR-NEW-010, FR-NEW-012 | US-0246 | 2 | no |
| US-0248 | a black list refuses the site as typed for `web.fetch` returning text | P1 | FR-NEW-001, FR-NEW-002, FR-NEW-005, FR-NEW-008 | US-0247 | 2 | no |
| US-0249 | the site is read as the web standard reads it, and the other web tools are left alone | P1 | FR-NEW-005, FR-NEW-006, FR-NEW-008, FR-NEW-010, FR-NEW-011 | US-0248 | 2 | no |
| US-0250 | a white list, and a white list minus a black list | P1 | FR-NEW-003, FR-NEW-004, FR-NEW-005, FR-NEW-008 | US-0249 | 2 | no |
| US-0251 | downloads and saved pages are refused cleanly, in the specified order | P1 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-NEW-004, FR-NEW-008, FR-NEW-009 | US-0250 | 2 | no |
| US-0252 | judge the addresses a site name resolves to, and connect to the checked address | P1 | FR-NEW-002, FR-NEW-003, FR-NEW-006, FR-NEW-009 | US-0251 | 1 | no |
| US-0253 | unreadable addresses are never sent, numeric forms are read as the plain address, the proxy limit is tested | P1 | FR-NEW-006 | US-0252 | 2 | no |
| US-0254 | redirects are checked hop by hop, and the spec is closed | P1 | FR-NEW-007, FR-NEW-009 | US-0253 | 1 | yes |
