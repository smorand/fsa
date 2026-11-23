# US-0299: the text only view and the full view

> Spec: SPEC-0104 (sdlc/specs/SPEC-0104-read-tools-for-the-document-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0298
> min_tier: 1
> Files touched: 8
> Closes spec: no

## Objective
Give the document view a `view` choice: `standard` (the default, unchanged), `text` (the converter text as is) and `full` (OCR lines after each image line and the structure), with the stated refusals.

## Requirements covered
- SPEC-0104/FR-NEW-004: WHEN a member reads a converted document's view THE Documents context SHALL accept the view `standard`, `text` or `full`, and `standard` when none is given.
- SPEC-0104/FR-NEW-005: WHILE the view is `text` THE Documents context SHALL return the conversion text exactly as the converter returned it, with no image line, no OCR line and no table rendering added.
- SPEC-0104/FR-NEW-006: WHILE the view is `full` THE Documents context SHALL return the standard view with, right after the image line of each image that has OCR text, the line `[OCR image-<n>: <text>]`, and with the document's structure.
- SPEC-0104/FR-MOD-003: WHEN a member reads a converted document's view without naming a view THE Documents context SHALL return exactly the answer it returned before this increment.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0104_ tools::artifacts docs::artifacts::render`. Tests read document D in the three views, the adjacent images and last line cases, accents and line breaks in OCR text, captions off, a table mode with each view, and the refusal texts.

## Design excerpt
### Decisions
- DEC-005 (SPEC-0104): `view` is parsed once in the tools layer into a three variant enum, checked after "not a file" and before the table mode; the table mode is parsed whatever the view. **Obliged by:** SPEC-0104/FR-NEW-004. **Rationale:** the stated refusal order. **Alternatives considered:** parsing in the renderer (loses the order). **Source:** spec. **Evidence:** `tools/artifacts.rs:313-322`, `docs/artifacts/render.rs:47-65`.
- DEC-006 (SPEC-0104): The `text` view returns the stored conversion text unchanged and skips the renderer. **Obliged by:** SPEC-0104/FR-NEW-005. **Rationale:** the renderer would add image lines. **Alternatives considered:** a renderer flag. **Source:** design. **Evidence:** `tools/artifacts.rs:324-331`.
- DEC-007 (SPEC-0104): The `full` view passes the OCR text through `ViewImageRow` and `ViewImage`; `render::document_view` writes the OCR line right after the image line (same terminator, the blank line after the image moves after it, one line break added when the image line ends the text), then the answer adds `structure` read unlimited. **Obliged by:** SPEC-0104/FR-NEW-006. **Rationale:** placement is a rendering rule, so it lives where the image line is written. **Alternatives considered:** post processing the finished text (fragile with adjacent images). **Source:** existing code. **Evidence:** `docs/artifacts/render.rs:68-212`, `storage/meta.rs:480-487`.
- DEC-012 (SPEC-0104): The new tool is added to the tool list, `TOOL_CONTRACT.txt` (by hand) and the golden JSON (regenerated with `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`), and the tool count assertions go to 136. **Obliged by:** SPEC-0104/FR-NEW-002. **Rationale:** the golden test fails otherwise. **Alternatives considered:** none. **Source:** `AGENTS.md`. **Evidence:** `tools/mod.rs:475`, `tools/contract_golden.rs`.
### Data model
No schema change.
### Contracts
`fs.get_document_view(..., view=null)`; `ViewImageRow` and `render::ViewImage` gain `ocr: Option<String>`; `artifact_view_images` selects `ocr_text`; `text` returns the stored text; `full` answers `{path, text, structure}` with all headings; `standard` and `text` answer `{path, text}`. Refusal order: not a file, `Unsupported view: <v> (use standard, text or full)`, table mode, no artifacts.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/meta.rs` | modify: `artifact_view_images` |
| `crates/core/src/docs/artifacts/render.rs` | modify: OCR line placement, unit tests |
| `crates/core/src/tools/artifacts.rs` | modify: `document_view`, view parsing |
| `crates/core/src/mcp/server.rs` | modify: `DocumentViewArgs.view` |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json` | modify (golden regenerated: new parameter) |
| `crates/core/src/tools/read_metadata_e2e.rs` | modify: tests |
### Existing patterns
Image line writer and terminator rules `docs/artifacts/render.rs:68-212`; `document_view` `tools/artifacts.rs:313-350`; table mode parse `render.rs:47-65`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0104/E2E-008: the text only view
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_008_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-NEW-005
- **Given** document D converted
- **When** Alice reads the view of `/report.pdf` as `text`
- **Then** she receives exactly the Markdown text the converter returned, with no image line and no table rendering added
- **And** the answer holds no `[Image` line and no `[OCR` line
- **Priority:** High

### SPEC-0104/E2E-009: the full view
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_009_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-NEW-006
- **Given** document D converted
- **When** Alice reads the view as `full`
- **Then** the text is the standard view with the line `[OCR image-1: Revenue 2024: 4.2 M EUR]` right after the image line of `image-1`, and no OCR line for `image-2`; the answer also carries the structure: `Report`, `Costs`, `Outlook` with their levels and pages
- **Priority:** High

### SPEC-0104/E2E-010: the default view is unchanged
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_010_*`)
- **Scenario:** SC-004
- **Requirements:** FR-MOD-003, FR-NEW-004
- **Given** document D converted
- **When** Alice reads the view without choosing a view, then as `standard`
- **Then** both answers are identical to the view the previous release gave for D (image lines with captions, tables in Markdown), with no OCR line and no structure
- **Priority:** High

### SPEC-0104/E2E-018: an unknown view
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_018_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004
- **Given** document D converted
- **When** Alice reads the view as `pretty`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Unsupported view: pretty (use standard, text or full)`
- **Priority:** High

### SPEC-0104/E2E-019: an unknown table mode with the full view
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_019_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-MOD-003
- **Given** document D converted
- **When** Alice reads the view as `full` with the table mode `grid`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Unsupported table mode: grid (use markdown, csv-reference or both)`
- **Priority:** Medium

### SPEC-0104/E2E-020: the view name is case sensitive
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_020_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004
- **Given** document D converted
- **When** Alice reads the view as `Full`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Unsupported view: Full (use standard, text or full)`
- **Priority:** Medium

### SPEC-0104/E2E-027: accents, emoji and line breaks in OCR text
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_027_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-006, FR-MOD-001
- **Given** a document whose `image-1` has the OCR text `Café 🌐
ligne 2` (a line break between)
- **When** Alice lists the images and reads the full view
- **Then** the list shows the text exactly as stored; the full view shows `[OCR image-1: Café 🌐` then `ligne 2]` on the next line
- **Priority:** Medium

### SPEC-0104/E2E-028: captions hidden, OCR text still shown
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_028_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** document D converted
- **When** Alice reads the full view with captions off
- **Then** the image line of `image-1` reads `[Image image-1]` with no caption and the line `[OCR image-1: Revenue 2024: 4.2 M EUR]` still follows it
- **Priority:** Medium

### SPEC-0104/E2E-029: the full view of a document with no headings
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_029_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-003
- **Given** a document converted whose text has no heading and whose converter gave an empty structure
- **When** Alice reads the full view and the structure
- **Then** the full view carries an empty structure; the structure read is an empty list with no marker, not a refusal
- **Priority:** Medium

### SPEC-0104/E2E-033: a table mode is ignored by the text view
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_033_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-005
- **Given** document D converted
- **When** Alice reads the view as `text` with the table mode `csv-reference`
- **Then** the answer equals the `text` view without a table mode
- **Priority:** Medium

### SPEC-0104/E2E-042: tables in the full view follow the chosen mode
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_042_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-MOD-003
- **Given** document D converted (its table has an exact CSV)
- **When** Alice reads the full view with the table mode `csv-reference`
- **Then** the table shows as the reference line the standard view gives for that mode, and the OCR line and structure are present
- **Priority:** Medium

### SPEC-0104/E2E-043: a table mode is checked whatever the view
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_043_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-NEW-005
- **Given** document D converted
- **When** Alice reads the view as `text` with the table mode `grid`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Unsupported table mode: grid (use markdown, csv-reference or both)`
- **Priority:** Medium

### SPEC-0104/E2E-049: the OCR line when the image line ends the text
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_049_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** a document whose text ends with its image line `[Image image-1]` with no final line break, `image-1` having OCR text `fin`
- **When** Alice reads the full view
- **Then** the text ends with the image line of `image-1`, one line break, then `[OCR image-1: fin]` and no final line break
- **Priority:** Medium

### SPEC-0104/E2E-050: two adjacent images
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_050_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** a document whose text has two pictures one after the other, `image-1` with OCR text `a` and `image-2` with OCR text `b`
- **When** Alice reads the full view
- **Then** the lines come in this order: the image line of `image-1`, `[OCR image-1: a]`, one blank line, the image line of `image-2`, `[OCR image-2: b]`
- **Priority:** Medium

## Constraints
- The `standard` answer stays byte identical to before.
- No web `view` query here (US-0300).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `TOOL_CONTRACT.txt` parameter description.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0299: the text only view and the full view` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0104` and the requirement ids.
