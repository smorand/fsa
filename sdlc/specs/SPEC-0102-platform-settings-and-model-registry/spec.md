# Platform settings in the database: the model registry, functional specification

> Id: SPEC-0102
> Nature: FEAT
> Depth: L
> Depth evidence: FEAT / depth L: 4 areas (settings store, admin tools, REST interface, browser screen) plus search, two new persisted concepts (providers and models) and a record of changes, 23 requirements
> Status: implemented
> Generated on: 2026-11-21
> Project: existing
> From backlog: BL-0164
> Depends on: none
> Security: internal finding
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
Today a platform administrator can change nothing about the models the platform uses without editing a configuration file, setting environment variables and restarting. This increment gives platform administrators a registry of providers (an endpoint, an authentication scheme and an encrypted secret that is never shown again) and of the models offered by them (text embedding, image embedding, reranker, OCR, chat). The registry is managed through admin tools, the REST interface and one browser screen, every change is recorded, and it takes effect at once. Administrators can also choose one text embedding model and one reranker as the server defaults; search then uses them, and keeps using the configuration when none is chosen. Later work (per project choices, converters, the other settings) builds on the registry's model ids.

## 2. Current State (MANDATORY, every depth)
The search does not call a reranker at all today. The embedding and reranking models are two server wide blocks of the configuration file, each naming an endpoint, a model and an environment variable that holds the key. The vector dimension is part of that block and is frozen once vectors exist. The platform has 17 admin tools, all about projects, members, quota, purge and index mode; none manages a server setting, and nothing changes without a file edit and a restart. Platform administrators are the people listed in the configuration. Admin actions leave no durable trace: the only audit buffer is per session and capped. Only three read only admin routes exist in the REST interface (quota and disk usage of a project); three browser screens exist (tokens, deleted projects, trash) and protect their forms with a one time token when the caller is identified by a browser cookie. Secrets stored by the platform today are the git access tokens, encrypted with a key taken from one environment variable.

### 2.1 What users can do today
- Change the embedding or reranking model only by editing the file and restarting (14.1 rows 1 to 3).
- Manage projects through admin tools, REST routes of the file plane and three screens, none of which touches settings (14.1 rows 4, 7 and 9).
- Store git tokens encrypted when the key variable is set (14.1 row 8).

### 2.2 Specs governing this area
- SPEC-0008 (search): the embedding and reranking behavior and their configuration; this increment adds a second source for them and keeps the first as fallback.
- SPEC-0011 (token store and the `/app/tokens` screen): the one time token convention, the cipher and its key variable are reused; that spec is not edited.
- SPEC-0010 and SPEC-0014 (admin tools): the platform administrator rule and the tool conventions.

### 2.3 What is verified today
Tests exercise the admin tools, the screens and the embedding and reranking calls against stub services. No test exercises a registry, an encrypted setting of this kind, a server default or a record of setting changes (14.1 row 10).

## 3. Scope
### 3.1 In scope
- Providers and models, their rules, tools, REST operations and one screen.
- Encrypted write only secrets with the existing master key variable.
- Two server defaults (text embedding, reranker) used by search at once; the configuration stays as fallback.
- A durable record of changes with a tool to read it.
- Platform administrator only access, reading included.

### 3.2 Out of scope, and why
- Per project choice of models and chunking: BL-0170.
- Converter and OCR use of registry models: BL-0165 (OCR and chat models can be stored here, nothing consumes them yet).
- Every other setting of the file: BL-0173.
- Vector storage for several dimensions and re indexing when a model changes: BL-0171; here the default text embedding model must have the engine's current dimension.
- Importing the file's embedding and reranking blocks into the registry: not asked; the operator creates the entries.
- A "test this model" action: not asked.
- Rotating the master key: secrets would have to be entered again.
- Custom header authentication: later; only `none` and `bearer` exist.
- Administrators stored in the database: BL-0173; they stay in the configuration.

### 3.3 Constraints imposed by the context
- The owner wants no secret readable after entry and wants every change audited without the secret.
- The owner wants the only environment variables to be the database URL and the master key; this increment reuses the existing key variable for the master key.

## 4. Actors
| Actor | Role |
|---|---|
| Platform administrator | creates, edits and deletes providers and models, chooses defaults, reads the record of changes |
| Member or project owner | not a platform administrator: refused everywhere in the registry |
| Operator | provides the master key, starts and restarts the platform, configures the administrators and the vector dimension |
| Search user | benefits from the defaults without seeing them |

## 4.5 Bounded contexts (MANDATORY at M and L)
| Context | What it owns | Key concepts |
|---|---|---|
| Platform settings | what the platform administrator can configure | provider, model, server default, record of changes, master key |
| Search | how documents are embedded and results reranked | embedding, reranking, vector dimension |

## 5. Usage scenarios
### SC-001: registering providers and models
**Actor:** Platform administrator
**Preconditions:** a master key is configured; the registry is empty.
**Flow:**
1. Alice creates the provider `Local GPU` (endpoint `http://localhost:9010/v1`, no authentication) → the provider is listed.
2. She creates the provider `Cloud Embeddings` with a bearer secret → the answer says a secret is stored and shows none.
3. She creates models under `Local GPU` → each gets an id and is listed under its provider.
**Postconditions:** providers and models exist.
**Exceptions:** a duplicate name, an invalid endpoint, scheme or dimension → refused with the exact text (SC-009).

### SC-002: handling secrets
**Actor:** Platform administrator, operator
**Flow:** Alice replaces or removes a secret; no answer, list, record or screen ever shows it; without a master key a secret cannot be stored; with a malformed key the platform does not start.
**Exceptions:** `secret` and `clear_secret` together, an over long secret, a bearer provider without secret.

### SC-003: editing and deleting
**Actor:** Platform administrator
**Flow:** Alice edits a provider (the change applies to its models at once), edits a model, deletes a model, deletes an empty provider.
**Exceptions:** unknown ids; a provider that still has models; a model that is a server default; changing the dimension of a default.

### SC-004: choosing the defaults and search using them
**Actor:** Platform administrator, search user
**Flow:**
1. Alice sets a text embedding model as the server default → the defaults show it.
2. A document is indexed and searched → the platform calls that model's endpoint with its name and its provider's authentication.
3. Alice sets a reranker as default → reranked searches use it.
4. Alice clears a default → the configuration is used again.
**Exceptions:** a model of the wrong kind, a kind without default, an unknown model, a dimension that differs from the vector store's, a service that is down or answers an error.

### SC-005: reading the registry and its history
**Actor:** Platform administrator
**Flow:** Alice lists providers, models (optionally by provider or kind), the defaults and the record of changes; lists are paged by 100 with a marker.
**Exceptions:** an unusable marker.

### SC-006: who may use it
**Actor:** Platform administrator, ordinary member
**Flow:** only platform administrators can do anything here, reading included.
**Exceptions:** Bob, a member or a project owner, is refused with `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`.

### SC-007: the REST interface
**Actor:** Platform administrator
**Flow:** every operation exists in the REST interface with the same inputs, answers, refusals and access rule, and the same data.

### SC-008: the models screen
**Actor:** Platform administrator
**Flow:** Alice opens the models screen, sees providers, models, defaults and latest changes, and creates, edits, deletes and sets defaults through forms; the secret field is never prefilled.
**Exceptions:** a refused action is shown with its text; a form without a valid one time token is refused; an ordinary member is refused.

### SC-009: input rules
**Actor:** Platform administrator
**Flow:** every field has its limit and exact refusal text; an update is checked as it is after the update.

### SC-010: nothing changes without a default
**Actor:** Operator
**Flow:** an existing platform upgrades with an empty registry and behaves as before; clearing a default returns to the configuration.

## 6. Functional requirements
### 6.1 New
#### FR-NEW-001 [EARS-E]: creating a provider
> WHEN a platform administrator creates a provider with a name, an endpoint, an authentication scheme and optionally a secret THE platform SHALL store it and answer with the provider.

- **Business rules:** The tool is `admin.create_provider` with `name`, `endpoint`, `auth_scheme` (`none` or `bearer`, default `none`) and `secret` (default empty).
- **Business rules:** The answer is `{provider_id, name, endpoint, auth_scheme, has_secret, created_at, updated_at}`; `provider_id` starts with `prov-`; times are UTC, RFC 3339 with seconds.
- **Business rules:** The name is trimmed; it holds 1 to 64 characters.
- **Business rules:** The endpoint is the base address of the service (for example `http://localhost:9010/v1`); it is trimmed, at most 2048 characters, has a host and carries no user name or password.
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-002 [EARS-O]: refusals of a provider
> IF the name, the endpoint, the authentication scheme or the secret of a provider is not valid THEN THE platform SHALL refuse the operation, store nothing and answer with the text of the rule that failed.

- **Business rules:** Texts, in this order of checks: `ERR_INVALID_ARGUMENT: name must be 1 to 64 characters`; `ERR_INVALID_ARGUMENT: a provider named '<name>' already exists` (names are compared without regard to case); `ERR_INVALID_ARGUMENT: endpoint must be an http or https URL` (also for an endpoint without host, with a user name or password, or longer than 2048 characters); `ERR_INVALID_ARGUMENT: auth_scheme must be 'none' or 'bearer'`; `ERR_INVALID_ARGUMENT: auth_scheme 'none' takes no secret`; `ERR_INVALID_ARGUMENT: auth_scheme 'bearer' needs a secret`; `ERR_INVALID_ARGUMENT: secret must be at most 4096 characters`; then the master key rule of FR-NEW-005.
- **Business rules:** On an update the order is: provider not found, `secret` and `clear_secret` together (FR-NEW-003), then the order above applied to the provider as it is after the update.
- **Business rules:** On an update the checks apply to the provider as it is after the update (FR-NEW-003).
- **Scenario:** SC-009
- **Priority:** Must

#### FR-NEW-003 [EARS-E]: updating a provider
> WHEN a platform administrator updates a provider THE platform SHALL change only the fields given, apply the change to every use of the provider at once, and answer with the provider.

- **Business rules:** The tool is `admin.update_provider` with `provider_id` and any of `name`, `endpoint`, `auth_scheme`, `secret`, `clear_secret` (true removes the secret); a field not given keeps its value.
- **Business rules:** `secret` and `clear_secret` together are refused with `ERR_INVALID_ARGUMENT: secret and clear_secret cannot be given together`.
- **Business rules:** An unknown provider is refused with `ERR_NOT_FOUND: provider not found: <provider_id>`.
- **Business rules:** `updated_at` changes; `created_at` and `provider_id` never change.
- **Business rules:** An update that gives no field, or only values equal to the current ones, answers with the provider unchanged (`updated_at` included) and is not recorded.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-004 [EARS-U]: secrets are write only
> The platform SHALL keep the secret of a provider encrypted, and SHALL NOT return it in any answer, listing, screen page, change record, log line or error text.

- **Business rules:** Answers carry `has_secret` (true or false) instead of the secret.
- **Business rules:** A secret sent in a refused request is not echoed in the refusal.
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-005 [EARS-O]: no master key
> IF no master key is configured THEN THE platform SHALL refuse to store a secret with `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`, and SHALL accept providers that carry no secret.

- **Business rules:** The master key is the value of the environment variable `FSA_TOKEN_KEY` (the one the git token store uses): 32 bytes in base64.
- **Business rules:** A master key that is present but is not base64 of 32 bytes stops the start with a non zero exit status and an error message that begins `FSA_TOKEN_KEY must be valid base64.` or `FSA_TOKEN_KEY must decode to 32 bytes`, whether or not the git token store is enabled.
- **Business rules:** Providers that already hold a secret stay usable while the key is present; with the key removed, calls that need the secret fail with `ERR_INTERNAL_ERROR: the secret of provider '<name>' cannot be read`.
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-006 [EARS-E]: deleting a provider
> WHEN a platform administrator deletes a provider that has no model THE platform SHALL remove the provider and its secret and answer `{deleted: true}`.

- **Business rules:** The tool is `admin.delete_provider` with `provider_id`.
- **Business rules:** A provider that still has models is refused with `ERR_INVALID_ARGUMENT: provider '<name>' still has <n> model(s)`; an unknown provider with `ERR_NOT_FOUND: provider not found: <provider_id>`.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-007 [EARS-E]: listing providers
> WHEN a platform administrator lists providers THE platform SHALL list them ordered by name without regard to case, 100 per answer, each with its fields and `has_secret`.

- **Business rules:** The tool is `admin.list_providers` with `marker` (optional); the answer is `{providers: [...], marker?}`; the marker is absent when none remain; an unusable marker is refused with `ERR_INVALID_ARGUMENT: marker is not valid`.
- **Business rules:** An empty registry gives `{providers: []}`.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-008 [EARS-E]: creating a model
> WHEN a platform administrator creates a model of a provider with a name, a kind, and for the embedding kinds a dimension, THE platform SHALL store it and answer with the model.

- **Business rules:** The tool is `admin.create_model` with `provider_id`, `name`, `kind` (`text_embedding`, `image_embedding`, `reranker`, `ocr` or `chat`), `dimension` and `max_input_tokens` (both optional).
- **Business rules:** The answer is `{model_id, provider_id, name, kind, dimension, max_input_tokens, is_default, created_at, updated_at}`; `model_id` starts with `mdl-`; absent values read `null`; `is_default` is false at creation.
- **Business rules:** The name is trimmed, 1 to 128 characters; the provider and the kind of a model never change.
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-009 [EARS-O]: refusals of a model
> IF the provider, the name, the kind, the dimension or the limit of a model is not valid THEN THE platform SHALL refuse the operation, store nothing and answer with the text of the rule that failed.

- **Business rules:** Texts, in order of checks: `ERR_NOT_FOUND: provider not found: <provider_id>`; `ERR_INVALID_ARGUMENT: name must be 1 to 128 characters`; `ERR_INVALID_ARGUMENT: kind must be one of text_embedding, image_embedding, reranker, ocr, chat`; `ERR_INVALID_ARGUMENT: dimension is required for kind '<kind>'` (the two embedding kinds); `ERR_INVALID_ARGUMENT: dimension applies to embedding kinds only`; `ERR_INVALID_ARGUMENT: dimension must be between 1 and 8192`; `ERR_INVALID_ARGUMENT: max_input_tokens must be between 1 and 1000000`; `ERR_INVALID_ARGUMENT: a model named '<name>' already exists for provider '<provider>'` (names compared without regard to case).
- **Business rules:** A model name is unique within its provider; two providers can each have a model of the same name.
- **Business rules:** A `dimension` given for a model that is not of an embedding kind is refused with `applies to embedding kinds only` before any range check.
- **Scenario:** SC-009
- **Priority:** Must

#### FR-NEW-010 [EARS-E]: updating a model
> WHEN a platform administrator updates a model THE platform SHALL change only the fields given among `name`, `dimension` and `max_input_tokens`, apply the change at once, and answer with the model.

- **Business rules:** The tool is `admin.update_model` with `model_id`; `max_input_tokens` given as `0` removes the limit.
- **Business rules:** Giving a `dimension` equal to the current one is accepted and changes nothing. The dimension of a model that is a server default cannot change: `ERR_INVALID_ARGUMENT: the dimension of a server default model cannot change`.
- **Business rules:** An unknown model is refused with `ERR_NOT_FOUND: model not found: <model_id>`; the checks of FR-NEW-009 apply to the model as it is after the update.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-011 [EARS-E]: deleting a model
> WHEN a platform administrator deletes a model that is not a server default THE platform SHALL remove it and answer `{deleted: true}`.

- **Business rules:** The tool is `admin.delete_model` with `model_id`.
- **Business rules:** A server default is refused with `ERR_INVALID_ARGUMENT: model '<name>' is a server default; clear the default first`; an unknown model with `ERR_NOT_FOUND: model not found: <model_id>`.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-012 [EARS-E]: listing models
> WHEN a platform administrator lists models THE platform SHALL list them ordered by provider name then model name without regard to case, 100 per answer, optionally limited to one provider or one kind.

- **Business rules:** The tool is `admin.list_models` with `provider_id`, `kind` and `marker`, all optional; the answer is `{models: [...], marker?}` with the fields of FR-NEW-008.
- **Business rules:** An unknown `provider_id` gives `ERR_NOT_FOUND: provider not found: <provider_id>`; an unknown `kind` gives the kind text of FR-NEW-009; an unusable marker gives `ERR_INVALID_ARGUMENT: marker is not valid`.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-013 [EARS-E]: choosing a server default model
> WHEN a platform administrator sets the server default for the kind `text_embedding` or `reranker` THE platform SHALL record that model as the default of its kind, replacing the previous default, and answer with the model.

- **Business rules:** The tool is `admin.set_default_model` with `kind` and `model_id`; the answer is the model with `is_default` true.
- **Business rules:** A kind other than those two is refused with `ERR_INVALID_ARGUMENT: kind must be 'text_embedding' or 'reranker'`; a model of another kind with `ERR_INVALID_ARGUMENT: model '<name>' is of kind '<model kind>', not '<kind>'`; an unknown model with `ERR_NOT_FOUND: model not found: <model_id>`.
- **Business rules:** A text embedding model whose dimension differs from the vector dimension of the search engine is refused with `ERR_INVALID_ARGUMENT: the default text embedding model must have dimension <n>`; the vector dimension is 1536 unless the operator configured another.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-014 [EARS-E]: clearing and reading the server defaults
> WHEN a platform administrator clears a server default or reads the defaults THE platform SHALL remove the default of that kind, or answer with the current default of each kind.

- **Business rules:** The tools are `admin.clear_default_model` with `kind` (answer `{cleared: true}`, also when none was set) and `admin.get_default_models` (answer `{text_embedding: <model or null>, reranker: <model or null>}`).
- **Business rules:** The kind text of FR-NEW-013 applies to `kind`.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-015 [EARS-S]: search embeds with the default text embedding model
> WHILE a server default text embedding model is set THE platform SHALL embed documents and queries with that model, through the endpoint, model name and authentication of its provider, instead of the search embedding settings of the configuration.

- **Business rules:** The change applies to the next embedding call after the default is set, cleared or edited; no restart.
- **Business rules:** The embedding call is a POST to the endpoint of the provider with `/embeddings` appended (trailing slashes of the endpoint ignored), whose body carries `model` and `input`.
- **Business rules:** A default text embedding model is used only when the search mode is `rag` or `both`; with mode `bm25` it has no effect on search. A search mode of `rag` or `both` still needs the configured embedding endpoint to be non empty at start.
- **Business rules:** A `bearer` provider sends its secret as `Authorization: Bearer <secret>`; a `none` provider sends no authorization.
- **Business rules:** The model name sent is the name of the model in the registry.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-016 [EARS-S]: search reranks with the default reranker
> WHILE a server default reranker is set THE platform SHALL rerank search results with that model, through the endpoint, model name and authentication of its provider, instead of the reranking settings of the configuration.

- **Business rules:** The rerank call is a POST to the endpoint of the provider with `/rerank` appended, whose body carries `model`, `query`, `documents` (the results found by the search) and `top_n` (the configured `top_n`); the search answer holds at most `top_n` results ordered by the reranker's scores.
- **Business rules:** A default reranker is used only when the search call asks for reranking (the default) and the search mode is `both`.
- **Business rules:** Today the search does not call a reranker at all, even a configured one: this requirement adds that call for the server default only.
- **Business rules:** Reranking happens when a default reranker is set and not otherwise.
- **Business rules:** A reranker that answers with an HTTP error status leaves the order of the results unchanged, as today.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-017 [EARS-U]: without a default, nothing changes
> WHILE no server default is set for a kind THE platform SHALL behave for that kind exactly as it did before the registry existed, using the search embedding and reranking settings of the configuration.

- **Business rules:** Existing deployments that never use the registry see no change: the configured embedding block is used as before, and no reranker is called.
- **Scenario:** SC-010
- **Priority:** Must

#### FR-NEW-018 [EARS-U]: every change is recorded
> The platform SHALL record every creation, update and deletion of a provider or a model and every default change or clear, with who did it, when, what it concerned and which fields changed, and SHALL NOT record any value of a secret.

- **Business rules:** A record holds `change_id`, `at` (UTC, RFC 3339 with seconds), `person`, `action` (`create_provider`, `update_provider`, `delete_provider`, `create_model`, `update_model`, `delete_model`, `set_default`, `clear_default`), `target_kind` (`provider`, `model`, `default`), `target_id`, `target_name` and `fields` (the names of the fields that changed; `secret` appears as a name only).
- **Business rules:** A refused operation is not recorded; neither is an update that changed nothing, a clear of a kind that has no default, nor setting the model that is already the default of its kind.
- **Business rules:** For a creation or a deletion `fields` is empty. For `set_default` and `clear_default`, `target_kind` is `default`, `target_id` is the id of the model concerned (the previous default for a clear), `target_name` is the kind (`text_embedding` or `reranker`) and `fields` is empty. `person` is the caller's identity in lower case.
- **Business rules:** `change_id` starts with `chg-` and increases with the order of changes, so two changes of the same second keep the order they were made in.
- **Business rules:** Records are kept without limit and survive a restart.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-019 [EARS-E]: reading the record of changes
> WHEN a platform administrator lists the changes THE platform SHALL list them newest first, 100 per answer.

- **Business rules:** The tool is `admin.list_setting_changes` with `marker`; the answer is `{changes: [...], marker?}`; records are ordered by `at` descending then `change_id` descending; an unusable marker gives `ERR_INVALID_ARGUMENT: marker is not valid`.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-020 [EARS-U]: platform administrators only
> The platform SHALL allow every registry operation, reading included, to platform administrators only.

- **Business rules:** A person who is not a platform administrator is refused with `ERR_FORBIDDEN: '<person>' is not a platform admin`, whatever the operation or the interface (tool, REST or screen), and nothing is changed or revealed.
- **Business rules:** Platform administrators are those listed in the configuration (`auth.admins`).
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-021 [EARS-U]: the REST interface offers the same operations
> The platform SHALL offer every registry operation through its REST interface with the same inputs, answers, refusals and access rule as the tools.

- **Business rules:** Refusals keep their error code and text; the REST description of the platform lists the operations.
- **Business rules:** Both interfaces call one implementation: a change made through one is visible through the other at once.
- **Business rules:** Public addresses (part of the contract with REST clients): `POST /api/admin/providers`, `GET /api/admin/providers` (query `marker`), `PATCH /api/admin/providers/{provider_id}`, `DELETE /api/admin/providers/{provider_id}`, `POST /api/admin/models`, `GET /api/admin/models` (query `provider_id`, `kind`, `marker`), `PATCH /api/admin/models/{model_id}`, `DELETE /api/admin/models/{model_id}`, `GET /api/admin/defaults`, `PUT /api/admin/defaults/{kind}` (body `model_id`), `DELETE /api/admin/defaults/{kind}`, `GET /api/admin/setting-changes` (query `marker`).
- **Business rules:** Bodies and answers are the tool inputs and answers as JSON; a refusal keeps its error code and text in the error envelope of the existing REST routes.
- **Scenario:** SC-007
- **Priority:** Must

#### FR-NEW-022 [EARS-E]: the settings screen
> WHEN a platform administrator opens the models screen THE platform SHALL show the providers, their models, the server defaults and the latest changes, and SHALL let the administrator create, edit and delete providers and models and set and clear the defaults.

- **Business rules:** The screen shows for each provider its name, endpoint, scheme and `Secret stored` or `No secret`; the secret field is never filled with an existing secret; leaving it empty on an edit keeps the secret.
- **Business rules:** A refused action shows the text of the refusal on the screen; the screen calls the same implementation as the tools.
- **Business rules:** A person who is not a platform administrator gets the refusal of FR-NEW-020.
- **Business rules:** The screen is served at `/app/models`; its forms post to `/app/models/<action>` (`provider/create`, `provider/update`, `provider/delete`, `model/create`, `model/update`, `model/delete`, `default/set`, `default/clear`).
- **Scenario:** SC-008
- **Priority:** Must

#### FR-NEW-023 [EARS-O]: the screen protects its forms
> IF a form of the models screen is sent without a valid one time token issued to the same person THEN THE platform SHALL refuse it and change nothing.

- **Business rules:** The refusal is `ERR_FORBIDDEN` with the text `missing or invalid csrf_token`, as the token screen does.
- **Business rules:** A request that carries the caller's identity in a header instead of the browser cookie needs no token, as for the token screen.
- **Scenario:** SC-008
- **Priority:** Must

### 6.2 Modified
None. (Search uses the registry only when a default is set; SPEC-0008 is unchanged otherwise.)

### 6.3 Removed
None.

## 7. Non functional requirements (user observable)
### 7.1 Performance
- Every registry operation and every list answers in under 1 second for a registry of 150 providers and 500 models.
### 7.2 Security and access
- Platform administrators only (FR-NEW-020); secrets write only and encrypted (FR-NEW-004, FR-NEW-005); screen forms protected (FR-NEW-023).
### 7.3 Privacy and compliance
- No secret appears in an answer, a record of changes, a screen page, a log line or an error text (FR-NEW-004).
### 7.5 Reliability
- A change applies at once and in one step: a call made during the change sees the old or the new provider and model, never a mix.
- A refused operation changes nothing.
### 7.6 Operability
- An operator can see, for any setting change, who made it, when and what it touched (FR-NEW-018, FR-NEW-019).

## 8. Edge cases
| Category | Case | Expected behavior | Test |
|---|---|---|---|
| Empty states | empty registry | empty lists, defaults null | E2E-054, E2E-103 |
| Boundary values | names of 1, 64, 65 characters; models of 128 and 129; dimension 1, 8192, 8193, 0; limit 1, 1000000, 1000001; 150 providers | exact bounds | E2E-090 to E2E-093, E2E-050 |
| Concurrent actions | change while an indexing call runs | the call sees one version | E2E-039 |
| Access states | ordinary member, owner, mixed case identity | refused or accepted as stated | E2E-060 to E2E-062 |
| Data variations | accents and spaces in a name; mixed case duplicates | trimmed; duplicates caseless | E2E-005, E2E-004, E2E-051 |
| Partial operations | refused update changes nothing | unchanged | E2E-095, E2E-057 |
| Dependency failures | embedding or reranker service down, answering an error | rebuild fails, rerank order kept | E2E-041, E2E-108 |
| Ordering and timing | change then immediate call; restart | applied at once; records survive | E2E-039, E2E-058 |
| Navigation | stale form token; reused token | refused | E2E-085, E2E-109 |
| Slow connectivity, Exhaustion | not applicable: no interactive long flow, no quota |  |  |

## 9. Acceptance tests (the contract)
> Behavioral and technology agnostic. /sdlc-design maps each test to a driver. An implementing agent uses this as its definition of done.
> Timing convention: stub services answer at once; a change is visible to the next call; "within the settle delay" reuses the convention of SPEC-0100. The vector dimension of the search engine is stated by a test when it matters (default 1536). "The platform logs" means the operator visible log output.

### 9.1 Summary
| Test | Action | Category | Scenario | Requirements | Priority |
|---|---|---|---|---|---|
| E2E-001 | New | journey | SC-001 | FR-NEW-001, FR-NEW-007 | Critical |
| E2E-002 | New | journey | SC-001 | FR-NEW-001, FR-NEW-004 | Critical |
| E2E-003 | New | journey | SC-001 | FR-NEW-008, FR-NEW-012 | Critical |
| E2E-004 | New | failure | SC-001 | FR-NEW-002 | High |
| E2E-005 | New | edge | SC-001 | FR-NEW-001 | Medium |
| E2E-006 | New | edge | SC-001 | FR-NEW-008, FR-NEW-009 | High |
| E2E-010 | New | journey | SC-002 | FR-NEW-003, FR-NEW-004 | High |
| E2E-011 | New | journey | SC-002 | FR-NEW-003, FR-NEW-004, FR-NEW-002 | High |
| E2E-012 | New | failure | SC-002 | FR-NEW-005 | High |
| E2E-013 | New | failure | SC-002 | FR-NEW-005 | High |
| E2E-014 | New | failure | SC-002 | FR-NEW-004 | High |
| E2E-015 | New | failure | SC-002 | FR-NEW-003 | High |
| E2E-016 | New | edge | SC-002 | FR-NEW-005 | Medium |
| E2E-017 | New | edge | SC-002 | FR-NEW-002 | Medium |
| E2E-020 | New | journey | SC-003 | FR-NEW-003 | High |
| E2E-021 | New | failure | SC-003 | FR-NEW-003 | High |
| E2E-022 | New | failure | SC-003 | FR-NEW-006 | High |
| E2E-023 | New | journey | SC-003 | FR-NEW-006 | High |
| E2E-024 | New | failure | SC-003 | FR-NEW-006 | High |
| E2E-025 | New | journey | SC-003 | FR-NEW-010 | High |
| E2E-026 | New | failure | SC-003 | FR-NEW-010, FR-NEW-011 | High |
| E2E-027 | New | journey | SC-003 | FR-NEW-011 | High |
| E2E-028 | New | failure | SC-003 | FR-NEW-011, FR-NEW-013 | High |
| E2E-029 | New | failure | SC-003 | FR-NEW-010, FR-NEW-013 | High |
| E2E-030 | New | journey | SC-004 | FR-NEW-013, FR-NEW-014 | Critical |
| E2E-031 | New | edge | SC-004 | FR-NEW-013 | Medium |
| E2E-032 | New | failure | SC-004 | FR-NEW-013 | High |
| E2E-033 | New | failure | SC-004 | FR-NEW-013, FR-NEW-014 | High |
| E2E-034 | New | failure | SC-004 | FR-NEW-013 | High |
| E2E-035 | New | failure | SC-004 | FR-NEW-013 | High |
| E2E-036 | New | journey | SC-004 | FR-NEW-014 | High |
| E2E-037 | New | journey | SC-004 | FR-NEW-015 | Critical |
| E2E-038 | New | journey | SC-004 | FR-NEW-015, FR-NEW-004 | High |
| E2E-039 | New | edge | SC-004 | FR-NEW-015 | High |
| E2E-040 | New | journey | SC-004 | FR-NEW-016 | High |
| E2E-041 | New | failure | SC-004 | FR-NEW-015 | High |
| E2E-042 | New | edge | SC-004 | FR-NEW-013, FR-NEW-015 | Medium |
| E2E-050 | New | journey | SC-005 | FR-NEW-007 | Medium |
| E2E-051 | New | edge | SC-005 | FR-NEW-007 | Medium |
| E2E-052 | New | failure | SC-005 | FR-NEW-007, FR-NEW-012, FR-NEW-019 | High |
| E2E-053 | New | journey | SC-005 | FR-NEW-012 | High |
| E2E-054 | New | edge | SC-005 | FR-NEW-007, FR-NEW-012, FR-NEW-014, FR-NEW-019 | Medium |
| E2E-055 | New | journey | SC-005 | FR-NEW-018, FR-NEW-019 | Critical |
| E2E-056 | New | edge | SC-005 | FR-NEW-018, FR-NEW-004 | Medium |
| E2E-057 | New | failure | SC-005 | FR-NEW-018 | High |
| E2E-058 | New | edge | SC-005 | FR-NEW-018, FR-NEW-019 | Medium |
| E2E-059 | New | journey | SC-005 | FR-NEW-019 | Medium |
| E2E-060 | New | failure | SC-006 | FR-NEW-020 | High |
| E2E-061 | New | failure | SC-006 | FR-NEW-020 | High |
| E2E-062 | New | edge | SC-006 | FR-NEW-020 | Medium |
| E2E-063 | New | journey | SC-006 | FR-NEW-020 | Medium |
| E2E-070 | New | journey | SC-007 | FR-NEW-021, FR-NEW-001, FR-NEW-007 | High |
| E2E-071 | New | failure | SC-007 | FR-NEW-021, FR-NEW-002, FR-NEW-009 | High |
| E2E-072 | New | failure | SC-007 | FR-NEW-021, FR-NEW-020 | High |
| E2E-073 | New | edge | SC-007 | FR-NEW-021 | Medium |
| E2E-074 | New | edge | SC-007 | FR-NEW-021 | Medium |
| E2E-080 | New | journey | SC-008 | FR-NEW-022, FR-NEW-007 | Critical |
| E2E-081 | New | journey | SC-008 | FR-NEW-022, FR-NEW-001, FR-NEW-008 | Critical |
| E2E-082 | New | edge | SC-008 | FR-NEW-022, FR-NEW-004 | Medium |
| E2E-083 | New | failure | SC-008 | FR-NEW-022, FR-NEW-002 | High |
| E2E-084 | New | journey | SC-008 | FR-NEW-022, FR-NEW-011, FR-NEW-013 | High |
| E2E-085 | New | failure | SC-008 | FR-NEW-023 | High |
| E2E-086 | New | edge | SC-008 | FR-NEW-023 | Medium |
| E2E-087 | New | failure | SC-008 | FR-NEW-022, FR-NEW-020 | High |
| E2E-090 | New | failure | SC-009 | FR-NEW-002 | High |
| E2E-091 | New | edge | SC-009 | FR-NEW-002, FR-NEW-001 | Medium |
| E2E-092 | New | failure | SC-009 | FR-NEW-009 | High |
| E2E-093 | New | edge | SC-009 | FR-NEW-009, FR-NEW-008 | Medium |
| E2E-094 | New | failure | SC-009 | FR-NEW-009 | High |
| E2E-095 | New | failure | SC-009 | FR-NEW-002, FR-NEW-003 | High |
| E2E-100 | New | journey | SC-010 | FR-NEW-017 | High |
| E2E-101 | New | edge | SC-010 | FR-NEW-017, FR-NEW-014 | Medium |
| E2E-102 | New | edge | SC-010 | FR-NEW-017 | Medium |
| E2E-103 | New | journey | SC-010 | FR-NEW-017, FR-NEW-007 | Medium |
| E2E-104 | New | failure | SC-010 | FR-NEW-006, FR-NEW-011 | High |
| E2E-105 | New | edge | SC-010 | FR-NEW-014 | Medium |
| E2E-106 | New | edge | SC-010 | FR-NEW-010, FR-NEW-015 | Medium |
| E2E-107 | New | edge | SC-004 | FR-NEW-016, FR-NEW-004 | Medium |
| E2E-108 | New | failure | SC-004 | FR-NEW-016 | High |
| E2E-109 | New | edge | SC-008 | FR-NEW-023 | Medium |
| E2E-110 | New | edge | SC-003 | FR-NEW-003, FR-NEW-002 | Medium |
| E2E-111 | New | journey | SC-009 | FR-NEW-001, FR-NEW-008 | Medium |
| E2E-112 | New | failure | SC-009 | FR-NEW-002, FR-NEW-001, FR-NEW-004 | High |
| E2E-113 | New | edge | SC-004 | FR-NEW-015 | Medium |
| E2E-114 | New | edge | SC-004 | FR-NEW-016 | Medium |
| E2E-115 | New | edge | SC-003 | FR-NEW-003, FR-NEW-018 | Medium |
| E2E-116 | New | edge | SC-003 | FR-NEW-010 | Medium |

Coverage: happy 26 (journey 26), failure 32, side effect 0, edge 29, state transition 0, security 0 as a category (the access rules are in the failure tests E2E-060, E2E-061, E2E-072 and E2E-087). 87 new tests and 0 modified. Happy to failure ratio 1:1.2.

### 9.2 New tests
#### E2E-001: registering a local provider
- **Category:** journey. **Scenario:** SC-001. **Requirements:** FR-NEW-001, FR-NEW-007
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates the provider `Local GPU` with endpoint `http://localhost:9010/v1` and no authentication scheme given, then lists the providers
- **Then** the answer holds a `provider_id` starting with `prov-`, `name` `Local GPU`, `auth_scheme` `none`, `has_secret` false and equal creation and update times; the list holds exactly that provider
- **And** the list answer has no marker
- **Priority:** Critical

#### E2E-002: registering a cloud provider with a secret
- **Category:** journey. **Scenario:** SC-001. **Requirements:** FR-NEW-001, FR-NEW-004
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates the provider `Cloud Embeddings` with endpoint `https://api.example.com/v1`, scheme `bearer` and secret `sk-test-123`
- **Then** the answer shows `has_secret` true and no field holds `sk-test-123`
- **And** listing the providers shows `has_secret` true and never the secret
- **Priority:** Critical

#### E2E-003: registering models
- **Category:** journey. **Scenario:** SC-001. **Requirements:** FR-NEW-008, FR-NEW-012
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; the provider `Local GPU` exists
- **When** Alice creates the model `qwen3-embedding-0.6b` of kind `text_embedding` with dimension 1024 and `max_input_tokens` 8192, and the model `bge-reranker` of kind `reranker`, then lists the models
- **Then** the first answer holds a `model_id` starting with `mdl-`, `kind` `text_embedding`, `dimension` 1024, `max_input_tokens` 8192, `is_default` false; the second holds `dimension` null and `max_input_tokens` null; the list holds both ordered by name
- **And** each model names its provider
- **Priority:** Critical

#### E2E-004: a provider name already taken
- **Category:** failure. **Scenario:** SC-001. **Requirements:** FR-NEW-002
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; the provider `Local GPU` exists
- **When** Alice creates a provider named `local gpu`
- **Then** the answer is `ERR_INVALID_ARGUMENT: a provider named 'local gpu' already exists`
- **And** the list still holds one provider
- **Priority:** High

#### E2E-005: the name is trimmed
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider named `  Café Embeddings  `
- **Then** the provider is named `Café Embeddings`
- **And** a second creation of `Café Embeddings` is refused as a duplicate
- **Priority:** Medium

#### E2E-006: the same model name under two providers
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-008, FR-NEW-009
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; providers `Local GPU` and `Cloud Embeddings` exist
- **When** Alice creates the model `qwen3-embedding-0.6b` (text_embedding, dimension 1024) under each, then a second one under `Local GPU`
- **Then** the two first creations succeed; the third is refused with `ERR_INVALID_ARGUMENT: a model named 'qwen3-embedding-0.6b' already exists for provider 'Local GPU'`
- **And** the list holds two models
- **Priority:** High

#### E2E-010: replacing a secret
- **Category:** journey. **Scenario:** SC-002. **Requirements:** FR-NEW-003, FR-NEW-004
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds the secret `sk-test-123`
- **When** Alice updates the provider with the secret `sk-new-456`
- **Then** the answer shows `has_secret` true, a later `updated_at`, and the same `provider_id` and `created_at`
- **And** the next embedding call of the platform sends `Authorization: Bearer sk-new-456` (E2E-038)
- **Priority:** High

#### E2E-011: removing a secret
- **Category:** journey. **Scenario:** SC-002. **Requirements:** FR-NEW-003, FR-NEW-004, FR-NEW-002
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds a secret
- **When** Alice updates it with `clear_secret` true and `auth_scheme` `none`
- **Then** the answer shows `has_secret` false and `auth_scheme` `none`
- **And** clearing the secret while the scheme stays `bearer` is refused with `ERR_INVALID_ARGUMENT: auth_scheme 'bearer' needs a secret`
- **Priority:** High

#### E2E-012: no master key
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** the same platform without a master key
- **When** Alice creates a provider with scheme `bearer` and secret `sk-test-123`, then creates a provider with scheme `none`
- **Then** the first answer is `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`; the second succeeds
- **And** only the second provider is listed
- **Priority:** High

#### E2E-013: a malformed master key
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** the master key variable holds `not-a-key`
- **When** the operator starts the platform
- **Then** the process exits with a non zero status and its error output begins `FSA_TOKEN_KEY must be valid base64.`
- **And** with a valid key the platform starts, and the same happens with the git token store disabled
- **Priority:** High

#### E2E-014: a secret is never revealed
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-004
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds the secret `sk-test-123`
- **When** Alice updates it with a secret of 5000 characters beginning with `sk-` (through the tool, then through the REST interface), lists the providers through both, reads the list of changes and the screen
- **Then** the update is refused with `ERR_INVALID_ARGUMENT: secret must be at most 4096 characters` and the refusal text does not contain the secret; no list, change record or screen contains `sk-test-123`
- **And** the platform logs contain no secret
- **Priority:** High

#### E2E-015: a secret together with clear_secret
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-003
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` exists
- **When** Alice updates it with `secret` `sk-x` and `clear_secret` true
- **Then** the answer is `ERR_INVALID_ARGUMENT: secret and clear_secret cannot be given together`
- **And** the provider is unchanged
- **Priority:** High

#### E2E-016: the master key is removed later
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds a secret; the operator restarts the platform without the master key, the default text embedding model belongs to that provider
- **When** Alice saves `/guide.md` and a search is made
- **Then** the rebuild job of the document fails with the reason `ERR_INTERNAL_ERROR: the secret of provider 'Cloud Embeddings' cannot be read` (read in the job list) and the search answers with the same refusal
- **And** listing providers still works and shows `has_secret` true
- **Priority:** Medium

#### E2E-017: an empty secret on a bearer provider
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-002
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider with scheme `bearer` and an empty secret
- **Then** the answer is `ERR_INVALID_ARGUMENT: auth_scheme 'bearer' needs a secret`
- **And** nothing was stored
- **Priority:** Medium

#### E2E-020: editing a provider applies to its models
- **Category:** journey. **Scenario:** SC-003. **Requirements:** FR-NEW-003
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` has one model
- **When** Alice changes the endpoint of `Local GPU` to `http://gpu2:9010/v1` and its name to `GPU 2`
- **Then** the answer shows the new values and a later `updated_at`
- **And** the model listing shows the provider unchanged by id and the model still belongs to it
- **Priority:** High

#### E2E-021: editing an unknown provider
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-003
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice updates the provider `prov-unknown`
- **Then** the answer is `ERR_NOT_FOUND: provider not found: prov-unknown`
- **And** nothing changed
- **Priority:** High

#### E2E-022: deleting a provider with models
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` has 2 models
- **When** Alice deletes `Local GPU`
- **Then** the answer is `ERR_INVALID_ARGUMENT: provider 'Local GPU' still has 2 model(s)`
- **And** the provider and its models remain
- **Priority:** High

#### E2E-023: deleting an empty provider
- **Category:** journey. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` has no model
- **When** Alice deletes it
- **Then** the answer is `{deleted: true}`
- **And** the provider is gone from the list, and its secret with it
- **Priority:** High

#### E2E-024: deleting an unknown provider
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice deletes `prov-unknown`
- **Then** the answer is `ERR_NOT_FOUND: provider not found: prov-unknown`
- **And** nothing changed
- **Priority:** High

#### E2E-025: updating a model
- **Category:** journey. **Scenario:** SC-003. **Requirements:** FR-NEW-010
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; model `bge-reranker` (reranker) exists
- **When** Alice gives it `max_input_tokens` 512, then `0`
- **Then** the first answer holds 512; the second holds `null`
- **And** its kind and provider are unchanged
- **Priority:** High

#### E2E-026: an unknown model
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-010, FR-NEW-011
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice updates, then deletes the model `mdl-unknown`
- **Then** each answer is `ERR_NOT_FOUND: model not found: mdl-unknown`
- **And** nothing changed
- **Priority:** High

#### E2E-027: deleting a model
- **Category:** journey. **Scenario:** SC-003. **Requirements:** FR-NEW-011
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; model `bge-reranker` exists and is not a default
- **When** Alice deletes it
- **Then** the answer is `{deleted: true}`
- **And** the model list no longer holds it; deleting it again answers `ERR_NOT_FOUND: model not found: <id>`
- **Priority:** High

#### E2E-028: a server default cannot be deleted
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-011, FR-NEW-013
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `qwen3-embedding-0.6b` (1024) is the default text embedding model
- **When** Alice deletes it
- **Then** the answer is `ERR_INVALID_ARGUMENT: model 'qwen3-embedding-0.6b' is a server default; clear the default first`
- **And** after clearing the default the deletion succeeds
- **Priority:** High

#### E2E-029: the dimension of a default is frozen
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-010, FR-NEW-013
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `qwen3-embedding-0.6b` (1024) is the default text embedding model
- **When** Alice changes its dimension to 768
- **Then** the answer is `ERR_INVALID_ARGUMENT: the dimension of a server default model cannot change`
- **And** its name can still change
- **Priority:** High

#### E2E-030: choosing the default text embedding model
- **Category:** journey. **Scenario:** SC-004. **Requirements:** FR-NEW-013, FR-NEW-014
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; the model `qwen3-embedding-0.6b` (1024) exists
- **When** Alice sets it as the default for `text_embedding`, then reads the defaults
- **Then** the answer shows `is_default` true; the defaults read `text_embedding` that model and `reranker` null
- **And** listing the models shows `is_default` true for it only
- **Priority:** Critical

#### E2E-031: replacing a default
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-013
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; two text embedding models of dimension 1024
- **When** Alice sets the first, then the second as the default
- **Then** the defaults read the second
- **And** the first shows `is_default` false
- **Priority:** Medium

#### E2E-032: a default of the wrong kind
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-013
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `bge-reranker` (reranker) exists
- **When** Alice sets it as the default for `text_embedding`
- **Then** the answer is `ERR_INVALID_ARGUMENT: model 'bge-reranker' is of kind 'reranker', not 'text_embedding'`
- **And** no default is set
- **Priority:** High

#### E2E-033: a kind that has no default
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-013, FR-NEW-014
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; model `Llama` of kind `chat` exists
- **When** Alice sets it as the default for `chat`, then clears the default for `ocr`
- **Then** each answer is `ERR_INVALID_ARGUMENT: kind must be 'text_embedding' or 'reranker'`
- **And** nothing changed
- **Priority:** High

#### E2E-034: a dimension that does not match the vector store
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-013
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1536; the model `qwen3-embedding-0.6b` has dimension 1024
- **When** Alice sets it as the default for `text_embedding`
- **Then** the answer is `ERR_INVALID_ARGUMENT: the default text embedding model must have dimension 1536`
- **And** no default is set
- **Priority:** High

#### E2E-035: an unknown model as default
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-013
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice sets `mdl-unknown` as the default for `reranker`
- **Then** the answer is `ERR_NOT_FOUND: model not found: mdl-unknown`
- **And** nothing changed
- **Priority:** High

#### E2E-036: clearing a default
- **Category:** journey. **Scenario:** SC-004. **Requirements:** FR-NEW-014
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; a default reranker is set
- **When** Alice clears the default for `reranker`, then clears it again
- **Then** both answers are `{cleared: true}`
- **And** the defaults read `reranker` null and the record of changes holds one `clear_default`
- **Priority:** High

#### E2E-037: the default text embedding model drives indexing
- **Category:** journey. **Scenario:** SC-004. **Requirements:** FR-NEW-015
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; project `proj` indexes with vectors; a stub embedding service at `http://localhost:9010/v1`; provider `Local GPU` (scheme `none`) with the default model `qwen3-embedding-0.6b` (1024)
- **When** Alice saves `/guide.md` and, after the settle delay, a search is made
- **Then** the stub service received the calls for the document and the query with the model name `qwen3-embedding-0.6b` and no authorization; the search finds `/guide.md`
- **And** the configured embedding settings were not called
- **Priority:** Critical

#### E2E-038: a bearer provider sends its secret
- **Category:** journey. **Scenario:** SC-004. **Requirements:** FR-NEW-015, FR-NEW-004
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; a stub service requiring `Authorization: Bearer sk-test-123`; provider `Cloud Embeddings` holding that secret with the default model of dimension 1024
- **When** a document is indexed
- **Then** the stub received `Authorization: Bearer sk-test-123` and accepted the call
- **And** after Alice replaces the secret by `sk-new-456` the next call carries `sk-new-456`
- **Priority:** High

#### E2E-039: a change applies at once
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-015
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; project indexing; the default text embedding model points to stub A
- **When** Alice changes the endpoint of its provider to stub B and a document is indexed right after
- **Then** stub B received the call and stub A none
- **And** no restart happened
- **Priority:** High

#### E2E-040: the default reranker drives reranking
- **Category:** journey. **Scenario:** SC-004. **Requirements:** FR-NEW-016
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; project with search; stub reranker at `http://localhost:9011/v1`; provider and model `bge-reranker` set as the default reranker; the configuration has reranking off
- **When** the search mode is `both`; a search with reranking asks for 20 results
- **Then** the stub received the query with the model name `bge-reranker`, 20 documents and `top_n` 10; the answer holds 10 results in the stub's order
- **And** clearing the default reranker returns to the behavior without reranking (no call)
- **Priority:** High

#### E2E-041: the embedding service of the default is down
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-015
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; the default model's provider endpoint refuses connections
- **When** a document is saved and its rebuild runs
- **Then** the rebuild fails with `embedding service unreachable`; the write succeeded
- **And** the previous entries remain searchable
- **Priority:** High

#### E2E-042: the dimension of vectors matches the default
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-013, FR-NEW-015
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; a default text embedding model of dimension 1024 whose service answers vectors of length 1024
- **When** a document is indexed
- **Then** the entries are stored and a search finds them
- **And** setting a model of dimension 1023 as default answers `ERR_INVALID_ARGUMENT: the default text embedding model must have dimension 1024`
- **Priority:** Medium

#### E2E-050: paging the providers
- **Category:** journey. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; 150 providers named `P001` to `P150`
- **When** Alice lists the providers, then asks for the next page with the marker
- **Then** the first answer holds `P001` to `P100` and a marker; the second holds `P101` to `P150` and no marker
- **And** the order is by name without regard to case
- **Priority:** Medium

#### E2E-051: the order is by name without regard to case
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; providers `beta`, `Alpha`, `charlie`
- **When** Alice lists the providers
- **Then** the order is `Alpha`, `beta`, `charlie`
- **And** ties on the name cannot exist (names are unique without regard to case)
- **Priority:** Medium

#### E2E-052: an unusable marker
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-007, FR-NEW-012, FR-NEW-019
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice lists providers, models and changes with the marker `zzz`
- **Then** each answer is `ERR_INVALID_ARGUMENT: marker is not valid`
- **And** nothing changed
- **Priority:** High

#### E2E-053: listing the models of one provider and one kind
- **Category:** journey. **Scenario:** SC-005. **Requirements:** FR-NEW-012
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; two providers with three models of kinds `text_embedding`, `reranker`, `chat`
- **When** Alice lists the models of `Local GPU`, then those of kind `reranker`
- **Then** the first answer holds the models of that provider ordered by model name; the second only the rerankers, ordered by provider name then model name across both providers
- **And** an unknown provider id answers `ERR_NOT_FOUND: provider not found: <id>` and an unknown kind `ERR_INVALID_ARGUMENT: kind must be one of text_embedding, image_embedding, reranker, ocr, chat`
- **Priority:** High

#### E2E-054: an empty registry
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007, FR-NEW-012, FR-NEW-014, FR-NEW-019
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice lists providers, models, defaults and changes
- **Then** the answers are `{providers: []}`, `{models: []}`, `{text_embedding: null, reranker: null}` and `{changes: []}`
- **And** no error
- **Priority:** Medium

#### E2E-055: the record of changes
- **Category:** journey. **Scenario:** SC-005. **Requirements:** FR-NEW-018, FR-NEW-019
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates the provider `Local GPU`, updates its endpoint, creates a model, sets it as default, then lists the changes
- **Then** four records, newest first: `set_default` (`target_kind` `default`, `target_id` the model id, `target_name` `text_embedding`, `fields` empty), `create_model` (`target_kind` `model`, the model id and name), `update_provider` (`target_kind` `provider`, `fields` [`endpoint`]), `create_provider`; each holds `admin@example.com`, a UTC time and a `change_id` starting with `chg-`, the four ids increasing with the order made
- **And** none contains a secret value
- **Priority:** Critical

#### E2E-056: a secret change is recorded as a name
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-018, FR-NEW-004
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` exists
- **When** Alice replaces its secret by `sk-new-456` and clears it later
- **Then** two `update_provider` records whose `fields` hold the name `secret`
- **And** no record holds `sk-new-456`
- **Priority:** Medium

#### E2E-057: a refused operation leaves no record
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-018
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider with endpoint `ftp://x`, then a duplicate
- **Then** both are refused
- **And** the list of changes is empty
- **Priority:** High

#### E2E-058: records survive a restart
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-018, FR-NEW-019
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; three changes were made
- **When** the operator restarts the platform and Alice lists the changes
- **Then** the three records are listed
- **And** nothing was removed
- **Priority:** Medium

#### E2E-059: paging the changes
- **Category:** journey. **Scenario:** SC-005. **Requirements:** FR-NEW-019
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; 120 changes were made
- **When** Alice lists the changes and asks for the next page
- **Then** the first answer holds the 100 newest and a marker, the second the 20 oldest and no marker
- **And** ties on the time are ordered by `change_id` descending
- **Priority:** Medium

#### E2E-060: an ordinary member is refused everywhere
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-020
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; one provider and one model exist
- **When** Bob lists providers, creates a provider, updates it, deletes it, lists models, sets a default and lists the changes
- **Then** each answer is `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** nothing was created, changed or revealed
- **Priority:** High

#### E2E-061: a project owner is not a platform administrator
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-020
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; Bob owns project `proj`
- **When** Bob reads the defaults
- **Then** the answer is `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** his ownership grants nothing here
- **Priority:** High

#### E2E-062: administrator identity is compared without regard to case
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-020
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** `Admin@Example.COM` lists the providers
- **Then** the call succeeds
- **And** a person not listed is refused
- **Priority:** Medium

#### E2E-063: a platform administrator is not a project member
- **Category:** journey. **Scenario:** SC-006. **Requirements:** FR-NEW-020
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; project `proj` of which Alice is not a member
- **When** Alice lists the providers
- **Then** the call succeeds
- **And** reading `proj` files still follows project membership
- **Priority:** Medium

#### E2E-070: the REST interface creates and lists
- **Category:** journey. **Scenario:** SC-007. **Requirements:** FR-NEW-021, FR-NEW-001, FR-NEW-007
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice, through the REST interface, creates the provider `Local GPU` and lists the providers
- **Then** the answers equal those of E2E-001
- **And** the provider is also listed by the tool `admin.list_providers`
- **Priority:** High

#### E2E-071: the REST interface refuses like the tools
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-021, FR-NEW-002, FR-NEW-009
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** through the REST interface Alice creates a duplicate provider, a model with dimension 0 and a model of an unknown provider
- **Then** the refusals carry the same codes and texts as the tools: a duplicate, `dimension must be between 1 and 8192`, `provider not found`
- **And** nothing was created
- **Priority:** High

#### E2E-072: the REST interface refuses a member
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-021, FR-NEW-020
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Bob, through the REST interface, lists providers and creates one
- **Then** each answer is the refusal `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** nothing was created
- **Priority:** High

#### E2E-073: a change through one interface is visible through the other
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-021
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a model with the tool, edits it through the REST interface, and reads it with the tool
- **Then** the tool shows the edit at once
- **And** the list of changes holds both records
- **Priority:** Medium

#### E2E-074: the REST description lists the operations
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-021
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** a client reads the REST description of the platform
- **Then** it lists providers, models, defaults and changes operations with their inputs
- **And** the operations are absent when the REST interface is disabled by configuration
- **Priority:** Medium

#### E2E-080: the models screen shows the registry
- **Category:** journey. **Scenario:** SC-008. **Requirements:** FR-NEW-022, FR-NEW-007
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; provider `Cloud Embeddings` (secret stored), `Local GPU`, two models and a default
- **When** Alice opens the models screen in her browser
- **Then** she sees both providers with endpoint and scheme, `Secret stored` for `Cloud Embeddings` and `No secret` for `Local GPU`, the models under their providers, the defaults and the latest changes
- **And** no secret value appears in the page
- **Priority:** Critical

#### E2E-081: creating a provider and a model on the screen
- **Category:** journey. **Scenario:** SC-008. **Requirements:** FR-NEW-022, FR-NEW-001, FR-NEW-008
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice submits the provider form (`Local GPU`, endpoint, scheme `none`) then the model form (`qwen3-embedding-0.6b`, kind `text_embedding`, dimension 1024)
- **Then** both appear on the screen after each submit
- **And** the tool lists both
- **Priority:** Critical

#### E2E-082: editing keeps the secret when the field is empty
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-022, FR-NEW-004
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds a secret
- **When** Alice edits its endpoint on the screen and leaves the secret field empty
- **Then** the endpoint changes and the provider still shows `Secret stored`
- **And** the edit form's secret field was empty before submit
- **Priority:** Medium

#### E2E-083: a refused action is explained on the screen
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-022, FR-NEW-002
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice submits the provider form with the name `local gpu`
- **Then** the page shows `a provider named 'local gpu' already exists`
- **And** nothing was created
- **Priority:** High

#### E2E-084: deleting and defaults on the screen
- **Category:** journey. **Scenario:** SC-008. **Requirements:** FR-NEW-022, FR-NEW-011, FR-NEW-013
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; a model of dimension 1024 and a vector dimension of 1024
- **When** Alice sets the model as the default on the screen, tries to delete it, clears the default, then deletes it
- **Then** the deletion is first refused with the text of FR-NEW-011 on the page, then succeeds
- **And** the model is gone from the screen
- **Priority:** High

#### E2E-085: a form without a valid token
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-023
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** a browser session sends the provider form without its one time token, then with a token already used, then with a token issued to Bob
- **Then** each answer is `ERR_FORBIDDEN` with `missing or invalid csrf_token`
- **And** nothing was created
- **Priority:** High

#### E2E-086: a request carrying the identity in a header needs no token
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-023
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice's client sends the provider form with the identity in a header and no token
- **Then** the provider is created
- **And** a cookie session without token is refused (E2E-085)
- **Priority:** Medium

#### E2E-087: the screen refuses an ordinary member
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-022, FR-NEW-020
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Bob opens the models screen and sends its forms
- **Then** he gets `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** nothing was created or revealed
- **Priority:** High

#### E2E-090: invalid provider fields
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-002
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates providers with an empty name, a name of 65 characters, endpoint `localhost:9010`, scheme `basic`, scheme `none` with the secret `x`, scheme `bearer` with no secret
- **Then** the answers are, in order, `ERR_INVALID_ARGUMENT: name must be 1 to 64 characters` twice, `endpoint must be an http or https URL`, `auth_scheme must be 'none' or 'bearer'`, `auth_scheme 'none' takes no secret`, `auth_scheme 'bearer' needs a secret`
- **And** no provider was created
- **Priority:** High

#### E2E-091: limits of a provider name
- **Category:** edge. **Scenario:** SC-009. **Requirements:** FR-NEW-002, FR-NEW-001
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates providers named with 1 and with 64 characters
- **Then** both succeed
- **And** a name of 65 characters is refused (E2E-090)
- **Priority:** Medium

#### E2E-092: invalid model fields
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-009
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice creates models: an embedding model without dimension, a chat model with dimension 1024, an embedding model with dimension 0 and with 8193, a model with `max_input_tokens` 0 and with 1000001, an unknown kind `vision`, an unknown provider
- **Then** the answers are the texts of FR-NEW-009 in that order: `dimension is required for kind 'text_embedding'`, `dimension applies to embedding kinds only`, `dimension must be between 1 and 8192` twice, `max_input_tokens must be between 1 and 1000000` twice, `kind must be one of ...`, `provider not found`; an unknown kind `vision` under an unknown provider answers `provider not found`; a duplicate name answers `a model named ... already exists for provider ...`; a chat model with dimension 0 answers `dimension applies to embedding kinds only`
- **And** no model was created
- **Priority:** High

#### E2E-093: limits of a model
- **Category:** edge. **Scenario:** SC-009. **Requirements:** FR-NEW-009, FR-NEW-008
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice creates embedding models with dimension 1 and 8192, a model named with 128 characters, and a model with `max_input_tokens` 1 and 1000000
- **Then** all succeed
- **And** a name of 129 characters is refused with `name must be 1 to 128 characters`
- **Priority:** Medium

#### E2E-094: an image embedding model needs a dimension too
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-009
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice creates a model of kind `image_embedding` without dimension
- **Then** the answer is `ERR_INVALID_ARGUMENT: dimension is required for kind 'image_embedding'`
- **And** with dimension 1024 it succeeds
- **Priority:** High

#### E2E-095: an update is checked as a whole
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-002, FR-NEW-003
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` (bearer, secret) and `Local GPU` exist
- **When** Alice renames `Cloud Embeddings` to `local gpu`, then sets its scheme to `none` without clearing the secret
- **Then** the answers are the duplicate name text and `auth_scheme 'none' takes no secret`
- **And** the provider is unchanged
- **Priority:** High

#### E2E-100: no default: the configuration is used
- **Category:** journey. **Scenario:** SC-010. **Requirements:** FR-NEW-017
- **Given** a platform with search in mode `both` configured with a stub embedding service C, an empty registry
- **When** a document is indexed and searched
- **Then** stub C received the calls
- **And** the search results are those the same platform gave before the upgrade
- **Priority:** High

#### E2E-101: clearing the default returns to the configuration
- **Category:** edge. **Scenario:** SC-010. **Requirements:** FR-NEW-017, FR-NEW-014
- **Given** as E2E-100 with a default text embedding model on stub A set
- **When** Alice clears the default and a document is indexed
- **Then** stub C received the call and stub A none
- **And** the previous entries remain searchable and no reranker is called
- **Priority:** Medium

#### E2E-102: a registry without defaults changes nothing
- **Category:** edge. **Scenario:** SC-010. **Requirements:** FR-NEW-017
- **Given** a platform with providers and models but no default, configuration embedding on stub C and reranking on stub R
- **When** a document is indexed and a reranked search is made
- **Then** stub C received the calls and stub R none
- **And** the search results equal those of the platform without the registry
- **Priority:** Medium

#### E2E-103: the upgrade creates the registry empty
- **Category:** journey. **Scenario:** SC-010. **Requirements:** FR-NEW-017, FR-NEW-007
- **Given** a platform started before the registry existed, then started with the new version
- **When** Alice lists providers and models
- **Then** both lists are empty and search works as before
- **And** no setting was imported from the configuration
- **Priority:** Medium

#### E2E-104: a provider whose model is a default cannot be deleted
- **Category:** failure. **Scenario:** SC-010. **Requirements:** FR-NEW-006, FR-NEW-011
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; a default text embedding model exists
- **When** Alice deletes the provider of the default model
- **Then** the answer is `ERR_INVALID_ARGUMENT: provider 'Local GPU' still has 1 model(s)`
- **And** searches keep using the default
- **Priority:** High

#### E2E-105: clearing the default of a kind that has none
- **Category:** edge. **Scenario:** SC-010. **Requirements:** FR-NEW-014
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice clears the default for `text_embedding` on an empty registry
- **Then** the answer is `{cleared: true}`
- **And** the record of changes holds no `clear_default`
- **Priority:** Medium

#### E2E-106: a changed model name is used by the next call
- **Category:** edge. **Scenario:** SC-010. **Requirements:** FR-NEW-010, FR-NEW-015
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; the default model is `qwen3-embedding-0.6b` on a stub
- **When** Alice renames the model to `qwen3-embedding-0.6b-v2` and a document is indexed
- **Then** the stub received the model name `qwen3-embedding-0.6b-v2`
- **And** the dimension was not touched
- **Priority:** Medium

#### E2E-107: a bearer reranker
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-016, FR-NEW-004
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; provider `Rerank Cloud` (bearer, secret `rk-1`) with the default reranker; a stub requiring `Authorization: Bearer rk-1`
- **When** a reranked search is made (mode `both`)
- **Then** the stub accepted the call and the answer order follows its scores
- **And** the secret appears in no answer
- **Priority:** Medium

#### E2E-108: a reranker that answers with an error
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-016
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; the default reranker's service answers HTTP 503
- **When** a reranked search is made (mode `both`)
- **Then** the search answers with the results in their unreranked order and no error
- **And** the failure does not change any setting
- **Priority:** High

#### E2E-109: a form token is single use
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-023
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** a browser session sends the provider form with a fresh token, then sends another form with the same token
- **Then** the first form is accepted; the second is refused with `ERR_FORBIDDEN` and `missing or invalid csrf_token`
- **And** a new page load issues a new token
- **Priority:** Medium

#### E2E-110: renaming a provider with other case of its own name
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-002
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice renames it to `LOCAL GPU`
- **Then** the rename succeeds (a provider does not clash with itself)
- **And** a rename to the name of another provider is refused
- **Priority:** Medium

#### E2E-111: valid inputs at once
- **Category:** journey. **Scenario:** SC-009. **Requirements:** FR-NEW-001, FR-NEW-008
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider and then a model with every field at its valid minimum and maximum
- **Then** both succeed with the values as given
- **And** the lists show them
- **Priority:** Medium

#### E2E-112: an endpoint with credentials
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-002, FR-NEW-001, FR-NEW-004
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a provider with endpoint `http://user:pw@host:9010/v1`
- **Then** the answer is `ERR_INVALID_ARGUMENT: endpoint must be an http or https URL`
- **And** the text `pw` appears in no answer, list, record, screen page or log
- **Priority:** High

#### E2E-113: a default embedding model with search mode bm25
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-015
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; project search mode `bm25`; a default text embedding model is set
- **When** a document is indexed and searched
- **Then** the search finds the document and the stub embedding service received no call
- **And** switching the search mode to `both` makes the next index call use the default
- **Priority:** Medium

#### E2E-114: reranking not asked for
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-016
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; a default reranker is set; search mode `both`
- **When** a search is made with reranking turned off
- **Then** the stub reranker received no call
- **And** the results are in their unreranked order
- **Priority:** Medium

#### E2E-115: an update that changes nothing
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-018
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice updates it with its current name and endpoint
- **Then** the answer is the provider unchanged, `updated_at` included
- **And** the record of changes holds no `update_provider` record for it
- **Priority:** Medium

#### E2E-116: giving the same dimension to a default
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-010
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry and a vector dimension of 1024; `qwen3-embedding-0.6b` (1024) is the default text embedding model
- **When** Alice updates it with `dimension` 1024
- **Then** the answer is the model unchanged
- **And** no record is written
- **Priority:** Medium

### 9.4 Modified tests
None.

### 9.5 Removed tests
None.

## 10. Traceability
| Scenario | Requirements | Happy | Failure | Edge |
|---|---|---|---|---|
| SC-001 | FR-NEW-001, FR-NEW-002, FR-NEW-004, FR-NEW-007, FR-NEW-008, FR-NEW-009, FR-NEW-012 | E2E-001, E2E-002, E2E-003 | E2E-004 | E2E-005, E2E-006 |
| SC-002 | FR-NEW-002, FR-NEW-003, FR-NEW-004, FR-NEW-005 | E2E-010, E2E-011 | E2E-012, E2E-013, E2E-014, E2E-015 | E2E-016, E2E-017 |
| SC-003 | FR-NEW-002, FR-NEW-003, FR-NEW-006, FR-NEW-010, FR-NEW-011, FR-NEW-013, FR-NEW-018 | E2E-020, E2E-023, E2E-025, E2E-027 | E2E-021, E2E-022, E2E-024, E2E-026, E2E-028, E2E-029 | E2E-110, E2E-115, E2E-116 |
| SC-004 | FR-NEW-004, FR-NEW-013, FR-NEW-014, FR-NEW-015, FR-NEW-016 | E2E-030, E2E-036, E2E-037, E2E-038, E2E-040 | E2E-032, E2E-033, E2E-034, E2E-035, E2E-041, E2E-108 | E2E-031, E2E-039, E2E-042, E2E-107, E2E-113, E2E-114 |
| SC-005 | FR-NEW-004, FR-NEW-007, FR-NEW-012, FR-NEW-014, FR-NEW-018, FR-NEW-019 | E2E-050, E2E-053, E2E-055, E2E-059 | E2E-052, E2E-057 | E2E-051, E2E-054, E2E-056, E2E-058 |
| SC-006 | FR-NEW-020 | E2E-063 | E2E-060, E2E-061 | E2E-062 |
| SC-007 | FR-NEW-001, FR-NEW-002, FR-NEW-007, FR-NEW-009, FR-NEW-020, FR-NEW-021 | E2E-070 | E2E-071, E2E-072 | E2E-073, E2E-074 |
| SC-008 | FR-NEW-001, FR-NEW-002, FR-NEW-004, FR-NEW-007, FR-NEW-008, FR-NEW-011, FR-NEW-013, FR-NEW-020, FR-NEW-022, FR-NEW-023 | E2E-080, E2E-081, E2E-084 | E2E-083, E2E-085, E2E-087 | E2E-082, E2E-086, E2E-109 |
| SC-009 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-NEW-004, FR-NEW-008, FR-NEW-009 | E2E-111 | E2E-090, E2E-092, E2E-094, E2E-095, E2E-112 | E2E-091, E2E-093 |
| SC-010 | FR-NEW-006, FR-NEW-007, FR-NEW-010, FR-NEW-011, FR-NEW-014, FR-NEW-015, FR-NEW-017 | E2E-100, E2E-103 | E2E-104 | E2E-101, E2E-102, E2E-105, E2E-106 |

Tests per requirement (at least 3 each):
- FR-NEW-001: E2E-001, E2E-002, E2E-005, E2E-070, E2E-081, E2E-091, E2E-111, E2E-112 (8 tests)
- FR-NEW-002: E2E-004, E2E-011, E2E-017, E2E-071, E2E-083, E2E-090, E2E-091, E2E-095, E2E-110, E2E-112 (10 tests)
- FR-NEW-003: E2E-010, E2E-011, E2E-015, E2E-020, E2E-021, E2E-095, E2E-110, E2E-115 (8 tests)
- FR-NEW-004: E2E-002, E2E-010, E2E-011, E2E-014, E2E-038, E2E-056, E2E-082, E2E-107, E2E-112 (9 tests)
- FR-NEW-005: E2E-012, E2E-013, E2E-016 (3 tests)
- FR-NEW-006: E2E-022, E2E-023, E2E-024, E2E-104 (4 tests)
- FR-NEW-007: E2E-001, E2E-050, E2E-051, E2E-052, E2E-054, E2E-070, E2E-080, E2E-103 (8 tests)
- FR-NEW-008: E2E-003, E2E-006, E2E-081, E2E-093, E2E-111 (5 tests)
- FR-NEW-009: E2E-006, E2E-071, E2E-092, E2E-093, E2E-094 (5 tests)
- FR-NEW-010: E2E-025, E2E-026, E2E-029, E2E-106, E2E-116 (5 tests)
- FR-NEW-011: E2E-026, E2E-027, E2E-028, E2E-084, E2E-104 (5 tests)
- FR-NEW-012: E2E-003, E2E-052, E2E-053, E2E-054 (4 tests)
- FR-NEW-013: E2E-028, E2E-029, E2E-030, E2E-031, E2E-032, E2E-033, E2E-034, E2E-035, E2E-042, E2E-084 (10 tests)
- FR-NEW-014: E2E-030, E2E-033, E2E-036, E2E-054, E2E-101, E2E-105 (6 tests)
- FR-NEW-015: E2E-037, E2E-038, E2E-039, E2E-041, E2E-042, E2E-106, E2E-113 (7 tests)
- FR-NEW-016: E2E-040, E2E-107, E2E-108, E2E-114 (4 tests)
- FR-NEW-017: E2E-100, E2E-101, E2E-102, E2E-103 (4 tests)
- FR-NEW-018: E2E-055, E2E-056, E2E-057, E2E-058, E2E-115 (5 tests)
- FR-NEW-019: E2E-052, E2E-054, E2E-055, E2E-058, E2E-059 (5 tests)
- FR-NEW-020: E2E-060, E2E-061, E2E-062, E2E-063, E2E-072, E2E-087 (6 tests)
- FR-NEW-021: E2E-070, E2E-071, E2E-072, E2E-073, E2E-074 (5 tests)
- FR-NEW-022: E2E-080, E2E-081, E2E-082, E2E-083, E2E-084, E2E-087 (6 tests)
- FR-NEW-023: E2E-085, E2E-086, E2E-109 (3 tests)

## 11. Open questions
None that change observable behavior.

## 12. Glossary (MANDATORY)
| Term | Definition | Context |
|---|---|---|
| Provider | a named service endpoint with an authentication scheme and an optional encrypted secret | Platform settings |
| Model | a named model of a provider, with a kind, an optional dimension and an optional input limit | Platform settings |
| Kind | `text_embedding`, `image_embedding`, `reranker`, `ocr` or `chat` | Platform settings |
| Authentication scheme | `none` (no authorization sent) or `bearer` (the secret sent as a bearer token) | Platform settings |
| Secret | the credential of a provider, stored encrypted, never shown after entry | Platform settings |
| Master key | the key that encrypts secrets, taken from the environment | Platform settings |
| Server default | the one text embedding model and the one reranker that search uses when set | Platform settings, Search |
| Record of changes | the durable list of every registry change without secrets | Platform settings |
| Platform administrator | a person listed as administrator in the configuration | Platform settings |
| Vector dimension | the width of the vectors the search engine stores | Search |
| Marker | the continuation token of a paged list | Platform settings |
| One time token | the token a browser form must carry, valid once | Platform settings |

## 13. Implementability gate (MANDATORY)
| Round | F | A | Verdict |
|---|---|---|---|
| 1 | 16 | 8 | NOT-IMPLEMENTABLE (service paths, reranker wiring, key start check, records content and order, check orders, endpoint rules, A: reranker not called today, REST admin routes exist, cross references) |
| 2 | 1 | 0 | NOT-IMPLEMENTABLE (REST and screen addresses) |
| 3 | 0 | 0 | IMPLEMENTABLE |

**Amendments applied:** every finding of rounds 1 and 2 is merged; E2E-112 to E2E-116 added; the public REST and screen addresses are stated in FR-NEW-021 and FR-NEW-022 as a contract with clients
**No implementation detail outside Section 14:** PASS

## 14. Evidence appendix (for /sdlc-design and the auditor, not for the product owner)
### 14.1 Current State evidence
| # | Claim (Section 2) | Evidence |
|---|---|---|
| 1 | embedding block: endpoint, model, key variable name, dimension | `crates/core/src/config.rs:783-785` (default dimension 1536), `:795-812` |
| 2 | reranking block | `crates/core/src/config.rs:818-835` |
| 3 | embedding call; a rerank function exists but no search path calls it; dimension frozen | `crates/core/src/search/embedding.rs:20`, `crates/core/src/search/rerank.rs:21` (HTTP error keeps the order, lines 45-47), `crates/core/src/mcp/server.rs:3318` and `crates/core/src/tools/search_semantic.rs:267` (the `rerank` flag is discarded), `.agent_docs/search.md:303-305` |
| 4 | 17 admin tools, none about settings | `crates/core/src/mcp/server.rs:2828-3284`, `crates/core/src/tools/admin.rs:4-25` |
| 5 | platform administrators come from the configuration; refusal text | `crates/core/src/config.rs:204,1245-1247`, `crates/core/src/state.rs:37-44` |
| 6 | audit buffer per session, capped | `crates/core/src/safety.rs:13` (capped at 500 entries), `:182`, `:190` |
| 7 | three screens, cookie plus one time token, refusal text | `crates/core/src/token_screen.rs:50-53,134,180,207-232`, `deleted_projects_screen.rs:44`, `trash_screen.rs:47` |
| 8 | cipher and master key variable | `crates/core/src/git/oauth/cipher.rs:20-75`, `crates/core/src/git/oauth/store.rs:26,110-123` |
| 9 | only three read only admin routes in REST; route table and OpenAPI table | `crates/core/src/api/dataplane.rs:45,96-103,126-159,177-179`, `crates/core/src/api/openapi.rs:71-75` |
| 10 | admin tool, screen and embedding tests | `crates/core/src/tools/admin.rs`, `crates/core/src/token_screen.rs`, `crates/core/src/search/e2e.rs` |
Test command: `make test` (`./test.sh`); admin and screen tests: `cargo test -p fsa-core --lib -- tools::admin token_screen`.

### 14.2 Class sweep
The document describes the storage of credentials and an access rule. Class: credentials entered by an administrator reaching a place where they can be read back (logs, answers, listings, records, screens, errors), CWE-522 and CWE-532. Searches to run by the design and the auditor: `rg -n 'secret|api_key|token' crates/core/src --glob '!*test*'` (answers and logs), `rg -n 'tracing::(info|warn|error|debug)!' crates/core/src/tools/admin.rs crates/core/src/logging.rs` (log lines). This increment adds a new store; no existing occurrence is in scope: the requirements make the new surface safe (FR-NEW-004, FR-NEW-005, FR-NEW-018, FR-NEW-020, FR-NEW-023), and E2E-014, E2E-056, E2E-080 are the exploit style tests (a secret value written in, then looked for in every output).

### 14.3 Design inputs
- Secrets need the cipher of the git token store (`git/oauth/cipher.rs`); that cipher is built lazily, at first use of the git store, from the key (`tools/git_auth.rs:43-49`, `git/oauth/store.rs:116-125`): a start time check of the key is new behavior (FR-NEW-005), and the registry needs the key independently of git. Signal: E2E-012, E2E-013.
- The search does not call the reranker today (`mcp/server.rs:3318`, `tools/search_semantic.rs:267`): FR-NEW-016 wires the call for the server default. Signal: E2E-040, E2E-102, E2E-114.
- Provider endpoints are base addresses; the embedding and rerank calls append `/embeddings` and `/rerank`, unlike the configured blocks that hold full URLs. Signal: E2E-037, E2E-040.
- Providers, models, defaults and the record of changes are four new persisted concepts of the global (admin) store; the vector dimension of the search engine is a startup value (`config.rs:803`) that the default check reads. Signal: E2E-034, E2E-042.
- Search builds its embedding and rerank clients at boot from the configuration (`search/mod.rs:271-325`); FR-NEW-015 and FR-NEW-016 require reading the defaults at call time. Signal: E2E-037, E2E-039, E2E-040.
- REST has three read only admin routes; the route table and the OpenAPI description must list the new ones (`api/dataplane.rs:45`, `api/openapi.rs:26`). Signal: E2E-070, E2E-074.
- A new screen follows `token_screen.rs`; the one time token store is private to each screen router (`token_screen.rs:62`). Signal: E2E-085, E2E-109.
- Tool count changes (12 new admin tools) and the contract files regenerate (`MCPFS_REWRITE_TOOL_CONTRACT=1`). Signal: the tool count tests.
- The master key is read from `FSA_TOKEN_KEY` (legacy `MCPFS_TOKEN_KEY` with a warning, `git/oauth/store.rs:26,29`). Signal: E2E-012, E2E-013, E2E-016.
