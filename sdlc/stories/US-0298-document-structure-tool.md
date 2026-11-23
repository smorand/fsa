# US-0298: read the structure of a document

> Spec: SPEC-0104 (sdlc/specs/SPEC-0104-read-tools-for-the-document-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0296
> min_tier: 1
> Files touched: 7
> Closes spec: no

## Objective
Add the tool that returns the heading structure of a converted document in document order, 100 per answer with a marker, an empty list when the document has none, and the existing refusals.

## Requirements covered
- SPEC-0104/FR-NEW-002: WHEN a member reads the structure of a converted document THE Documents context SHALL return its headings in document order, each with its level (1 to 6), its title and its page, 100 headings per answer with a marker for the next answer.
- SPEC-0104/FR-NEW-003: IF the document is not a file, or was never converted, THEN THE Documents context SHALL refuse the structure read with the existing refusal for that case, AND IF the document was converted but has no structure THEN THE Documents context SHALL answer an empty list.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0104_ tool_contract mcp::`. Tests convert documents with 3, 150 and 10 000 headings and read them page by page through `fs.get_document_structure`, then check the refusals and that the golden contract test passes after regeneration.

## Design excerpt
### Decisions
- DEC-003 (SPEC-0104): Headings are read by a new `artifact_headings(set_id, limit, offset)` (limit and offset like images) and one unlimited call for the full view. **Obliged by:** SPEC-0104/FR-NEW-002, FR-NEW-006. **Rationale:** mirrors `list_artifact_images`; the table `doc_headings` has `(volume_id, set_id, seq)` as key so `ORDER BY seq` is the document order. **Alternatives considered:** loading all and slicing (up to 10 000 rows per page request). **Source:** existing code. **Evidence:** `storage/meta.rs:301,1316-1337`.
- DEC-004 (SPEC-0104): The structure uses a new marker kind `ListKind::Structure`, so a marker of another read is `Invalid continuation marker: <raw>`, and an offset past the end gives an empty list with `marker: null`. **Obliged by:** SPEC-0104/FR-NEW-002, FR-NEW-003. **Rationale:** the marker already binds project, kind and path. **Alternatives considered:** reuse `Images` (would accept an image marker). **Source:** existing code. **Evidence:** `docs/artifacts/mod.rs:52,75-90`.
- DEC-012 (SPEC-0104): The new tool is added to the tool list, `TOOL_CONTRACT.txt` (by hand) and the golden JSON (regenerated with `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`), and the tool count assertions go to 136. **Obliged by:** SPEC-0104/FR-NEW-002. **Rationale:** the golden test fails otherwise. **Alternatives considered:** none. **Source:** `AGENTS.md`. **Evidence:** `tools/mod.rs:475`, `tools/contract_golden.rs`.
### Data model
No schema change; reads `doc_headings` (`volume_id`, `set_id`, `seq`, `level`, `title`, `page`).
### Contracts
`fs.get_document_structure(mount_id, path, marker=null)` returns `{"headings": [{"level", "title", "page"}], "marker"}`, 100 per answer; `ListKind::Structure` marker kind; `artifact_headings(set_id, limit, offset)` ordered by `seq`; `tools::artifacts::document_structure(state, mount, path, marker)` with the order not a file, invalid marker, no artifacts. Tool count 135 to 136 everywhere asserted.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/meta.rs` | modify: `artifact_headings` |
| `crates/core/src/docs/artifacts/mod.rs` | modify: `ListKind::Structure` |
| `crates/core/src/tools/artifacts.rs` | modify: `document_structure`, registry glue |
| `crates/core/src/mcp/server.rs` | modify: args struct and `#[tool]` method |
| `crates/core/src/tools/mod.rs` | modify: tool name list and count |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json` | modify (golden regenerated) |
| `crates/core/src/tools/read_metadata_e2e.rs` | modify: tests |
### Existing patterns
`list_images` and its marker `tools/artifacts.rs:135-160`, `docs/artifacts/mod.rs:52-90`; tool registration `mcp/server.rs:1351,2777`; regeneration `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`, entries of `TOOL_CONTRACT.txt` by hand.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0104/E2E-006: the structure in reading order
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_006_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002
- **Given** document D converted
- **When** Alice reads the structure of `/report.pdf`
- **Then** three headings in this order: level 1 `Report` page 1, level 2 `Costs` page 2, level 2 `Outlook` page 3, and no next page marker
- **Priority:** High

### SPEC-0104/E2E-007: the structure is read 100 headings at a time
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_007_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002
- **Given** a document converted with 150 headings titled `Section 1` to `Section 150`
- **When** Alice reads the structure, then reads again with the next page marker
- **Then** the first answer holds `Section 1` to `Section 100` and a marker; the second holds `Section 101` to `Section 150` and no marker
- **Priority:** High

### SPEC-0104/E2E-015: structure of a document never converted
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_015_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** a file `/plain.pdf` that was never converted
- **When** Alice reads its structure
- **Then** she is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Priority:** High

### SPEC-0104/E2E-016: structure of a path that is not a file
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_016_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** project `proj` with a folder `/docs`
- **When** Alice reads the structure of `/docs`
- **Then** she is refused with `ERR_NOT_FOUND: not a file: /docs`
- **Priority:** High

### SPEC-0104/E2E-017: structure with a marker that is not valid
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_017_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002, FR-NEW-003
- **Given** document D converted
- **When** Alice reads the structure with the marker `garbage`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Invalid continuation marker: garbage`
- **And** no heading is returned
- **Priority:** Medium

### SPEC-0104/E2E-034: a heading without a page
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_034_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002
- **Given** a document converted whose converter gave the heading `Appendix` level 2 with no page
- **When** Alice reads the structure
- **Then** the heading shows level 2, title `Appendix` and page `null`
- **Priority:** Medium

### SPEC-0104/E2E-045: a marker from another read is refused
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_045_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002, FR-NEW-003
- **Given** a document (not D) converted with 150 images; Alice holds the marker the image list gave her
- **When** Alice reads the structure of `/report.pdf` with that marker
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Invalid continuation marker: <that marker>`
- **And** no heading is returned
- **Priority:** Medium

### SPEC-0104/E2E-046: 10 000 headings read page by page
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_046_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002
- **Given** a document converted with 10 000 headings titled `H1` to `H10000`
- **When** Alice reads the structure following every marker
- **Then** 100 answers of 100 headings, in order `H1` to `H10000`, the last with marker `null`; the full view of the same document carries all 10 000 headings
- **Priority:** Medium

## Constraints
- No web route here (US-0300).
- The tool is read only, idempotent, not open world, like the image list.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `TOOL_CONTRACT.txt` entry; counts in `AGENTS.md` at US-0301.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0298: read the structure of a document` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0104` and the requirement ids.
