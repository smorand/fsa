# US-0289: the models screen, docs and contract

> Spec: SPEC-0102 (sdlc/specs/SPEC-0102-platform-settings-and-model-registry/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0288
> min_tier: 1
> Files touched: 8
> Closes spec: yes

## Objective
Add the models screen for platform administrators: providers, models, defaults and the latest changes, forms for every operation, secrets never shown, forms protected by the one time token of the other screens. Close the spec: docs, contract and the whole acceptance set.

## Requirements covered
- SPEC-0102/FR-NEW-022: WHEN a platform administrator opens the models screen THE platform SHALL show the providers, their models, the server defaults and the latest changes, and SHALL let the administrator create, edit and delete providers and models and set and clear the defaults.
- SPEC-0102/FR-NEW-023: IF a form of the models screen is sent without a valid one time token issued to the same person THEN THE platform SHALL refuse it and change nothing.
- SPEC-0102/FR-NEW-020 (partial: the access rule on the screen): The platform SHALL allow every registry operation, reading included, to platform administrators only.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0102_ e2e_0101_ e2e_0100_` and `make check`. The screen tests drive the router with `oneshot` using the cookie and header identities, fetch a token from the page, submit forms and read the page back; no page ever contains a secret.

## Design excerpt
### Decisions
- DEC-005 (SPEC-0102): A `SecretText` newtype (no `Display`, `Debug` prints `Secret(set)`) carries secrets in memory; answer structs have `has_secret` only; refusal texts are built from names and numbers, never from request bodies; the registry tools log nothing of their arguments. **Obliged by:** SPEC-0102/FR-NEW-004. **Rationale:** no output path can format a secret by accident. **Alternatives considered:** discipline only. **Source:** design. **Evidence:** `config.rs:31,51` (`Dsn` redaction precedent).
- DEC-006 (SPEC-0102): One `settings::Registry` holds every operation and rule; tools, REST handlers and the screen call it and add nothing but access control and shaping. **Obliged by:** SPEC-0102/FR-NEW-020, FR-NEW-021, FR-NEW-022. **Rationale:** the repository rule that MCP and REST share one implementation. **Alternatives considered:** per surface logic. **Source:** existing code. **Evidence:** `AGENTS.md` conventions (one implementation).
- DEC-014 (SPEC-0102): The models screen reuses the token screen's `CsrfStore`, `IdentitySource`, `resolve_person`, `check_csrf`, `forbidden_csrf` and `escape_html` by making them `pub(crate)`; each router owns its own store instance. **Obliged by:** SPEC-0102/FR-NEW-022, FR-NEW-023. **Rationale:** same behavior and refusal text without a third copy. **Alternatives considered:** copy as `deleted_projects_screen.rs` did (duplicates security code), move to a shared module (touches three screens). **Source:** existing code. **Evidence:** `token_screen.rs:60-232,308,344`.
### Data model
No schema change.
### Contracts
`GET /app/models` page and `POST /app/models/{provider/create, provider/update, provider/delete, model/create, model/update, model/delete, default/set, default/clear}` form posts (form fields named like the tool inputs plus `csrf_token`); the page shows providers (`Secret stored` or `No secret`), models under providers, defaults, the latest 20 changes; a refusal is rendered with its text; a missing, used or foreign token answers 403 `{"error":"ERR_FORBIDDEN","detail":"missing or invalid csrf_token"}`; header identity needs no token; an ordinary member gets `ERR_FORBIDDEN: '<person>' is not a platform admin`; token is single use and a page load issues a new one.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/models_screen.rs`, `lib.rs`, `app.rs` | create / modify: router mounted always |
| `crates/core/src/token_screen.rs` | modify: items `pub(crate)` (no behavior change) |
| `crates/core/src/settings/e2e.rs` | modify: these tests |
| `README.md`, `AGENTS.md`, `.agent_docs/{tools,search,architecture,config,api,testing}.md` | modify: registry section, tool counts (135), routes, screen, test command `cargo test -p fsa-core --lib -- e2e_0102_` |
### Existing patterns
`token_screen.rs:49-344` (router, `ScreenState`, `CsrfStore`, `resolve_person`, `check_csrf`, `page_shell`, `escape_html`), `deleted_projects_screen.rs:44` for an unconditional mount.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0102/E2E-080: the models screen shows the registry
- Level / Driver / Location: functional e2e / screen router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_080_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; provider `Cloud Embeddings` (secret stored), `Local GPU`, two models and a default
- **When** Alice opens the models screen in her browser
- **Then** she sees both providers with endpoint and scheme, `Secret stored` for `Cloud Embeddings` and `No secret` for `Local GPU`, the models under their providers, the defaults and the latest changes
- **And** no secret value appears in the page
- **Priority:** Critical

### SPEC-0102/E2E-081: creating a provider and a model on the screen
- Level / Driver / Location: functional e2e / screen router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_081_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice submits the provider form (`Local GPU`, endpoint, scheme `none`) then the model form (`qwen3-embedding-0.6b`, kind `text_embedding`, dimension 1024)
- **Then** both appear on the screen after each submit
- **And** the tool lists both
- **Priority:** Critical

### SPEC-0102/E2E-082: editing keeps the secret when the field is empty
- Level / Driver / Location: functional e2e / screen router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_082_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Cloud Embeddings` holds a secret
- **When** Alice edits its endpoint on the screen and leaves the secret field empty
- **Then** the endpoint changes and the provider still shows `Secret stored`
- **And** the edit form's secret field was empty before submit
- **Priority:** Medium

### SPEC-0102/E2E-083: a refused action is explained on the screen
- Level / Driver / Location: functional e2e / screen router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_083_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; `Local GPU` exists
- **When** Alice submits the provider form with the name `local gpu`
- **Then** the page shows `a provider named 'local gpu' already exists`
- **And** nothing was created
- **Priority:** High

### SPEC-0102/E2E-084: deleting and defaults on the screen
- Level / Driver / Location: functional e2e / screen router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_084_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; a model of dimension 1024 and a vector dimension of 1024
- **When** Alice sets the model as the default on the screen, tries to delete it, clears the default, then deletes it
- **Then** the deletion is first refused with the text of FR-NEW-011 on the page, then succeeds
- **And** the model is gone from the screen
- **Priority:** High

### SPEC-0102/E2E-085: a form without a valid token
- Level / Driver / Location: functional e2e / screen router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_085_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** a browser session sends the provider form without its one time token, then with a token already used, then with a token issued to Bob
- **Then** each answer is `ERR_FORBIDDEN` with `missing or invalid csrf_token`
- **And** nothing was created
- **Priority:** High

### SPEC-0102/E2E-086: a request carrying the identity in a header needs no token
- Level / Driver / Location: functional e2e / screen router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_086_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Alice's client sends the provider form with the identity in a header and no token
- **Then** the provider is created
- **And** a cookie session without token is refused (E2E-085)
- **Priority:** Medium

### SPEC-0102/E2E-087: the screen refuses an ordinary member
- Level / Driver / Location: functional e2e / screen router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_087_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** Bob opens the models screen and sends its forms
- **Then** he gets `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** nothing was created or revealed
- **Priority:** High

### SPEC-0102/E2E-109: a form token is single use
- Level / Driver / Location: functional e2e / screen router with `oneshot` / `crates/core/src/settings/e2e.rs` (`e2e_0102_109_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry
- **When** a browser session sends the provider form with a fresh token, then sends another form with the same token
- **Then** the first form is accepted; the second is refused with `ERR_FORBIDDEN` and `missing or invalid csrf_token`
- **And** a new page load issues a new token
- **Priority:** Medium

## Constraints
- Escape every value written to HTML.
- Secret inputs are password fields and never prefilled.
- Tool counts must match everywhere (rg the previous totals).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: All of `README.md`, `AGENTS.md`, `.agent_docs/*` named in Paths; no dashes as punctuation.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no secret value in any output; no dash used as punctuation in code, comments or docs.
- [ ] Closes spec: the spec full acceptance set green (every E2E-001 to E2E-116) and every requirement of SPEC-0102 cited by at least one story with status done or landed; regenerate the tool contract and review the diff.
- [ ] One commit `US-0289: the models screen, docs and contract` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0102` and the requirement ids.
