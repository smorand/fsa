# US-0283: providers, secrets, the master key and the record of changes

> Spec: SPEC-0102 (sdlc/specs/SPEC-0102-platform-settings-and-model-registry/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> min_tier: 1
> Files touched: 9
> Closes spec: no

## Objective
Create the platform settings store and the first half of the registry: providers with their encrypted write only secrets, the master key check at start, the durable record of changes, and the tools `admin.create_provider`, `update_provider`, `delete_provider`, `list_providers` and `list_setting_changes`, all platform administrator only. After this story an administrator can register providers and read what changed, and no secret can be read back.

## Requirements covered
- SPEC-0102/FR-NEW-001: WHEN a platform administrator creates a provider with a name, an endpoint, an authentication scheme and optionally a secret THE platform SHALL store it and answer with the provider.
- SPEC-0102/FR-NEW-002: IF the name, the endpoint, the authentication scheme or the secret of a provider is not valid THEN THE platform SHALL refuse the operation, store nothing and answer with the text of the rule that failed.
- SPEC-0102/FR-NEW-003: WHEN a platform administrator updates a provider THE platform SHALL change only the fields given, apply the change to every use of the provider at once, and answer with the provider.
- SPEC-0102/FR-NEW-004: The platform SHALL keep the secret of a provider encrypted, and SHALL NOT return it in any answer, listing, screen page, change record, log line or error text.
- SPEC-0102/FR-NEW-005 (partial: boot check and refusal; unreadable secret at call time comes with US-0286): IF no master key is configured THEN THE platform SHALL refuse to store a secret with `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`, and SHALL accept providers that carry no secret.
- SPEC-0102/FR-NEW-006 (partial: provider without model; the "still has models" refusal comes with US-0284): WHEN a platform administrator deletes a provider that has no model THE platform SHALL remove the provider and its secret and answer `{deleted: true}`.
- SPEC-0102/FR-NEW-007: WHEN a platform administrator lists providers THE platform SHALL list them ordered by name without regard to case, 100 per answer, each with its fields and `has_secret`.
- SPEC-0102/FR-NEW-018 (partial: provider changes): The platform SHALL record every creation, update and deletion of a provider or a model and every default change or clear, with who did it, when, what it concerned and which fields changed, and SHALL NOT record any value of a secret.
- SPEC-0102/FR-NEW-019: WHEN a platform administrator lists the changes THE platform SHALL list them newest first, 100 per answer.
- SPEC-0102/FR-NEW-020 (partial: provider and record tools): The platform SHALL allow every registry operation, reading included, to platform administrators only.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0102_00 e2e_0102_01 e2e_0102_02 e2e_0102_05 e2e_0102_06 e2e_0102_09 e2e_0102_11 settings::`. The tests create providers through the handlers, search every output (answers, lists, records, logs captured with a tracing subscriber) for the literal secret `sk-test-123`, restart the registry with and without the key, and assert the exact refusal texts.

## Design excerpt
### Decisions
- DEC-001 (SPEC-0102): No new dependency; the stack is the repository's. **Obliged by:** n/a (no code impact): the spec names no technology. **Rationale:** cipher, relational layer, HTTP client and ids are in the manifest. **Alternatives considered:** a secrets crate, a vault client (new dependency, not asked). **Source:** existing code. **Evidence:** `git/oauth/cipher.rs:20-75`, `Cargo.toml`.
- DEC-002 (SPEC-0102): Providers, models, defaults and changes are four tables of the global admin store, primary keys on the caseless name (`name_key`) so equal names cannot coexist on either engine, ids found through secondary indexes. **Obliged by:** SPEC-0102/FR-NEW-002, FR-NEW-009, FR-NEW-018. **Rationale:** the admin store is global, on both engines, and already holds platform concepts; a primary key on the name makes concurrent duplicate creation safe without a unique index (the schema layer has none). **Alternatives considered:** the YAML file with reload (no audit, no admin tools), a new store, per volume tables. **Source:** existing code. **Evidence:** `storage/admin.rs:23-150`, `storage/rel/schema.rs` (`Index` is non unique).
- DEC-003 (SPEC-0102): Secrets are encrypted with the existing AES 256 GCM cipher and the master key read from `FSA_TOKEN_KEY` (legacy name through `env_compat`); `app.rs` checks the key at boot after the admin store is connected and stops the start with the cipher's message (the text of its `message`, which begins `FSA_TOKEN_KEY must be valid base64.` or `FSA_TOKEN_KEY must decode to 32 bytes`) and a non zero exit through `cli::run`; a valid key is handed to the admin store with `set_master_key`. **Obliged by:** SPEC-0102/FR-NEW-004, FR-NEW-005. **Rationale:** the owner asked to reuse the key; the git store builds its cipher lazily, so a boot check is new. **Alternatives considered:** a key file, a second variable, lazy failure only. **Source:** existing code and user answer 3. **Evidence:** `git/oauth/cipher.rs:29-46`, `git/oauth/store.rs:26-33,116`, `tools/git_auth.rs:43-49`, `cli.rs:136`.
- DEC-004 (SPEC-0102): Ids are `prov-`, `mdl-` plus 12 hex of a uuid v4; `change_id` is `chg-` plus 12 zero padded digits taken as the largest existing plus one inside the writing transaction; times are `chrono` UTC RFC 3339 with seconds. **Obliged by:** SPEC-0102/FR-NEW-001, FR-NEW-008, FR-NEW-018. **Rationale:** the spec names the prefixes and a monotonic change order; a primary key conflict on a race is retried once. **Alternatives considered:** uuid change ids (no order), a sequence (not portable across the two engines). **Source:** design. **Evidence:** n/a.
- DEC-005 (SPEC-0102): A `SecretText` newtype (no `Display`, `Debug` prints `Secret(set)`) carries secrets in memory; answer structs have `has_secret` only; refusal texts are built from names and numbers, never from request bodies; the registry tools log nothing of their arguments. **Obliged by:** SPEC-0102/FR-NEW-004. **Rationale:** no output path can format a secret by accident. **Alternatives considered:** discipline only. **Source:** design. **Evidence:** `config.rs:31,51` (`Dsn` redaction precedent).
- DEC-006 (SPEC-0102): One `settings::Registry` holds every operation and rule; tools, REST handlers and the screen call it and add nothing but access control and shaping. **Obliged by:** SPEC-0102/FR-NEW-020, FR-NEW-021, FR-NEW-022. **Rationale:** the repository rule that MCP and REST share one implementation. **Alternatives considered:** per surface logic. **Source:** existing code. **Evidence:** `AGENTS.md` conventions (one implementation).
- DEC-009 (SPEC-0102): A change record is inserted in the same transaction as the change it describes; refused or no op changes write nothing; `fields` lists the names whose values differ. **Obliged by:** SPEC-0102/FR-NEW-018. **Rationale:** the record can never disagree with the data. **Alternatives considered:** a record after commit. **Source:** design. **Evidence:** `storage/admin.rs:244-262` (transaction use).
- DEC-010 (SPEC-0102): Lists order by `name_key` (providers), by provider `name_key` then model `name_key` (models), by `change_id` descending (changes), page 100 with the marker of design 5.1; the listing queries are keyset queries. **Obliged by:** SPEC-0102/FR-NEW-007, FR-NEW-012, FR-NEW-019. **Rationale:** stable paging while data changes. **Alternatives considered:** offset paging. **Source:** existing code. **Evidence:** `tools/derived.rs` marker pattern (SPEC-0100).
- DEC-011 (SPEC-0102): Validation lives in `Registry` in the order of the spec (FR-NEW-002, 003, 009, 010, 013) on the state after the update; `clear_secret` and `secret` together are refused before any other update check. **Obliged by:** SPEC-0102/FR-NEW-002, FR-NEW-003, FR-NEW-009, FR-NEW-010. **Rationale:** exact texts and order are contract. **Alternatives considered:** validate in each surface. **Source:** spec. **Evidence:** n/a.
- DEC-015 (SPEC-0102): The admin store exposes `settings()` and `set_master_key()` as provided methods (default none and no op), like `derived_jobs()`. **Obliged by:** SPEC-0102/FR-NEW-001, FR-NEW-005. **Rationale:** no `AppState` field, so the ~25 hand built states in tests stay untouched. **Alternatives considered:** an `AppState.registry` field. **Source:** existing code. **Evidence:** `storage/traits.rs:318`, `storage/admin.rs:270`, `state.rs:16`.
### Data model
Tables `provider`, `setting_change` of the admin store exactly as design section 4 (`model` and `default_model` arrive with their stories). `secret_enc` is the AES 256 GCM blob (12 byte nonce, ciphertext, 16 byte tag) of `git/oauth/cipher.rs::encrypt`.
### Contracts
Tools: `admin.create_provider(name, endpoint, auth_scheme = "none", secret = "")`, `admin.update_provider(provider_id, name?, endpoint?, auth_scheme?, secret?, clear_secret?)`, `admin.delete_provider(provider_id)`, `admin.list_providers(marker = "")`, `admin.list_setting_changes(marker = "")`; shapes of design 5.1; refusal order and exact texts of FR-NEW-002 and FR-NEW-003 (checks apply to the provider as it is after the update; no op updates are not recorded and keep `updated_at`); `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`; the boot check stops `serve` with the cipher message (`message` field, not the `ERR_` display) and a non zero exit. Rust: `settings::{Registry, SettingsStore, SecretText}`, `AdminBackend::settings()` and `set_master_key()` (provided methods), `settings::secrets::master_key_from_env() -> Result<Option<[u8;32]>>`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/settings/{mod,model,store,registry,secrets}.rs` | create |
| `crates/core/src/storage/admin.rs`, `storage/traits.rs` | modify: two tables, provided methods, key slot |
| `crates/core/src/app.rs` | modify: key check after `admin.connect()`, `set_master_key` |
| `crates/core/src/tools/models.rs`, `tools/mod.rs`, `mcp/server.rs` | create / modify: five tools |
| `crates/core/src/settings/e2e.rs` | create: registry fixture (admin store in memory, key set, handlers, log capture) and these tests |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, `crates/core/tests/fixtures/schema_*.sql`, admin DDL fingerprint test, tool count tests and docs | regenerate and fix (123 to 128; admin 17 to 22): `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`, `MCPFS_REWRITE_SCHEMA_GOLDEN=1 cargo test -p fsa-core --test schema_golden`; fix `production_ddl_is_unchanged_on_every_engine` fingerprints, `mcp/server.rs` count tests (~4217, 4236, 4239, 4576), `tools/contract_golden.rs` (151, 274, 330, 344), `README.md`, `AGENTS.md`, `.agent_docs/tools.md` |
### Existing patterns
Tool and engine pattern: `mcp/server.rs:3216` (`admin.set_project_quota`: args struct, `require_admin`, `to_call_result`) and `tools/admin.rs:679`; provided store method `AdminBackend::derived_jobs()` (`storage/traits.rs:318`, override `storage/admin.rs:270`) and the SPEC-0100 store `search/derived/store.rs`; marker codec `tools/derived.rs`; cipher `git/oauth/cipher.rs:29-75`; key variable and compat read `git/oauth/store.rs:26-33`; boot failure path `app.rs` then `cli.rs:136`; Debug redaction `config.rs:31,51`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0102/E2E-001: registering a local provider
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_001_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates the provider `Local GPU` with endpoint `http://localhost:9010/v1` and no authentication scheme given, then lists the providers
- **Then** the answer holds a `provider_id` starting with `prov-`, `name` `Local GPU`, `auth_scheme` `none`, `has_secret` false and equal creation and update times; the list holds exactly that provider
- **And** the list answer has no marker
- **Priority:** Critical

### SPEC-0102/E2E-002: registering a cloud provider with a secret
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_002_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates the provider `Cloud Embeddings` with endpoint `https://api.example.com/v1`, scheme `bearer` and secret `sk-test-123`
- **Then** the answer shows `has_secret` true and no field holds `sk-test-123`
- **And** listing the providers shows `has_secret` true and never the secret
- **Priority:** Critical

### SPEC-0102/E2E-004: a provider name already taken
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_004_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; the provider `Local GPU` exists
- **When** Alice creates a provider named `local gpu`
- **Then** the answer is `ERR_INVALID_ARGUMENT: a provider named 'local gpu' already exists`
- **And** the list still holds one provider
- **Priority:** High

### SPEC-0102/E2E-005: the name is trimmed
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_005_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider named `  Café Embeddings  `
- **Then** the provider is named `Café Embeddings`
- **And** a second creation of `Café Embeddings` is refused as a duplicate
- **Priority:** Medium

### SPEC-0102/E2E-010: replacing a secret
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_010_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds the secret `sk-test-123`
- **When** Alice updates the provider with the secret `sk-new-456`
- **Then** the answer shows `has_secret` true, a later `updated_at`, and the same `provider_id` and `created_at`
- **And** the next embedding call of the platform sends `Authorization: Bearer sk-new-456` (E2E-038)
- **Priority:** High

### SPEC-0102/E2E-011: removing a secret
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_011_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds a secret
- **When** Alice updates it with `clear_secret` true and `auth_scheme` `none`
- **Then** the answer shows `has_secret` false and `auth_scheme` `none`
- **And** clearing the secret while the scheme stays `bearer` is refused with `ERR_INVALID_ARGUMENT: auth_scheme 'bearer' needs a secret`
- **Priority:** High

### SPEC-0102/E2E-012: no master key
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_012_*`)
- **Given** the same platform without a master key
- **When** Alice creates a provider with scheme `bearer` and secret `sk-test-123`, then creates a provider with scheme `none`
- **Then** the first answer is `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`; the second succeeds
- **And** only the second provider is listed
- **Priority:** High

### SPEC-0102/E2E-013: a malformed master key
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_013_*`)
- **Given** the master key variable holds `not-a-key`
- **When** the operator starts the platform
- **Then** the process exits with a non zero status and its error output begins `FSA_TOKEN_KEY must be valid base64.`
- **And** with a valid key the platform starts, and the same happens with the git token store disabled
- **Priority:** High

### SPEC-0102/E2E-014: a secret is never revealed
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_014_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds the secret `sk-test-123`
- **When** Alice updates it with a secret of 5000 characters beginning with `sk-` (through the tool, then through the REST interface), lists the providers through both, reads the list of changes and the screen
- **Then** the update is refused with `ERR_INVALID_ARGUMENT: secret must be at most 4096 characters` and the refusal text does not contain the secret; no list, change record or screen contains `sk-test-123`
- **And** the platform logs contain no secret
- **Priority:** High

### SPEC-0102/E2E-015: a secret together with clear_secret
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_015_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` exists
- **When** Alice updates it with `secret` `sk-x` and `clear_secret` true
- **Then** the answer is `ERR_INVALID_ARGUMENT: secret and clear_secret cannot be given together`
- **And** the provider is unchanged
- **Priority:** High

### SPEC-0102/E2E-017: an empty secret on a bearer provider
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_017_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider with scheme `bearer` and an empty secret
- **Then** the answer is `ERR_INVALID_ARGUMENT: auth_scheme 'bearer' needs a secret`
- **And** nothing was stored
- **Priority:** Medium

### SPEC-0102/E2E-020: editing a provider applies to its models
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_020_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` has one model
- **When** Alice changes the endpoint of `Local GPU` to `http://gpu2:9010/v1` and its name to `GPU 2`
- **Then** the answer shows the new values and a later `updated_at`
- **And** the model listing shows the provider unchanged by id and the model still belongs to it
- **Priority:** High

### SPEC-0102/E2E-021: editing an unknown provider
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_021_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice updates the provider `prov-unknown`
- **Then** the answer is `ERR_NOT_FOUND: provider not found: prov-unknown`
- **And** nothing changed
- **Priority:** High

### SPEC-0102/E2E-023: deleting an empty provider
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_023_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` has no model
- **When** Alice deletes it
- **Then** the answer is `{deleted: true}`
- **And** the provider is gone from the list, and its secret with it
- **Priority:** High

### SPEC-0102/E2E-024: deleting an unknown provider
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_024_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice deletes `prov-unknown`
- **Then** the answer is `ERR_NOT_FOUND: provider not found: prov-unknown`
- **And** nothing changed
- **Priority:** High

### SPEC-0102/E2E-050: paging the providers
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_050_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; 150 providers named `P001` to `P150`
- **When** Alice lists the providers, then asks for the next page with the marker
- **Then** the first answer holds `P001` to `P100` and a marker; the second holds `P101` to `P150` and no marker
- **And** the order is by name without regard to case
- **Priority:** Medium

### SPEC-0102/E2E-051: the order is by name without regard to case
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_051_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; providers `beta`, `Alpha`, `charlie`
- **When** Alice lists the providers
- **Then** the order is `Alpha`, `beta`, `charlie`
- **And** ties on the name cannot exist (names are unique without regard to case)
- **Priority:** Medium

### SPEC-0102/E2E-056: a secret change is recorded as a name
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_056_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` exists
- **When** Alice replaces its secret by `sk-new-456` and clears it later
- **Then** two `update_provider` records whose `fields` hold the name `secret`
- **And** no record holds `sk-new-456`
- **Priority:** Medium

### SPEC-0102/E2E-057: a refused operation leaves no record
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_057_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider with endpoint `ftp://x`, then a duplicate
- **Then** both are refused
- **And** the list of changes is empty
- **Priority:** High

### SPEC-0102/E2E-058: records survive a restart
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_058_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; three changes were made
- **When** the operator restarts the platform and Alice lists the changes
- **Then** the three records are listed
- **And** nothing was removed
- **Priority:** Medium

### SPEC-0102/E2E-059: paging the changes
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_059_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; 120 changes were made
- **When** Alice lists the changes and asks for the next page
- **Then** the first answer holds the 100 newest and a marker, the second the 20 oldest and no marker
- **And** ties on the time are ordered by `change_id` descending
- **Priority:** Medium

### SPEC-0102/E2E-062: administrator identity is compared without regard to case
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_062_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** `Admin@Example.COM` lists the providers
- **Then** the call succeeds
- **And** a person not listed is refused
- **Priority:** Medium

### SPEC-0102/E2E-063: a platform administrator is not a project member
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_063_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; project `proj` of which Alice is not a member
- **When** Alice lists the providers
- **Then** the call succeeds
- **And** reading `proj` files still follows project membership
- **Priority:** Medium

### SPEC-0102/E2E-090: invalid provider fields
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_090_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates providers with an empty name, a name of 65 characters, endpoint `localhost:9010`, scheme `basic`, scheme `none` with the secret `x`, scheme `bearer` with no secret
- **Then** the answers are, in order, `ERR_INVALID_ARGUMENT: name must be 1 to 64 characters` twice, `endpoint must be an http or https URL`, `auth_scheme must be 'none' or 'bearer'`, `auth_scheme 'none' takes no secret`, `auth_scheme 'bearer' needs a secret`
- **And** no provider was created
- **Priority:** High

### SPEC-0102/E2E-091: limits of a provider name
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_091_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates providers named with 1 and with 64 characters
- **Then** both succeed
- **And** a name of 65 characters is refused (E2E-090)
- **Priority:** Medium

### SPEC-0102/E2E-095: an update is checked as a whole
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_095_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` (bearer, secret) and `Local GPU` exist
- **When** Alice renames `Cloud Embeddings` to `local gpu`, then sets its scheme to `none` without clearing the secret
- **Then** the answers are the duplicate name text and `auth_scheme 'none' takes no secret`
- **And** the provider is unchanged
- **Priority:** High

### SPEC-0102/E2E-110: renaming a provider with other case of its own name
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_110_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice renames it to `LOCAL GPU`
- **Then** the rename succeeds (a provider does not clash with itself)
- **And** a rename to the name of another provider is refused
- **Priority:** Medium

### SPEC-0102/E2E-112: an endpoint with credentials
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_112_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider with endpoint `http://user:pw@host:9010/v1`
- **Then** the answer is `ERR_INVALID_ARGUMENT: endpoint must be an http or https URL`
- **And** the text `pw` appears in no answer, list, record, screen page or log
- **Priority:** High

### SPEC-0102/E2E-115: an update that changes nothing
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_115_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice updates it with its current name and endpoint
- **Then** the answer is the provider unchanged, `updated_at` included
- **And** the record of changes holds no `update_provider` record for it
- **Priority:** Medium

## Constraints
- No model, default, REST or screen code here.
- No secret in any `Display`, `Debug`, error, log line, answer or record.
- No new dependency.
- Every write and its record share one transaction.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/tools.md` rows for the five tools; `.agent_docs/config.md` a paragraph on the master key variable; counts in `README.md` and `AGENTS.md`.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no secret value in any output; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0283: providers, secrets, the master key and the record of changes` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0102` and the requirement ids.
