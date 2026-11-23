# US-0296: OCR text in the image list and the image

> Spec: SPEC-0104 (sdlc/specs/SPEC-0104-read-tools-for-the-document-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> min_tier: 2
> Files touched: 5
> Closes spec: no

## Objective
Make the stored OCR text readable next to each image: the image list and the image answer carry `ocr_text`, `null` when none was kept. Creates the test module.

## Requirements covered
- SPEC-0104/FR-MOD-001: WHEN a member lists the images of a converted document THE Documents context SHALL give with each image its OCR text, `null` when none was kept.
- SPEC-0104/FR-MOD-002: WHEN a member gets one image of a converted document THE Documents context SHALL give its OCR text with the existing answer, `null` when none was kept.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0104_ tools::artifacts api::dataplane`. Tests convert document D (two images, one with OCR text `Revenue 2024: 4.2 M EUR`) and list and get its images through the tool and the web route, then check 100/101 image paging and a whitespace only text.

## Design excerpt
### Decisions
- DEC-001 (SPEC-0104): The image list and the image answer read `ocr_text` through the two existing store readers (`list_artifact_images`, `artifact_image`) and return it as `ocr_text`, `null` when the stored value is `None`. **Obliged by:** SPEC-0104/FR-MOD-001, FR-MOD-002. **Rationale:** one reader per read, parity with the web twin by construction. **Alternatives considered:** a separate reader per field (two queries). **Source:** existing code. **Evidence:** `storage/meta.rs:1316-1363`, `tools/artifacts.rs:135-186`.
- DEC-010 (SPEC-0104): All tests live in `tools/read_metadata_e2e.rs` and reuse the SPEC-0103 converter stand ins. **Obliged by:** SPEC-0104/FR-MOD-001 (tests of every requirement). **Rationale:** one module, one run command. **Alternatives considered:** extending `tools/artifacts.rs` tests (4 400 lines already). **Source:** design. **Evidence:** `docs/manifest_e2e.rs`.
### Data model
No schema change; `ocr_text` is the nullable column of SPEC-0103 (`doc_images`).
### Contracts
`fs.list_images` item `{id, caption, page, ocr_text}`; `fs.get_image` answer `{id, format, caption, page, ocr_text, base64}`. `DocImageRow` and `DocImage` gain `ocr_text: Option<String>`; both SELECTs add the column; only a zero length text is `null`. `TOOL_CONTRACT.txt` return examples (lines 794 and 795) gain the field.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/meta.rs` | modify: both readers and row structs |
| `crates/core/src/tools/artifacts.rs` | modify: the two JSON builders; update whole answer assertions of existing tests |
| `crates/core/src/tools/read_metadata_e2e.rs`, `crates/core/src/tools/mod.rs` | create / modify: test module registration |
| `TOOL_CONTRACT.txt` | modify: the two return examples |
| `crates/core/src/api/dataplane.rs` | modify only the existing web tests that compare a whole list or image answer |
### Existing patterns
Readers `storage/meta.rs:1316-1363`; builders `tools/artifacts.rs:135-186`; SPEC-0103 stand ins `docs/manifest_e2e.rs`. `crates/core/src/tools/read_metadata_e2e.rs` reuses the SPEC-0103 stand ins in `docs/manifest_e2e.rs` (`program()`, bundle helpers, the shared `atlas`/`proj` fixture) and drives `McpServer` tool methods and the router.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0104/E2E-002: the image list carries the OCR text
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_002_*`)
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** document D converted
- **When** Alice lists the images of `/report.pdf`
- **Then** two entries in reading order: `image-1` (caption `Figure 1: Revenue 2024`, page 1, OCR text `Revenue 2024: 4.2 M EUR`) and `image-2` (caption `Schéma réseau 🌐`, page 3, OCR text `null`), and no next page marker
- **Priority:** High

### SPEC-0104/E2E-003: one image carries the OCR text
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_003_*`)
- **Scenario:** SC-001
- **Requirements:** FR-MOD-002
- **Given** document D converted
- **When** Alice gets `image-1`
- **Then** the answer holds the format `png`, the caption, the page 1, the OCR text `Revenue 2024: 4.2 M EUR` and the encoded picture, as before plus the text
- **Priority:** High

### SPEC-0104/E2E-021: get an unknown image
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_021_*`)
- **Scenario:** SC-001
- **Requirements:** FR-MOD-002
- **Given** document D converted
- **When** Alice gets `image-9`
- **Then** she is refused with `ERR_NOT_FOUND: Not found: image-9`
- **Priority:** Medium

### SPEC-0104/E2E-025: an image with no OCR text
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_025_*`)
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001, FR-MOD-002
- **Given** document D converted
- **When** Alice gets `image-2`
- **Then** the OCR text is `null`, the caption `Schéma réseau 🌐` and page 3 are as converted
- **And** a picture whose OCR text was empty or absent is also `null`, never an empty string
- **Priority:** High

### SPEC-0104/E2E-031: exactly 100 and 101 images
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_031_*`)
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** a document converted with 100 images, and another with 101, each with OCR text `t<n>`
- **When** Alice lists the images of each
- **Then** 100 images and no marker for the first; 100 images and a marker for the second, and the next page holds `image-101` with `t101`
- **Priority:** Medium

### SPEC-0104/E2E-038: the image list through the web interface
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_038_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-008, FR-MOD-001
- **Given** document D converted
- **When** Alice lists the images through the web interface and through the tool
- **Then** both answers are identical and carry the OCR text
- **Priority:** High

### SPEC-0104/E2E-048: OCR text made only of spaces
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_048_*`)
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** a document whose `image-1` has the OCR text `   ` (three spaces) and `image-2` an empty OCR text
- **When** Alice lists the images
- **Then** `image-1` shows the OCR text `   ` as stored and `image-2` shows `null`
- **Priority:** Medium

## Constraints
- No new tool, route or argument here.
- The golden contract JSON does not change (descriptions and input schemas are the same).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `TOOL_CONTRACT.txt` return examples only (full docs at US-0301).
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0296: OCR text in the image list and the image` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0104` and the requirement ids.
