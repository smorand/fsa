# US-0294: the converter output limit, the quota and the two forms

> Spec: SPEC-0103 (sdlc/specs/SPEC-0103-converter-answer-with-the-full-document-metadata/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0293
> min_tier: 2
> Files touched: 4
> Closes spec: no

## Objective
Make the same converter answer give the same total in both forms: the output cap counts the Markdown, every bundle file and the manifest in the HTTP form as the folder form already does, the new metadata counts in the storage charge, and one full answer converts identically through both forms. Large answers store within the time bound.

## Requirements covered
- SPEC-0103/FR-NEW-018: The Documents context SHALL read the same fields, apply the same rules and produce the same stored data whether the converter answers as a program (an output folder with the manifest) or as an HTTP service (one JSON answer).
- SPEC-0103/FR-NEW-022: The Documents context SHALL count the files of the answer and the manifest in the converter output limit, count the OCR text, the CSV and the structure in the storage charged for the document's metadata, and SHALL leave no CSV file in the project's files.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0103_065 e2e_0103_076 e2e_0103_092 e2e_0103_093 e2e_0103_094 e2e_0103_075 e2e_0103_079 e2e_0103_066 e2e_0103_067 e2e_0103_002`. Tests run bundles of exactly 1 000 000 and 1 000 001 bytes in both forms, a quota of 1 000 bytes, base64 errors, extra top level keys, an unreferenced file, and the same answer in both forms compared list by list.

## Design excerpt
### Decisions
- DEC-009 (SPEC-0103): The converter output cap counts the Markdown, every bundle file and the manifest in both forms: the folder form already counts every regular file (`service.rs:716-727`); the HTTP form adds the byte length of `markdown` and of the compact JSON text of `manifest` to the decoded file bytes; the existing `over_cap` text is kept. **Obliged by:** SPEC-0103/FR-NEW-022, FR-NEW-018. **Rationale:** the same content must give the same total in both forms. **Alternatives considered:** count only files in both (would drop the folder form's Markdown). **Source:** existing code. **Evidence:** `docs/service.rs:716-727,772-783,290`.
- DEC-010 (SPEC-0103): The storage charge `PendingSet::bytes()` adds the OCR text bytes, the exact CSV bytes (instead of the derived CSV length for such a table) and the title bytes of every heading, so the existing admission and refusal texts (`ERR_PROJECT_QUOTA_EXCEEDED`, `session write quota of <n> bytes exceeded`) apply unchanged. **Obliged by:** SPEC-0103/FR-NEW-022. **Rationale:** one charge, as today. **Alternatives considered:** a second charge. **Source:** existing code. **Evidence:** `capture.rs:137-145`, `core/fs_ops.rs:799-875`.
### Data model
No schema change.
### Contracts
HTTP form total = decoded file bytes + byte length of `markdown` + byte length of the compact JSON text of `manifest`; folder form unchanged (every regular file); the existing text `document service output is over the doc_service.max_input_bytes limit of <limit>` and the base64 text `document service bundle file '<key>' is not base64: <cause>` are kept; unknown top level keys ignored; files not referenced are kept nowhere; quota and session refusals as today (`ERR_PROJECT_QUOTA_EXCEEDED`, `session write quota of <n> bytes exceeded`) with the new bytes in `PendingSet::bytes()`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/docs/service.rs` | modify: HTTP form accounting |
| `crates/core/src/docs/artifacts/capture.rs` | modify: `bytes()` complete for CSV, OCR and titles (verify) |
| `crates/core/src/docs/manifest_e2e.rs` | modify: these tests |
| `crates/core/src/docs/service.rs` tests | modify only where an existing cap test assumed the HTTP form did not count the Markdown |
### Existing patterns
Cap accounting `docs/service.rs:716-727,772-783,290`; charge `capture.rs:137-145`, `core/fs_ops.rs:799-875`; existing cap tests `docs/service.rs` (search `over_cap`) and `tools/converter_bound_e2e.rs`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0103/E2E-065: a bundle exactly at the converter output limit
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_065_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter output limit is 1 000 000 bytes; the Markdown, pictures, CSV files and the manifest add up to exactly 1 000 000 bytes
- **When** Alice converts it
- **Then** the conversion succeeds
- **And** the tables read exact
- **Priority:** Medium

### SPEC-0103/E2E-076: the bundle cap counts the CSV files
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_076_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter output limit is 1 000 000 bytes; the Markdown, the picture files, the CSV files and the manifest add up to 1 000 001 bytes
- **When** Alice converts it
- **Then** the conversion fails with the existing text `document service output is over the doc_service.max_input_bytes limit of 1000000`
- **And** nothing is kept
- **Priority:** High

### SPEC-0103/E2E-092: the manifest bytes count in the output limit, program form
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_092_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter output limit is 1 000 000 bytes; the Markdown, the files and the manifest (mostly OCR text) add up to 1 000 000 bytes, then to 1 000 001
- **When** Alice converts each
- **Then** the first conversion succeeds; the second fails with `document service output is over the doc_service.max_input_bytes limit of 1000000`
- **And** nothing is kept for the second
- **Priority:** Medium

### SPEC-0103/E2E-093: the same total in the HTTP form
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_093_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP; the same content as E2E-092 (limit 1 000 000 bytes; the Markdown, the files and the manifest text add up to 1 000 000 and then 1 000 001 bytes)
- **When** Alice converts each
- **Then** the first succeeds, the second fails with the same text as the program form
- **And** both forms give the same totals
- **Priority:** Medium

### SPEC-0103/E2E-094: a large answer is stored in time
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_094_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the answer holds 1 000 pictures, 500 tables and 10 000 headings
- **When** Alice converts it
- **Then** the conversion completes within 10 seconds more than the converter itself took
- **And** the report counts 1 000 images, 500 tables and 10 000 headings
- **Priority:** Medium

### SPEC-0103/E2E-075: storage quota counts the new metadata
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_075_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the project has 1 000 bytes of storage left; the answer carries OCR text and CSV files of 4 000 bytes in total
- **When** Alice converts the document
- **Then** the conversion is refused with the existing quota error `ERR_PROJECT_QUOTA_EXCEEDED`
- **And** no image, table, OCR text or CSV is kept
- **Priority:** High

### SPEC-0103/E2E-079: the HTTP form with a base64 error
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_079_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP; a CSV file with invalid base64
- **When** Alice converts it
- **Then** the conversion fails with the existing text `document service bundle file 'tab/t1.csv' is not base64:` followed by the cause
- **And** nothing is kept
- **Priority:** High

### SPEC-0103/E2E-066: HTTP form with unknown top level keys
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_066_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP; the answer holds the keys `markdown`, `files`, `manifest` and `extra`
- **When** Alice converts it
- **Then** the conversion succeeds and `extra` is ignored
- **And** the report is as for the same answer without `extra`
- **Priority:** Medium

### SPEC-0103/E2E-067: folder form with the manifest next to other files
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_067_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the output folder holds `document.md`, `artifacts.json`, `fig/a.png`, `tab/t1.csv` and `notes.txt` (not referenced)
- **When** Alice converts it
- **Then** the report counts only referenced pictures and tables
- **And** `notes.txt` is not kept anywhere
- **Priority:** Medium

### SPEC-0103/E2E-002: the same answer from the HTTP form
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_002_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP (the manifest and the files travel in one JSON answer, files in base64); same content as E2E-001
- **When** Alice converts `/report.pdf` with the HTTP form
- **Then** the report and the tables read exactly as in E2E-001
- **And** the two forms give the same `fs.list_images` and `fs.list_tables` answers
- **Priority:** Critical

## Constraints
- Keep every existing error text.
- Do not change the folder form counting.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0295.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0294: the converter output limit, the quota and the two forms` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0103` and the requirement ids.
