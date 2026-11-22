# US-0284: models and their tools

> Spec: SPEC-0102 (sdlc/specs/SPEC-0102-platform-settings-and-model-registry/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0283
> min_tier: 2
> Files touched: 7
> Closes spec: no

## Objective
Add models to the registry: the table, the rules of FR-NEW-009 in their order, and the tools `admin.create_model`, `update_model`, `delete_model` and `list_models`; deleting a provider that still has models is now refused. After this story an administrator can describe the models a provider offers.

## Requirements covered
- SPEC-0102/FR-NEW-006 (partial: the refusal when the provider still has models): WHEN a platform administrator deletes a provider that has no model THE platform SHALL remove the provider and its secret and answer `{deleted: true}`.
- SPEC-0102/FR-NEW-008: WHEN a platform administrator creates a model of a provider with a name, a kind, and for the embedding kinds a dimension, THE platform SHALL store it and answer with the model.
- SPEC-0102/FR-NEW-009: IF the provider, the name, the kind, the dimension or the limit of a model is not valid THEN THE platform SHALL refuse the operation, store nothing and answer with the text of the rule that failed.
- SPEC-0102/FR-NEW-010 (partial: name, dimension and limit updates; the default lock comes with US-0285): WHEN a platform administrator updates a model THE platform SHALL change only the fields given among `name`, `dimension` and `max_input_tokens`, apply the change at once, and answer with the model.
- SPEC-0102/FR-NEW-011 (partial: deletion of a model that is not a default; the default refusal comes with US-0285): WHEN a platform administrator deletes a model that is not a server default THE platform SHALL remove it and answer `{deleted: true}`.
- SPEC-0102/FR-NEW-012: WHEN a platform administrator lists models THE platform SHALL list them ordered by provider name then model name without regard to case, 100 per answer, optionally limited to one provider or one kind.
- SPEC-0102/FR-NEW-018 (partial: model changes): The platform SHALL record every creation, update and deletion of a provider or a model and every default change or clear, with who did it, when, what it concerned and which fields changed, and SHALL NOT record any value of a secret.
- SPEC-0102/FR-NEW-020 (partial: model tools): The platform SHALL allow every registry operation, reading included, to platform administrators only.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0102_003 e2e_0102_006 e2e_0102_022 e2e_0102_025 e2e_0102_026 e2e_0102_027 e2e_0102_052 e2e_0102_053 e2e_0102_09 e2e_0102_111 settings::`. The tests create providers and models, list by provider and kind, hit every refusal text in order and check the audit records of model changes.

## Design excerpt
### Decisions
- DEC-002 (SPEC-0102): Providers, models, defaults and changes are four tables of the global admin store, primary keys on the caseless name (`name_key`) so equal names cannot coexist on either engine, ids found through secondary indexes. **Obliged by:** SPEC-0102/FR-NEW-002, FR-NEW-009, FR-NEW-018. **Rationale:** the admin store is global, on both engines, and already holds platform concepts; a primary key on the name makes concurrent duplicate creation safe without a unique index (the schema layer has none). **Alternatives considered:** the YAML file with reload (no audit, no admin tools), a new store, per volume tables. **Source:** existing code. **Evidence:** `storage/admin.rs:23-150`, `storage/rel/schema.rs` (`Index` is non unique).
- DEC-004 (SPEC-0102): Ids are `prov-`, `mdl-` plus 12 hex of a uuid v4; `change_id` is `chg-` plus 12 zero padded digits taken as the largest existing plus one inside the writing transaction; times are `chrono` UTC RFC 3339 with seconds. **Obliged by:** SPEC-0102/FR-NEW-001, FR-NEW-008, FR-NEW-018. **Rationale:** the spec names the prefixes and a monotonic change order; a primary key conflict on a race is retried once. **Alternatives considered:** uuid change ids (no order), a sequence (not portable across the two engines). **Source:** design. **Evidence:** n/a.
- DEC-006 (SPEC-0102): One `settings::Registry` holds every operation and rule; tools, REST handlers and the screen call it and add nothing but access control and shaping. **Obliged by:** SPEC-0102/FR-NEW-020, FR-NEW-021, FR-NEW-022. **Rationale:** the repository rule that MCP and REST share one implementation. **Alternatives considered:** per surface logic. **Source:** existing code. **Evidence:** `AGENTS.md` conventions (one implementation).
- DEC-009 (SPEC-0102): A change record is inserted in the same transaction as the change it describes; refused or no op changes write nothing; `fields` lists the names whose values differ. **Obliged by:** SPEC-0102/FR-NEW-018. **Rationale:** the record can never disagree with the data. **Alternatives considered:** a record after commit. **Source:** design. **Evidence:** `storage/admin.rs:244-262` (transaction use).
- DEC-010 (SPEC-0102): Lists order by `name_key` (providers), by provider `name_key` then model `name_key` (models), by `change_id` descending (changes), page 100 with the marker of design 5.1; the listing queries are keyset queries. **Obliged by:** SPEC-0102/FR-NEW-007, FR-NEW-012, FR-NEW-019. **Rationale:** stable paging while data changes. **Alternatives considered:** offset paging. **Source:** existing code. **Evidence:** `tools/derived.rs` marker pattern (SPEC-0100).
- DEC-011 (SPEC-0102): Validation lives in `Registry` in the order of the spec (FR-NEW-002, 003, 009, 010, 013) on the state after the update; `clear_secret` and `secret` together are refused before any other update check. **Obliged by:** SPEC-0102/FR-NEW-002, FR-NEW-003, FR-NEW-009, FR-NEW-010. **Rationale:** exact texts and order are contract. **Alternatives considered:** validate in each surface. **Source:** spec. **Evidence:** n/a.
### Data model
Table `model` as design section 4 (primary key `(provider_id, name_key)`, indexes `idx_model_id`, `idx_model_kind`).
### Contracts
Tools `admin.create_model(provider_id, name, kind, dimension?, max_input_tokens?)`, `admin.update_model(model_id, name?, dimension?, max_input_tokens?)` (`max_input_tokens` 0 removes the limit), `admin.delete_model(model_id)`, `admin.list_models(provider_id = "", kind = "", marker = "")`; Model shape of design 5.1 (`is_default` false until US-0285); check order of FR-NEW-009: provider, name, kind, dimension required, dimension applies (before any range), dimension range, max_input_tokens range, duplicate name within the provider; texts verbatim from the spec; `ERR_INVALID_ARGUMENT: provider '<name>' still has <n> model(s)` on provider deletion.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/admin.rs` | modify: table `model` |
| `crates/core/src/settings/{model,store,registry}.rs` | modify |
| `crates/core/src/tools/models.rs`, `mcp/server.rs` | modify: four tools |
| `crates/core/src/settings/e2e.rs` | modify: these tests |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, schema fixtures, DDL fingerprints, count tests and docs | regenerate and fix (128 to 132; admin 22 to 26) |
### Existing patterns
The provider code of US-0283 (same store, registry, tool and test patterns); keyset listing and markers as in US-0283.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0102/E2E-003: registering models
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_003_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; the provider `Local GPU` exists
- **When** Alice creates the model `qwen3-embedding-0.6b` of kind `text_embedding` with dimension 1024 and `max_input_tokens` 8192, and the model `bge-reranker` of kind `reranker`, then lists the models
- **Then** the first answer holds a `model_id` starting with `mdl-`, `kind` `text_embedding`, `dimension` 1024, `max_input_tokens` 8192, `is_default` false; the second holds `dimension` null and `max_input_tokens` null; the list holds both ordered by name
- **And** each model names its provider
- **Priority:** Critical

### SPEC-0102/E2E-006: the same model name under two providers
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_006_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; providers `Local GPU` and `Cloud Embeddings` exist
- **When** Alice creates the model `qwen3-embedding-0.6b` (text_embedding, dimension 1024) under each, then a second one under `Local GPU`
- **Then** the two first creations succeed; the third is refused with `ERR_INVALID_ARGUMENT: a model named 'qwen3-embedding-0.6b' already exists for provider 'Local GPU'`
- **And** the list holds two models
- **Priority:** High

### SPEC-0102/E2E-022: deleting a provider with models
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_022_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` has 2 models
- **When** Alice deletes `Local GPU`
- **Then** the answer is `ERR_INVALID_ARGUMENT: provider 'Local GPU' still has 2 model(s)`
- **And** the provider and its models remain
- **Priority:** High

### SPEC-0102/E2E-025: updating a model
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_025_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; model `bge-reranker` (reranker) exists
- **When** Alice gives it `max_input_tokens` 512, then `0`
- **Then** the first answer holds 512; the second holds `null`
- **And** its kind and provider are unchanged
- **Priority:** High

### SPEC-0102/E2E-026: an unknown model
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_026_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice updates, then deletes the model `mdl-unknown`
- **Then** each answer is `ERR_NOT_FOUND: model not found: mdl-unknown`
- **And** nothing changed
- **Priority:** High

### SPEC-0102/E2E-027: deleting a model
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_027_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; model `bge-reranker` exists and is not a default
- **When** Alice deletes it
- **Then** the answer is `{deleted: true}`
- **And** the model list no longer holds it; deleting it again answers `ERR_NOT_FOUND: model not found: <id>`
- **Priority:** High

### SPEC-0102/E2E-052: an unusable marker
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_052_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice lists providers, models and changes with the marker `zzz`
- **Then** each answer is `ERR_INVALID_ARGUMENT: marker is not valid`
- **And** nothing changed
- **Priority:** High

### SPEC-0102/E2E-053: listing the models of one provider and one kind
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_053_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; two providers with three models of kinds `text_embedding`, `reranker`, `chat`
- **When** Alice lists the models of `Local GPU`, then those of kind `reranker`
- **Then** the first answer holds the models of that provider ordered by model name; the second only the rerankers, ordered by provider name then model name across both providers
- **And** an unknown provider id answers `ERR_NOT_FOUND: provider not found: <id>` and an unknown kind `ERR_INVALID_ARGUMENT: kind must be one of text_embedding, image_embedding, reranker, ocr, chat`
- **Priority:** High

### SPEC-0102/E2E-092: invalid model fields
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_092_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice creates models: an embedding model without dimension, a chat model with dimension 1024, an embedding model with dimension 0 and with 8193, a model with `max_input_tokens` 0 and with 1000001, an unknown kind `vision`, an unknown provider
- **Then** the answers are the texts of FR-NEW-009 in that order: `dimension is required for kind 'text_embedding'`, `dimension applies to embedding kinds only`, `dimension must be between 1 and 8192` twice, `max_input_tokens must be between 1 and 1000000` twice, `kind must be one of ...`, `provider not found`; an unknown kind `vision` under an unknown provider answers `provider not found`; a duplicate name answers `a model named ... already exists for provider ...`; a chat model with dimension 0 answers `dimension applies to embedding kinds only`
- **And** no model was created
- **Priority:** High

### SPEC-0102/E2E-093: limits of a model
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_093_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice creates embedding models with dimension 1 and 8192, a model named with 128 characters, and a model with `max_input_tokens` 1 and 1000000
- **Then** all succeed
- **And** a name of 129 characters is refused with `name must be 1 to 128 characters`
- **Priority:** Medium

### SPEC-0102/E2E-094: an image embedding model needs a dimension too
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_094_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice creates a model of kind `image_embedding` without dimension
- **Then** the answer is `ERR_INVALID_ARGUMENT: dimension is required for kind 'image_embedding'`
- **And** with dimension 1024 it succeeds
- **Priority:** High

### SPEC-0102/E2E-111: valid inputs at once
- Level / Driver / Location: functional e2e / admin handlers (`McpServer::admin_*`) on the registry fixture, stub services where noted / `crates/core/src/settings/e2e.rs` (`e2e_0102_111_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider and then a model with every field at its valid minimum and maximum
- **Then** both succeed with the values as given
- **And** the lists show them
- **Priority:** Medium

## Constraints
- No default handling here (US-0285); `is_default` is always false.
- Models never change provider or kind.
- No new dependency.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/tools.md` rows for the four tools; counts.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no secret value in any output; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0284: models and their tools` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0102` and the requirement ids.
