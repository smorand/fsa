# US-0288: the REST interface of the registry

> Spec: SPEC-0102 (sdlc/specs/SPEC-0102-platform-settings-and-model-registry/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0287
> min_tier: 2
> Files touched: 6
> Closes spec: no

## Objective
Offer every registry operation through the REST interface, with the same inputs, answers, refusals and access rule as the tools, listed in the OpenAPI description, calling the same registry code.

## Requirements covered
- SPEC-0102/FR-NEW-021: The platform SHALL offer every registry operation through its REST interface with the same inputs, answers, refusals and access rule as the tools.
- SPEC-0102/FR-NEW-020 (partial: the access rule on REST): The platform SHALL allow every registry operation, reading included, to platform administrators only.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0102_070 e2e_0102_071 e2e_0102_072 e2e_0102_073 e2e_0102_074 api::`. The tests drive the router with `oneshot` as Alice and as Bob, compare answers with the tool answers, check the error envelope and the OpenAPI listing, and check the routes are absent when `api.enabled` is false.

## Design excerpt
### Decisions
- DEC-006 (SPEC-0102): One `settings::Registry` holds every operation and rule; tools, REST handlers and the screen call it and add nothing but access control and shaping. **Obliged by:** SPEC-0102/FR-NEW-020, FR-NEW-021, FR-NEW-022. **Rationale:** the repository rule that MCP and REST share one implementation. **Alternatives considered:** per surface logic. **Source:** existing code. **Evidence:** `AGENTS.md` conventions (one implementation).
- DEC-013 (SPEC-0102): REST routes live in `api/admin_settings.rs`, mounted with the REST router, 12 rows added to `REST_ROUTES` and to the OpenAPI operation table; handlers authenticate with `person_of` and answer with `error_response`. **Obliged by:** SPEC-0102/FR-NEW-021. **Rationale:** same envelope and gating as the three existing admin routes. **Alternatives considered:** a separate prefix, no OpenAPI rows (the sync test forbids). **Source:** existing code. **Evidence:** `api/dataplane.rs:45,177-179,207,259`, `api/openapi.rs:440,1696-1710`.
### Data model
No schema change.
### Contracts
Routes of design 5.3 (12), JSON bodies equal to the tool inputs, answers equal to the tool answers, `PUT /api/admin/defaults/{kind}` body `{model_id}`, `GET /api/admin/models?provider_id&kind&marker`; identity `person_of`; refusals `{error: <code>, detail: <text>}` with the status of `ToolError::http_status()`; `REST_ROUTES` 49 to 61 and 12 OpenAPI `Op` rows; descriptions come from the tools.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/api/admin_settings.rs`, `api/mod.rs` | create / modify: handlers and router |
| `crates/core/src/api/dataplane.rs` | modify: `REST_ROUTES` rows and the count test |
| `crates/core/src/api/openapi.rs` | modify: 12 `Op` rows |
| `crates/core/src/settings/e2e.rs` | modify: these tests |
### Existing patterns
Existing admin routes `api/dataplane.rs:177-179` and handler `:410`, `person_of` `:207`, `error_response` `:259`, OpenAPI table `api/openapi.rs:440,1202`, sync tests `:1696-1710`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0102/E2E-070: the REST interface creates and lists
- Level / Driver / Location: functional e2e / REST router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_070_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice, through the REST interface, creates the provider `Local GPU` and lists the providers
- **Then** the answers equal those of E2E-001
- **And** the provider is also listed by the tool `admin.list_providers`
- **Priority:** High

### SPEC-0102/E2E-071: the REST interface refuses like the tools
- Level / Driver / Location: functional e2e / REST router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_071_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** through the REST interface Alice creates a duplicate provider, a model with dimension 0 and a model of an unknown provider
- **Then** the refusals carry the same codes and texts as the tools: a duplicate, `dimension must be between 1 and 8192`, `provider not found`
- **And** nothing was created
- **Priority:** High

### SPEC-0102/E2E-072: the REST interface refuses a member
- Level / Driver / Location: functional e2e / REST router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_072_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Bob, through the REST interface, lists providers and creates one
- **Then** each answer is the refusal `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** nothing was created
- **Priority:** High

### SPEC-0102/E2E-073: a change through one interface is visible through the other
- Level / Driver / Location: functional e2e / REST router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_073_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice creates a model with the tool, edits it through the REST interface, and reads it with the tool
- **Then** the tool shows the edit at once
- **And** the list of changes holds both records
- **Priority:** Medium

### SPEC-0102/E2E-074: the REST description lists the operations
- Level / Driver / Location: functional e2e / REST router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_074_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** a client reads the REST description of the platform
- **Then** it lists providers, models, defaults and changes operations with their inputs
- **And** the operations are absent when the REST interface is disabled by configuration
- **Priority:** Medium

## Constraints
- No logic in the handlers beyond access control and shaping.
- Do not change existing routes.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/api.md` (the admin routes).
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no secret value in any output; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0288: the REST interface of the registry` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0102` and the requirement ids.
