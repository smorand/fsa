# US-0319: the web routes for converters

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0318
> min_tier: 2
> Files touched: 4
> Closes spec: no

## Objective
Offer every converter operation through ten `/api/admin` routes with the same inputs, answers, refusals and access rule as the tools.

## Requirements covered
- SPEC-0106/FR-NEW-018: The platform SHALL offer every converter operation through its web interface with the same inputs, answers, refusals and access rule as the tools.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_ api::dataplane api::openapi`. Tests create and list through the web and the tool and compare the answers, refuse a non administrator, compare refusal texts and page 101 converters.

## Design excerpt
### Decisions
- DEC-015 (SPEC-0106): Every tool function starts with `state.require_admin(person)` through the existing `registry()` helper; REST uses `run`; the screen follows the identity, administrator, token order of `models_screen.rs`. **Obliged by:** SPEC-0106/FR-NEW-017, FR-NEW-018, FR-NEW-019. **Rationale:** the refusal must come before body shape and existence checks. **Alternatives considered:** checking inside each method. **Source:** existing code. **Evidence:** `tools/models.rs:239-247`, `api/admin_settings.rs:44-70`, `models_screen.rs:62-110`.
- DEC-017 (SPEC-0106): The ten tool names go in `tools/admin.rs` name list and `mcp/server.rs` wrappers, with description constants checked by the existing equality test; REST rows go in `REST_ROUTES` and `OPERATIONS`; counts and the golden are updated in the story that adds them. **Obliged by:** SPEC-0106/FR-NEW-018. **Rationale:** the contract tests fail otherwise. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `tools/models.rs:19-40`, `tools/admin.rs:975-1008`, `api/dataplane.rs:45,110-124`, `api/openapi.rs:521`.
### Data model
No schema change.
### Contracts
`POST|GET /api/admin/converters`, `PATCH|DELETE /api/admin/converters/{converter_id}`, `GET|PUT|DELETE /api/admin/converter-default`, `GET|PUT|DELETE /api/admin/projects/{project_id}/converter`; the same status codes as the existing admin routes; `REST_ROUTES` gains 10 rows (63 to 73) and `OPERATIONS` the same rows.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/api/admin_converters.rs`, `api/mod.rs` | create / modify |
| `crates/core/src/api/dataplane.rs`, `crates/core/src/api/openapi.rs` | modify: route tables and counts |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
### Existing patterns
`api/admin_settings.rs:24-70`, `api/dataplane.rs:45,110-124,2845`, `api/openapi.rs:521,1910`. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-013: REST creates and lists like the tools
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_013_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-018
- **Given** no converter exists
- **When** Alice creates `Pandoc local` through the web interface and lists through the tool and through the web interface
- **Then** both listings are identical and the creation answer equals the tool's answer for the same input
- **Priority:** High

### SPEC-0106/E2E-051: the web interface refuses a non administrator the same way
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_051_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-017, FR-NEW-018
- **Given** Bob is not a platform administrator
- **When** Bob sends a creation and a listing through the web interface
- **Then** each is refused with `ERR_FORBIDDEN` and the detail `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **Priority:** High

### SPEC-0106/E2E-058: a web refusal equals the tool refusal
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_058_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-018
- **Given** no converter exists
- **When** Alice updates `conv-000000000000` through the web interface and through the tool
- **Then** the error code and the text are the same
- **Priority:** Medium

### SPEC-0106/E2E-099: the web interface pages like the tool
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_099_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-018, FR-NEW-006
- **Given** 101 converters
- **When** Alice lists through the web interface and follows its marker
- **Then** she gets 100 then 1, identical to the tool's pages
- **Priority:** Medium

## Constraints
- The handlers only call the tool functions.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0319: the web routes for converters` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
