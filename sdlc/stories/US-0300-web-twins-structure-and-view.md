# US-0300: the structure and the view through the web interface

> Spec: SPEC-0104 (sdlc/specs/SPEC-0104-read-tools-for-the-document-metadata/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0299, US-0297
> min_tier: 2
> Files touched: 4
> Closes spec: no

## Objective
Give the structure and the view names the same answers through the web interface, with the same refusals, and register them in the route and OpenAPI tables.

## Requirements covered
- SPEC-0104/FR-NEW-008: The Documents context SHALL give through the web interface of the file plane the same answers, with the same refusals and texts, as through the tools for the image list, the image, the structure and the view.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0104_ api::dataplane api::openapi`. Tests compare the tool and web answers for the structure and the three views byte for byte, and a non member through the web interface (list images, structure, download).

## Design excerpt
### Decisions
- DEC-008 (SPEC-0104): The web `structure` route, the `view` query and `image-file` call the same functions as the tools; success payloads are equal, refusals use `error_response`. **Obliged by:** SPEC-0104/FR-NEW-008. **Rationale:** SPEC-0019 DEC-001 rule. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `api/dataplane.rs:73-77,807-890`, `api/openapi.rs:1184-1250`.
### Data model
No schema change.
### Contracts
`GET /api/fs/{mount_id}/structure?path&marker` calling `document_structure`; `GET /api/fs/{mount_id}/document-view` accepting `view`; both in `REST_ROUTES` and the OpenAPI table (`tool: fs.get_document_structure`, the `view` query), route count assertions updated.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/api/dataplane.rs` | modify: handler, route, table entry, count |
| `crates/core/src/api/openapi.rs` | modify: row and `view` query |
| `crates/core/src/tools/read_metadata_e2e.rs` | modify: tests |
### Existing patterns
`list_images` web handler `api/dataplane.rs:807-822`; openapi rows `api/openapi.rs:1184-1250`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0104/E2E-036: the structure through the web interface
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_036_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-008
- **Given** document D converted
- **When** Alice reads the structure through the web interface and through the tool
- **Then** both answers are identical, byte for byte
- **Priority:** High

### SPEC-0104/E2E-037: every view through the web interface
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_037_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-008
- **Given** document D converted
- **When** Alice reads the view as `standard`, `text` and `full` through the web interface and through the tool
- **Then** each pair of answers is identical, and a bad view is refused with the same code and text
- **Priority:** High

### SPEC-0104/E2E-044: a non member through the web interface
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_044_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-008, FR-NEW-009
- **Given** document D converted; Eve (`eve@example.com`) is not a member of `proj`
- **When** Eve lists the images, reads the structure and downloads `image-1` through the web interface
- **Then** each is refused with the status of `ERR_FORBIDDEN` and the body `error` = `ERR_FORBIDDEN`, `detail` = `ERR_FORBIDDEN: 'eve@example.com' is not a member of 'proj'`
- **And** no picture byte is delivered
- **Priority:** High

## Constraints
- No second implementation: handlers only call the tool functions.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/api.md` at US-0301.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0300: the structure and the view through the web interface` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0104` and the requirement ids.
