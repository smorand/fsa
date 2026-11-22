# SPEC-0102: platform settings in the database, the model registry, design

> Spec: sdlc/specs/SPEC-0102-platform-settings-and-model-registry/spec.md
> Nature: FEAT
> Depth: L
> Security: internal finding
> Designed on: 2026-11-21
> Target tier: 2 (default, no `Tier:` in `sdlc/constitution.md`, no `.spec.json`)

## 1. Stack and skills
| Element | Choice | Version | DEC |
|---|---|---|---|
| Language, runtime, web, MCP | Rust 2024, tokio, axum, rmcp | workspace manifest (rmcp 3.5.0) | existing |
| Relational layer | `storage::rel` on the admin store (SQLite or PostgreSQL) | in tree | DEC-002 |
| Secret cipher | AES 256 GCM of `git/oauth/cipher.rs` (`aes-gcm`, `base64`) | in tree | DEC-003 |
| HTTP client | `reqwest` already used by search | in tree | DEC-007 |
| Ids, times, JSON | `uuid`, `chrono`, `serde_json` | workspace manifest | DEC-004 |

No dependency is added, so no library lookup was needed. Skill loaded: `rust` (already in context). Effect: `make check` is the gate (format, clippy `-D warnings --all-features`, tests), tests inside `crates/core/src/**`, comments explain WHY, no dashes as punctuation, `ToolError` for errors, secrets never logged.

## 2. Approaches
### Chosen: a `settings` module over four tables of the admin store, one service used by tools, REST and the screen, resolved at call time by search
A `settings::SettingsStore` owns the SQL (providers, models, defaults, record of changes) on the admin database. `settings::Registry` is the one implementation of every operation (validation, secrets, audit in the same transaction). The 12 admin tools, the REST handlers and the screen are thin adapters over it. Search reads the defaults per call through an `EmbedTargets` and a `RerankTargets` source, with the YAML blocks as fallback. Pros: one implementation for three surfaces (the repository rule), atomic change plus audit, the registry change applies to the next call. Cons: one admin query per index or query call. Scope: `settings/`, `storage/admin.rs`, `search/{mod,embedding,rerank,vector_*}.rs`, `tools/`, `api/`, a new screen, `app.rs`.
### Rejected: keep providers and models in the YAML file with a reload
Fails the owner's goal (no file editing) and FR-NEW-018 (who changed what). Rejected (DEC-002).
### Rejected: a new database or a settings service
The admin store already exists on both engines and holds global concepts (`storage/admin.rs:23-95`). Rejected (DEC-002).
### Rejected: cache the resolved defaults with a time to live
A TTL breaks "applies at once" (FR-NEW-015). Rejected (DEC-007).
### Rejected: a new cipher or per secret keys
The AES 256 GCM cipher and its single master key exist and the owner asked to reuse them. Rejected (DEC-003).

## 3. Architecture
```
 admin.* tools (mcp/server.rs)   REST /api/admin/... (api/admin_settings.rs)   screen /app/models (models_screen.rs)
                  \                         |                          /
                   +------------ settings::Registry (service: validate, encrypt, audit) ------------+
                                              |                                                    |
                              settings::SettingsStore (SQL on admin DB)               settings::secrets (master key, cipher)
                                              |
        search::embedding::EmbedTargets / search::rerank::RerankTargets (call time: default from registry, else YAML)
```
Bounded contexts: Platform settings (`settings/`, tools, REST, screen) and Search (`search/*`). Existing structure cited: admin tools `mcp/server.rs:3216` (pattern) and `tools/admin.rs:679`, admin schema `storage/admin.rs:23`, provided store method `storage/traits.rs:318`, cipher `git/oauth/cipher.rs:29-75`, REST admin routes `api/dataplane.rs:177-179` and error envelope `:259`, token screen `token_screen.rs:49-308`, embedding call `search/embedding.rs:20`, rerank `search/rerank.rs:21`, search query `tools/search_semantic.rs:119`.

### 3.1 Call time resolution
`EmbedTargets::target(&self) -> Result<EmbedTarget>` where `EmbedTarget { url, model, bearer: Option<String> }`. The registry implementation asks the store for the default `text_embedding` model joined with its provider; when present it returns `url = endpoint + "/embeddings"` (trailing slashes of the endpoint ignored), the model name and the decrypted secret (scheme `bearer`); when absent it returns the YAML block (`url = embedding.endpoint`, `bearer` from `api_key_env` at call time as today). The vector engines hold an `Arc<dyn EmbedTargets>` instead of an `EmbeddingConfig` and call it once per `index_chunks_checked` and once per `query_vector`. `RerankTargets` is the same for the reranker with `url = endpoint + "/rerank"` and no YAML fallback (nothing calls a configured reranker today, DEC-008).
### 3.2 Reranking
`search_semantic::query` gains `rerank: bool`; when mode is `both`, `rerank` is true and a default reranker exists, the fused results (up to `top_k`) go to `rerank::rerank` and the answer is cut to the configured `top_n`.

## 4. Data model
Four tables in the admin store schema (`storage/admin.rs::schema()`, global, no `volume_id`; the schema golden fixtures and DDL fingerprints regenerate):

`provider`: `name_key` Text (trimmed lower case name), `provider_id` Text (`prov-` plus 12 hex of a uuid), `name` Text, `endpoint` Text, `auth_scheme` Text, `secret_enc` Blob null (AES 256 GCM: 12 byte nonce, ciphertext, 16 byte tag), `created_at` Text, `updated_at` Text. Primary key `(name_key)`; index `idx_provider_id (provider_id)`.
`model`: `provider_id` Text, `name_key` Text, `model_id` Text (`mdl-` plus 12 hex), `name` Text, `kind` Text, `dimension` BigInt null, `max_input_tokens` BigInt null, `created_at`, `updated_at`. Primary key `(provider_id, name_key)`; indexes `idx_model_id (model_id)`, `idx_model_kind (kind)`.
`default_model`: `kind` Text primary key (`text_embedding` or `reranker`), `model_id` Text, `set_at` Text.
`setting_change`: `change_id` Text primary key (`chg-` plus 12 digits, zero padded, from the largest existing plus one inside the writing transaction), `at` Text, `person` Text, `action` Text, `target_kind` Text, `target_id` Text, `target_name` Text, `fields` Text (JSON list of names).
No foreign key (the schema layer only cascades); the registry checks references inside the same transaction. Migration: new tables, `CREATE TABLE IF NOT EXISTS`, additive; rollback is dropping them. The master key is not stored.

## 5. Contracts and interfaces
### 5.1 Tools (12, all `admin.*`, platform administrator only through `AppState::require_admin`; 135 tools, 29 admin)
`admin.create_provider(name, endpoint, auth_scheme = "none", secret = "")`, `admin.update_provider(provider_id, name?, endpoint?, auth_scheme?, secret?, clear_secret?)`, `admin.delete_provider(provider_id)`, `admin.list_providers(marker = "")`, `admin.create_model(provider_id, name, kind, dimension?, max_input_tokens?)`, `admin.update_model(model_id, name?, dimension?, max_input_tokens?)`, `admin.delete_model(model_id)`, `admin.list_models(provider_id = "", kind = "", marker = "")`, `admin.set_default_model(kind, model_id)`, `admin.clear_default_model(kind)`, `admin.get_default_models()`, `admin.list_setting_changes(marker = "")`. Shapes: Provider `{provider_id, name, endpoint, auth_scheme, has_secret, created_at, updated_at}`; Model `{model_id, provider_id, name, kind, dimension, max_input_tokens, is_default, created_at, updated_at}`; Change `{change_id, at, person, action, target_kind, target_id, target_name, fields}`. Lists `{providers|models|changes: [...], marker?}`, 100 per page; marker is base64 url safe of JSON `[<sort key>, <id>]` and any decode failure is `ERR_INVALID_ARGUMENT: marker is not valid`.
### 5.2 Order of checks and exact texts
Exactly those of the spec (FR-NEW-002, 003, 009, 010, 011, 013), implemented in `Registry` in that order; `person` in a change is the lower case caller identity; the master key refusal is `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`; an unreadable secret at call time is `ERR_INTERNAL_ERROR: the secret of provider '<name>' cannot be read`.
### 5.3 REST (`api/admin_settings.rs`, merged with the REST router, so disabled with `api.enabled: false`)
`POST|GET /api/admin/providers`, `PATCH|DELETE /api/admin/providers/{provider_id}`, `POST|GET /api/admin/models`, `PATCH|DELETE /api/admin/models/{model_id}`, `GET /api/admin/defaults`, `PUT|DELETE /api/admin/defaults/{kind}`, `GET /api/admin/setting-changes`; identity through `person_of`; JSON bodies equal the tool inputs; `error_response` envelope. 12 rows in `REST_ROUTES` (49 to 61) and 12 `Op` rows in the OpenAPI table.
### 5.4 Screen
`models_screen.rs`, `GET /app/models` and `POST /app/models/{provider/create, provider/update, provider/delete, model/create, model/update, model/delete, default/set, default/clear}`; identity, one time token and refusal as `token_screen.rs`; page built as a string with escaped values; secret inputs `type="password"` and never prefilled.
### 5.5 Internal signatures (Rust)
```rust
// settings/mod.rs
pub struct Registry { store: SettingsStore, key: Option<[u8; 32]>, vector_dimension: u32 }
impl Registry { pub async fn create_provider(&self, person: &str, i: ProviderInput) -> Result<Provider>; /* update, delete, list, models, defaults, changes */ }
// storage/traits.rs, AdminBackend (provided methods)
fn settings(&self) -> Option<SettingsStore> { None }
fn set_master_key(&self, key: Option<[u8; 32]>) {}
// search/embedding.rs
pub struct EmbedTarget { pub url: String, pub model: String, pub bearer: Option<String> }
#[async_trait] pub trait EmbedTargets: Send + Sync { async fn target(&self) -> Result<EmbedTarget>; }
pub async fn embed(client: &Client, target: &EmbedTarget, text: &str) -> Result<Vec<f32>>;
```

## 6. File structure impact
| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/core/src/settings/{mod,model,store,registry,secrets}.rs` | create | FR-NEW-001..014, 018, 019 | US-0283..US-0285 |
| `crates/core/src/storage/admin.rs`, `storage/traits.rs` | modify: four tables, `settings()`, `set_master_key()` | FR-NEW-001, 008, 013, 018 | US-0283..US-0285 |
| `crates/core/src/app.rs` | modify: master key check and set, embed and rerank sources, screen and REST mount | FR-NEW-005, 015, 016, 021, 022 | US-0283, US-0286, US-0288, US-0289 |
| `crates/core/src/tools/models.rs`, `tools/mod.rs`, `mcp/server.rs` | create / modify: 12 tools | FR-NEW-001..020 | US-0283..US-0285 |
| `crates/core/src/search/{embedding,rerank,mod,vector_pg,vector_sqlite}.rs` | modify: targets | FR-NEW-015, 016, 017 | US-0286, US-0287 |
| `crates/core/src/tools/search_semantic.rs` | modify: `rerank` wired | FR-NEW-016 | US-0287 |
| `crates/core/src/api/admin_settings.rs`, `api/mod.rs`, `api/dataplane.rs`, `api/openapi.rs` | create / modify | FR-NEW-021 | US-0288 |
| `crates/core/src/models_screen.rs`, `token_screen.rs` (items `pub(crate)`), `lib.rs` | create / modify | FR-NEW-022, 023 | US-0289 |
| `crates/core/src/settings/e2e.rs` | create (test module, all spec E2E) | all | US-0283..US-0289 |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, `crates/core/tests/fixtures/schema_*.sql`, DDL fingerprints, tool count tests | regenerate / modify | FR-NEW-001..019 | US-0283..US-0285 |
| `README.md`, `AGENTS.md`, `.agent_docs/{tools,search,architecture,config,testing}.md` | modify | all | US-0289 |
Affected existing tests: `search/e2e.rs` and engine tests building `EmbeddingConfig` (keep, built through a static source), tool count tests (123 to 135), `dataplane.rs:2776` route count, the OpenAPI table sync test.

## 7. Compatibility and risks
- No existing behavior changes while the registry is empty (FR-NEW-017). The search engines receive a source instead of a config: same calls.
- Risk 1, secrets: a secret must never reach a log, an error, an answer or a record; `SecretText` has no `Display`, a redacted `Debug`, and the service never formats it. Exploit style tests E2E-014, E2E-056, E2E-080 and E2E-112 look for the literal secret in every output.
- Risk 2, master key: a malformed key stops the start (new, DEC-003); removing the key later makes secret reads fail with a clear text.
- Risk 3, one admin query per embedding or query call: fine at the current rate; a cache is rejected (DEC-007).
- Risk 4, the vector dimension is the YAML `embedding.dimensions` (table width), so a default text embedding model with another dimension is refused (FR-NEW-013); lifting that is BL-0171.
- No new dependency. Rollback: revert the branch; the tables are additive.

## 8. Security design
Threat surface: a platform administrator enters credentials (trust boundary: authenticated admin only, FR-NEW-020) that must come back out nowhere. Sinks: answers, listings, change records, screen HTML, error texts, `tracing` lines, test output.
- Mechanism: encrypt on write with the master key (DEC-003); store only `secret_enc`; read only inside `Registry::target_for` when an outbound call needs the bearer; every outbound type and answer struct has no secret field, only `has_secret`; refusals build their text from names and numbers, never from the request body (DEC-005).
- Exploit style tests (literal values, other channel): E2E-014 (secret `sk-test-123` and a 5000 character `sk-` secret searched in the list, the record, the screen and the logs), E2E-056 (`sk-new-456` in the record), E2E-080 (the page), E2E-112 (`pw` of `http://user:pw@host`). They are red before the code only in the sense that the feature does not exist, so each story that creates an output surface adds its leak assertion in its own tests.
- Least privilege: every operation needs `require_admin`; reading is admin only.
- Never logged: secrets, endpoint credentials (refused at input), request bodies of the registry tools.

## 9. Test strategy
Every spec test is a functional e2e driven through the real `McpServer` admin handlers, the REST router (`tower::ServiceExt::oneshot`) and the screen router, on a fixture with an in memory admin store, a master key set in the fixture, stub embedding and rerank services (axum on a random port, recording method, path, headers and body), a paused clock where timing matters. Reuse `search/derived_e2e.rs` helpers (`Env`, `Stub`) and add a registry fixture in `settings/e2e.rs`. All tests named `e2e_0102_<id>_<slug>`; run with `cargo test -p fsa-core --lib -- e2e_0102_`. Unit tests of validation and of the store live in `settings/*.rs`. The table below is the map.

| Test | Level | Driver | Location | Story |
|---|---|---|---|---|
| E2E-001 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-002 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-004 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-005 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-010 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-011 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-012 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-013 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-014 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-015 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-017 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-020 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-021 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-023 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-024 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-050 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-051 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-056 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-057 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-058 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-059 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-062 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-063 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-090 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-091 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-095 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-110 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-112 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-115 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0283 |
| E2E-003 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-006 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-022 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-025 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-026 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-027 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-052 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-053 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-092 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-093 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-094 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-111 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0284 |
| E2E-028 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-029 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-030 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-031 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-032 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-033 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-034 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-035 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-036 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-054 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-055 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-060 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-061 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-104 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-105 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-116 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0285 |
| E2E-016 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-037 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-038 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-039 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-041 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-042 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-100 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-101 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-103 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-106 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-113 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0286 |
| E2E-040 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0287 |
| E2E-102 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0287 |
| E2E-107 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0287 |
| E2E-108 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0287 |
| E2E-114 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0287 |
| E2E-070 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0288 |
| E2E-071 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0288 |
| E2E-072 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0288 |
| E2E-073 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0288 |
| E2E-074 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0288 |
| E2E-080 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0289 |
| E2E-081 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0289 |
| E2E-082 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0289 |
| E2E-083 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0289 |
| E2E-084 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0289 |
| E2E-085 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0289 |
| E2E-086 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0289 |
| E2E-087 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0289 |
| E2E-109 | functional e2e | admin handlers, REST router or screen router, stub services | `settings/e2e.rs` | US-0289 |

Test data: generated (stub services, in memory stores); nothing user provided.

## 10. Decisions log
- **DEC-001:** No new dependency; the stack is the repository's. **Obliged by:** n/a (no code impact): the spec names no technology. **Rationale:** cipher, relational layer, HTTP client and ids are in the manifest. **Alternatives considered:** a secrets crate, a vault client (new dependency, not asked). **Source:** existing code. **Evidence:** `git/oauth/cipher.rs:20-75`, `Cargo.toml`.
- **DEC-002:** Providers, models, defaults and changes are four tables of the global admin store, primary keys on the caseless name (`name_key`) so equal names cannot coexist on either engine, ids found through secondary indexes. **Obliged by:** SPEC-0102/FR-NEW-002, FR-NEW-009, FR-NEW-018. **Rationale:** the admin store is global, on both engines, and already holds platform concepts; a primary key on the name makes concurrent duplicate creation safe without a unique index (the schema layer has none). **Alternatives considered:** the YAML file with reload (no audit, no admin tools), a new store, per volume tables. **Source:** existing code. **Evidence:** `storage/admin.rs:23-150`, `storage/rel/schema.rs` (`Index` is non unique).
- **DEC-003:** Secrets are encrypted with the existing AES 256 GCM cipher and the master key read from `FSA_TOKEN_KEY` (legacy name through `env_compat`); `app.rs` checks the key at boot after the admin store is connected and stops the start with the cipher's message (the text of its `message`, which begins `FSA_TOKEN_KEY must be valid base64.` or `FSA_TOKEN_KEY must decode to 32 bytes`) and a non zero exit through `cli::run`; a valid key is handed to the admin store with `set_master_key`. **Obliged by:** SPEC-0102/FR-NEW-004, FR-NEW-005. **Rationale:** the owner asked to reuse the key; the git store builds its cipher lazily, so a boot check is new. **Alternatives considered:** a key file, a second variable, lazy failure only. **Source:** existing code and user answer 3. **Evidence:** `git/oauth/cipher.rs:29-46`, `git/oauth/store.rs:26-33,116`, `tools/git_auth.rs:43-49`, `cli.rs:136`.
- **DEC-004:** Ids are `prov-`, `mdl-` plus 12 hex of a uuid v4; `change_id` is `chg-` plus 12 zero padded digits taken as the largest existing plus one inside the writing transaction; times are `chrono` UTC RFC 3339 with seconds. **Obliged by:** SPEC-0102/FR-NEW-001, FR-NEW-008, FR-NEW-018. **Rationale:** the spec names the prefixes and a monotonic change order; a primary key conflict on a race is retried once. **Alternatives considered:** uuid change ids (no order), a sequence (not portable across the two engines). **Source:** design. **Evidence:** n/a.
- **DEC-005:** A `SecretText` newtype (no `Display`, `Debug` prints `Secret(set)`) carries secrets in memory; answer structs have `has_secret` only; refusal texts are built from names and numbers, never from request bodies; the registry tools log nothing of their arguments. **Obliged by:** SPEC-0102/FR-NEW-004. **Rationale:** no output path can format a secret by accident. **Alternatives considered:** discipline only. **Source:** design. **Evidence:** `config.rs:31,51` (`Dsn` redaction precedent).
- **DEC-006:** One `settings::Registry` holds every operation and rule; tools, REST handlers and the screen call it and add nothing but access control and shaping. **Obliged by:** SPEC-0102/FR-NEW-020, FR-NEW-021, FR-NEW-022. **Rationale:** the repository rule that MCP and REST share one implementation. **Alternatives considered:** per surface logic. **Source:** existing code. **Evidence:** `AGENTS.md` conventions (one implementation).
- **DEC-007:** Search resolves the defaults at call time through `EmbedTargets` and `RerankTargets` (one admin query per index or query call, no cache); the YAML block is the embedding fallback; the vector engines take `Arc<dyn EmbedTargets>` instead of an `EmbeddingConfig`; the URL is the provider endpoint plus `/embeddings` or `/rerank`. **Obliged by:** SPEC-0102/FR-NEW-015, FR-NEW-016, FR-NEW-017. **Rationale:** "applies at once" forbids a cache or a boot snapshot. **Alternatives considered:** a TTL cache, restart on change, an `Arc<RwLock<EmbeddingConfig>>` refreshed on change (needs cross process invalidation). **Source:** existing code. **Evidence:** `search/mod.rs:271-351`, `vector_pg.rs:32,99,203`, `vector_sqlite.rs:35,112,271`.
- **DEC-008:** `search_semantic::query` gains `rerank`; reranking runs only for mode `both`, `rerank` true and a default reranker, with no YAML fallback because nothing calls a configured reranker today; the documents are the fused results up to `top_k` and the answer is cut to the configured `top_n`. **Obliged by:** SPEC-0102/FR-NEW-016, FR-NEW-017. **Rationale:** the spec states these rules and says no reranker is called without a default. **Alternatives considered:** wiring the configured block too (proposal, not asked). **Source:** spec. **Evidence:** `tools/search_semantic.rs:119,267`, `mcp/server.rs:3318`, `search/rerank.rs:21-47`.
- **DEC-009:** A change record is inserted in the same transaction as the change it describes; refused or no op changes write nothing; `fields` lists the names whose values differ. **Obliged by:** SPEC-0102/FR-NEW-018. **Rationale:** the record can never disagree with the data. **Alternatives considered:** a record after commit. **Source:** design. **Evidence:** `storage/admin.rs:244-262` (transaction use).
- **DEC-010:** Lists order by `name_key` (providers), by provider `name_key` then model `name_key` (models), by `change_id` descending (changes), page 100 with the marker of design 5.1; the listing queries are keyset queries. **Obliged by:** SPEC-0102/FR-NEW-007, FR-NEW-012, FR-NEW-019. **Rationale:** stable paging while data changes. **Alternatives considered:** offset paging. **Source:** existing code. **Evidence:** `tools/derived.rs` marker pattern (SPEC-0100).
- **DEC-011:** Validation lives in `Registry` in the order of the spec (FR-NEW-002, 003, 009, 010, 013) on the state after the update; `clear_secret` and `secret` together are refused before any other update check. **Obliged by:** SPEC-0102/FR-NEW-002, FR-NEW-003, FR-NEW-009, FR-NEW-010. **Rationale:** exact texts and order are contract. **Alternatives considered:** validate in each surface. **Source:** spec. **Evidence:** n/a.
- **DEC-012:** The vector dimension a default text embedding model must have is `config.search.embedding.dimensions` (the table width the engines were created with), read from the server configuration; a model's dimension cannot change while it is a default. **Obliged by:** SPEC-0102/FR-NEW-010, FR-NEW-013. **Rationale:** the only dimension the stored vectors have today. **Alternatives considered:** read the table width from the database (differs per engine), no check. **Source:** existing code. **Evidence:** `config.rs:783-803`, `vector_pg.rs:55,73`, `vector_sqlite.rs:48,83`.
- **DEC-013:** REST routes live in `api/admin_settings.rs`, mounted with the REST router, 12 rows added to `REST_ROUTES` and to the OpenAPI operation table; handlers authenticate with `person_of` and answer with `error_response`. **Obliged by:** SPEC-0102/FR-NEW-021. **Rationale:** same envelope and gating as the three existing admin routes. **Alternatives considered:** a separate prefix, no OpenAPI rows (the sync test forbids). **Source:** existing code. **Evidence:** `api/dataplane.rs:45,177-179,207,259`, `api/openapi.rs:440,1696-1710`.
- **DEC-014:** The models screen reuses the token screen's `CsrfStore`, `IdentitySource`, `resolve_person`, `check_csrf`, `forbidden_csrf` and `escape_html` by making them `pub(crate)`; each router owns its own store instance. **Obliged by:** SPEC-0102/FR-NEW-022, FR-NEW-023. **Rationale:** same behavior and refusal text without a third copy. **Alternatives considered:** copy as `deleted_projects_screen.rs` did (duplicates security code), move to a shared module (touches three screens). **Source:** existing code. **Evidence:** `token_screen.rs:60-232,308,344`.
- **DEC-015:** The admin store exposes `settings()` and `set_master_key()` as provided methods (default none and no op), like `derived_jobs()`. **Obliged by:** SPEC-0102/FR-NEW-001, FR-NEW-005. **Rationale:** no `AppState` field, so the ~25 hand built states in tests stay untouched. **Alternatives considered:** an `AppState.registry` field. **Source:** existing code. **Evidence:** `storage/traits.rs:318`, `storage/admin.rs:270`, `state.rs:16`.

## 11. Stories
| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0283 | providers, secrets, the master key and the record of changes | P1 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-NEW-004, FR-NEW-005, FR-NEW-006, FR-NEW-007, FR-NEW-018, FR-NEW-019, FR-NEW-020 | main | 1 | no |
| US-0284 | models and their tools | P1 | FR-NEW-006, FR-NEW-008, FR-NEW-009, FR-NEW-010, FR-NEW-011, FR-NEW-012, FR-NEW-018, FR-NEW-020 | US-0283 | 2 | no |
| US-0285 | server defaults for text embedding and reranker | P1 | FR-NEW-013, FR-NEW-014, FR-NEW-010, FR-NEW-011, FR-NEW-018, FR-NEW-020 | US-0284 | 2 | no |
| US-0286 | search embeds with the default text embedding model | P1 | FR-NEW-015, FR-NEW-017, FR-NEW-005 | US-0285 | 1 | no |
| US-0287 | search reranks with the default reranker | P2 | FR-NEW-016, FR-NEW-017 | US-0286 | 2 | no |
| US-0288 | the REST interface of the registry | P2 | FR-NEW-021, FR-NEW-020 | US-0287 | 2 | no |
| US-0289 | the models screen, docs and contract | P2 | FR-NEW-022, FR-NEW-023, FR-NEW-020 | US-0288 | 1 | yes |
