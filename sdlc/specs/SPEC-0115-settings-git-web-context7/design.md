# SPEC-0115: Git, web and context7 settings in the database, design

> Spec: sdlc/specs/SPEC-0115-settings-git-web-context7/spec.md
> Nature: FEAT
> Depth: M
> Security: no secret value in the three sections (see DEC-004)
> Designed on: 2026-11-26
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. No new dependency (S-22). No new table: the `server_setting` table and the three tools of SPEC-0114 are reused (S-2B).

## 2. Approaches
### Chosen: extend the SPEC-0114 mechanism by three section names
`SECTIONS` grows to eleven, `section_json` and `with_section` get three arms, the boot overlay and the seed iterate `SECTIONS` already. The two real differences are handled where they live: `git.hosts` (a free key map, not a fixed key set) and the host map publication at boot.
### Rejected: a table of hosts, one row per host
A second store and a second validator for one list; the SPEC-0114 row holds the section as the YAML does and `check_hosts` stays the only rule.
### Rejected: a live reload of the host map or the limits
Consumers read their section once at the start (`git/http/mod.rs:114`, `mcp/server.rs:71`); same as AUTO-038, no approved scenario needs a live value, so `max_pack_size_mb` is restart flagged (spec 3.2).

## 3. Architecture
```
set_setting(git|web|context7) -> ServerSettings::try_set -> check_keys (hosts is a free map) -> merge (hosts replaced whole)
   -> with_section -> ServerConfig::check() (check_hosts, web lists) -> row + record (one tx)
boot: build_with_env -> server_settings_boot::apply -> rows replace sections -> check() -> publish host map when a git row applied
```

## 4. Data model
None new. A `git` row is the JSON of `GitConfig` with `hosts` as an object; `web` the JSON of `WebConfig` (the two test only fields are `serde(skip)`); `context7` the JSON of `Context7Config`.

## 5. Contracts and interfaces
- `settings/server_settings.rs`: `SECTIONS: [&str; 11]`; arms in `section_json`, `with_section` (the web arm keeps `test_base` and `test_net` of the old value, D4); `free_paths(section)` returns `["hosts"]` for `git`; `check_keys` does not descend into a free path; after `merge`, a free path present in the patch replaces the merged value; the `git.hosts` read is sorted by key (D7, D8, object keys sorted explicitly).
- `git/remote.rs`: `HostMap` serializes as an object (`serialize_map`, entries in stored order) and deserializes by `deserialize_any`, a map or a sequence of two text pairs (a pair of another shape fails with the serde message under `invalid setting 'git':`); `check_hosts` is unchanged, so a repeated host in a pair list is refused with today's text.
- `settings/server_settings_boot.rs`: after a stored `git` row is applied and `check()` passes, `crate::git::remote::validate_hosts(&config.git)` publishes the map (D3, D6: never from `config.rs`).
- Tool descriptions and the contract: parameter description and list description name the eleven sections (14.2 of the spec).

## 6. File structure impact
`settings/server_settings.rs`, `settings/server_settings_boot.rs`, `git/remote.rs` (HostMap only), `tools/settings.rs`, `mcp/server.rs` (two descriptions), `settings_screen.rs` only if it hard codes the names (checked at story time), test files `settings/server_settings_e2e.rs` (declared edits), new `settings/server_settings3_e2e.rs` for `e2e_0115_`, docs, `TOOL_CONTRACT.txt`, golden.

## 7. Compatibility and risks
YAML alone behaves as today: `HostMap` Deserialize is a superset (map accepted as before), Serialize changes shape but is read only by the settings store (`ServerConfig` is not serialized anywhere else; recount at story time with `rg 'to_value|to_string\(&.*config'`). `e2e_new_006` and `e2e_new_224` unchanged. Risk: the process wide host map (published by `validate_hosts`) is shared by tests: every test that boots with a stored `git` row takes `lock_for_test`. Risk: `deserialize_any` with YAML: the map path calls `visit_map` as before; proven by `e2e_new_006`.

## 8. Security design
No secret value. Provider client secrets are the NAMES of environment variables (`github_client_secret_env`, `gitlab_client_secret_env`), never a value; git host credentials live in the OAuth token store. A key named like a secret is an unknown key (E2E-009). The SPEC-0114 secret machinery is untouched, so a future secret key of these sections only needs `secret_paths`.

## 9. Test strategy
Tests in `settings/server_settings3_e2e.rs` (`e2e_0115_NNN`), written first and run RED on the unchanged production code, per story. PostgreSQL ones (021, 035, 048) in `server_settings_pg_e2e.rs` style, real clock, scratch pgvector container.

## 10. Decisions log
- DEC-001 (S-2B) `hosts` is a free key map replaced whole by a patch: an object merge cannot remove a host, and a removed host is a real operation (S4). Rejected: a null value removes a key (a second convention).
- DEC-002 (S-05) the pair list form exists because a JSON object cannot carry a repeated key: the duplicate refusal of today needs a value that can hold one.
- DEC-003 (S-2B) the map is published in `server_settings_boot::apply`, the one place the stored configuration is made, not in `config.rs` (D6).
- DEC-004 (S-26) `secret_paths` unchanged for the three sections: there is none to protect, and inventing one would be a second secret store.
- DEC-005 (S-10) every key of the three sections is restart flagged, including `max_pack_size_mb`.
- DEC-006 (S-05) key order of the host read is byte order, sorted at read.

## 11. Stories
| Story | Covers | Tier |
|---|---|---|
| US-0345 the three sections and their rules | FR-NEW-001 to 006, 012, 013, 015 to 017 | 1 |
| US-0346 boot reads the three sections | FR-NEW-007 to 011 | 1 |
| US-0347 PostgreSQL, docs and contract | FR-NEW-014 | 1 |

### Test to story map
US-0345: E2E-001, 002, 006 to 010, 012, 015, 016, 018, 019, 022 to 028, 033, 034, 036, 037, 039, 040, 041, 047, 052, 053, 054.
US-0346: E2E-003, 004, 005, 011, 013, 014, 017, 020, 029, 030, 031, 032, 038, 042 to 046, 049, 050, 051.
US-0347: E2E-021, 035, 048 and the regenerated golden, the docs, the migrate check.

## 12. Drift notes (implementation)
- DRIFT-001 The spec words the clone tool `git.clone`; the served name is `git.remote_clone` (`mcp/server.rs:4591`). The tests call `git.remote_clone`; the refusal text is the one the spec quotes.
- DRIFT-002 E2E-014 pushes a pack of zero bytes: "accepted" means "not refused for its size" (the body is not a pack). The 413 refusal is the body limit layer, whose text is not pinned.
- DRIFT-003 E2E-002 is split: the stored half in `e2e_0115_002`, the fetch before and after the restart in `e2e_0115_002b`.
- DRIFT-004 E2E-004 uses a GitLab host with a `glpat-` token, read back from the token store instead of the screen.
- DRIFT-005 A review found that a YAML `hosts:` with no value failed after `HostMap` read pairs; fixed (`visit_unit`), the settings change refuses null explicitly. Test `e2e_0115_055` (not in the spec table, 55 tests in all).
- DRIFT-006 The host map is published only when the stored list differs from the configuration's (test `e2e_0115_056`, not in the spec table, 56 tests in all): the YAML seed writes a git row, so publishing on every applied row raced the lock of the other tests of the process wide map.
