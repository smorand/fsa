# SPEC-0106: Document converters set up by an administrator, design

> Spec: sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/spec.md
> Nature: FEAT
> Depth: L
> Security: internal finding
> Designed on: 2026-11-24
> Target tier: 2 (default: no `Tier:` in the constitution, no `.spec.json`)

## 1. Stack and skills
| Element | Choice | Version | DEC |
|---|---|---|---|
| Language, web, storage, tests | the existing stack (Rust 2024, axum, rmcp, reqwest, rusqlite and sqlx, `cargo test`) | as in `Cargo.toml` | none |
| New dependency | none (header name and value checks use `reqwest::header`) | n/a | DEC-005 |

Skills loaded: `rust` (already in context). Effect: `make check` is the gate; tests are `#[cfg(test)]` modules driven through the real `McpServer` tool methods, the real router and the real screen router. Existing project (`git ls-files '*.rs'` far above 300 files).

## 2. Approaches
### Chosen: extend the settings registry, resolve the converter on every conversion
The SPEC-0102 registry (`settings/`) already holds encrypted secrets, a record of changes, platform administrator only tools, `/api/admin` routes and a screen. Converters become a second family in that module: three persisted concepts in the admin store (the converters, the platform default, a column `project.converter_id`), one rule module for the input checks, `impl Registry` methods, ten admin tools, ten routes and one screen. A conversion no longer reads `AppState.doc_service` directly: a new `AppState::doc_service_for(mount_id)` resolves project, default, then the configuration file service held in `AppState.doc_service`, and builds the service from the stored row at each call. Pros: no `AppState` field (the many literals in tests stay valid), no cache to invalidate, changes apply to the next call, the registry pattern is reused whole. Cons: a client is built per call for the http form; the signature of the conversion entry points changes. Affected scope: `settings/`, `docs/service.rs`, `state.rs`, `core/fs_ops.rs`, `tools/`, `mcp/server.rs`, `api/`, a new screen. Best suited when the stored value is small and the conversion is far more costly than building a client.
### Rejected: swap the service held in `AppState` at run time
It gives one converter for the whole platform, not one per project (FR-NEW-009), and needs interior mutability in a shared struct (DEC-006).
### Rejected: a separate converters service or crate
A new deployable for one table and one resolver; the owner wants fewer moving parts, not more (DEC-002).
### Rejected: cache built services by converter version
Faster, but a cache is a second place to invalidate on update, delete and key change, with no requirement asking for the speed (DEC-006).

## 3. Architecture
```
 tools/converters.rs  (admin tools)        api/admin_converters.rs (web)     converters_screen.rs
          \                                       |                                /
           +--------------------> settings::Registry  (converters.rs: create, update, delete, list,
                                   |                    default, project, resolve) ---> settings/converter_rules.rs
                                   v
                       settings/converter_store.rs  --- admin store: converter, converter_default, project.converter_id

 fs.write_bytes / upload / documentize ---> AppState::doc_service_for(mount_id)
                                              1. project.converter_id  2. converter_default  3. AppState.doc_service (file)
                                              --> docs/resolve.rs builds CliDocService or ApiDocService (or BrokenSecretService)
```
Bounded contexts: Settings (registry, record) and Documents (conversion). Documents calls Settings only through `doc_service_for`.

## 4. Data model
Admin store, both engines:
- `converter` (new): `converter_id` text PK, `name_key` text unique (caseless name), `name`, `form` (`program` or `http`), `command` text (JSON list, null for http), `url`, `auth_header`, `secret_enc` blob (null when none), `file_field`, `response_field`, `extensions` text (JSON list), `max_input_bytes` bigint, `timeout_seconds` bigint, `created_at`, `updated_at` text.
- `converter_default` (new): `slot` text PK (always `default`), `converter_id` text.
- `project.converter_id` (new nullable column, `ColumnMigration`): the project's converter. A hard removal deletes the row, so the selection goes with it (FR-NEW-021); a soft removal keeps it.
Rollback: the new table and column are additive; an older binary ignores them. Fingerprints: the two `storage::admin` hashes of `production_ddl_is_unchanged_on_every_engine` change once per story that touches the schema (US-0309 and US-0313).

## 5. Contracts and interfaces
- Tools (platform administrator only, refusal first): `admin.create_converter`, `admin.update_converter`, `admin.delete_converter`, `admin.list_converters`, `admin.set_default_converter`, `admin.clear_default_converter`, `admin.get_default_converter`, `admin.set_project_converter`, `admin.clear_project_converter`, `admin.get_project_converter`. Inputs are `serde_json::Value` fields checked by the rule module so the type texts of the spec can be produced; answers and refusal texts are those of the spec (FR-NEW-001 to 009).
- Web: the ten routes of FR-NEW-018 under `/api/admin`, through the `run` wrapper of `api/admin_settings.rs` (401, then administrator, then the operation).
- Screen: `/app/converters` and `/app/converters/<action>` with the actions of FR-NEW-019.
- Internal: `AppState::doc_service_for(&self, mount_id: &str) -> Result<Option<Arc<dyn DocService>>>`; `trait DocService` gains `fn preflight(&self) -> Result<()> { Ok(()) }`; `docs::resolve::build(converter: &Converter, secret: Option<SecretText>) -> Arc<dyn DocService>`; `docs::redact(text: &str, secret: &str) -> String`.

## 6. File structure impact
| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/core/src/settings/converter.rs` | create: `Converter`, `ConverterForm`, input and answer types | FR-NEW-001 | US-0309 |
| `crates/core/src/settings/converter_rules.rs` | create: input checks in the order of the spec | FR-NEW-001 to 005 | US-0309 to US-0312 |
| `crates/core/src/settings/converter_store.rs` | create: SQL for converters, default, project selection | FR-NEW-001 to 009 | US-0309, US-0313 |
| `crates/core/src/settings/converters.rs` | create: `impl Registry` operations and `resolve` | FR-NEW-001 to 010 | US-0309 to US-0315 |
| `crates/core/src/settings/registry.rs`, `store.rs` | modify: fields `pub(super)`, `Target` kinds | FR-NEW-016 | US-0309 |
| `crates/core/src/storage/admin.rs`, `storage/rel/schema.rs` | modify: tables, column, fingerprints | FR-NEW-001, 008, 009, 021 | US-0309, US-0313 |
| `crates/core/src/tools/converters.rs`, `tools/admin.rs`, `mcp/server.rs` | create / modify: tool functions, wrappers, name list | FR-NEW-001 to 009, 017 | US-0309 to US-0314 |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, count assertions | modify (golden regenerated) | tool additions | each story adding tools |
| `crates/core/src/docs/service.rs`, `docs/resolve.rs`, `docs/mod.rs` | modify / create: `preflight`, redirect policy, redaction, builder | FR-NEW-003, 010, 012, 013, 014, 015, FR-MOD-001, 002 | US-0310, US-0315 to US-0317 |
| `crates/core/src/state.rs`, `core/fs_ops.rs`, `tools/write.rs`, `tools/document.rs`, `mcp/server.rs`, `api/dataplane.rs` | modify: callers use `doc_service_for` | FR-NEW-010, 011 | US-0315 |
| `crates/core/src/api/admin_converters.rs`, `api/mod.rs`, `api/dataplane.rs`, `api/openapi.rs` | create / modify: routes and tables | FR-NEW-018 | US-0319 |
| `crates/core/src/converters_screen.rs`, `app.rs`, `lib.rs` | create / modify | FR-NEW-019 | US-0320 |
| `crates/core/src/settings/converters_e2e.rs`, `settings/mod.rs` | create / modify: tests `e2e_0106_*` | all | US-0309 to US-0321 |
| `README.md`, `AGENTS.md`, `.agent_docs/*.md`, `config/local.yaml.template` | modify | all | US-0321 |
Affected existing tests: tool and route counts (fs 53, admin 29 to 39, totals 140 to 150, with the frozen contract 136 to 146, REST routes 63 to 73): modify mechanically in the stories that add them. One existing test of a converter failure that quotes the answer (E2E-MOD-001): modify in US-0317. Config template: the `doc_service` comment says the registry takes precedence.
Affected documentation: as listed.

## 7. Compatibility and risks
Two declared changes to the configuration file converter: no http converter follows a redirect, and failure texts redact the token. Everything else is unchanged when the registry is empty (DEC-018). Risk: building the http client at every call costs about a millisecond against a conversion of seconds. Risk: `doc_service_for` is awaited before the write, so a registry read failure refuses the conversion (reported as the internal error text). Rollback: revert; rows stay unused.

## 8. Security design
Threat surface: an administrator supplied command (sink: the program launch at `docs/service.rs:404`) and a stored secret (sinks: answers, listings, records, screens, logs, failure texts, redirects).
- **A1** (`docs/service.rs:404`): the registry feeds the same `CliDocService` and `expand_argv`; no shell is added (DEC-016). Exploit test E2E-092 and E2E-067 in US-0317: literal payloads `$(touch /tmp/fsa-pwned)`, a backtick form and `x | touch /tmp/fsa-pwned`; the recorded argument list is read through the converter's own output, absence of the file through a stat, and both are red against an implementation that builds `sh -c`.
- **B1** (`docs/service.rs:611-615`): the failure text is built from the body after `redact` (DEC-012). E2E-093, E2E-117, E2E-059 and E2E-MOD-001 in US-0317; red on today's code for the configuration file token.
- **B2** (answers, records, screens): `SecretText` has no `Display`, rows carry `has_secret` only, records carry field names only (DEC-011, DEC-013). E2E-087, E2E-094 in US-0317 (a second channel: the stored form read by an operator through the store, and the server log).
- **B6** (`docs/service.rs:542-546`): `redirect::Policy::none()` for every http converter (DEC-014). E2E-118, E2E-135 and E2E-141 in US-0317 read the second stub for any request.
- Secrets: encrypted with the existing key; never logged (`tracing` calls name ids only, DEC-011); the input structs derive no `Debug`. Least privilege: administrator check first on every interface (DEC-015).

## 9. Test strategy
All tests: functional e2e in `crates/core/src/settings/converters_e2e.rs`, names `e2e_0106_<id>_<slug>` (`e2e_0106_mod001_...`), through `tool_json` on the real `McpServer`, the real router (`rest`) and the screen helpers (`page`, `open`, `submit`) of `settings/e2e.rs` (the module reuses its `Fx`, which gains a converters helper in US-0309). Stand ins: the script converters of `docs/manifest_e2e.rs` and its http stub `serve` (made `pub(crate)`), a `record-args` script that writes its arguments into the Markdown it returns, a stub that answers only for an exact header (401 otherwise), a stub that echoes the received header into a 500 body, a redirecting stub pair. Restart tests use `Fx::restart`. E2E-082 drives `ServerConfig` validation directly. Run: `cargo test -p fsa-core --lib -- e2e_0106_`. Test data: generated.

| Stories | Tests |
|---|---|
| US-0309 | 001, 003, 017, 018, 019, 020, 021, 022, 023, 024, 025, 026, 049, 050, 061, 073, 085, 121 |
| US-0310 | 002, 027, 028, 029, 030, 031, 032, 037, 038, 097, 125, 126, 127, 128, 143, 144 |
| US-0311 | 033, 034, 035, 036, 062, 063, 112, 145, 146 |
| US-0312 | 005, 039, 040, 041, 042, 083, 084, 116, 133, 134, 137, 138, 147, 149, 150, 066 |
| US-0313 | 004, 006, 007, 046, 047, 048, 069, 071, 072, 074, 108, 119, 120, 123, 136 |
| US-0314 | 008, 043, 044, 045, 070, 091, 096, 122, 124 |
| US-0315 | 009, 010, 011, 012, 055, 060, 068, 075, 079, 080, 081, 082, 095, 098, 104, 106, 107, 110 |
| US-0316 | 054, 056, 057, 064, 065, 111, 113, 114, 129, 130, 131, 132, 148, 076 |
| US-0317 | 059, 067, 077, 078, 087, 092, 093, 094, 101, 117, 118, 135, 141, MOD-001 |
| US-0318 | 015, 086, 102, 103, 109 |
| US-0319 | 013, 051, 058, 099 |
| US-0320 | 014, 052, 053, 088, 089, 090, 100, 115, 139, 140, 142 |
| US-0321 | 016, 105 |

## 10. Decisions log
- **DEC-001:** Tests live in `settings/converters_e2e.rs` and reuse the fixture `Fx` and the SPEC-0103 stand ins. **Obliged by:** SPEC-0106/FR-NEW-001 (tests of every requirement). **Rationale:** one module, one command; the fixture already sets up an administrator, a member, a master key and the screen helpers. **Alternatives considered:** extending `settings/e2e.rs` (98 tests already). **Source:** existing code. **Evidence:** `settings/e2e.rs:50-167`, `docs/manifest_e2e.rs:76-163`.
- **DEC-002:** Converters extend the settings module (files `converter.rs`, `converter_rules.rs`, `converter_store.rs`, `converters.rs`); `Registry` and `SettingsStore` fields become `pub(super)` so the new files can add `impl` blocks. **Obliged by:** SPEC-0106/FR-NEW-001, FR-NEW-016, FR-NEW-017. **Rationale:** the record, the key, the transaction helper and the admin check are already there. **Alternatives considered:** a new top level module (duplicates `record` and the cipher wiring). **Source:** existing code. **Evidence:** `settings/registry.rs:169-199`, `settings/store.rs:30-38`.
- **DEC-003:** Tables `converter` and `converter_default`, and the nullable column `project.converter_id` with a `ColumnMigration`; the name is unique through `name_key` (the same caseless key as providers); ids are `conv-` plus 12 hex digits of a v4 uuid; update checks the name key ignoring the converter being updated. **Obliged by:** SPEC-0106/FR-NEW-001, FR-NEW-005, FR-NEW-008, FR-NEW-009, FR-NEW-021. **Rationale:** a column dies with its project row, which gives FR-NEW-021 with no code; a soft removed row keeps it. **Alternatives considered:** a `project_converter` table with a foreign key cascade (an extra join for the usage count). **Source:** existing code. **Evidence:** `storage/admin.rs:104-183,397-403`, `settings/registry.rs:226-229`.
- **DEC-004:** The tool inputs are `serde_json::Value` fields (no `Debug` derive) and the rule module `converter_rules.rs` produces the type texts (`<field> must be text`, the range texts, `command must be a list of text arguments`, `clear_secret must be true or false`); a field is given when present and not `null`, and the order of checks is the one written in FR-NEW-001 to FR-NEW-005. **Obliged by:** SPEC-0106/FR-NEW-001, 002, 003, 004, 005. **Rationale:** typed argument structs would turn a wrong type into a generic parse error with another text. **Alternatives considered:** typed structs with custom deserializers (text mapping scattered). **Source:** spec. **Evidence:** `mcp/server.rs:1582` (current typed provider args).
- **DEC-005:** The secret uses `SecretText`, `encrypt` and `decrypt` of the registry; a blank secret is no secret; the header name and the value are checked with `reqwest::header::HeaderName` and `HeaderValue` (no dependency added); update keeps the stored blob unless a non blank secret or `clear_secret` is given, so it needs no key otherwise. **Obliged by:** SPEC-0106/FR-NEW-003, FR-NEW-005, FR-NEW-015. **Rationale:** the cipher and its failure text exist. **Alternatives considered:** a hand written header grammar. **Source:** existing code. **Evidence:** `settings/secrets.rs:12-57`, `settings/registry.rs:77`.
- **DEC-006:** `AppState::doc_service_for(mount_id)` resolves in the order project, default, `AppState.doc_service`, builds the service from the row at each call and returns an `Arc`; the callers hold it for the whole conversion, which gives FR-NEW-011 (a running conversion keeps its converter). No cache, no new `AppState` field. **Obliged by:** SPEC-0106/FR-NEW-010, FR-NEW-011. **Rationale:** SPEC-0102 DEC-007 resolves search targets at call time the same way. **Alternatives considered:** a cache keyed by id and update time; an `ArcSwap`. **Source:** existing code. **Evidence:** `settings/registry.rs:716-741`, `state.rs:25`, `app.rs:126`.
- **DEC-007:** `DocService` gains `fn preflight(&self) -> Result<()>` (default `Ok`); a registry converter whose secret cannot be read is built as `BrokenSecretService`, which accepts the same extensions and cap and whose `preflight` and `convert` answer `ERR_INTERNAL_ERROR: the secret of converter '<name>' cannot be read`; `write_bytes_documented` and `documentize` call it after `ensure_input_size` and before any write. **Obliged by:** SPEC-0106/FR-NEW-010, FR-NEW-012. **Rationale:** it gives the order no converter, extension, size, secret. **Alternatives considered:** failing at resolution (the extension and size texts would be lost). **Source:** existing code. **Evidence:** `core/fs_ops.rs:635-712,746-770`.
- **DEC-008:** The stored extensions have no dot and lower case without duplicates; `build` passes them with a dot to `resolve_extensions`; an empty list falls back to `DOC_SERVICE_EXTS`; matching is the existing `eligible_in` (a path ends with the dotted extension). **Obliged by:** SPEC-0106/FR-NEW-004, FR-NEW-012. **Rationale:** the refusal text lists them as `.pdf, .docx` already. **Alternatives considered:** storing the dot (the spec stores none). **Source:** existing code. **Evidence:** `docs/service.rs:44-52,143-190`, `core/fs_ops.rs:746-760`.
- **DEC-009:** `max_input_bytes` is passed as the service cap, which `ensure_input_size` and the answer limit already read. **Obliged by:** SPEC-0106/FR-NEW-004, FR-NEW-013. **Rationale:** one cap, as for the file converter. **Alternatives considered:** two fields. **Source:** existing code. **Evidence:** `core/fs_ops.rs:763-770`, `docs/service.rs:299`.
- **DEC-010:** `ApiDocService::new` sets `redirect(Policy::none())`; a 3xx falls in the existing non success branch (`document service returned HTTP <status>: <slice>`). **Obliged by:** SPEC-0106/FR-MOD-002, FR-NEW-003. **Rationale:** one client builder serves both sources. **Alternatives considered:** a policy that strips custom headers (leaves the request to another host). **Source:** existing code. **Evidence:** `docs/service.rs:537-546,611-615`.
- **DEC-011:** Secrets never reach a log: `tracing` calls of the registry carry converter ids and names only; no input or answer struct derives `Debug` over a secret; answers carry `has_secret`. **Obliged by:** SPEC-0106/FR-NEW-015. **Rationale:** the SPEC-0102 rule. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `settings/secrets.rs:29-37`.
- **DEC-012:** `redact(text, secret)` replaces every occurrence of the trimmed secret (nothing when blank) in the whole response body before `head_of` cuts it, for the registry secret and the file token; applied to failure texts only. **Obliged by:** SPEC-0106/FR-MOD-001, FR-NEW-015. **Rationale:** cutting first would leak a prefix of a straddling secret. **Alternatives considered:** redacting after the cut. **Source:** spec. **Evidence:** `docs/service.rs:611-615,833`.
- **DEC-013:** Records use new `Target` constructors (`converter`, `default_converter`, `project_converter`), the actions of FR-NEW-016, field names in the order of the spec, and no record for a refused operation, an unchanged update or a repeated set or clear; the screen reads them through a `list_changes_of_kinds` store call. **Obliged by:** SPEC-0106/FR-NEW-016, FR-NEW-019. **Rationale:** one record table, one reader. **Alternatives considered:** a second table. **Source:** existing code. **Evidence:** `settings/registry.rs:651`, `settings/store.rs:335-346,444`.
- **DEC-014:** The project selection is read and written through `SettingsStore` on the same database handle; project existence is a row in `project` whatever `deleted_at` holds; the usage count is a count of rows with the converter id. **Obliged by:** SPEC-0106/FR-NEW-007, FR-NEW-009, FR-NEW-021. **Rationale:** a soft removed project still uses its converter. **Alternatives considered:** going through `AdminBackend` project calls (they hide soft removed rows). **Source:** existing code. **Evidence:** `storage/admin.rs:26-35,397-403`.
- **DEC-015:** Every tool function starts with `state.require_admin(person)` through the existing `registry()` helper; REST uses `run`; the screen follows the identity, administrator, token order of `models_screen.rs`. **Obliged by:** SPEC-0106/FR-NEW-017, FR-NEW-018, FR-NEW-019. **Rationale:** the refusal must come before body shape and existence checks. **Alternatives considered:** checking inside each method. **Source:** existing code. **Evidence:** `tools/models.rs:239-247`, `api/admin_settings.rs:44-70`, `models_screen.rs:62-110`.
- **DEC-016:** Program converters are built with `CliDocService::new(DocServiceCliConfig{command, timeout_secs}, ...)`, the existing launch with an argument list and `expand_argv`; no shell is introduced and the environment of the child is not cleared (proposal P2). **Obliged by:** SPEC-0106/FR-NEW-014, FR-NEW-002. **Rationale:** the same sink with the same controls. **Alternatives considered:** clearing the environment (changes the file converter). **Source:** existing code. **Evidence:** `docs/service.rs:355,404-412,656-683`.
- **DEC-017:** The ten tool names go in `tools/admin.rs` name list and `mcp/server.rs` wrappers, with description constants checked by the existing equality test; REST rows go in `REST_ROUTES` and `OPERATIONS`; counts and the golden are updated in the story that adds them. **Obliged by:** SPEC-0106/FR-NEW-018. **Rationale:** the contract tests fail otherwise. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `tools/models.rs:19-40`, `tools/admin.rs:975-1008`, `api/dataplane.rs:45,110-124`, `api/openapi.rs:521`.
- **DEC-018:** The configuration file converter, its start check (`validate_doc_service`) and its `AppState` slot stay as they are; with an empty registry `doc_service_for` returns the slot unchanged. **Obliged by:** SPEC-0106/FR-NEW-020. **Rationale:** non regression. **Alternatives considered:** moving it into the registry (BL-0173). **Source:** existing code. **Evidence:** `config.rs:1181,1317-1380`, `app.rs:126`.
- **DEC-019:** The batch check of an upload keeps resolving once for the converter and extension (as today), and each file resolves again when its own conversion starts; a refusal then stops the upload. **Obliged by:** SPEC-0106/FR-NEW-010. **Rationale:** matches today's partial writes. **Alternatives considered:** pre checking the size (a behavior change). **Source:** existing code. **Evidence:** `api/dataplane.rs:689-708`.
- **DEC-020:** The screen is a copy of the `models_screen.rs` structure with its own `CsrfStore`, a `command` text area parsed one argument per line, a `clear_secret` checkbox and an always sent `response_field` and `extensions` on edit. **Obliged by:** SPEC-0106/FR-NEW-019. **Rationale:** same conventions. **Alternatives considered:** sharing one token store across screens (proposal, not decided here). **Source:** existing code. **Evidence:** `models_screen.rs:37-110`.

## 11. Stories
| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0309 | create and list program converters | P1 | FR-NEW-001, FR-NEW-002, FR-NEW-006, FR-NEW-017, FR-NEW-016 | main | 1 | no |
| US-0310 | http converters and their encrypted secret | P1 | FR-NEW-003, FR-NEW-015 | US-0309 | 1 | no |
| US-0311 | extensions, answer cap, timeout and value types | P2 | FR-NEW-004, FR-NEW-001 | US-0310 | 2 | no |
| US-0312 | update a converter | P1 | FR-NEW-005, FR-NEW-016 | US-0311 | 2 | no |
| US-0313 | the platform default and the converter of a project | P1 | FR-NEW-008, FR-NEW-009, FR-NEW-021, FR-NEW-016 | US-0312 | 1 | no |
| US-0314 | delete a converter | P2 | FR-NEW-007 | US-0313 | 2 | no |
| US-0315 | a conversion uses the resolved converter | P1 | FR-NEW-010, FR-NEW-011, FR-NEW-013, FR-NEW-020 | US-0314 | 1 | no |
| US-0316 | extensions, size cap, unreadable secret and batch behavior | P1 | FR-NEW-012, FR-NEW-013, FR-NEW-010, FR-NEW-004 | US-0315 | 1 | no |
| US-0317 | arguments stay literal, secrets stay hidden, no redirect | P1 | FR-NEW-014, FR-NEW-015, FR-MOD-001, FR-MOD-002 | US-0316 | 1 | no |
| US-0318 | reading the record of converter changes | P2 | FR-NEW-016, FR-NEW-017 | US-0317 | 2 | no |
| US-0319 | the web routes for converters | P2 | FR-NEW-018 | US-0318 | 2 | no |
| US-0320 | the converters screen | P2 | FR-NEW-019 | US-0319 | 2 | no |
| US-0321 | the journey, the upgrade and the docs | P2 | FR-NEW-020, FR-NEW-016 | US-0320 | 2 | yes |
