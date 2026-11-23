# SPEC-0104: Read tools for the document metadata, design

> Spec: sdlc/specs/SPEC-0104-read-tools-for-the-document-metadata/spec.md
> Nature: FEAT
> Depth: M
> Security: n/a
> Designed on: 2026-11-23
> Target tier: 2 (default: no `Tier:` in the constitution, no `.spec.json`)

## 1. Stack and skills
| Element | Choice | Version | DEC |
|---|---|---|---|
| Language, web, storage, tests | the existing stack (Rust 2024, axum, rmcp, rusqlite and sqlx, `cargo test`) | as in `Cargo.toml` | none |
| New dependency | none | n/a | none |

Skills loaded: `rust` (already in context). Effect: `make check` is the gate, tests are `#[cfg(test)]` modules driven through the real `McpServer` and the real router. Existing project: `git ls-files '*.rs' | wc -l` is above 300.

## 2. Approaches
### Chosen: extend the five existing artifact functions and add two thin ones
Add `ocr_text` to the two image readers, a heading reader, one marker kind, one raw image function and one structure function in `tools/artifacts.rs`; the web routes and the tools call the same functions (the SPEC-0019 DEC-001 rule). Pros: no second implementation, parity is by construction, no schema change. Cons: the contract files change in two stories. Affected scope: `storage/meta.rs`, `docs/artifacts/`, `tools/artifacts.rs`, `mcp/server.rs`, `api/dataplane.rs`, `api/openapi.rs`. Best suited when the data is already stored (it is, SPEC-0103).
### Rejected: a separate "metadata" tool returning images, tables and structure together
One call, but a payload of unbounded size and a second place that must stay equal to the five existing reads (DEC-001).
### Rejected: the raw picture as an MCP tool
The tool interface carries text; the entry's own note keeps raw bytes on the web side (DEC-002).

## 3. Architecture
```
 tools (mcp/server.rs) ──┐
                         ├─> tools/artifacts.rs (list_images, get_image, image_file, document_structure, document_view)
 web (api/dataplane.rs) ─┘          │
                                    ├─> storage/meta.rs readers (images with ocr_text, headings, view images)
                                    └─> docs/artifacts/render.rs (view assembly, OCR line)
```
Bounded context: Documents only. The membership guard, the refusal texts and `ensure_file`, `current_set`, `current_page` are reused as they are (`tools/artifacts.rs:~157-230`).

## 4. Data model
No change. Reads of `doc_images.ocr_text` and `doc_headings` (SPEC-0103, `storage/meta.rs:274,301`). A document converted before SPEC-0103 has null OCR text and no heading rows, which the readers return as `None` and an empty list (DEC-011).

## 5. Contracts and interfaces
- `fs.list_images` items: `{id, caption, page, ocr_text}`; `fs.get_image`: `{id, format, caption, page, ocr_text, base64}`.
- `fs.get_document_structure(mount_id, path, marker=null)` returns `{"headings": [{"level": int, "title": string, "page": int|null}], "marker": string|null}`, 100 per answer. Refusal order: not a file, invalid marker, no artifacts.
- `fs.get_document_view(mount_id, path, captions=true, table_mode=null, view=null)`: `view` is `standard` (default), `text` or `full`; `standard` and `text` answer `{path, text}`, `full` answers `{path, text, structure}` with `structure` the same item shape, unpaged. Order: not a file, unsupported view, unsupported table mode, no artifacts. Text: `Unsupported view: <v> (use standard, text or full)`.
- Web: `GET /api/fs/{mount_id}/structure?path&marker`; `GET /api/fs/{mount_id}/document-view?...&view`; `GET /api/fs/{mount_id}/image-file?path&id` answering the bytes with `Content-Type` (`image/png`, `image/jpeg`, `image/gif`, `image/webp`, `image/tiff`, `image/bmp`), `Content-Disposition: attachment; filename="<id>.<ext>"` (`png`, `jpg`, `gif`, `webp`, `tiff`, `bmp`), `X-Content-Type-Options: nosniff`, `Content-Length`; refusals use the existing JSON error body.
- OCR line in the full view: `[OCR image-<n>: <text>]` placed per FR-NEW-006.

## 6. File structure impact
| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/core/src/storage/meta.rs` | modify: `ocr_text` in `list_artifact_images`/`artifact_image`/`artifact_view_images`; new `artifact_headings` | FR-MOD-001, FR-MOD-002, FR-NEW-002, FR-NEW-006 | US-0296, US-0298, US-0299 |
| `crates/core/src/tools/artifacts.rs` | modify: list/get image fields, `image_file`, `document_structure`, view names | all | US-0296 to US-0299 |
| `crates/core/src/docs/artifacts/mod.rs` | modify: `ListKind::Structure` | FR-NEW-002 | US-0298 |
| `crates/core/src/docs/artifacts/render.rs` | modify: OCR line, view kind | FR-NEW-006 | US-0299 |
| `crates/core/src/mcp/server.rs`, `crates/core/src/tools/mod.rs` | modify: structure tool, `view` argument | FR-NEW-002, FR-NEW-004 | US-0298, US-0299 |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json` | modify (golden regenerated) | FR-NEW-002, FR-NEW-004, FR-MOD-001 | US-0296, US-0298, US-0299 |
| `crates/core/src/api/dataplane.rs`, `crates/core/src/api/openapi.rs` | modify: three routes and the `view` query | FR-NEW-001, FR-NEW-008 | US-0297, US-0300 |
| `crates/core/src/tools/read_metadata_e2e.rs` | create: tests `e2e_0104_*` | all | US-0296 to US-0301 |
| `README.md`, `AGENTS.md`, `.agent_docs/tools.md`, `.agent_docs/api.md` | modify: tool count, routes, reads | all | US-0301 |
Affected existing tests: the list and image tests of `tools/artifacts.rs` and `api/dataplane.rs` that compare a whole answer gain the `ocr_text` field (modify, mechanical); the tool count assertions (135 to 136) in `tools/mod.rs`, `api/openapi.rs`, `api/dataplane.rs` (modify).
Affected documentation: as listed.

## 7. Compatibility and risks
Additive: one new field, one new optional argument whose default is today's answer, one new tool, two new routes. The tool count goes from 135 to 136. A 100 image list with long OCR texts can reach about 10 MB (spec 14.3, no bound by FR-MOD-001). Rollback: revert the commit, nothing is stored.

## 9. Test strategy
All tests: functional e2e through the real `McpServer` methods and the real router, in `crates/core/src/tools/read_metadata_e2e.rs`, names `e2e_0104_<id>_<slug>`, converting through the SPEC-0103 stand in converters (`docs/manifest_e2e.rs`: `program()`, the bundle helpers). Run: `cargo test -p fsa-core --lib -- e2e_0104_`. Test data: generated.

| Stories | Tests |
|---|---|
| US-0296 | E2E-002, 003, 021, 025, 031, 038, 048 |
| US-0297 | E2E-004, 005, 012, 013, 014, 022, 024, 032, 040, 041, 047, 051 |
| US-0298 | E2E-006, 007, 015, 016, 017, 034, 045, 046 |
| US-0299 | E2E-008, 009, 010, 018, 019, 020, 027, 028, 029, 033, 042, 043, 049, 050 |
| US-0300 | E2E-036, 037, 044 |
| US-0301 | E2E-001, 011, 023, 026, 030, 035, 039 |

## 10. Decisions log
- **DEC-001:** The image list and the image answer read `ocr_text` through the two existing store readers (`list_artifact_images`, `artifact_image`) and return it as `ocr_text`, `null` when the stored value is `None`. **Obliged by:** SPEC-0104/FR-MOD-001, FR-MOD-002. **Rationale:** one reader per read, parity with the web twin by construction. **Alternatives considered:** a separate reader per field (two queries). **Source:** existing code. **Evidence:** `storage/meta.rs:1316-1363`, `tools/artifacts.rs:135-186`.
- **DEC-002:** The picture download is `tools::artifacts::image_file(state, mount, path, id) -> (format, bytes)` (same refusal order as `get_image`), and a new web handler built on the existing `guarded` sets the headers; no MCP tool. **Obliged by:** SPEC-0104/FR-NEW-001. **Rationale:** `guarded` already returns a `Response`; only the success body differs. **Alternatives considered:** a tool returning base64 (exists). **Source:** existing code. **Evidence:** `api/dataplane.rs:229-264`, `tools/artifacts.rs:165-186`.
- **DEC-003:** Headings are read by a new `artifact_headings(set_id, limit, offset)` (limit and offset like images) and one unlimited call for the full view. **Obliged by:** SPEC-0104/FR-NEW-002, FR-NEW-006. **Rationale:** mirrors `list_artifact_images`; the table `doc_headings` has `(volume_id, set_id, seq)` as key so `ORDER BY seq` is the document order. **Alternatives considered:** loading all and slicing (up to 10 000 rows per page request). **Source:** existing code. **Evidence:** `storage/meta.rs:301,1316-1337`.
- **DEC-004:** The structure uses a new marker kind `ListKind::Structure`, so a marker of another read is `Invalid continuation marker: <raw>`, and an offset past the end gives an empty list with `marker: null`. **Obliged by:** SPEC-0104/FR-NEW-002, FR-NEW-003. **Rationale:** the marker already binds project, kind and path. **Alternatives considered:** reuse `Images` (would accept an image marker). **Source:** existing code. **Evidence:** `docs/artifacts/mod.rs:52,75-90`.
- **DEC-005:** `view` is parsed once in the tools layer into a three variant enum, checked after "not a file" and before the table mode; the table mode is parsed whatever the view. **Obliged by:** SPEC-0104/FR-NEW-004. **Rationale:** the stated refusal order. **Alternatives considered:** parsing in the renderer (loses the order). **Source:** spec. **Evidence:** `tools/artifacts.rs:313-322`, `docs/artifacts/render.rs:47-65`.
- **DEC-006:** The `text` view returns the stored conversion text unchanged and skips the renderer. **Obliged by:** SPEC-0104/FR-NEW-005. **Rationale:** the renderer would add image lines. **Alternatives considered:** a renderer flag. **Source:** design. **Evidence:** `tools/artifacts.rs:324-331`.
- **DEC-007:** The `full` view passes the OCR text through `ViewImageRow` and `ViewImage`; `render::document_view` writes the OCR line right after the image line (same terminator, the blank line after the image moves after it, one line break added when the image line ends the text), then the answer adds `structure` read unlimited. **Obliged by:** SPEC-0104/FR-NEW-006. **Rationale:** placement is a rendering rule, so it lives where the image line is written. **Alternatives considered:** post processing the finished text (fragile with adjacent images). **Source:** existing code. **Evidence:** `docs/artifacts/render.rs:68-212`, `storage/meta.rs:480-487`.
- **DEC-008:** The web `structure` route, the `view` query and `image-file` call the same functions as the tools; success payloads are equal, refusals use `error_response`. **Obliged by:** SPEC-0104/FR-NEW-008. **Rationale:** SPEC-0019 DEC-001 rule. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `api/dataplane.rs:73-77,807-890`, `api/openapi.rs:1184-1250`.
- **DEC-009:** The new reads touch no access time and write nothing (no call to `touch_atime`). **Obliged by:** SPEC-0104/FR-NEW-009. **Rationale:** the existing artifact reads already call none; only `tools/diff_files.rs:87` does. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `tools/diff_files.rs:87-88`.
- **DEC-010:** All tests live in `tools/read_metadata_e2e.rs` and reuse the SPEC-0103 converter stand ins. **Obliged by:** SPEC-0104/FR-MOD-001 (tests of every requirement). **Rationale:** one module, one run command. **Alternatives considered:** extending `tools/artifacts.rs` tests (4 400 lines already). **Source:** design. **Evidence:** `docs/manifest_e2e.rs`.
- **DEC-011:** A document converted before SPEC-0103 or by the built in reading needs no code path: its null OCR text and missing heading rows read as `null` and an empty list. **Obliged by:** SPEC-0104/FR-NEW-007. **Rationale:** additive columns, nothing converts. **Alternatives considered:** a migration (forbidden by SPEC-0103 FR-NEW-020). **Source:** SPEC-0103. **Evidence:** `storage/meta.rs:274,301`.
- **DEC-012:** The new tool is added to the tool list, `TOOL_CONTRACT.txt` (by hand) and the golden JSON (regenerated with `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`), and the tool count assertions go to 136. **Obliged by:** SPEC-0104/FR-NEW-002. **Rationale:** the golden test fails otherwise. **Alternatives considered:** none. **Source:** `AGENTS.md`. **Evidence:** `tools/mod.rs:475`, `tools/contract_golden.rs`.

## 11. Stories
| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0296 | OCR text in the image list and the image | P1 | FR-MOD-001, FR-MOD-002 | main | 2 | no |
| US-0297 | download a picture file | P1 | FR-NEW-001 | US-0296 | 2 | no |
| US-0298 | read the structure of a document | P1 | FR-NEW-002, FR-NEW-003 | US-0296 | 1 | no |
| US-0299 | the text only view and the full view | P1 | FR-NEW-004, FR-NEW-005, FR-NEW-006, FR-MOD-003 | US-0298 | 1 | no |
| US-0300 | the structure and the view through the web interface | P2 | FR-NEW-008 | US-0299, US-0297 | 2 | no |
| US-0301 | access, older documents, re-conversion and the docs | P2 | FR-NEW-007, FR-NEW-009 | US-0300 | 2 | yes |
