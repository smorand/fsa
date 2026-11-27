# SPEC-0116: Server and auth settings in the database, design

> Spec: sdlc/specs/SPEC-0116-settings-server-auth/spec.md
> Nature: FEAT
> Depth: M
> Security: yes (keys that verify callers, reserved variable names, export links)
> Designed on: 2026-11-27
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. No new dependency (S-22): the `rsa` crate with its PEM feature and `jsonwebtoken` are already in the tree. No new table: the `server_setting` table and the three tools of SPEC-0114 are reused (S-2B).

## 2. Approaches
### Chosen: two more sections of the SPEC-0114 store, with the rules of a change kept in one new module
`SECTIONS` grows to thirteen; `section_json` and `with_section` get the `server` and `auth` arms; every rule that only a change obeys (a given key judged alone, a lock out guard, the secret variable names) lives in `settings/change_rules.rs`, called by `ServerSettings::try_set`; every rule the start obeys stays in `config.rs` (`check()` and two new functions for the stored sections), so a YAML only start is unchanged (FR-NEW-020).
### Rejected: put the new rules in `ServerConfig::check()`
`check()` also runs on a YAML file and on every stored section: the lock out guards, the port range and the variable name form would then stop a YAML start that works today.
### Rejected: an operator allow list of variable names
An environment variable more (S-24) and a decision the owner did not take; the owner's instruction of this run is a refusal of the master names and a conservative form, chosen as the `FSA_SECRET_<SINK>_` namespace.
### Rejected: a second token verifier for keys outside 2048 to 8192 bits
Such a key verifies nothing today and the change refuses it; a YAML key of that size only starts (FR-NEW-015).

## 3. Architecture
```
set_setting(server|auth|git|extract|search) -> ServerSettings::try_set
   -> change_rules::names (five variable names, FR-018)      [patch]
   -> check_keys (unknown key)                                [patch]
   -> change_rules::types (server, auth: FR-043)              [patch]
   -> change_rules::given (port, keys, empties, issuer, base url policy)  [patch + section in force]
   -> merge -> with_section -> candidate.check() + check_server_forms()   [section as it will be]
   -> change_rules::guards (no key / admin / claim removals, key count)   [section as it will be]
   -> row + record (one tx)
boot: build_with_env -> server_settings_boot::apply -> rows replace sections -> check()
   -> check_stored_server_auth() (host, path, base url, keys) -> names (FR-034/036/044) -> seed (skip rules FR-028/031)
serve: serve_with_env -> build_with_env -> banner -> key check (FR-011) -> bind state.config.server
identity: IdentityResolver { keys: Vec<DecodingKey> } tried in order (FR-040)
```

## 4. Data model
None new. A `server` row is the JSON of `HttpConfig`; an `auth` row the JSON of `AuthConfig` with `jwt.public_keys`.

## 5. Contracts and interfaces
- `config.rs`: `JwtConfig.public_keys: Vec<String>` (default empty) with a hand written `Debug` that prints the six fields of today and `public_keys` only when non empty (the snapshots of the config tests stay equal); `MASTER_NAMES` (twelve names) and `is_master_name`; `ServerConfig::check_server_forms()` (host grammar, public base URL form, the mount rule of the MCP path via `mcp_mount::refusal(path)`), `check_auth_keys(only: Option<&[String]>)` (private label, form, count), `has_auth_key()`; `validate()` (YAML load) adds the MCP path rule and the key checks; `check()` is not changed.
- `app.rs`: `serve(config)` calls `serve_with_env(config, &env, origin)`; the address, the banner and the key check come from `state.config` after the overlay; `banner_line(version, &config, origin)` is the pure text the banner prints; the mount check of `mcp_path` in `build_with_env` calls `mcp_mount::refusal` (no panic).
- `mcp_mount.rs` (new, small): the route table of the server (fixed routes, routes with a variable part, with every optional family on) and `refusal(path) -> Option<String>`.
- `identity.rs`: `IdentityResolver` holds the keys of `public_key_path` (when readable) and of `public_keys`, tries them in the order of FR-NEW-040, keeps `invalid token: <error>` wording and the 30 second leeway; a warning when the path cannot be read and the list is not empty.
- `settings/change_rules.rs` (new): the rules of a change, the exact messages of the spec, the key parse (`rsa` with a size limit above 8192, PKCS1 and SPKI), the type table of `server` and `auth`.
- `settings/server_settings.rs`: `SECTIONS: [&str; 13]`, the arms, the call of `change_rules`, no change to the merge.
- `settings/server_settings_boot.rs`: stored `server`/`auth` checks after the overlay, the seed skips (`server` invalid host/base URL, `auth` without key), the stored name rules for the five keys, the master name warning, the FR-024 warning.
- `git/oauth/device_flow.rs`, `docs/ocr.rs`, `search/embedding.rs`: refuse a master name at use with the spec messages (`ERR_INVALID_ARGUMENT`), before any request.
- Tool descriptions and contract: parameter description and list description name the thirteen sections (spec 14.2).

## 6. File structure impact
`config.rs`, `app.rs`, `cli.rs` (banner), `identity.rs`, `mcp_mount.rs`, `settings/{server_settings,server_settings_boot,change_rules}.rs`, `git/oauth/device_flow.rs`, `docs/ocr.rs`, `search/embedding.rs`, `tools/settings.rs`, `mcp/server.rs` (two descriptions), `settings_screen.rs` only if it hard codes the names, tests `settings/server_settings4_e2e.rs` (rules, `e2e_0116_NNN`), `settings/server_settings4_boot_e2e.rs` (start), `settings/server_settings4_pg_e2e.rs`, `crates/fsa/tests/serve_e2e.rs` (program level), docs, `TOOL_CONTRACT.txt`, golden.

## 7. Compatibility and risks
YAML alone behaves as today except a serve with no key (FR-NEW-011). `JwtConfig` printed form unchanged for an empty list (D1). Risk: process wide environment in the pin and use time tests: each owns its variable names and removes them. Risk: the route table of `mcp_mount` drifts from the real routers: a test builds the real router with every family on and checks that nesting the listed fixed routes conflicts (the dry run named by D23/D24).

## 8. Security design
A key is public data: private labels are refused by text before any parse and never echoed; type refusals never echo a value (FR-043); the five secret variable names are limited to the defaults and the `FSA_SECRET_<SINK>_` namespace at a change, the twelve master names are refused at use; `public_base_url` cannot be redirected by a change (FR-042); the guards of FR-006 stop a change that would lock every caller out. Kept occurrences (destination class, command execution, a stored key that signs as any member) are decisions D-0116-2, 4, 5 of the spec, pinned by tests.

## 9. Test strategy
Tests in `e2e_0116_NNN` names, written first and run RED on the unchanged production code, per story. PostgreSQL ones (E2E-047 to 049 of the spec) real clock, scratch pgvector container. Program level tests run the built `fsa` binary. Stand ins: the provider, OCR, embedding and converter stand ins of the existing suites.

## 10. Decisions log
- DEC-001 (S-2B) the rules of a change live in one module, the rules of the start stay with the configuration.
- DEC-002 (S-10) `jwt.public_keys` is a list of PEM texts next to the file path; no key id, no JWKS.
- DEC-003 (S-05) a refused change reports the first rule of the order of FR-NEW-023, 047 and 043.
- DEC-004 (S-26) no secret in the two sections; the only secret like value, a private key, is refused by label.
- DEC-005 (S-04) the master names are a closed list of twelve; the namespace closes every other name for a change.
- DEC-006 (S-10) the route table of the MCP path is static and tested against the real router.

## 11. Stories
| Story | Covers | Tier |
|---|---|---|
| US-0348 the server section, the section list and the change order | FR-NEW-001 to 003, 038, 042, 046, 048 and the shared rows | 1 |
| US-0349 the auth section and its change rules | FR-NEW-005, 006, 013, 022, 023, 026, 027, 029, 030, 037, 039, 041, 043, 045, 047 | 1 |
| US-0350 identity with several keys | FR-NEW-007 to 009, 025, 035, 040 | 1 |
| US-0351 the start: address, banner, key check, stored and seed rules | FR-NEW-004, 010 to 012, 015, 020, 021, 028, 031, 032, 049 | 1 |
| US-0352 the secret variable names | FR-NEW-018, 019, 024, 034, 036, 044 and the pins | 1 |
| US-0353 PostgreSQL, docs and contract | FR-NEW-017 | 1 |

### Test to story map
US-0348: E2E-001, 003 to 005, 007, 037 to 039, 044 to 046, 066, 119, 125, 139, 154 to 156, 172 to 177, 179, 191 to 196, 198.
US-0349: E2E-002, 006, 011 to 017, 035 to 036, 059 to 062, 064, 074 to 077, 083, 094 to 102, 113 to 116, 140 to 142, 144 to 146, 151 to 153, 158 to 162, 166 to 169, 180 to 182.
US-0350: E2E-018 to 026, 063, 085, 087 to 089, 117 to 118, 122, 133 to 135, 143, 147 to 150, 157, 170 to 171, 197.
US-0351: E2E-008 to 010, 027 to 034, 040 to 043, 067 to 071, 080 to 081, 103, 108, 110 to 112, 120 to 121, 123 to 124, 126 to 127, 132, 178, 199 to 200.
US-0352: E2E-050 to 058, 065, 072 to 073, 078 to 079, 082, 084, 086, 090 to 093, 104 to 107, 109, 128 to 131, 136 to 138, 163 to 165, 183 to 190.
US-0353: E2E-047 to 049.

## 12. Drift notes (implementation)
- DRIFT-001 The audience of a token is checked when the token carries one (the library requires only `exp` by default): E2E-027 says "another audience refused", the "no audience refused" of the first wording was false against today's verifier; spec corrected, reported as a concern.
- DRIFT-002 The tests of the spec were written where they belong by what they observe, not by the story map of section 11: the start tests (stored shape, YAML, serve, banner) in `server_settings7_e2e.rs` (US-0351), the identity tests in `server_settings6_e2e.rs` (US-0350), the names in `server_settings8_e2e.rs` (US-0352), the PostgreSQL tests in `server_settings9_pg_e2e.rs` (US-0353).
- DRIFT-003 Kept occurrence pins E2E-163, 183, 189, 190 (the converter command, the pandoc binary, the model provider registry endpoint and the converter registry URL) are not written; 184 to 188 are. Reported with the decisions D-0116-2 and D-0116-5.
- DRIFT-004 "Blank" is white space in the whole (`str::trim`), not only spaces, tabs and carriage returns: a review found that a key file path of a line feed passed the guard of a change and made the start refuse to serve. Test `e2e_0116_201`. A port past the signed 64 bit range got the serde message instead of the port message, test `e2e_0116_202`.
- DRIFT-005 `crates/core/src/mcp_mount.rs` collapses the fifty REST plane routes into `/api/fs/{mount_id}/{op}`: a path under `/api/fs/<x>/<y>` is refused whatever the operation, a conservative reading checked against the real router by `e2e_0116_d23`.
- DRIFT-006 US-0352 and US-0353 shared one full workspace gate (gate 7): the two earlier runs stopped at the meta tests `old_name_gone` (the old spellings of the default names moved to `config.rs`) and `security_gate` (a helper name of a test).
