# US-0286: search embeds with the default text embedding model

> Spec: SPEC-0102 (sdlc/specs/SPEC-0102-platform-settings-and-model-registry/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0285
> min_tier: 1
> Files touched: 8
> Closes spec: no

## Objective
Make the vector engines resolve their embedding endpoint, model and authentication at call time: the server default of the registry when set, else the YAML block as before. Indexing and queries use it at once, with the provider secret as a bearer token, and a missing key or a down service fails the rebuild with the spec texts.

## Requirements covered
- SPEC-0102/FR-NEW-015: WHILE a server default text embedding model is set THE platform SHALL embed documents and queries with that model, through the endpoint, model name and authentication of its provider, instead of the search embedding settings of the configuration.
- SPEC-0102/FR-NEW-017 (partial: the embedding part): WHILE no server default is set for a kind THE platform SHALL behave for that kind exactly as it did before the registry existed, using the search embedding and reranking settings of the configuration.
- SPEC-0102/FR-NEW-005 (partial: a secret that cannot be read at call time): IF no master key is configured THEN THE platform SHALL refuse to store a secret with `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`, and SHALL accept providers that carry no secret.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0102_016 e2e_0102_037 e2e_0102_038 e2e_0102_039 e2e_0102_041 e2e_0102_042 e2e_0102_10 e2e_0102_106 e2e_0102_113 search:: e2e_0100_ e2e_0101_`. A stub service records the path, headers and body of every call; the tests assert `POST /embeddings` with `model`, `Authorization: Bearer ...` or none, the switch after an endpoint change and the fallback after a clear; the whole search and SPEC-0100 and SPEC-0101 suites stay green.

## Design excerpt
### Decisions
- DEC-005 (SPEC-0102): A `SecretText` newtype (no `Display`, `Debug` prints `Secret(set)`) carries secrets in memory; answer structs have `has_secret` only; refusal texts are built from names and numbers, never from request bodies; the registry tools log nothing of their arguments. **Obliged by:** SPEC-0102/FR-NEW-004. **Rationale:** no output path can format a secret by accident. **Alternatives considered:** discipline only. **Source:** design. **Evidence:** `config.rs:31,51` (`Dsn` redaction precedent).
- DEC-007 (SPEC-0102): Search resolves the defaults at call time through `EmbedTargets` and `RerankTargets` (one admin query per index or query call, no cache); the YAML block is the embedding fallback; the vector engines take `Arc<dyn EmbedTargets>` instead of an `EmbeddingConfig`; the URL is the provider endpoint plus `/embeddings` or `/rerank`. **Obliged by:** SPEC-0102/FR-NEW-015, FR-NEW-016, FR-NEW-017. **Rationale:** "applies at once" forbids a cache or a boot snapshot. **Alternatives considered:** a TTL cache, restart on change, an `Arc<RwLock<EmbeddingConfig>>` refreshed on change (needs cross process invalidation). **Source:** existing code. **Evidence:** `search/mod.rs:271-351`, `vector_pg.rs:32,99,203`, `vector_sqlite.rs:35,112,271`.
### Data model
No schema change.
### Contracts
Rust: `search::embedding::{EmbedTarget{url, model, bearer}, EmbedTargets (async trait, fn target()), StaticTargets(EmbeddingConfig)}`, `embed(client, &EmbedTarget, text)`; `settings::RegistryTargets{admin: Arc<dyn AdminBackend>, fallback: EmbeddingConfig}` implementing `EmbedTargets`; `build_backend(config, relational, http_client, targets: Arc<dyn EmbedTargets>)`; the vector engines keep `dimensions` from config (table width) and hold `Arc<dyn EmbedTargets>`; target = default model joined with its provider: `url = endpoint + "/embeddings"` (trailing slashes trimmed), `model = model name`, `bearer = decrypted secret` when scheme `bearer`; an unreadable secret `ERR_INTERNAL_ERROR: the secret of provider '<name>' cannot be read`; fallback = YAML (`url = embedding.endpoint`, bearer from `api_key_env` at call time). Used only by the engines of mode `rag` or `both`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/search/embedding.rs` | modify: `EmbedTarget`, `EmbedTargets`, `embed` |
| `crates/core/src/search/mod.rs`, `search/vector_pg.rs`, `search/vector_sqlite.rs` | modify: take the source; unit tests use `StaticTargets` |
| `crates/core/src/settings/{registry,store}.rs` | modify: `RegistryTargets`, default lookup with provider |
| `crates/core/src/app.rs` | modify: build `RegistryTargets` and pass it |
| `crates/core/src/settings/e2e.rs` | modify: stub embedding service helper and these tests |
### Existing patterns
Backend construction `search/mod.rs:271-351`; embed call sites `vector_pg.rs:99,203`, `vector_sqlite.rs:112,271`; guard on `embedding.endpoint` `search/mod.rs:286`, `tools/search_semantic.rs:130` (unchanged: a mode `rag` or `both` still needs the YAML endpoint at start); e2e stubs `search/derived_e2e.rs` (`Stub`).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0102/E2E-016: the master key is removed later
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_016_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds a secret; the operator restarts the platform without the master key, the default text embedding model belongs to that provider
- **When** Alice saves `/guide.md` and a search is made
- **Then** the rebuild job of the document fails with the reason `ERR_INTERNAL_ERROR: the secret of provider 'Cloud Embeddings' cannot be read` (read in the job list) and the search answers with the same refusal
- **And** listing providers still works and shows `has_secret` true
- **Priority:** Medium

### SPEC-0102/E2E-037: the default text embedding model drives indexing
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_037_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; project `proj` indexes with vectors; a stub embedding service at `http://localhost:9010/v1`; provider `Local GPU` (scheme `none`) with the default model `qwen3-embedding-0.6b` (1024)
- **When** Alice saves `/guide.md` and, after the settle delay, a search is made
- **Then** the stub service received the calls for the document and the query with the model name `qwen3-embedding-0.6b` and no authorization; the search finds `/guide.md`
- **And** the configured embedding settings were not called
- **Priority:** Critical

### SPEC-0102/E2E-038: a bearer provider sends its secret
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_038_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; a stub service requiring `Authorization: Bearer sk-test-123`; provider `Cloud Embeddings` holding that secret with the default model of dimension 1024
- **When** a document is indexed
- **Then** the stub received `Authorization: Bearer sk-test-123` and accepted the call
- **And** after Alice replaces the secret by `sk-new-456` the next call carries `sk-new-456`
- **Priority:** High

### SPEC-0102/E2E-039: a change applies at once
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_039_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; project indexing; the default text embedding model points to stub A
- **When** Alice changes the endpoint of its provider to stub B and a document is indexed right after
- **Then** stub B received the call and stub A none
- **And** no restart happened
- **Priority:** High

### SPEC-0102/E2E-041: the embedding service of the default is down
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_041_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; the default model's provider endpoint refuses connections
- **When** a document is saved and its rebuild runs
- **Then** the rebuild fails with `embedding service unreachable`; the write succeeded
- **And** the previous entries remain searchable
- **Priority:** High

### SPEC-0102/E2E-042: the dimension of vectors matches the default
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_042_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; a default text embedding model of dimension 1024 whose service answers vectors of length 1024
- **When** a document is indexed
- **Then** the entries are stored and a search finds them
- **And** setting a model of dimension 1023 as default answers `ERR_INVALID_ARGUMENT: the default text embedding model must have dimension 1024`
- **Priority:** Medium

### SPEC-0102/E2E-100: no default: the configuration is used
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_100_*`)
- **Given** a platform with search in mode `both` configured with a stub embedding service C, an empty registry
- **When** a document is indexed and searched
- **Then** stub C received the calls
- **And** the search results are those the same platform gave before the upgrade
- **Priority:** High

### SPEC-0102/E2E-101: clearing the default returns to the configuration
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_101_*`)
- **Given** as E2E-100 with a default text embedding model on stub A set
- **When** Alice clears the default and a document is indexed
- **Then** stub C received the call and stub A none
- **And** the previous entries remain searchable and no reranker is called
- **Priority:** Medium

### SPEC-0102/E2E-103: the upgrade creates the registry empty
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_103_*`)
- **Given** a platform started before the registry existed, then started with the new version
- **When** Alice lists providers and models
- **Then** both lists are empty and search works as before
- **And** no setting was imported from the configuration
- **Priority:** Medium

### SPEC-0102/E2E-106: a changed model name is used by the next call
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_106_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; the default model is `qwen3-embedding-0.6b` on a stub
- **When** Alice renames the model to `qwen3-embedding-0.6b-v2` and a document is indexed
- **Then** the stub received the model name `qwen3-embedding-0.6b-v2`
- **And** the dimension was not touched
- **Priority:** Medium

### SPEC-0102/E2E-113: a default embedding model with search mode bm25
- Level / Driver / Location: functional e2e / admin handlers, stub embedding service / `crates/core/src/settings/e2e.rs` (`e2e_0102_113_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; project search mode `bm25`; a default text embedding model is set
- **When** a document is indexed and searched
- **Then** the search finds the document and the stub embedding service received no call
- **And** switching the search mode to `both` makes the next index call use the default
- **Priority:** Medium

## Constraints
- One admin query per index or query call; no cache.
- The YAML dimension still sets the table width.
- Do not touch reranking (US-0287).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/search.md` (embedding source) and `.agent_docs/config.md`.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no secret value in any output; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0286: search embeds with the default text embedding model` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0102` and the requirement ids.
