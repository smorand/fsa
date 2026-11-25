# US-0311: extensions, answer cap, timeout and value types

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0310
> min_tier: 2
> Files touched: 3
> Closes spec: no

## Objective
Add the limits common to both forms: the extension list, the answer cap, the timeout, and the refusal of wrong typed values.

## Requirements covered
- SPEC-0106/FR-NEW-004: The platform SHALL hold for every converter a list of accepted extensions, an answer cap in bytes and a timeout in seconds.
- SPEC-0106/FR-NEW-001 (partial: wrong typed values): WHEN a platform administrator creates a converter with a name and a form THE platform SHALL store it and answer with the converter.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_`. Tests create converters at and beyond each limit (timeout 1, 86400, 0; cap 1, 4294967296, 0; extensions with a dot, too long, 65 entries, duplicates and upper case) and with numbers that are not whole and texts given as numbers.

## Design excerpt
### Decisions
- DEC-004 (SPEC-0106): The tool inputs are `serde_json::Value` fields (no `Debug` derive) and the rule module `converter_rules.rs` produces the type texts (`<field> must be text`, the range texts, `command must be a list of text arguments`, `clear_secret must be true or false`); a field is given when present and not `null`, and the order of checks is the one written in FR-NEW-001 to FR-NEW-005. **Obliged by:** SPEC-0106/FR-NEW-001, 002, 003, 004, 005. **Rationale:** typed argument structs would turn a wrong type into a generic parse error with another text. **Alternatives considered:** typed structs with custom deserializers (text mapping scattered). **Source:** spec. **Evidence:** `mcp/server.rs:1582` (current typed provider args).
- DEC-008 (SPEC-0106): The stored extensions have no dot and lower case without duplicates; `build` passes them with a dot to `resolve_extensions`; an empty list falls back to `DOC_SERVICE_EXTS`; matching is the existing `eligible_in` (a path ends with the dotted extension). **Obliged by:** SPEC-0106/FR-NEW-004, FR-NEW-012. **Rationale:** the refusal text lists them as `.pdf, .docx` already. **Alternatives considered:** storing the dot (the spec stores none). **Source:** existing code. **Evidence:** `docs/service.rs:44-52,143-190`, `core/fs_ops.rs:746-760`.
- DEC-009 (SPEC-0106): `max_input_bytes` is passed as the service cap, which `ensure_input_size` and the answer limit already read. **Obliged by:** SPEC-0106/FR-NEW-004, FR-NEW-013. **Rationale:** one cap, as for the file converter. **Alternatives considered:** two fields. **Source:** existing code. **Evidence:** `core/fs_ops.rs:763-770`, `docs/service.rs:299`.
### Data model
Columns `extensions` (JSON list), `max_input_bytes`, `timeout_seconds`.
### Contracts
Texts: `extensions must be a list of up to 64 names of 1 to 16 letters or digits`, `max_input_bytes must be between 1 and 4294967296`, `timeout_seconds must be between 1 and 86400`, `<field> must be text`. Extensions stored lower case without dot, duplicates removed keeping the first.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/settings/converter_rules.rs` | modify |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
| `crates/core/src/settings/converters.rs` | modify only if the defaults move |
### Existing patterns
Rule order in `converter_rules.rs` from US-0309.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-033: a timeout of 0
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_033_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter exists
- **When** Alice creates a valid program converter with `timeout_seconds` 0
- **Then** she is refused with `ERR_INVALID_ARGUMENT: timeout_seconds must be between 1 and 86400`
- **Priority:** High

### SPEC-0106/E2E-034: a timeout of 86401
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_034_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter exists
- **When** Alice creates a valid program converter with `timeout_seconds` 86401
- **Then** she is refused with `ERR_INVALID_ARGUMENT: timeout_seconds must be between 1 and 86400`
- **Priority:** High

### SPEC-0106/E2E-035: an answer cap of 0
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_035_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter exists
- **When** Alice creates a valid program converter with `max_input_bytes` 0
- **Then** she is refused with `ERR_INVALID_ARGUMENT: max_input_bytes must be between 1 and 4294967296`
- **Priority:** High

### SPEC-0106/E2E-036: extensions that are not names
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_036_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter exists
- **When** Alice creates a valid program converter with `extensions` `[".pdf"]`, then with an extension of 17 letters, then with 65 extensions
- **Then** each is refused with `ERR_INVALID_ARGUMENT: extensions must be a list of up to 64 names of 1 to 16 letters or digits`
- **Priority:** High

### SPEC-0106/E2E-062: timeout bounds
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_062_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter
- **When** Alice creates two converters, with `timeout_seconds` 1 and 86400
- **Then** both are created
- **Priority:** Medium

### SPEC-0106/E2E-063: an answer cap bound
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_063_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter
- **When** Alice creates two converters with `max_input_bytes` 1 and 4294967296
- **Then** both are created
- **Priority:** Medium

### SPEC-0106/E2E-112: extensions are stored lower case without duplicates
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_112_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter
- **When** Alice creates a converter with `extensions` `["pdf", "PDF", "Docx"]`
- **Then** the answer shows `["pdf", "docx"]`
- **Priority:** Medium

### SPEC-0106/E2E-145: numbers that are not whole
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_145_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter
- **When** Alice creates a valid program converter with `timeout_seconds` 60.5, then "60", then true
- **Then** each is refused with `ERR_INVALID_ARGUMENT: timeout_seconds must be between 1 and 86400`
- **Priority:** Low

### SPEC-0106/E2E-146: a text field given as a number
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_146_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001, FR-NEW-004
- **Given** no converter
- **When** Alice creates a converter with the name 42, and an `http` converter with the url 42
- **Then** each is refused with `ERR_INVALID_ARGUMENT: name must be text` and `ERR_INVALID_ARGUMENT: url must be text`
- **Priority:** Low

## Constraints
- No change to the conversion path here (US-0316 reads these limits).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0311: extensions, answer cap, timeout and value types` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
