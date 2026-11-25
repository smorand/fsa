# US-0309: create and list program converters

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> min_tier: 1
> Files touched: 12
> Closes spec: no

## Objective
Create the converter registry foundation: the `converter` table, the rule module, `Registry` create and list, the record of a creation, and the tools `admin.create_converter` and `admin.list_converters`, for the program form, for platform administrators only. Creates the test module.

## Requirements covered
- SPEC-0106/FR-NEW-001 (partial: create, answer, name rules, ids, given semantics): WHEN a platform administrator creates a converter with a name and a form THE platform SHALL store it and answer with the converter.
- SPEC-0106/FR-NEW-002: IF a converter has the form `program` THEN THE platform SHALL accept it only with a command that is a list of text arguments holding the placeholder `{document}` exactly once and the placeholder `{outdir}` at most once, and SHALL accept none of the fields of the http form.
- SPEC-0106/FR-NEW-006: WHEN a platform administrator lists the converters THE platform SHALL return them ordered by name without regard to letter case, 100 per answer, with a marker for the next answer.
- SPEC-0106/FR-NEW-017 (partial: tools): The platform SHALL allow every converter operation, reading included, to platform administrators only.
- SPEC-0106/FR-NEW-016 (partial: the record of a creation and of a refused creation): The platform SHALL record every creation, update and deletion of a converter and every change of the platform default and of the converter of a project, with who did it, when, what it concerned and which fields changed, AND SHALL NOT record any value.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_ production_ddl tool_contract settings::`. Tests create program converters through the tool and list them (paging 100 and 101), check every program rule text, the name rules and the administrator refusal before any other check.

## Design excerpt
### Decisions
- DEC-001 (SPEC-0106): Tests live in `settings/converters_e2e.rs` and reuse the fixture `Fx` and the SPEC-0103 stand ins. **Obliged by:** SPEC-0106/FR-NEW-001 (tests of every requirement). **Rationale:** one module, one command; the fixture already sets up an administrator, a member, a master key and the screen helpers. **Alternatives considered:** extending `settings/e2e.rs` (98 tests already). **Source:** existing code. **Evidence:** `settings/e2e.rs:50-167`, `docs/manifest_e2e.rs:76-163`.
- DEC-002 (SPEC-0106): Converters extend the settings module (files `converter.rs`, `converter_rules.rs`, `converter_store.rs`, `converters.rs`); `Registry` and `SettingsStore` fields become `pub(super)` so the new files can add `impl` blocks. **Obliged by:** SPEC-0106/FR-NEW-001, FR-NEW-016, FR-NEW-017. **Rationale:** the record, the key, the transaction helper and the admin check are already there. **Alternatives considered:** a new top level module (duplicates `record` and the cipher wiring). **Source:** existing code. **Evidence:** `settings/registry.rs:169-199`, `settings/store.rs:30-38`.
- DEC-003 (SPEC-0106): Tables `converter` and `converter_default`, and the nullable column `project.converter_id` with a `ColumnMigration`; the name is unique through `name_key` (the same caseless key as providers); ids are `conv-` plus 12 hex digits of a v4 uuid; update checks the name key ignoring the converter being updated. **Obliged by:** SPEC-0106/FR-NEW-001, FR-NEW-005, FR-NEW-008, FR-NEW-009, FR-NEW-021. **Rationale:** a column dies with its project row, which gives FR-NEW-021 with no code; a soft removed row keeps it. **Alternatives considered:** a `project_converter` table with a foreign key cascade (an extra join for the usage count). **Source:** existing code. **Evidence:** `storage/admin.rs:104-183,397-403`, `settings/registry.rs:226-229`.
- DEC-004 (SPEC-0106): The tool inputs are `serde_json::Value` fields (no `Debug` derive) and the rule module `converter_rules.rs` produces the type texts (`<field> must be text`, the range texts, `command must be a list of text arguments`, `clear_secret must be true or false`); a field is given when present and not `null`, and the order of checks is the one written in FR-NEW-001 to FR-NEW-005. **Obliged by:** SPEC-0106/FR-NEW-001, 002, 003, 004, 005. **Rationale:** typed argument structs would turn a wrong type into a generic parse error with another text. **Alternatives considered:** typed structs with custom deserializers (text mapping scattered). **Source:** spec. **Evidence:** `mcp/server.rs:1582` (current typed provider args).
- DEC-013 (SPEC-0106): Records use new `Target` constructors (`converter`, `default_converter`, `project_converter`), the actions of FR-NEW-016, field names in the order of the spec, and no record for a refused operation, an unchanged update or a repeated set or clear; the screen reads them through a `list_changes_of_kinds` store call. **Obliged by:** SPEC-0106/FR-NEW-016, FR-NEW-019. **Rationale:** one record table, one reader. **Alternatives considered:** a second table. **Source:** existing code. **Evidence:** `settings/registry.rs:651`, `settings/store.rs:335-346,444`.
- DEC-015 (SPEC-0106): Every tool function starts with `state.require_admin(person)` through the existing `registry()` helper; REST uses `run`; the screen follows the identity, administrator, token order of `models_screen.rs`. **Obliged by:** SPEC-0106/FR-NEW-017, FR-NEW-018, FR-NEW-019. **Rationale:** the refusal must come before body shape and existence checks. **Alternatives considered:** checking inside each method. **Source:** existing code. **Evidence:** `tools/models.rs:239-247`, `api/admin_settings.rs:44-70`, `models_screen.rs:62-110`.
- DEC-017 (SPEC-0106): The ten tool names go in `tools/admin.rs` name list and `mcp/server.rs` wrappers, with description constants checked by the existing equality test; REST rows go in `REST_ROUTES` and `OPERATIONS`; counts and the golden are updated in the story that adds them. **Obliged by:** SPEC-0106/FR-NEW-018. **Rationale:** the contract tests fail otherwise. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `tools/models.rs:19-40`, `tools/admin.rs:975-1008`, `api/dataplane.rs:45,110-124`, `api/openapi.rs:521`.
### Data model
New table `converter` (columns in DEC-003 and design section 4); the http only columns are stored null for a program converter. `converter_default` and `project.converter_id` come in US-0313.
### Contracts
Tools `admin.create_converter(name, form, command, url, auth_header, secret, file_field, response_field, extensions, max_input_bytes, timeout_seconds)` and `admin.list_converters(marker)`; all inputs `serde_json::Value`; answer `{converter_id, name, form, command, url, auth_header, has_secret, file_field, response_field, extensions, max_input_bytes, timeout_seconds, created_at, updated_at}`; list `{converters, marker}`. Refusal order: administrator, name length, name taken, form, program rules, then (US-0310, US-0311) the other checks. `form` `http` is refused here with the `form must be` text only for unknown values; the http rules arrive in US-0310.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/settings/converter.rs`, `converter_rules.rs`, `converter_store.rs`, `converters.rs`, `settings/mod.rs` | create / modify: model, rules, store, registry methods |
| `crates/core/src/settings/registry.rs`, `settings/store.rs` | modify: `pub(super)` fields, `Target::converter` |
| `crates/core/src/storage/admin.rs`, `crates/core/src/storage/rel/schema.rs` | modify: table `converter`, new fingerprints |
| `crates/core/src/tools/converters.rs`, `tools/mod.rs`, `tools/admin.rs`, `crates/core/src/mcp/server.rs` | create / modify: tool functions, wrappers, name list |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json` (regenerated with `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`) and the tool count assertions listed in the story constraints | modify |
| `crates/core/src/settings/converters_e2e.rs` | create: tests |
### Existing patterns
Provider create path `settings/registry.rs:199-250`; record `settings/registry.rs:651`, `settings/store.rs:335-346`; tools `tools/models.rs:239-260` and wrappers `mcp/server.rs:3408-3427`; paging `Registry::list_providers` `settings/registry.rs:376`. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-001: create a program converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_001_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-002, FR-NEW-004
- **Given** no converter exists
- **When** Alice creates the converter `Pandoc local`, form `program`, command `["pandoc-convert", "--out", "{outdir}", "{document}"]`
- **Then** the answer holds a converter id starting with `conv-`, the name, the form `program`, the command as given, `timeout_seconds` 900, `max_input_bytes` 536870912, an empty extension list, `has_secret` false and equal creation and update times
- **And** listing the converters shows exactly this one
- **Priority:** Critical

### SPEC-0106/E2E-003: converters are listed by name without regard to case
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_003_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-006
- **Given** the converters `docling`, `Pandoc local` and `alpha`
- **When** Alice lists the converters
- **Then** the order is `alpha`, `docling`, `Pandoc local`, with no marker
- **Priority:** High

### SPEC-0106/E2E-017: an empty name
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_017_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001
- **Given** no converter exists
- **When** Alice creates a converter with the name `   `
- **Then** she is refused with `ERR_INVALID_ARGUMENT: name must be 1 to 64 characters`
- **Priority:** High

### SPEC-0106/E2E-018: a name of 65 characters
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_018_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001
- **Given** no converter exists
- **When** Alice creates a converter whose name has 65 characters
- **Then** she is refused with `ERR_INVALID_ARGUMENT: name must be 1 to 64 characters`
- **Priority:** High

### SPEC-0106/E2E-019: a name taken in another letter case
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_019_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice creates `PANDOC LOCAL`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: a converter named 'PANDOC LOCAL' already exists`
- **Priority:** High

### SPEC-0106/E2E-020: an unknown form
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_020_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001
- **Given** no converter exists
- **When** Alice creates a converter with the form `ftp`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: form must be 'program' or 'http'`
- **Priority:** High

### SPEC-0106/E2E-021: a program converter without a command
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_021_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with the command `["", "  "]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command is required for a program converter`
- **Priority:** High

### SPEC-0106/E2E-022: a command that is not a list
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_022_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002, FR-NEW-014
- **Given** no converter exists
- **When** Alice creates a `program` converter whose command is the text `pandoc-convert {document}`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command must be a list of text arguments`
- **Priority:** High

### SPEC-0106/E2E-023: a command without the document placeholder
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_023_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with the command `["pandoc-convert"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found 0`
- **Priority:** High

### SPEC-0106/E2E-024: a command with the document placeholder twice
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_024_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with the command `["c", "{document}", "{document}"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found 2`
- **Priority:** High

### SPEC-0106/E2E-025: a command with the output folder placeholder twice
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_025_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with the command `["c", "{outdir}", "{outdir}", "{document}"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command may contain at most one '{outdir}' placeholder across its arguments, found 2`
- **Priority:** High

### SPEC-0106/E2E-026: an http field given to a program converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_026_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with a valid command and the url `http://localhost:8099/convert`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url is not valid for a program converter`
- **Priority:** High

### SPEC-0106/E2E-049: a page marker that is not valid
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_049_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-006
- **Given** no converter exists
- **When** Alice lists the converters with the marker `garbage`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: marker is not valid`
- **Priority:** High

### SPEC-0106/E2E-050: only platform administrators use the converter tools
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_050_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-017
- **Given** Bob is a member and owner of `proj` and not a platform administrator; a converter exists
- **When** Bob creates, updates, deletes, lists, sets the default, reads the default, sets and reads the converter of `proj`
- **Then** each is refused with `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** nothing is changed and no converter name appears in any refusal
- **Priority:** Critical

### SPEC-0106/E2E-061: a name of 64 characters and accents
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_061_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001
- **Given** no converter
- **When** Alice creates a converter named with 64 characters ending in `é🌐`
- **Then** it is created and listed with its exact name
- **Priority:** Medium

### SPEC-0106/E2E-073: 101 converters are paged
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_073_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-006
- **Given** 101 converters named `c001` to `c101`
- **When** Alice lists, then follows the marker
- **Then** 100 converters and a marker, then one converter and no marker
- **Priority:** Medium

### SPEC-0106/E2E-085: refused and no-op changes are not recorded
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_085_*`)
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice sends a refused creation and an update that changes nothing
- **Then** the record gained no entry
- **Priority:** High

### SPEC-0106/E2E-121: a non administrator is refused before anything else
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_121_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-017
- **Given** Bob is not a platform administrator
- **When** Bob deletes `conv-000000000000`, sets `conv-000000000000` for `ghost` and lists with the marker `garbage`
- **Then** each is refused with `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`, never with a not found or a marker text
- **Priority:** High

## Constraints
- Count assertions to raise by the number of tools this story adds: `tools/admin.rs:1008` (admin 29), `tools/all.rs:75,92,145,164`, `tools/contract_golden.rs:152,275,331,345` (136), `mcp/server.rs:4587-4591,4928,4949-4950`, `tools/tool_surface_e2e.rs:455`, and the inline counts in `README.md:4,659`. Add 2.
- No http rule, update, delete or selection here.
- No `Debug` derive on an input holding a secret.
- Do not log values (DEC-011).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0309: create and list program converters` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
