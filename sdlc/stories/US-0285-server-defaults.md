# US-0285: server defaults for text embedding and reranker

> Spec: SPEC-0102 (sdlc/specs/SPEC-0102-platform-settings-and-model-registry/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0284
> min_tier: 2
> Files touched: 6
> Closes spec: no

## Objective
Let an administrator choose one text embedding model and one reranker as the server defaults, clear them and read them, with the dimension rule against the engine, the lock on a default model, and the records. Search does not use them yet (US-0286, US-0287).

## Requirements covered
- SPEC-0102/FR-NEW-013: WHEN a platform administrator sets the server default for the kind `text_embedding` or `reranker` THE platform SHALL record that model as the default of its kind, replacing the previous default, and answer with the model.
- SPEC-0102/FR-NEW-014: WHEN a platform administrator clears a server default or reads the defaults THE platform SHALL remove the default of that kind, or answer with the current default of each kind.
- SPEC-0102/FR-NEW-010 (partial: the dimension of a default cannot change): WHEN a platform administrator updates a model THE platform SHALL change only the fields given among `name`, `dimension` and `max_input_tokens`, apply the change at once, and answer with the model.
- SPEC-0102/FR-NEW-011 (partial: a default cannot be deleted): WHEN a platform administrator deletes a model that is not a server default THE platform SHALL remove it and answer `{deleted: true}`.
- SPEC-0102/FR-NEW-018 (partial: default changes): The platform SHALL record every creation, update and deletion of a provider or a model and every default change or clear, with who did it, when, what it concerned and which fields changed, and SHALL NOT record any value of a secret.
- SPEC-0102/FR-NEW-020 (partial: default tools and the access matrix): The platform SHALL allow every registry operation, reading included, to platform administrators only.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0102_028 e2e_0102_029 e2e_0102_03 e2e_0102_054 e2e_0102_055 e2e_0102_06 e2e_0102_104 e2e_0102_105 e2e_0102_116 settings::`. The tests set the vector dimension in the fixture config (1024 and 1536), set, replace, clear and read defaults, try the wrong kind, an unknown model and a dimension mismatch, and read the records.

## Design excerpt
### Decisions
- DEC-002 (SPEC-0102): Providers, models, defaults and changes are four tables of the global admin store, primary keys on the caseless name (`name_key`) so equal names cannot coexist on either engine, ids found through secondary indexes. **Obliged by:** SPEC-0102/FR-NEW-002, FR-NEW-009, FR-NEW-018. **Rationale:** the admin store is global, on both engines, and already holds platform concepts; a primary key on the name makes concurrent duplicate creation safe without a unique index (the schema layer has none). **Alternatives considered:** the YAML file with reload (no audit, no admin tools), a new store, per volume tables. **Source:** existing code. **Evidence:** `storage/admin.rs:23-150`, `storage/rel/schema.rs` (`Index` is non unique).
- DEC-006 (SPEC-0102): One `settings::Registry` holds every operation and rule; tools, REST handlers and the screen call it and add nothing but access control and shaping. **Obliged by:** SPEC-0102/FR-NEW-020, FR-NEW-021, FR-NEW-022. **Rationale:** the repository rule that MCP and REST share one implementation. **Alternatives considered:** per surface logic. **Source:** existing code. **Evidence:** `AGENTS.md` conventions (one implementation).
- DEC-009 (SPEC-0102): A change record is inserted in the same transaction as the change it describes; refused or no op changes write nothing; `fields` lists the names whose values differ. **Obliged by:** SPEC-0102/FR-NEW-018. **Rationale:** the record can never disagree with the data. **Alternatives considered:** a record after commit. **Source:** design. **Evidence:** `storage/admin.rs:244-262` (transaction use).
- DEC-011 (SPEC-0102): Validation lives in `Registry` in the order of the spec (FR-NEW-002, 003, 009, 010, 013) on the state after the update; `clear_secret` and `secret` together are refused before any other update check. **Obliged by:** SPEC-0102/FR-NEW-002, FR-NEW-003, FR-NEW-009, FR-NEW-010. **Rationale:** exact texts and order are contract. **Alternatives considered:** validate in each surface. **Source:** spec. **Evidence:** n/a.
- DEC-012 (SPEC-0102): The vector dimension a default text embedding model must have is `config.search.embedding.dimensions` (the table width the engines were created with), read from the server configuration; a model's dimension cannot change while it is a default. **Obliged by:** SPEC-0102/FR-NEW-010, FR-NEW-013. **Rationale:** the only dimension the stored vectors have today. **Alternatives considered:** read the table width from the database (differs per engine), no check. **Source:** existing code. **Evidence:** `config.rs:783-803`, `vector_pg.rs:55,73`, `vector_sqlite.rs:48,83`.
### Data model
Table `default_model` as design section 4.
### Contracts
Tools `admin.set_default_model(kind, model_id)`, `admin.clear_default_model(kind)`, `admin.get_default_models()`; answers `Model` with `is_default` true, `{cleared: true}`, `{text_embedding: Model|null, reranker: Model|null}`; texts: `ERR_INVALID_ARGUMENT: kind must be 'text_embedding' or 'reranker'`, `model '<name>' is of kind '<model kind>', not '<kind>'`, `model not found`, `the default text embedding model must have dimension <n>` with `n` = `config.search.embedding.dimensions`; update of the dimension of a default (other than the same value) `ERR_INVALID_ARGUMENT: the dimension of a server default model cannot change`; delete of a default `ERR_INVALID_ARGUMENT: model '<name>' is a server default; clear the default first`; setting the model already default and clearing a kind with no default write no record; `Model.is_default` computed from the table.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/admin.rs` | modify: table `default_model` |
| `crates/core/src/settings/{store,registry}.rs` | modify |
| `crates/core/src/tools/models.rs`, `mcp/server.rs` | modify: three tools |
| `crates/core/src/settings/e2e.rs` | modify: these tests |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, schema fixtures, DDL fingerprints, count tests and docs | regenerate and fix (132 to 135; admin 26 to 29) |
### Existing patterns
US-0283 and US-0284 code; the registry reads `AppState.config.search.embedding.dimensions` through a value passed to `Registry::new` (`config.rs:803`).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0102/E2E-028: a server default cannot be deleted
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_028_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `qwen3-embedding-0.6b` (1024) is the default text embedding model
- **When** Alice deletes it
- **Then** the answer is `ERR_INVALID_ARGUMENT: model 'qwen3-embedding-0.6b' is a server default; clear the default first`
- **And** after clearing the default the deletion succeeds
- **Priority:** High

### SPEC-0102/E2E-029: the dimension of a default is frozen
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_029_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `qwen3-embedding-0.6b` (1024) is the default text embedding model
- **When** Alice changes its dimension to 768
- **Then** the answer is `ERR_INVALID_ARGUMENT: the dimension of a server default model cannot change`
- **And** its name can still change
- **Priority:** High

### SPEC-0102/E2E-030: choosing the default text embedding model
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_030_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; the model `qwen3-embedding-0.6b` (1024) exists
- **When** Alice sets it as the default for `text_embedding`, then reads the defaults
- **Then** the answer shows `is_default` true; the defaults read `text_embedding` that model and `reranker` null
- **And** listing the models shows `is_default` true for it only
- **Priority:** Critical

### SPEC-0102/E2E-031: replacing a default
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_031_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; two text embedding models of dimension 1024
- **When** Alice sets the first, then the second as the default
- **Then** the defaults read the second
- **And** the first shows `is_default` false
- **Priority:** Medium

### SPEC-0102/E2E-032: a default of the wrong kind
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_032_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `bge-reranker` (reranker) exists
- **When** Alice sets it as the default for `text_embedding`
- **Then** the answer is `ERR_INVALID_ARGUMENT: model 'bge-reranker' is of kind 'reranker', not 'text_embedding'`
- **And** no default is set
- **Priority:** High

### SPEC-0102/E2E-033: a kind that has no default
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_033_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; model `Llama` of kind `chat` exists
- **When** Alice sets it as the default for `chat`, then clears the default for `ocr`
- **Then** each answer is `ERR_INVALID_ARGUMENT: kind must be 'text_embedding' or 'reranker'`
- **And** nothing changed
- **Priority:** High

### SPEC-0102/E2E-034: a dimension that does not match the vector store
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_034_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1536; the model `qwen3-embedding-0.6b` has dimension 1024
- **When** Alice sets it as the default for `text_embedding`
- **Then** the answer is `ERR_INVALID_ARGUMENT: the default text embedding model must have dimension 1536`
- **And** no default is set
- **Priority:** High

### SPEC-0102/E2E-035: an unknown model as default
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_035_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice sets `mdl-unknown` as the default for `reranker`
- **Then** the answer is `ERR_NOT_FOUND: model not found: mdl-unknown`
- **And** nothing changed
- **Priority:** High

### SPEC-0102/E2E-036: clearing a default
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_036_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; a default reranker is set
- **When** Alice clears the default for `reranker`, then clears it again
- **Then** both answers are `{cleared: true}`
- **And** the defaults read `reranker` null and the record of changes holds one `clear_default`
- **Priority:** High

### SPEC-0102/E2E-054: an empty registry
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_054_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice lists providers, models, defaults and changes
- **Then** the answers are `{providers: []}`, `{models: []}`, `{text_embedding: null, reranker: null}` and `{changes: []}`
- **And** no error
- **Priority:** Medium

### SPEC-0102/E2E-055: the record of changes
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_055_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates the provider `Local GPU`, updates its endpoint, creates a model, sets it as default, then lists the changes
- **Then** four records, newest first: `set_default` (`target_kind` `default`, `target_id` the model id, `target_name` `text_embedding`, `fields` empty), `create_model` (`target_kind` `model`, the model id and name), `update_provider` (`target_kind` `provider`, `fields` [`endpoint`]), `create_provider`; each holds `admin@example.com`, a UTC time and a `change_id` starting with `chg-`, the four ids increasing with the order made
- **And** none contains a secret value
- **Priority:** Critical

### SPEC-0102/E2E-060: an ordinary member is refused everywhere
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_060_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; one provider and one model exist
- **When** Bob lists providers, creates a provider, updates it, deletes it, lists models, sets a default and lists the changes
- **Then** each answer is `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** nothing was created, changed or revealed
- **Priority:** High

### SPEC-0102/E2E-061: a project owner is not a platform administrator
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_061_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; Bob owns project `proj`
- **When** Bob reads the defaults
- **Then** the answer is `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** his ownership grants nothing here
- **Priority:** High

### SPEC-0102/E2E-104: a provider whose model is a default cannot be deleted
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_104_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; a default text embedding model exists
- **When** Alice deletes the provider of the default model
- **Then** the answer is `ERR_INVALID_ARGUMENT: provider 'Local GPU' still has 1 model(s)`
- **And** searches keep using the default
- **Priority:** High

### SPEC-0102/E2E-105: clearing the default of a kind that has none
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_105_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice clears the default for `text_embedding` on an empty registry
- **Then** the answer is `{cleared: true}`
- **And** the record of changes holds no `clear_default`
- **Priority:** Medium

### SPEC-0102/E2E-116: giving the same dimension to a default
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_116_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; `qwen3-embedding-0.6b` (1024) is the default text embedding model
- **When** Alice updates it with `dimension` 1024
- **Then** the answer is the model unchanged
- **And** no record is written
- **Priority:** Medium

## Constraints
- Search is not changed here.
- Only the kinds `text_embedding` and `reranker` have a default.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/tools.md` rows for the three tools; counts.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no secret value in any output; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0285: server defaults for text embedding and reranker` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0102` and the requirement ids.
