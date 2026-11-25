# US-0312: update a converter

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0311
> min_tier: 2
> Files touched: 5
> Closes spec: no

## Objective
Update a converter: any creation field but the form, the secret kept, replaced or cleared, the answer sent back as an update accepted, a rename that changes only the letter case, and the record of an update with field names.

## Requirements covered
- SPEC-0106/FR-NEW-005: WHEN a platform administrator updates a converter THE platform SHALL apply the given fields and answer with the converter, keeping its id and its form.
- SPEC-0106/FR-NEW-016 (partial: the record of an update): The platform SHALL record every creation, update and deletion of a converter and every change of the platform default and of the converter of a project, with who did it, when, what it concerned and which fields changed, AND SHALL NOT record any value.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_ tool_contract`. Tests rename, change limits, keep, replace and clear the secret, send the whole answer back, name another converter id in the body, give `clear_secret` to a program converter, and read the record.

## Design excerpt
### Decisions
- DEC-003 (SPEC-0106): Tables `converter` and `converter_default`, and the nullable column `project.converter_id` with a `ColumnMigration`; the name is unique through `name_key` (the same caseless key as providers); ids are `conv-` plus 12 hex digits of a v4 uuid; update checks the name key ignoring the converter being updated. **Obliged by:** SPEC-0106/FR-NEW-001, FR-NEW-005, FR-NEW-008, FR-NEW-009, FR-NEW-021. **Rationale:** a column dies with its project row, which gives FR-NEW-021 with no code; a soft removed row keeps it. **Alternatives considered:** a `project_converter` table with a foreign key cascade (an extra join for the usage count). **Source:** existing code. **Evidence:** `storage/admin.rs:104-183,397-403`, `settings/registry.rs:226-229`.
- DEC-004 (SPEC-0106): The tool inputs are `serde_json::Value` fields (no `Debug` derive) and the rule module `converter_rules.rs` produces the type texts (`<field> must be text`, the range texts, `command must be a list of text arguments`, `clear_secret must be true or false`); a field is given when present and not `null`, and the order of checks is the one written in FR-NEW-001 to FR-NEW-005. **Obliged by:** SPEC-0106/FR-NEW-001, 002, 003, 004, 005. **Rationale:** typed argument structs would turn a wrong type into a generic parse error with another text. **Alternatives considered:** typed structs with custom deserializers (text mapping scattered). **Source:** spec. **Evidence:** `mcp/server.rs:1582` (current typed provider args).
- DEC-005 (SPEC-0106): The secret uses `SecretText`, `encrypt` and `decrypt` of the registry; a blank secret is no secret; the header name and the value are checked with `reqwest::header::HeaderName` and `HeaderValue` (no dependency added); update keeps the stored blob unless a non blank secret or `clear_secret` is given, so it needs no key otherwise. **Obliged by:** SPEC-0106/FR-NEW-003, FR-NEW-005, FR-NEW-015. **Rationale:** the cipher and its failure text exist. **Alternatives considered:** a hand written header grammar. **Source:** existing code. **Evidence:** `settings/secrets.rs:12-57`, `settings/registry.rs:77`.
- DEC-013 (SPEC-0106): Records use new `Target` constructors (`converter`, `default_converter`, `project_converter`), the actions of FR-NEW-016, field names in the order of the spec, and no record for a refused operation, an unchanged update or a repeated set or clear; the screen reads them through a `list_changes_of_kinds` store call. **Obliged by:** SPEC-0106/FR-NEW-016, FR-NEW-019. **Rationale:** one record table, one reader. **Alternatives considered:** a second table. **Source:** existing code. **Evidence:** `settings/registry.rs:651`, `settings/store.rs:335-346,444`.
- DEC-017 (SPEC-0106): The ten tool names go in `tools/admin.rs` name list and `mcp/server.rs` wrappers, with description constants checked by the existing equality test; REST rows go in `REST_ROUTES` and `OPERATIONS`; counts and the golden are updated in the story that adds them. **Obliged by:** SPEC-0106/FR-NEW-018. **Rationale:** the contract tests fail otherwise. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `tools/models.rs:19-40`, `tools/admin.rs:975-1008`, `api/dataplane.rs:45,110-124`, `api/openapi.rs:521`.
### Data model
Updates `converter` rows; `updated_at` moves only when something changed.
### Contracts
`admin.update_converter(converter_id, <creation fields>, clear_secret)`; order: administrator, converter found, `form` and `converter_id` of the body, `secret` with `clear_secret`, then the creation checks on the converter as it is after the update. Texts: `converter not found: <id>`, `form cannot be changed`, `converter_id in the body does not match`, `secret and clear_secret cannot be given together`, `clear_secret is not valid for a program converter`, `clear_secret must be true or false`. Record `update_converter` with `fields` in the spec order and `target_name` the new name.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/settings/converters.rs`, `converter_rules.rs`, `converter_store.rs` | modify |
| `crates/core/src/tools/converters.rs`, `crates/core/src/mcp/server.rs`, `tools/admin.rs` | modify: tool, wrapper, name list |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json` (regenerated with `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`) and the tool count assertions listed in the story constraints | modify |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
### Existing patterns
Provider update `settings/registry.rs:253-345`. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-005: a converter is updated
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_005_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005, FR-NEW-004
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice renames it `Pandoc 3`, sets `timeout_seconds` 120 and `extensions` `["PDF", "docx"]`
- **Then** the answer keeps the same converter id and shows the name `Pandoc 3`, the timeout 120 and the extensions `["pdf", "docx"]` (lower case); the update time is not before the creation time
- **Priority:** High

### SPEC-0106/E2E-039: a secret kept and cleared together
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_039_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005, FR-NEW-003
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists with a secret
- **When** Alice updates `Docling API` giving `secret` `new` and `clear_secret` true
- **Then** she is refused with `ERR_INVALID_ARGUMENT: secret and clear_secret cannot be given together` and the stored secret is unchanged
- **Priority:** High

### SPEC-0106/E2E-040: an update of an unknown converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_040_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005
- **Given** no converter exists
- **When** Alice updates the converter `conv-000000000000`
- **Then** she is refused with `ERR_NOT_FOUND: converter not found: conv-000000000000`
- **Priority:** High

### SPEC-0106/E2E-041: a change of form
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_041_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice updates `Pandoc local` giving the form `http`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: form cannot be changed`
- **Priority:** High

### SPEC-0106/E2E-042: an update that breaks a rule
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_042_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005, FR-NEW-002
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice updates `Pandoc local` with the command `["c"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found 0` and the converter is unchanged
- **Priority:** High

### SPEC-0106/E2E-083: an update renames without changing the id
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_083_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) is the default and the converter of `proj`
- **When** Alice renames it `Pandoc 4`
- **Then** the default and the project converter are still it, and a conversion still uses it
- **Priority:** Medium

### SPEC-0106/E2E-084: the record of changes holds field names only
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_084_*`)
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists
- **When** Alice updates its url, its secret and its command timeout
- **Then** the record shows the action `update_converter`, `target_kind` `converter` and `fields` `["url", "secret", "timeout_seconds"]`, and holds no url, no secret and no number
- **Priority:** Critical

### SPEC-0106/E2E-116: an answer sent back as an update is accepted
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_116_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005, FR-NEW-001
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice updates it sending the whole answer she received (with `converter_id`, `form`, `has_secret`, `created_at`, `updated_at` and `url`, `auth_header`, `file_field`, `response_field` null)
- **Then** the update succeeds, changes nothing and leaves `updated_at` unchanged; a field of the other form given with a value (an empty text included) is refused with `ERR_INVALID_ARGUMENT: url is not valid for a program converter`
- **Priority:** Medium

### SPEC-0106/E2E-133: an update that names another converter in its body
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_133_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists and so does `Docling API`
- **When** Alice updates `Pandoc local` with a body whose `converter_id` is the id of `Docling API`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter_id in the body does not match` and nothing changes
- **Priority:** Medium

### SPEC-0106/E2E-134: a rename that changes only the letter case
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_134_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005, FR-NEW-001
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice renames it `PANDOC LOCAL`, then `PANDOC LOCAL` again
- **Then** the first is accepted, the second changes nothing; renaming it to the name of another converter is refused with `a converter named '<name>' already exists`
- **Priority:** Medium

### SPEC-0106/E2E-137: clearing the secret of a converter that has none
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_137_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists without a secret
- **When** Alice updates it with `clear_secret` true, then with `secret` `   ` and `clear_secret` true
- **Then** both succeed and change nothing
- **Priority:** Low

### SPEC-0106/E2E-138: clearing the secret of a program converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_138_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005, FR-NEW-002
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice updates it with `clear_secret` true
- **Then** she is refused with `ERR_INVALID_ARGUMENT: clear_secret is not valid for a program converter`
- **Priority:** Low

### SPEC-0106/E2E-147: clear_secret given as a text
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_147_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists
- **When** Alice updates it with `clear_secret` "yes"
- **Then** she is refused with `ERR_INVALID_ARGUMENT: clear_secret must be true or false`
- **Priority:** Low

### SPEC-0106/E2E-149: a rename is recorded with the new name
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_149_*`)
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-005
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice renames it `Pandoc 3`
- **Then** the record shows `update_converter`, `target_kind` `converter`, the converter id, `target_name` `Pandoc 3` and `fields` `["name"]`
- **Priority:** Medium

### SPEC-0106/E2E-150: an update that touches no secret does not need the master key
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_150_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-005, FR-NEW-003
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret; the platform restarts without a master key
- **When** Alice renames it, then updates it with `clear_secret` true
- **Then** both succeed (the secret is removed by the second); giving a new secret is refused with `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`
- **Priority:** Low

### SPEC-0106/E2E-066: a secret kept, replaced and cleared
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_066_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-005, FR-NEW-003
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) with the secret `s1`
- **When** Alice updates it without a secret, then with `secret` `s2`, then with `clear_secret` true
- **Then** `has_secret` stays true after the first update, stays true after the second (the converter now sends `s2`), and is false after the third
- **Priority:** High

## Constraints
- Count assertions to raise by the number of tools this story adds: `tools/admin.rs:1008` (admin 29), `tools/all.rs:75,92,145,164`, `tools/contract_golden.rs:152,275,331,345` (136), `mcp/server.rs:4587-4591,4928,4949-4950`, `tools/tool_surface_e2e.rs:455`, and the inline counts in `README.md:4,659`. Add 1.
- No default or project selection here.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0312: update a converter` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
