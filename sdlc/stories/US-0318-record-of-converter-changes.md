# US-0318: reading the record of converter changes

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0317
> min_tier: 2
> Files touched: 3
> Closes spec: no

## Objective
Read the record of converter changes through `admin.list_setting_changes`, newest first with increasing ids, refused to non administrators and without an entry for a refused attempt, kept across a restart.

## Requirements covered
- SPEC-0106/FR-NEW-016: The platform SHALL record every creation, update and deletion of a converter and every change of the platform default and of the converter of a project, with who did it, when, what it concerned and which fields changed, AND SHALL NOT record any value.
- SPEC-0106/FR-NEW-017 (partial: the record read): The platform SHALL allow every converter operation, reading included, to platform administrators only.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_ e2e_0102_`. Tests list the changes after a mix of operations, make 3 changes in the same second, list as a non administrator, restart and list again, and check the SPEC-0102 listing still passes.

## Design excerpt
### Decisions
- DEC-013 (SPEC-0106): Records use new `Target` constructors (`converter`, `default_converter`, `project_converter`), the actions of FR-NEW-016, field names in the order of the spec, and no record for a refused operation, an unchanged update or a repeated set or clear; the screen reads them through a `list_changes_of_kinds` store call. **Obliged by:** SPEC-0106/FR-NEW-016, FR-NEW-019. **Rationale:** one record table, one reader. **Alternatives considered:** a second table. **Source:** existing code. **Evidence:** `settings/registry.rs:651`, `settings/store.rs:335-346,444`.
- DEC-015 (SPEC-0106): Every tool function starts with `state.require_admin(person)` through the existing `registry()` helper; REST uses `run`; the screen follows the identity, administrator, token order of `models_screen.rs`. **Obliged by:** SPEC-0106/FR-NEW-017, FR-NEW-018, FR-NEW-019. **Rationale:** the refusal must come before body shape and existence checks. **Alternatives considered:** checking inside each method. **Source:** existing code. **Evidence:** `tools/models.rs:239-247`, `api/admin_settings.rs:44-70`, `models_screen.rs:62-110`.
### Data model
No schema change.
### Contracts
No new tool: `list_changes` already returns any `target_kind`; `SettingsStore::list_changes_of_kinds(kinds, limit)` is added for the screen (US-0320).
### Paths
| Path | Action |
|---|---|
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
| `crates/core/src/settings/store.rs` | modify only if the listing needs a fix |
### Existing patterns
`settings/registry.rs:642`, `settings/store.rs:444`. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-015: the record of changes lists the changes
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_015_*`)
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016
- **Given** Alice created `Pandoc local`, renamed it `Pandoc 3`, set it as default and deleted nothing
- **When** she lists the setting changes
- **Then** the three latest entries are, newest first, the default set, the update with the field `name`, the creation, each with `person` `alice@example.com`
- **Priority:** High

### SPEC-0106/E2E-086: the record keeps the order of changes
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_086_*`)
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016
- **Given** Alice makes 3 changes in the same second
- **When** she lists the changes
- **Then** they appear newest first with increasing change ids
- **Priority:** Medium

### SPEC-0106/E2E-102: only administrators read the record of changes
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_102_*`)
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-017
- **Given** changes exist; Bob is not a platform administrator
- **When** Bob lists the setting changes
- **Then** he is refused with `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **Priority:** Medium

### SPEC-0106/E2E-103: the record of changes survives a restart
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_103_*`)
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016
- **Given** 3 converter changes recorded
- **When** the platform restarts and Alice lists the changes
- **Then** the same 3 entries with the same ids appear
- **Priority:** Medium

### SPEC-0106/E2E-109: a refused change by a non administrator leaves no record
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_109_*`)
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-017
- **Given** Bob is not a platform administrator
- **When** Bob tries to create a converter, then Alice lists the changes
- **Then** the list holds no entry for Bob's attempt
- **Priority:** Medium

## Constraints
- No production change expected.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0318: reading the record of converter changes` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
