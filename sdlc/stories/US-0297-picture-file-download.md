# US-0297: download a picture file

> Spec: SPEC-0104 (sdlc/specs/SPEC-0104-read-tools-for-the-document-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0296
> min_tier: 2
> Files touched: 5
> Closes spec: no

## Objective
Let a member download an image of a converted document as the picture file itself, through the web interface, with its media type, an attachment name and `nosniff`.

## Requirements covered
- SPEC-0104/FR-NEW-001: WHEN a member downloads an image of a converted document THE Documents context SHALL deliver the picture file exactly as the converter supplied it, labelled with the media type of its format.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0104_ api::dataplane`. Tests download PNG, JPEG and TIFF pictures (2 048, 1 500 and 5 000 000 byte files) and compare the received bytes and headers with the file the converter supplied; refusals check the JSON error body and that no bytes arrive.

## Design excerpt
### Decisions
- DEC-002 (SPEC-0104): The picture download is `tools::artifacts::image_file(state, mount, path, id) -> (format, bytes)` (same refusal order as `get_image`), and a new web handler built on the existing `guarded` sets the headers; no MCP tool. **Obliged by:** SPEC-0104/FR-NEW-001. **Rationale:** `guarded` already returns a `Response`; only the success body differs. **Alternatives considered:** a tool returning base64 (exists). **Source:** existing code. **Evidence:** `api/dataplane.rs:229-264`, `tools/artifacts.rs:165-186`.
- DEC-009 (SPEC-0104): The new reads touch no access time and write nothing (no call to `touch_atime`). **Obliged by:** SPEC-0104/FR-NEW-009. **Rationale:** the existing artifact reads already call none; only `tools/diff_files.rs:87` does. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `tools/diff_files.rs:87-88`.
- DEC-008 (SPEC-0104): The web `structure` route, the `view` query and `image-file` call the same functions as the tools; success payloads are equal, refusals use `error_response`. **Obliged by:** SPEC-0104/FR-NEW-008. **Rationale:** SPEC-0019 DEC-001 rule. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `api/dataplane.rs:73-77,807-890`, `api/openapi.rs:1184-1250`.
### Data model
No schema change.
### Contracts
`tools::artifacts::image_file(state, mount_id, path, id) -> Result<(String format, Vec<u8>)>` with the order not a file, no artifacts, unknown image (the id rule of `get_image`). Web: `GET /api/fs/{mount_id}/image-file?path&id` built on `guarded`; success headers `Content-Type` (png, jpeg, gif, webp, tiff, bmp mapped to `image/<name>`), `Content-Disposition: attachment; filename="<id>.<ext>"` (`jpeg` gives `jpg`), `X-Content-Type-Options: nosniff`, `Content-Length`. The route is added to `REST_ROUTES` and the OpenAPI table with no tool. No MCP tool.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/tools/artifacts.rs` | modify: `image_file` |
| `crates/core/src/api/dataplane.rs` | modify: route, handler, `REST_ROUTES` entry, route count assertion |
| `crates/core/src/api/openapi.rs` | modify: row for `image-file` (no tool) |
| `crates/core/src/tools/read_metadata_e2e.rs` | modify: tests |
### Existing patterns
`get_image` `tools/artifacts.rs:165-186`; `guarded` `api/dataplane.rs:229-264`; route table `api/dataplane.rs:60-80` and `api/openapi.rs:1184-1250` (the two are asserted equal by a test).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0104/E2E-004: download a PNG picture
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_004_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted; the converter supplied `image-1` as a PNG file of 2 048 bytes
- **When** Alice downloads `image-1`
- **Then** she receives 2 048 bytes identical to the file the converter supplied, labelled `image/png`
- **Priority:** High

### SPEC-0104/E2E-005: download a JPEG picture
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_005_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted; `image-2` is a JPEG file of 1 500 bytes
- **When** Alice downloads `image-2`
- **Then** she receives 1 500 bytes identical to the supplied file, labelled `image/jpeg`
- **Priority:** High

### SPEC-0104/E2E-012: download an unknown image
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_012_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted (two images)
- **When** Alice downloads `image-9`
- **Then** she is refused with `ERR_NOT_FOUND: Not found: image-9`
- **And** no bytes are delivered
- **Priority:** High

### SPEC-0104/E2E-013: download from a path that is not a file
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_013_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** project `proj` with a folder `/docs`
- **When** Alice downloads `image-1` of `/docs`
- **Then** she is refused with `ERR_NOT_FOUND: not a file: /docs`
- **Priority:** High

### SPEC-0104/E2E-014: download from a document never converted
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_014_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** a file `/plain.pdf` that was never converted
- **When** Alice downloads `image-1` of `/plain.pdf`
- **Then** she is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Priority:** High

### SPEC-0104/E2E-022: download with an image id that is not canonical
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_022_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted
- **When** Alice downloads `image-01`, then `image-0`, then `picture-1`
- **Then** each is refused with `ERR_NOT_FOUND: Not found: <the id typed>`
- **Priority:** Medium

### SPEC-0104/E2E-024: an image that a re-conversion removed
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_024_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted with two images, then converted again by a converter that returns one image
- **When** Alice downloads `image-2`
- **Then** she is refused with `ERR_NOT_FOUND: Not found: image-2`
- **And** `image-1` still downloads and is the new picture
- **Priority:** Medium

### SPEC-0104/E2E-032: a large picture downloads whole
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_032_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** a document converted whose `image-1` is a PNG file of 5 000 000 bytes
- **When** Alice downloads `image-1`
- **Then** she receives 5 000 000 bytes identical to the supplied file
- **Priority:** Medium

### SPEC-0104/E2E-040: downloading twice gives the same bytes
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_040_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001, FR-NEW-009
- **Given** document D converted
- **When** Alice downloads `image-1` twice
- **Then** both downloads are identical, and equal the supplied file
- **Priority:** Medium

### SPEC-0104/E2E-041: a TIFF picture is labelled as TIFF
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_041_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** a document converted whose `image-1` is a TIFF file
- **When** Alice downloads `image-1`
- **Then** she receives the file as supplied, labelled `image/tiff`
- **And** the same holds for GIF (`image/gif`), WebP (`image/webp`) and BMP (`image/bmp`)
- **Priority:** Medium

### SPEC-0104/E2E-047: a refused download through the web interface
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_047_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-001, FR-NEW-008
- **Given** document D converted
- **When** Alice downloads `image-9` through the web interface
- **Then** the status is that of `ERR_NOT_FOUND` and the body is `error` = `ERR_NOT_FOUND`, `detail` = `ERR_NOT_FOUND: Not found: image-9`
- **And** no picture byte is delivered
- **Priority:** High

### SPEC-0104/E2E-051: download headers
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_051_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted; `image-1` is 2 048 bytes and `image-2` 1 500 bytes
- **When** Alice downloads `image-1` and `image-2` through the web interface
- **Then** the first answer carries `Content-Disposition: attachment; filename="image-1.png"`, `X-Content-Type-Options: nosniff` and `Content-Length: 2048`; the second carries `filename="image-2.jpg"` and `Content-Length: 1500`
- **Priority:** High

## Constraints
- A download writes nothing and bumps no access time.
- No change to the JSON routes.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/api.md` at US-0301.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0297: download a picture file` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0104` and the requirement ids.
