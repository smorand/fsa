# US-0314: delete a converter

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0313
> min_tier: 2
> Files touched: 4
> Closes spec: no

## Objective
Delete a converter unless it is the platform default or the converter of a project (a project in its grace period counts, one removed for good does not).

## Requirements covered
- SPEC-0106/FR-NEW-007: WHEN a platform administrator deletes a converter THE platform SHALL remove it unless it is the platform default or the converter of a project.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_ tool_contract`. Tests delete an unused converter, refuse the default, a used converter (counting 1 and 2 projects), an unknown id, a soft removed project, and delete after clearing.

## Design excerpt
### Decisions
- DEC-014 (SPEC-0106): The project selection is read and written through `SettingsStore` on the same database handle; project existence is a row in `project` whatever `deleted_at` holds; the usage count is a count of rows with the converter id. **Obliged by:** SPEC-0106/FR-NEW-007, FR-NEW-009, FR-NEW-021. **Rationale:** a soft removed project still uses its converter. **Alternatives considered:** going through `AdminBackend` project calls (they hide soft removed rows). **Source:** existing code. **Evidence:** `storage/admin.rs:26-35,397-403`.
- DEC-015 (SPEC-0106): Every tool function starts with `state.require_admin(person)` through the existing `registry()` helper; REST uses `run`; the screen follows the identity, administrator, token order of `models_screen.rs`. **Obliged by:** SPEC-0106/FR-NEW-017, FR-NEW-018, FR-NEW-019. **Rationale:** the refusal must come before body shape and existence checks. **Alternatives considered:** checking inside each method. **Source:** existing code. **Evidence:** `tools/models.rs:239-247`, `api/admin_settings.rs:44-70`, `models_screen.rs:62-110`.
- DEC-017 (SPEC-0106): The ten tool names go in `tools/admin.rs` name list and `mcp/server.rs` wrappers, with description constants checked by the existing equality test; REST rows go in `REST_ROUTES` and `OPERATIONS`; counts and the golden are updated in the story that adds them. **Obliged by:** SPEC-0106/FR-NEW-018. **Rationale:** the contract tests fail otherwise. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `tools/models.rs:19-40`, `tools/admin.rs:975-1008`, `api/dataplane.rs:45,110-124`, `api/openapi.rs:521`.
### Data model
Deletes the `converter` row and its secret.
### Contracts
`admin.delete_converter(converter_id)` answering `{converter_id}`; order: administrator, converter found, `converter '<name>' is the platform default`, `converter '<name>' is used by <n> project(s)`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/settings/converters.rs`, `converter_store.rs` | modify |
| `crates/core/src/tools/converters.rs`, `crates/core/src/mcp/server.rs`, `tools/admin.rs` | modify: tool, wrapper, name list |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json` (regenerated with `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`) and the tool count assertions listed in the story constraints | modify |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
### Existing patterns
Provider delete `settings/registry.rs:348`; count text pattern `provider '<name>' still has <n> model(s)`. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-008: an unused converter is deleted
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_008_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists, used by nothing
- **When** Alice deletes it
- **Then** the answer confirms the deletion and listing shows no converter
- **Priority:** High

### SPEC-0106/E2E-043: the default converter cannot be deleted
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_043_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) is the platform default
- **When** Alice deletes the converter that is the platform default
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter 'Pandoc local' is the platform default`
- **Priority:** High

### SPEC-0106/E2E-044: a converter used by projects cannot be deleted
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_044_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of the projects `proj` and `other`
- **When** Alice deletes the converter chosen by `proj` and by `other`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter 'Docling API' is used by 2 project(s)`
- **Priority:** High

### SPEC-0106/E2E-045: a deletion of an unknown converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_045_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** no converter exists
- **When** Alice deletes `conv-000000000000`
- **Then** she is refused with `ERR_NOT_FOUND: converter not found: conv-000000000000`
- **Priority:** High

### SPEC-0106/E2E-070: deleting after unsetting
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_070_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) is the default
- **When** Alice clears the default then deletes it
- **Then** the deletion succeeds
- **Priority:** Medium

### SPEC-0106/E2E-091: project selection appears in the usage count
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_091_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007, FR-NEW-009
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of `proj`
- **When** Alice clears the selection and deletes the converter
- **Then** the deletion succeeds (it was refused with `converter 'Docling API' is used by 1 project(s)` while the selection existed)
- **Priority:** Medium

### SPEC-0106/E2E-096: a project removed for good no longer counts as using a converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_096_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-021, FR-NEW-007
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of `proj` only; the platform removes `proj` for good
- **When** Alice deletes `Docling API`
- **Then** it is deleted without the usage refusal
- **Priority:** Medium

### SPEC-0106/E2E-122: a default that is also used is refused as the default
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_122_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the platform default and the converter of `proj`
- **When** Alice deletes it
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter 'Docling API' is the platform default`
- **Priority:** Medium

### SPEC-0106/E2E-124: a soft deleted project still uses its converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_124_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007, FR-NEW-021
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of `proj`; `proj` is removed within its grace period
- **When** Alice deletes `Docling API`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter 'Docling API' is used by 1 project(s)`
- **Priority:** Medium

## Constraints
- Count assertions to raise by the number of tools this story adds: `tools/admin.rs:1008` (admin 29), `tools/all.rs:75,92,145,164`, `tools/contract_golden.rs:152,275,331,345` (136), `mcp/server.rs:4587-4591,4928,4949-4950`, `tools/tool_surface_e2e.rs:455`, and the inline counts in `README.md:4,659`. Add 1.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0314: delete a converter` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
