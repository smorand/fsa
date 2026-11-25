# US-0320: the converters screen

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0319
> min_tier: 2
> Files touched: 3
> Closes spec: no

## Objective
Add the `/app/converters` screen: list, create, edit, delete, default and project selection with one time tokens, the secret state only, a command text area and a clear secret checkbox.

## Requirements covered
- SPEC-0106/FR-NEW-019: WHEN a platform administrator opens the converters screen THE platform SHALL show the converters, the platform default, each project's converter and the latest changes, and SHALL let the administrator create, edit, delete, set the default and set each project's converter, with a one time token on each form.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_`. Tests drive the screen with `open` and `submit`: create with the token, refuse without or with a wrong token, a non administrator, show `Secret stored`, keep the secret on an empty field, clear it, blank the response field and extensions, show the refusal text with its status, list the 20 latest converter changes.

## Design excerpt
### Decisions
- DEC-013 (SPEC-0106): Records use new `Target` constructors (`converter`, `default_converter`, `project_converter`), the actions of FR-NEW-016, field names in the order of the spec, and no record for a refused operation, an unchanged update or a repeated set or clear; the screen reads them through a `list_changes_of_kinds` store call. **Obliged by:** SPEC-0106/FR-NEW-016, FR-NEW-019. **Rationale:** one record table, one reader. **Alternatives considered:** a second table. **Source:** existing code. **Evidence:** `settings/registry.rs:651`, `settings/store.rs:335-346,444`.
- DEC-015 (SPEC-0106): Every tool function starts with `state.require_admin(person)` through the existing `registry()` helper; REST uses `run`; the screen follows the identity, administrator, token order of `models_screen.rs`. **Obliged by:** SPEC-0106/FR-NEW-017, FR-NEW-018, FR-NEW-019. **Rationale:** the refusal must come before body shape and existence checks. **Alternatives considered:** checking inside each method. **Source:** existing code. **Evidence:** `tools/models.rs:239-247`, `api/admin_settings.rs:44-70`, `models_screen.rs:62-110`.
- DEC-020 (SPEC-0106): The screen is a copy of the `models_screen.rs` structure with its own `CsrfStore`, a `command` text area parsed one argument per line, a `clear_secret` checkbox and an always sent `response_field` and `extensions` on edit. **Obliged by:** SPEC-0106/FR-NEW-019. **Rationale:** same conventions. **Alternatives considered:** sharing one token store across screens (proposal, not decided here). **Source:** existing code. **Evidence:** `models_screen.rs:37-110`.
### Data model
No schema change.
### Contracts
Routes `GET /app/converters`, `POST /app/converters/<action>` with `converter/create`, `converter/update`, `converter/delete`, `default/set`, `default/clear`, `project/set`, `project/clear`; success redirects to `/app/converters`; refusal re-renders with the status of the error code.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/converters_screen.rs`, `crates/core/src/lib.rs`, `crates/core/src/app.rs` | create / modify |
| `crates/core/src/settings/store.rs` | modify: `list_changes_of_kinds` |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
### Existing patterns
`models_screen.rs:37-110`, `token_screen` one time token helpers. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-014: the screen creates a converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_014_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019
- **Given** Alice opens the converters screen in a browser session
- **When** she fills the form for `Docling API` (form `http`, the url, the secret `Bearer tok-123`) with the one time token and submits
- **Then** the screen lists `Docling API` with `Secret stored` and no secret text
- **Priority:** High

### SPEC-0106/E2E-052: the screen refuses a non administrator
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_052_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-017, FR-NEW-019
- **Given** Bob opens the converters screen and posts a creation
- **When** the screen answers
- **Then** both are refused with the platform administrator refusal and nothing is created
- **Priority:** High

### SPEC-0106/E2E-053: a form without its one time token
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_053_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019
- **Given** Alice is in a browser session
- **When** she posts the creation form without a token, then with a token already used, then with a token issued to Bob
- **Then** each is refused with `ERR_FORBIDDEN: missing or invalid csrf_token` and nothing is created
- **Priority:** High

### SPEC-0106/E2E-088: the screen shows the secret state only
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_088_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret
- **When** Alice opens the screen and its edit form
- **Then** the screen shows `Secret stored`, the secret field is empty, and leaving it empty on save keeps the secret
- **Priority:** High

### SPEC-0106/E2E-089: the screen shows the refusal text
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_089_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019
- **Given** Alice is in a browser session
- **When** she creates a converter with the name `   ` on the screen
- **Then** the screen is shown again with the status of a refused request and the text `name must be 1 to 64 characters`, and creates nothing; a creation that succeeds redirects to the converters screen
- **Priority:** Medium

### SPEC-0106/E2E-090: a change through one interface is visible through the others
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_090_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-018, FR-NEW-019
- **Given** no converter
- **When** Alice creates through the screen, then lists through the tool and the web interface
- **Then** the converter is listed by all three
- **Priority:** High

### SPEC-0106/E2E-100: the screen renames a converter without changing its id
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_100_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of `proj`; Alice is in a browser session
- **When** she renames it `Docling v2` on the screen
- **Then** the screen shows the new name and `proj` still shows it as its converter
- **Priority:** Medium

### SPEC-0106/E2E-115: the screen creates a program converter from text fields
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_115_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-002
- **Given** Alice is in a browser session
- **When** she enters the name `Pandoc local`, form `program`, a command text area holding the lines `pandoc-convert`, a blank line and `{document}`, and submits with the token
- **Then** the converter is created with the command `["pandoc-convert", "{document}"]`
- **Priority:** High

### SPEC-0106/E2E-139: the screen clears a secret
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_139_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret; Alice is in a browser session
- **When** she ticks `Clear the secret` on its edit form and saves
- **Then** the screen shows `No secret` and the listing shows `has_secret` false
- **Priority:** Medium

### SPEC-0106/E2E-140: the screen shows the 20 latest converter changes
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_140_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-016
- **Given** 21 converter changes and 3 model registry changes were made, the model changes being the newest
- **When** Alice opens the screen
- **Then** it shows the 20 most recent converter changes, newest first, the oldest one is missing, and no model registry change appears
- **Priority:** Low

### SPEC-0106/E2E-142: the screen empties the response field and the extensions
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_142_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has `response_field` `markdown` and `extensions` `["pdf"]`; Alice is in a browser session
- **When** she blanks both fields on the edit form and saves
- **Then** the converter now has an empty `response_field` and an empty `extensions` list, and its url is unchanged
- **Priority:** Medium

## Constraints
- Secret inputs never carry a value.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0320: the converters screen` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
