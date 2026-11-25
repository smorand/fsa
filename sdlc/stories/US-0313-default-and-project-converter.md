# US-0313: the platform default and the converter of a project

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0312
> min_tier: 1
> Files touched: 9
> Closes spec: no

## Objective
Choose the platform default converter and the converter of a project, read and clear them, keep a project in its grace period selectable, and drop the selection when the project is removed for good.

## Requirements covered
- SPEC-0106/FR-NEW-008: WHEN a platform administrator sets, clears or reads the platform default converter THE platform SHALL keep one default at most and answer with it.
- SPEC-0106/FR-NEW-009: WHEN a platform administrator sets, clears or reads the converter of a project THE platform SHALL keep at most one converter for that project and answer with it.
- SPEC-0106/FR-NEW-021: WHEN a project is removed for good THE platform SHALL drop its converter selection, AND a project removed and restored within its grace period SHALL keep it.
- SPEC-0106/FR-NEW-016 (partial: default and project records): The platform SHALL record every creation, update and deletion of a converter and every change of the platform default and of the converter of a project, with who did it, when, what it concerned and which fields changed, AND SHALL NOT record any value.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_ production_ddl tool_contract`. Tests set, replace, read and clear the default and a project converter, refuse unknown ids in the stated order, set for a soft removed project, remove a project for good and recreate it, and check the records.

## Design excerpt
### Decisions
- DEC-003 (SPEC-0106): Tables `converter` and `converter_default`, and the nullable column `project.converter_id` with a `ColumnMigration`; the name is unique through `name_key` (the same caseless key as providers); ids are `conv-` plus 12 hex digits of a v4 uuid; update checks the name key ignoring the converter being updated. **Obliged by:** SPEC-0106/FR-NEW-001, FR-NEW-005, FR-NEW-008, FR-NEW-009, FR-NEW-021. **Rationale:** a column dies with its project row, which gives FR-NEW-021 with no code; a soft removed row keeps it. **Alternatives considered:** a `project_converter` table with a foreign key cascade (an extra join for the usage count). **Source:** existing code. **Evidence:** `storage/admin.rs:104-183,397-403`, `settings/registry.rs:226-229`.
- DEC-013 (SPEC-0106): Records use new `Target` constructors (`converter`, `default_converter`, `project_converter`), the actions of FR-NEW-016, field names in the order of the spec, and no record for a refused operation, an unchanged update or a repeated set or clear; the screen reads them through a `list_changes_of_kinds` store call. **Obliged by:** SPEC-0106/FR-NEW-016, FR-NEW-019. **Rationale:** one record table, one reader. **Alternatives considered:** a second table. **Source:** existing code. **Evidence:** `settings/registry.rs:651`, `settings/store.rs:335-346,444`.
- DEC-014 (SPEC-0106): The project selection is read and written through `SettingsStore` on the same database handle; project existence is a row in `project` whatever `deleted_at` holds; the usage count is a count of rows with the converter id. **Obliged by:** SPEC-0106/FR-NEW-007, FR-NEW-009, FR-NEW-021. **Rationale:** a soft removed project still uses its converter. **Alternatives considered:** going through `AdminBackend` project calls (they hide soft removed rows). **Source:** existing code. **Evidence:** `storage/admin.rs:26-35,397-403`.
- DEC-015 (SPEC-0106): Every tool function starts with `state.require_admin(person)` through the existing `registry()` helper; REST uses `run`; the screen follows the identity, administrator, token order of `models_screen.rs`. **Obliged by:** SPEC-0106/FR-NEW-017, FR-NEW-018, FR-NEW-019. **Rationale:** the refusal must come before body shape and existence checks. **Alternatives considered:** checking inside each method. **Source:** existing code. **Evidence:** `tools/models.rs:239-247`, `api/admin_settings.rs:44-70`, `models_screen.rs:62-110`.
- DEC-017 (SPEC-0106): The ten tool names go in `tools/admin.rs` name list and `mcp/server.rs` wrappers, with description constants checked by the existing equality test; REST rows go in `REST_ROUTES` and `OPERATIONS`; counts and the golden are updated in the story that adds them. **Obliged by:** SPEC-0106/FR-NEW-018. **Rationale:** the contract tests fail otherwise. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `tools/models.rs:19-40`, `tools/admin.rs:975-1008`, `api/dataplane.rs:45,110-124`, `api/openapi.rs:521`.
### Data model
New table `converter_default(slot, converter_id)`; new nullable column `project.converter_id` with a `ColumnMigration`; the fingerprints change again.
### Contracts
Tools `admin.set_default_converter(converter_id)`, `admin.clear_default_converter()`, `admin.get_default_converter()` answering `{converter}`; `admin.set_project_converter(project_id, converter_id)`, `admin.clear_project_converter(project_id)`, `admin.get_project_converter(project_id)` answering `{project_id, converter}`. Orders: administrator, project found (a row in `project` whatever `deleted_at`), converter found. Records `set_default_converter`, `clear_default_converter`, `set_project_converter`, `clear_project_converter` with `target_kind` `default_converter` or `project_converter`; a clear of nothing and a repeated set are not recorded.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/admin.rs`, `crates/core/src/storage/rel/schema.rs` | modify: table, column migration, fingerprints |
| `crates/core/src/settings/converters.rs`, `converter_store.rs`, `settings/store.rs` | modify |
| `crates/core/src/tools/converters.rs`, `crates/core/src/mcp/server.rs`, `tools/admin.rs` | modify: six tools |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json` (regenerated with `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`) and the tool count assertions listed in the story constraints | modify |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
### Existing patterns
Column migrations `storage/admin.rs:183`; hard removal `storage/admin.rs:397-403`; soft removal `deleted_at`. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-004: the platform default is read
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_004_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-008
- **Given** no default set
- **When** Alice reads the default, sets `Pandoc local` as default, reads it again
- **Then** the first answer holds no converter (`null`), the second holds `Pandoc local` with its id
- **Priority:** High

### SPEC-0106/E2E-006: the platform default is set and replaced
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_006_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-008
- **Given** `Pandoc local` and `Docling API` exist
- **When** Alice sets `Pandoc local` as default, then `Docling API`
- **Then** reading the default answers `Docling API`
- **Priority:** High

### SPEC-0106/E2E-007: a project converter is set and read
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_007_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** `Docling API` exists and project `proj` exists
- **When** Alice sets `Docling API` as the converter of `proj` and reads it
- **Then** the answer holds `Docling API` and the project id `proj`
- **Priority:** High

### SPEC-0106/E2E-046: a default that does not exist
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_046_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-008
- **Given** no converter exists
- **When** Alice sets `conv-000000000000` as the default
- **Then** she is refused with `ERR_NOT_FOUND: converter not found: conv-000000000000`
- **Priority:** High

### SPEC-0106/E2E-047: a project that does not exist
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_047_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists
- **When** Alice sets `Docling API` as the converter of `ghost`
- **Then** she is refused with `ERR_PROJECT_NOT_FOUND: project 'ghost' not found`
- **Priority:** High

### SPEC-0106/E2E-048: a project converter that does not exist
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_048_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** no converter exists
- **When** Alice sets `conv-000000000000` as the converter of `proj`
- **Then** she is refused with `ERR_NOT_FOUND: converter not found: conv-000000000000`
- **Priority:** High

### SPEC-0106/E2E-069: clearing the project converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_069_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** `proj` has a converter
- **When** Alice clears it and reads it
- **Then** the answer holds no converter and listing shows the converter unused (deletable)
- **Priority:** Medium

### SPEC-0106/E2E-071: a project removed for good loses its selection
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_071_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-021
- **Given** `proj` has converter B; the platform removes `proj` for good after its grace period
- **When** Alice deletes B
- **Then** B is deleted (it is used by no project); a recreated `proj` has no converter
- **Priority:** Medium

### SPEC-0106/E2E-072: a project soft deleted and restored keeps its converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_072_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-021
- **Given** `proj` has converter B and is removed then restored within its grace period
- **When** Alice reads the converter of `proj`
- **Then** it is B
- **Priority:** Medium

### SPEC-0106/E2E-074: an administrator who is not a member can select but not read files
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_074_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009, FR-NEW-017
- **Given** Alice is a platform administrator and not a member of `proj`
- **When** Alice sets a converter for `proj` and lists the files of `proj`
- **Then** the selection succeeds and the file listing is refused with `ERR_FORBIDDEN: 'alice@example.com' is not a member of 'proj'`
- **Priority:** Medium

### SPEC-0106/E2E-108: selecting a converter changes no file
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_108_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** `proj` holds 3 files
- **When** Alice sets and clears a converter for `proj`
- **Then** the 3 files and their access times are unchanged
- **Priority:** Low

### SPEC-0106/E2E-119: each kind of change is recorded with its target
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_119_*`)
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-008, FR-NEW-009
- **Given** `Pandoc local` exists and `proj` exists
- **When** Alice sets it as the default, clears the default, sets it for `proj` and clears it
- **Then** the four records show `set_default_converter`, `clear_default_converter`, `set_project_converter`, `clear_project_converter`, with `target_kind` `default_converter`, `default_converter`, `project_converter`, `project_converter`, `target_id` the converter id or the project id, `target_name` the converter name, and empty `fields`
- **Priority:** High

### SPEC-0106/E2E-120: a clear of nothing is answered null and not recorded
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_120_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-008, FR-NEW-009, FR-NEW-016
- **Given** no default and no converter for `proj`
- **When** Alice clears the default and clears the converter of `proj`
- **Then** each answers `converter` null and the record gained no entry
- **Priority:** Medium

### SPEC-0106/E2E-123: an unknown project is refused before an unknown converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_123_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** no project `ghost`
- **When** Alice sets `conv-000000000000` as the converter of `ghost`
- **Then** she is refused with `ERR_PROJECT_NOT_FOUND: project 'ghost' not found`
- **Priority:** Medium

### SPEC-0106/E2E-136: a project in its grace period can still be given a converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_136_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009, FR-NEW-021
- **Given** `proj` is removed within its grace period
- **When** Alice sets, reads and clears a converter for `proj`
- **Then** each succeeds; for a project removed for good each is refused with `ERR_PROJECT_NOT_FOUND: project 'proj' not found`
- **Priority:** Medium

## Constraints
- Count assertions to raise by the number of tools this story adds: `tools/admin.rs:1008` (admin 29), `tools/all.rs:75,92,145,164`, `tools/contract_golden.rs:152,275,331,345` (136), `mcp/server.rs:4587-4591,4928,4949-4950`, `tools/tool_surface_e2e.rs:455`, and the inline counts in `README.md:4,659`. Add 6.
- A hard removed project loses its selection with its row; do not add code for that.
- Do not let a deleted converter name leak in a refusal for a non administrator.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0313: the platform default and the converter of a project` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
